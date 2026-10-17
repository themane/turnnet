//! Builders for chain tests (test-only).

use super::{CommitChain, CommitKind, CommitMeta};
use crate::ids::{
    ActionId, CommitIndex, Hash32, JoinOrder, MembershipEpoch, PeerId, SessionId, StateVersion,
    Term,
};

pub fn peer(b: u8) -> PeerId {
    PeerId::from_bytes([b; 16])
}

pub fn action(b: u8) -> ActionId {
    ActionId::from_bytes([b; 16])
}

pub fn session() -> SessionId {
    SessionId::from_bytes([0xAA; 16])
}

pub fn initial(host: PeerId) -> CommitMeta {
    CommitMeta {
        session_id: session(),
        term: Term::INITIAL,
        commit_index: CommitIndex::FIRST,
        previous_commit_hash: Hash32::ZERO,
        commit_kind: CommitKind::InitialState,
        state_version: StateVersion::INITIAL,
        membership_epoch: MembershipEpoch::INITIAL,
        host_peer_id: host,
        roster_hash: Hash32::from_bytes([1; 32]),
        logical_state_hash: Hash32::from_bytes([2; 32]),
        next_join_order: JoinOrder::new(1),
        migration_eligible_peers: vec![host],
        migration_eligibility_digest: Hash32::from_bytes([3; 32]),
        policy_metadata_digest: None,
        committed_action_id: None,
        affected_peer_id: None,
        commit_hash: Hash32::ZERO,
    }
    .sealed()
}

/// The valid successor of `prev` of the given kind, authored by `host` in `term`.
pub fn next(prev: &CommitMeta, kind: CommitKind, host: PeerId, term: Term, tag: u8) -> CommitMeta {
    let mut m = prev.clone();
    m.term = term;
    m.commit_index = prev.commit_index.next().unwrap();
    m.previous_commit_hash = prev.commit_hash;
    m.commit_kind = kind;
    m.host_peer_id = host;
    m.committed_action_id = None;
    m.affected_peer_id = None;
    // Vary the state so different branches get different hashes.
    m.logical_state_hash = Hash32::from_bytes([tag; 32]);
    match kind {
        CommitKind::GameAction => {
            m.state_version = prev.state_version.next().unwrap();
            m.committed_action_id = Some(action(tag));
        }
        CommitKind::MemberJoin => {
            m.membership_epoch = prev.membership_epoch.next().unwrap();
            m.next_join_order = prev.next_join_order.next().unwrap();
            m.affected_peer_id = Some(peer(tag));
        }
        CommitKind::MemberLeave | CommitKind::MemberRemove => {
            m.membership_epoch = prev.membership_epoch.next().unwrap();
            m.affected_peer_id = Some(peer(tag));
        }
        CommitKind::PolicyMetadataChange => {
            m.policy_metadata_digest = Some(Hash32::from_bytes([tag; 32]));
        }
        CommitKind::InitialState => unreachable!(),
    }
    m.sealed()
}

/// A chain of `n` game-action commits by `host` in term 1 on top of the initial commit.
pub fn action_chain(host: PeerId, n: u8) -> CommitChain {
    let mut chain = CommitChain::new(initial(host)).unwrap();
    for i in 1..=n {
        let m = next(chain.head(), CommitKind::GameAction, host, Term::INITIAL, i);
        chain.append(m).unwrap();
    }
    chain
}
