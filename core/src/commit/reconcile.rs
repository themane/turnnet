//! Chain reconciliation against an authority claim (plan §7.8, C-13).
//!
//! `reconcile` is pure: it inspects the local chain and says what to do. It
//! never mutates. The caller applies the verdict (fetch, truncate, or fence).

use super::CommitChain;
use crate::ids::{CommitIndex, Hash32, PeerId, Term};

/// What a new or returning authority says its chain looks like: the commit its
/// term builds on.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Claim {
    pub term: Term,
    pub base_commit_index: CommitIndex,
    pub base_commit_hash: Hash32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ConflictReason {
    /// `base_commit_index` is 0, which no chain contains.
    InvalidBase,
    /// We hold a different commit at the claimed base index.
    HashMismatch { at: CommitIndex },
    /// We hold commits above the base that another peer already ACKed.
    /// Those must never be discarded.
    AckedTail { first_acked_tail: CommitIndex },
    /// We hold commits above the base that we did not author.
    ForeignTail { first_foreign: CommitIndex },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Reconciliation {
    /// We hold the base and nothing above it.
    Compatible,
    /// Our head is below the base. Fetch commits from `have + 1` and verify
    /// the hash linkage reaches the base before adopting.
    Behind { have: CommitIndex },
    /// We hold the base plus commits above it that we authored and nobody else
    /// ACKed. Safe to truncate to `base`, emitting `OrphanTailDiscarded` (C-13).
    CompatibleWithOrphanTail { base: CommitIndex, discarded: u64 },
    /// Incompatible branches. Fence; never merge.
    Conflict(ConflictReason),
    /// The claim's term is below our head's term. Reject as stale.
    StaleTerm { local_term: Term },
}

/// Compares `chain` with `claim`, from the point of view of peer `me`.
pub fn reconcile(chain: &CommitChain, me: PeerId, claim: &Claim) -> Reconciliation {
    if claim.term < chain.term() {
        return Reconciliation::StaleTerm {
            local_term: chain.term(),
        };
    }
    let head = chain.head_index();
    if claim.base_commit_index > head {
        return Reconciliation::Behind { have: head };
    }
    let Some(local_base) = chain.get(claim.base_commit_index) else {
        return Reconciliation::Conflict(ConflictReason::InvalidBase);
    };
    if !local_base.commit_hash.ct_eq(&claim.base_commit_hash) {
        return Reconciliation::Conflict(ConflictReason::HashMismatch {
            at: claim.base_commit_index,
        });
    }
    if claim.base_commit_index == head {
        return Reconciliation::Compatible;
    }

    let tail = chain.since(CommitIndex::new(claim.base_commit_index.get() + 1));
    let first_tail = tail[0].commit_index;
    if chain.acked_through().is_some_and(|a| a >= first_tail) {
        return Reconciliation::Conflict(ConflictReason::AckedTail {
            first_acked_tail: first_tail,
        });
    }
    if let Some(foreign) = tail.iter().find(|m| m.host_peer_id != me) {
        return Reconciliation::Conflict(ConflictReason::ForeignTail {
            first_foreign: foreign.commit_index,
        });
    }
    Reconciliation::CompatibleWithOrphanTail {
        base: claim.base_commit_index,
        discarded: tail.len() as u64,
    }
}
