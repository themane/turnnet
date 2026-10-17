//! The authoritative commit chain (plan §4.2, C-10, C-14).
//!
//! A [`CommitChain`] holds the metadata of every commit in the session, in
//! order. `append` is the single place chain rules are enforced, so a chain
//! built through it is internally consistent by construction.

use std::collections::HashMap;

use thiserror::Error;

use super::{CommitKind, CommitMeta};
use crate::ids::JoinOrder;
use crate::ids::{ActionId, CommitIndex, Hash32, MembershipEpoch, SessionId, StateVersion, Term};

/// Why a commit cannot be added to a chain.
#[derive(Clone, PartialEq, Eq, Debug, Error)]
pub enum ChainError {
    #[error("commit belongs to a different session")]
    WrongSession,
    #[error("commit_hash does not match the commit contents")]
    BadHash,
    #[error("expected commit_index {expected}, got {got}")]
    BadIndex { expected: u64, got: u64 },
    #[error("previous_commit_hash does not match the chain head")]
    BadPreviousHash,
    #[error("term went backwards: head {head}, commit {got}")]
    TermRegressed { head: u64, got: u64 },
    #[error("commit kind {0:?} is not allowed here")]
    BadKind(CommitKind),
    #[error("state_version must be {expected}, got {got}")]
    BadStateVersion { expected: u64, got: u64 },
    #[error("membership_epoch must be {expected}, got {got}")]
    BadEpoch { expected: u64, got: u64 },
    #[error("next_join_order must be {expected}, got {got}")]
    BadNextJoinOrder { expected: u32, got: u32 },
    #[error("commit kind requires committed_action_id / affected_peer_id to match its kind")]
    BadSubject,
    #[error("migration_eligible_peers must be sorted and unique")]
    EligibleNotCanonical,
    #[error("action_id was already committed at index {0}")]
    DuplicateAction(u64),
    #[error("the chain would overflow a counter")]
    Overflow,
    #[error("chain must not be empty")]
    Empty,
}

/// How a peer's copy of a commit relates to the chain when merging a sync.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum SyncError {
    /// A received commit at an index we hold has a different hash. Divergence.
    Diverged { at: CommitIndex },
    /// The received commits start beyond our head, leaving a gap.
    Gap {
        have: CommitIndex,
        first_received: CommitIndex,
    },
    /// A received commit is invalid on top of our chain.
    Invalid(ChainError),
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CommitChain {
    /// `metas[i].commit_index == i + 1`.
    metas: Vec<CommitMeta>,
    /// C-14: dedupe set derived from the chain itself.
    action_index: HashMap<ActionId, CommitIndex>,
    /// Highest index known to be held by at least one *other* peer (C-13).
    /// Hash chaining means an ACK of `n` implies the acker holds `1..=n`.
    acked_through: Option<CommitIndex>,
}

impl CommitChain {
    /// Starts a chain from the session's `InitialState` commit.
    pub fn new(initial: CommitMeta) -> Result<Self, ChainError> {
        Self::from_metas(vec![initial])
    }

    /// Validates and adopts a full chain (for example from `JOIN_ACCEPT` or a recovery blob).
    pub fn from_metas(metas: Vec<CommitMeta>) -> Result<Self, ChainError> {
        let mut iter = metas.into_iter();
        let first = iter.next().ok_or(ChainError::Empty)?;
        let mut chain = Self {
            metas: Vec::new(),
            action_index: HashMap::new(),
            acked_through: None,
        };
        chain.check_initial(&first)?;
        chain.metas.push(first);
        for m in iter {
            chain.append(m)?;
        }
        Ok(chain)
    }

