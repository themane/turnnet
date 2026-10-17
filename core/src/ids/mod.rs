//! Identity and counter types (Implementation Spec §19).
//!
//! 128-bit identifiers are random and opaque. Their `Debug`/`Display` output is
//! redacted to a short prefix unless the `unredacted-debug` feature is enabled;
//! use `to_hex()` when full output is explicitly required.

use std::fmt;

use rand_core::RngCore;

mod counters;
mod secret;

pub use counters::{CommitIndex, JoinOrder, MembershipEpoch, RouteGeneration, StateVersion, Term};
pub use secret::ReconnectSecret;

/// Error returned when a byte slice cannot be interpreted as an identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("{kind}: expected {expected} bytes, got {actual}")]
pub struct IdLengthError {
    pub kind: &'static str,
    pub expected: usize,
    pub actual: usize,
}

pub(crate) fn to_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(DIGITS[(b >> 4) as usize] as char);
        out.push(DIGITS[(b & 0x0f) as usize] as char);
    }
    out
}

/// Writes `Name(abcd…)`, or the full hex with `unredacted-debug`.
pub(crate) fn fmt_redacted(f: &mut fmt::Formatter<'_>, name: &str, bytes: &[u8]) -> fmt::Result {
    if cfg!(feature = "unredacted-debug") {
        write!(f, "{name}({})", to_hex(bytes))
    } else {
        write!(f, "{name}({}…)", to_hex(&bytes[..2]))
    }
}

macro_rules! id128 {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name([u8; 16]);

        impl $name {
            pub const LEN: usize = 16;

            pub const fn from_bytes(bytes: [u8; 16]) -> Self {
                Self(bytes)
            }

            pub const fn as_bytes(&self) -> &[u8; 16] {
                &self.0
            }

            pub fn random<R: RngCore + ?Sized>(rng: &mut R) -> Self {
                let mut bytes = [0u8; 16];
                rng.fill_bytes(&mut bytes);
                Self(bytes)
            }

            pub fn try_from_slice(slice: &[u8]) -> Result<Self, IdLengthError> {
                <[u8; 16]>::try_from(slice).map(Self).map_err(|_| IdLengthError {
                    kind: stringify!($name),
                    expected: 16,
                    actual: slice.len(),
                })
            }

            /// Full lowercase hex. Not redacted; avoid in logs.
            pub fn to_hex(&self) -> String {
                to_hex(&self.0)
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt_redacted(f, stringify!($name), &self.0)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Debug::fmt(self, f)
            }
        }
    };
}

id128!(
    /// Random 128-bit session identity, stable across host migration.
    SessionId
);
id128!(
    /// Random 128-bit TurnNet participant identity, independent of transport identity.
    PeerId
);
id128!(
    /// Logical game-action identity; stable across retries, reconnect, and migration.
    ActionId
);
id128!(
    /// Protocol-message dedupe identity; a retry gets a fresh one.
    MessageId
);

/// 32-byte BLAKE3 digest.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Hash32([u8; 32]);

impl Hash32 {
    pub const LEN: usize = 32;
    /// `previous_commit_hash` of the first commit.
    pub const ZERO: Self = Self([0u8; 32]);

    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn try_from_slice(slice: &[u8]) -> Result<Self, IdLengthError> {
        <[u8; 32]>::try_from(slice)
            .map(Self)
            .map_err(|_| IdLengthError {
                kind: "Hash32",
                expected: 32,
                actual: slice.len(),
            })
    }

    pub fn to_hex(&self) -> String {
        to_hex(&self.0)
    }

    /// Constant-time equality, for comparing secret-derived verifiers.
    pub fn ct_eq(&self, other: &Self) -> bool {
        blake3::Hash::from_bytes(self.0) == blake3::Hash::from_bytes(other.0)
    }
}

impl From<blake3::Hash> for Hash32 {
    fn from(h: blake3::Hash) -> Self {
        Self(*h.as_bytes())
    }
}

impl fmt::Debug for Hash32 {
    // Hashes are not secret; a short prefix keeps logs readable.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Hash32({}…)", to_hex(&self.0[..4]))
    }
}

impl fmt::Display for Hash32 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand_chacha::ChaCha8Rng;
    use rand_core::SeedableRng;

    #[test]
    fn random_ids_are_deterministic_per_seed_and_distinct() {
        let mut a = ChaCha8Rng::seed_from_u64(7);
        let mut b = ChaCha8Rng::seed_from_u64(7);
        let p1 = PeerId::random(&mut a);
        let p2 = PeerId::random(&mut a);
        assert_eq!(p1, PeerId::random(&mut b));
        assert_ne!(p1, p2);
    }

    #[test]
    fn try_from_slice_checks_length() {
        assert!(SessionId::try_from_slice(&[0u8; 16]).is_ok());
        let err = SessionId::try_from_slice(&[0u8; 15]).unwrap_err();
        assert_eq!((err.expected, err.actual), (16, 15));
        assert!(Hash32::try_from_slice(&[0u8; 33]).is_err());
    }

    #[cfg(not(feature = "unredacted-debug"))]
    #[test]
    fn debug_is_redacted() {
        let id = PeerId::from_bytes([0xab; 16]);
        assert_eq!(format!("{id:?}"), "PeerId(abab…)");
        assert_eq!(id.to_hex().len(), 32);
    }

    #[test]
    fn ct_eq_matches_eq() {
        let a = Hash32::from_bytes([1; 32]);
        let mut other = [1; 32];
        assert!(a.ct_eq(&Hash32::from_bytes(other)));
        other[31] = 2;
        assert!(!a.ct_eq(&Hash32::from_bytes(other)));
    }
}
