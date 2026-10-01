//! Authoritative commit model (Implementation Spec §21).
//!
//! M1 defines the commit kinds; `CommitMeta` and `CommitChain` arrive in M2.

/// Kind of authoritative commit. Discriminants are the wire and hash codes.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[repr(u8)]
pub enum CommitKind {
    InitialState = 1,
    GameAction = 2,
    MemberJoin = 3,
    MemberLeave = 4,
    MemberRemove = 5,
    PolicyMetadataChange = 6,
}

impl CommitKind {
    pub const fn code(self) -> u8 {
        self as u8
    }

    pub const fn from_code(code: i32) -> Option<Self> {
        Some(match code {
            1 => Self::InitialState,
            2 => Self::GameAction,
            3 => Self::MemberJoin,
            4 => Self::MemberLeave,
            5 => Self::MemberRemove,
            6 => Self::PolicyMetadataChange,
            _ => return None,
        })
    }

    /// Whether this commit changes committed membership (and so `membership_epoch`).
    pub const fn is_membership(self) -> bool {
        matches!(
            self,
            Self::MemberJoin | Self::MemberLeave | Self::MemberRemove
        )
    }
}
