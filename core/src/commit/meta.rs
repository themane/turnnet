//! Authoritative commit metadata (Implementation Spec §21.2).

use super::CommitKind;
use crate::hash::{self, CommitHashInput};
use crate::ids::{
    ActionId, CommitIndex, Hash32, JoinOrder, MembershipEpoch, PeerId, SessionId, StateVersion,
    Term,
};

/// Everything a peer keeps about one commit (C-10: all of these, for the whole session).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CommitMeta {
    pub session_id: SessionId,
    pub term: Term,
    pub commit_index: CommitIndex,
    /// [`Hash32::ZERO`] for commit index 1.
    pub previous_commit_hash: Hash32,
    pub commit_kind: CommitKind,
    pub state_version: StateVersion,
    pub membership_epoch: MembershipEpoch,
    pub host_peer_id: PeerId,
    pub roster_hash: Hash32,
    pub logical_state_hash: Hash32,
    pub next_join_order: JoinOrder,
    /// Sorted, de-duplicated migration-eligible peers (C-15).
    pub migration_eligible_peers: Vec<PeerId>,
    pub migration_eligibility_digest: Hash32,
    pub policy_metadata_digest: Option<Hash32>,
    /// Set for `GameAction` commits only.
    pub committed_action_id: Option<ActionId>,
    /// Set for `Member*` commits only.
    pub affected_peer_id: Option<PeerId>,
    pub commit_hash: Hash32,
}

impl CommitMeta {
    /// The fields bound by `commit_hash`.
    pub fn hash_input(&self) -> CommitHashInput {
        CommitHashInput {
            session_id: self.session_id,
            term: self.term,
            commit_index: self.commit_index,
            previous_commit_hash: self.previous_commit_hash,
            commit_kind: self.commit_kind,
            state_version: self.state_version,
            membership_epoch: self.membership_epoch,
            host_peer_id: self.host_peer_id,
            roster_hash: self.roster_hash,
            logical_state_hash: self.logical_state_hash,
            next_join_order: self.next_join_order,
            migration_eligibility_digest: self.migration_eligibility_digest,
            policy_metadata_digest: self.policy_metadata_digest,
            committed_action_id: self.committed_action_id,
            affected_peer_id: self.affected_peer_id,
        }
    }

    /// Recomputes `commit_hash` from the other fields.
    pub fn compute_hash(&self) -> Hash32 {
        hash::commit_hash(&self.hash_input())
    }

    /// Fills in `commit_hash`. Used by the host when it authors a commit.
    pub fn sealed(mut self) -> Self {
        self.commit_hash = self.compute_hash();
        self
    }

    pub fn hash_is_valid(&self) -> bool {
        self.commit_hash.ct_eq(&self.compute_hash())
    }
}