    fn check_initial(&self, m: &CommitMeta) -> Result<(), ChainError> {
        if !m.hash_is_valid() {
            return Err(ChainError::BadHash);
        }
        if m.commit_index != CommitIndex::FIRST {
            return Err(ChainError::BadIndex {
                expected: CommitIndex::FIRST.get(),
                got: m.commit_index.get(),
            });
        }
        if m.previous_commit_hash != Hash32::ZERO {
            return Err(ChainError::BadPreviousHash);
        }
        if m.commit_kind != CommitKind::InitialState {
            return Err(ChainError::BadKind(m.commit_kind));
        }
        if m.state_version != StateVersion::INITIAL {
            return Err(ChainError::BadStateVersion {
                expected: StateVersion::INITIAL.get(),
                got: m.state_version.get(),
            });
        }
        if m.membership_epoch != MembershipEpoch::INITIAL {
            return Err(ChainError::BadEpoch {
                expected: MembershipEpoch::INITIAL.get(),
                got: m.membership_epoch.get(),
            });
        }
        let expected_next = JoinOrder::FIRST_HOST.get() + 1;
        if m.next_join_order.get() != expected_next {
            return Err(ChainError::BadNextJoinOrder {
                expected: expected_next,
                got: m.next_join_order.get(),
            });
        }
        check_subject(m)?;
        check_eligible(m)
    }

    /// Adds the next commit, enforcing every chain rule.
    pub fn append(&mut self, m: CommitMeta) -> Result<(), ChainError> {
        let head = self.head();
        if m.session_id != head.session_id {
            return Err(ChainError::WrongSession);
        }
        if !m.hash_is_valid() {
            return Err(ChainError::BadHash);
        }
        let expected = head.commit_index.next().ok_or(ChainError::Overflow)?;
        if m.commit_index != expected {
            return Err(ChainError::BadIndex {
                expected: expected.get(),
                got: m.commit_index.get(),
            });
        }
        if m.previous_commit_hash != head.commit_hash {
            return Err(ChainError::BadPreviousHash);
        }
        if m.term < head.term {
            return Err(ChainError::TermRegressed {
                head: head.term.get(),
                got: m.term.get(),
            });
        }
        if m.commit_kind == CommitKind::InitialState {
            return Err(ChainError::BadKind(m.commit_kind));
        }
        let is_action = m.commit_kind == CommitKind::GameAction;
        let expected_sv = if is_action {
            head.state_version.next().ok_or(ChainError::Overflow)?
        } else {
            head.state_version
        };
        if m.state_version != expected_sv {
            return Err(ChainError::BadStateVersion {
                expected: expected_sv.get(),
                got: m.state_version.get(),
            });
        }
        let expected_epoch = if m.commit_kind.is_membership() {
            head.membership_epoch.next().ok_or(ChainError::Overflow)?
        } else {
            head.membership_epoch
        };
        if m.membership_epoch != expected_epoch {
            return Err(ChainError::BadEpoch {
                expected: expected_epoch.get(),
                got: m.membership_epoch.get(),
            });
        }
        let expected_join = if m.commit_kind == CommitKind::MemberJoin {
            head.next_join_order.next().ok_or(ChainError::Overflow)?
        } else {
            head.next_join_order
        };
        if m.next_join_order != expected_join {
            return Err(ChainError::BadNextJoinOrder {
                expected: expected_join.get(),
                got: m.next_join_order.get(),
            });
        }
        check_subject(&m)?;
        check_eligible(&m)?;
        if let Some(id) = m.committed_action_id
            && let Some(prev) = self.action_index.get(&id)
        {
            return Err(ChainError::DuplicateAction(prev.get()));
        }
        if let Some(id) = m.committed_action_id {
            self.action_index.insert(id, m.commit_index);
        }
        self.metas.push(m);
        Ok(())
    }

