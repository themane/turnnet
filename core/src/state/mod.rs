//! Authoritative state and per-peer representations (Implementation Spec §22).

/// Shape of a state representation delivered to one peer. Discriminants are
/// the wire and hash codes.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[repr(u8)]
pub enum RepresentationKind {
    /// Full authoritative state (trusted full replication).
    Full = 1,
    /// Public state plus optional recipient-specific private/recovery material.
    PublicPrivate = 2,
}

impl RepresentationKind {
    pub const fn code(self) -> u8 {
        self as u8
    }

    pub const fn from_code(code: i32) -> Option<Self> {
        Some(match code {
            1 => Self::Full,
            2 => Self::PublicPrivate,
            _ => return None,
        })
    }
}
