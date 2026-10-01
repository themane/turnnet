//! Secret bearer material.

use std::fmt;

use rand_core::RngCore;

use super::IdLengthError;

/// 256-bit reconnect secret issued by the admitting host (plan C-11).
///
/// Only the owning peer holds it, in memory and in its recovery blob. The
/// committed roster stores `hash::reconnect_verifier(..)` instead. `Debug`
/// never prints the value.
#[derive(Clone, PartialEq, Eq)]
pub struct ReconnectSecret([u8; 32]);

impl ReconnectSecret {
    pub const LEN: usize = 32;

    pub fn random<R: RngCore + ?Sized>(rng: &mut R) -> Self {
        let mut bytes = [0u8; 32];
        rng.fill_bytes(&mut bytes);
        Self(bytes)
    }

    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn try_from_slice(slice: &[u8]) -> Result<Self, IdLengthError> {
        <[u8; 32]>::try_from(slice)
            .map(Self)
            .map_err(|_| IdLengthError {
                kind: "ReconnectSecret",
                expected: 32,
                actual: slice.len(),
            })
    }

    /// Raw secret bytes, for hashing and wire encoding only.
    pub const fn expose(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Debug for ReconnectSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ReconnectSecret(<redacted>)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_never_prints_secret() {
        let s = ReconnectSecret::from_bytes([0x5a; 32]);
        let out = format!("{s:?}");
        assert!(!out.contains("5a"));
        assert_eq!(out, "ReconnectSecret(<redacted>)");
    }
}