    /// Merges commits received from another peer (state sync).
    ///
    /// Commits we already hold must match exactly; the rest are appended.
    /// Returns how many commits were new. On error the chain is unchanged.
    pub fn merge(&mut self, received: &[CommitMeta]) -> Result<usize, SyncError> {
        let Some(first) = received.first() else {
            return Ok(0);
        };
        let head = self.head_index();
        if first.commit_index > head.next().unwrap_or(head) {
            return Err(SyncError::Gap {
                have: head,
                first_received: first.commit_index,
            });
        }
        let mut candidate = self.clone();
        let mut added = 0;
        for m in received {
            match candidate.get(m.commit_index) {
                Some(mine) if mine == m => {}
                Some(_) => return Err(SyncError::Diverged { at: m.commit_index }),
                None => {
                    candidate.append(m.clone()).map_err(SyncError::Invalid)?;
                    added += 1;
                }
            }
        }
        *self = candidate;
        Ok(added)
    }

    pub fn head(&self) -> &CommitMeta {
        self.metas.last().expect("chain is never empty")
    }

    pub fn head_index(&self) -> CommitIndex {
        self.head().commit_index
    }

    pub fn len(&self) -> usize {
        self.metas.len()
    }

    /// Always `false`; a chain starts with its initial commit.
    pub fn is_empty(&self) -> bool {
        false
    }

    pub fn session_id(&self) -> SessionId {
        self.head().session_id
    }

    pub fn term(&self) -> Term {
        self.head().term
    }

    pub fn get(&self, index: CommitIndex) -> Option<&CommitMeta> {
        let i = usize::try_from(index.get().checked_sub(1)?).ok()?;
        self.metas.get(i)
    }

    /// All commits from `index` (inclusive) to the head.
    pub fn since(&self, index: CommitIndex) -> &[CommitMeta] {
        let i = index.get().saturating_sub(1);
        let i = usize::try_from(i)
            .unwrap_or(usize::MAX)
            .min(self.metas.len());
        &self.metas[i..]
    }

    pub fn metas(&self) -> &[CommitMeta] {
        &self.metas
    }

    /// The commit that carried `action_id`, if any (ACT-002/003).
    pub fn action_commit(&self, action_id: ActionId) -> Option<CommitIndex> {
        self.action_index.get(&action_id).copied()
    }

    /// Records that another peer holds the chain through `index` (an ACK).
    /// Ignores indexes beyond the head and never moves backwards.
    pub fn mark_acked(&mut self, index: CommitIndex) {
        if index > self.head_index() {
            return;
        }
        if self.acked_through.is_none_or(|a| index > a) {
            self.acked_through = Some(index);
        }
    }

    /// Highest commit index known to be held by another peer.
    pub fn acked_through(&self) -> Option<CommitIndex> {
        self.acked_through
    }

    /// Drops every commit above `index` (orphan-tail discard, C-13). Returns the
    /// number dropped. Callers decide whether truncating is allowed; see
    /// [`super::reconcile`].
    pub fn truncate_to(&mut self, index: CommitIndex) -> usize {
        let keep = usize::try_from(index.get()).unwrap_or(usize::MAX).max(1);
        if keep >= self.metas.len() {
            return 0;
        }
        let dropped = self.metas.len() - keep;
        for m in self.metas.drain(keep..) {
            if let Some(id) = m.committed_action_id {
                self.action_index.remove(&id);
            }
        }
        if self.acked_through.is_some_and(|a| a > self.head_index()) {
            self.acked_through = Some(self.head_index());
        }
        dropped
    }
}

fn check_subject(m: &CommitMeta) -> Result<(), ChainError> {
    let action_ok = (m.commit_kind == CommitKind::GameAction) == m.committed_action_id.is_some();
    let affected_ok = m.commit_kind.is_membership() == m.affected_peer_id.is_some();
    if action_ok && affected_ok {
        Ok(())
    } else {
        Err(ChainError::BadSubject)
    }
}

fn check_eligible(m: &CommitMeta) -> Result<(), ChainError> {
    if m.migration_eligible_peers.windows(2).all(|w| w[0] < w[1]) {
        Ok(())
    } else {
        Err(ChainError::EligibleNotCanonical)
    }
}
