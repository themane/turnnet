//! Committed membership (Implementation Spec §20).

mod roster;

pub use roster::{MAX_DISPLAY_METADATA, PeerMembership, Roster, RosterError};

/// Committed roster status. Discriminants are the wire and hash codes.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[repr(u8)]
pub enum MemberStatus {
    Active = 1,
    Left = 2,
    Removed = 3,
}

impl MemberStatus {
    pub const fn code(self) -> u8 {
        self as u8
    }

    pub const fn from_code(code: i32) -> Option<Self> {
        Some(match code {
            1 => Self::Active,
            2 => Self::Left,
            3 => Self::Removed,
            _ => return None,
        })
    }
}
