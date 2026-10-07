//! Chain and reconcile tests.

use super::testkit::*;
use super::*;
use crate::ids::{CommitIndex, Hash32, JoinOrder, MembershipEpoch, StateVersion, Term};

const T1: Term = Term::INITIAL;

fn ci(n: u64) -> CommitIndex {
    CommitIndex::new(n)
}

fn claim_at(chain: &CommitChain, index: u64, term: Term) -> Claim {
    Claim {
        term,
        base_commit_index: ci(index),
        base_commit_hash: chain.get(ci(index)).unwrap().commit_hash,
    }
}

// ---- meta ----

#[test]
fn sealed_hash_is_valid_and_tamper_evident() {
    let m = initial(peer(1));
    assert!(m.hash_is_valid());
    let mut t = m.clone();
    t.logical_state_hash = Hash32::from_bytes([9; 32]);
    assert!(!t.hash_is_valid());
    let mut t = m;
    t.term = Term::new(2);
    assert!(!t.hash_is_valid());
}

// ---- chain: construction ----

#[test]
fn new_chain_starts_at_initial_values() {
    let c = CommitChain::new(initial(peer(1))).unwrap();
    assert_eq!(c.len(), 1);
    assert_eq!(c.head_index(), CommitIndex::FIRST);
    assert_eq!(c.head().state_version, StateVersion::INITIAL);
    assert_eq!(c.head().membership_epoch, MembershipEpoch::INITIAL);
}

#[test]
fn initial_commit_rules() {
    let good = initial(peer(1));
    let reject = |f: &dyn Fn(&mut CommitMeta)| {
        let mut m = good.clone();
        f(&mut m);
        CommitChain::new(m.sealed()).unwrap_err()
    };
    assert!(matches!(
        reject(&|m| m.commit_index = ci(2)),
        ChainError::BadIndex { .. }
    ));
    assert_eq!(
        reject(&|m| m.previous_commit_hash = Hash32::from_bytes([1; 32])),
        ChainError::BadPreviousHash
    );
    assert_eq!(
        reject(&|m| m.commit_kind = CommitKind::GameAction),
        ChainError::BadKind(CommitKind::GameAction)
    );
    assert!(matches!(
        reject(&|m| m.state_version = StateVersion::new(1)),
        ChainError::BadStateVersion { .. }
    ));
    assert!(matches!(
        reject(&|m| m.membership_epoch = MembershipEpoch::new(2)),
        ChainError::BadEpoch { .. }
    ));
    assert!(matches!(
        reject(&|m| m.next_join_order = JoinOrder::new(0)),
        ChainError::BadNextJoinOrder { .. }
    ));
    let mut unsealed = good.clone();
    unsealed.logical_state_hash = Hash32::from_bytes([7; 32]);
    assert_eq!(CommitChain::new(unsealed).unwrap_err(), ChainError::BadHash);
    assert_eq!(
        CommitChain::from_metas(vec![]).unwrap_err(),
        ChainError::Empty
    );
}

// ---- chain: append rules ----

#[test]
fn action_commit_advances_index_and_state_version_only() {
    let c = action_chain(peer(1), 1);
    let h = c.head();
    assert_eq!(h.commit_index, ci(2));
    assert_eq!(h.state_version, StateVersion::new(1));
    assert_eq!(h.membership_epoch, MembershipEpoch::INITIAL);
    assert_eq!(h.previous_commit_hash, c.get(ci(1)).unwrap().commit_hash);
}

#[test]
fn membership_commit_advances_epoch_not_state_version() {
    let mut c = action_chain(peer(1), 1);
    let m = next(c.head(), CommitKind::MemberJoin, peer(1), T1, 9);
    c.append(m).unwrap();
    let h = c.head();
    assert_eq!(h.commit_index, ci(3));
    assert_eq!(h.state_version, StateVersion::new(1));
    assert_eq!(h.membership_epoch, MembershipEpoch::new(2));
    assert_eq!(h.next_join_order, JoinOrder::new(2));
    let m = next(c.head(), CommitKind::MemberLeave, peer(1), T1, 9);
    c.append(m).unwrap();
    assert_eq!(c.head().membership_epoch, MembershipEpoch::new(3));
    assert_eq!(
        c.head().next_join_order,
        JoinOrder::new(2),
        "leave must not consume a join_order"
    );
}

#[test]
fn policy_change_advances_neither_counter() {
    let mut c = action_chain(peer(1), 1);
    let m = next(c.head(), CommitKind::PolicyMetadataChange, peer(1), T1, 5);
    c.append(m).unwrap();
    assert_eq!(c.head().state_version, StateVersion::new(1));
    assert_eq!(c.head().membership_epoch, MembershipEpoch::INITIAL);
}

#[test]
fn append_rejects_each_rule_violation() {
    let base = action_chain(peer(1), 2);
    let good = next(base.head(), CommitKind::GameAction, peer(1), T1, 50);
    let try_with = |f: &dyn Fn(&mut CommitMeta)| {
        let mut c = base.clone();
        let mut m = good.clone();
        f(&mut m);
        let err = c.append(m.sealed()).unwrap_err();
        assert_eq!(c, base, "failed append must leave the chain unchanged");
        err
    };
    assert!(matches!(
        try_with(&|m| m.commit_index = ci(9)),
        ChainError::BadIndex { .. }
    ));
    assert_eq!(
        try_with(&|m| m.previous_commit_hash = Hash32::from_bytes([1; 32])),
        ChainError::BadPreviousHash
    );
    assert_eq!(
        try_with(&|m| m.session_id = crate::ids::SessionId::from_bytes([1; 16])),
        ChainError::WrongSession
    );
    assert!(matches!(
        try_with(&|m| m.state_version = StateVersion::new(9)),
        ChainError::BadStateVersion { .. }
    ));
    assert!(matches!(
        try_with(&|m| m.membership_epoch = MembershipEpoch::new(9)),
        ChainError::BadEpoch { .. }
    ));
    assert!(matches!(
        try_with(&|m| m.next_join_order = JoinOrder::new(9)),
        ChainError::BadNextJoinOrder { .. }
    ));
    assert_eq!(
        try_with(&|m| m.committed_action_id = None),
        ChainError::BadSubject
    );
    assert_eq!(
        try_with(&|m| m.affected_peer_id = Some(peer(2))),
        ChainError::BadSubject
    );
    assert_eq!(
        try_with(&|m| m.commit_kind = CommitKind::InitialState),
        ChainError::BadKind(CommitKind::InitialState)
    );
    assert_eq!(
        try_with(&|m| m.migration_eligible_peers = vec![peer(3), peer(2)]),
        ChainError::EligibleNotCanonical
    );
    let mut bad_hash = good.clone();
    bad_hash.commit_hash = Hash32::from_bytes([4; 32]);
    assert_eq!(
        base.clone().append(bad_hash).unwrap_err(),
        ChainError::BadHash
    );
}

#[test]
fn term_may_rise_but_not_fall() {
    let mut c = action_chain(peer(1), 1);
    let m = next(c.head(), CommitKind::GameAction, peer(2), Term::new(2), 20);
    c.append(m).unwrap();
    assert_eq!(c.term(), Term::new(2));
    assert_eq!(c.head().host_peer_id, peer(2));
    let back = next(c.head(), CommitKind::GameAction, peer(2), T1, 21);
    assert_eq!(
        c.append(back).unwrap_err(),
        ChainError::TermRegressed { head: 2, got: 1 }
    );
}

#[test]
fn action_id_is_committed_at_most_once() {
    let mut c = action_chain(peer(1), 3);
    assert_eq!(c.action_commit(action(2)), Some(ci(3)));
    assert_eq!(c.action_commit(action(99)), None);
    let mut dup = next(c.head(), CommitKind::GameAction, peer(1), T1, 80);
    dup.committed_action_id = Some(action(2));
    assert_eq!(
        c.append(dup.sealed()).unwrap_err(),
        ChainError::DuplicateAction(3)
    );
}

#[test]
fn action_dedupe_survives_a_host_change() {
    // ACT-003: a new host's chain still knows actions the old host committed.
    let mut c = action_chain(peer(1), 2);
    let m = next(c.head(), CommitKind::MemberLeave, peer(2), Term::new(2), 1);
    c.append(m).unwrap();
    let mut retry = next(c.head(), CommitKind::GameAction, peer(2), Term::new(2), 60);
    retry.committed_action_id = Some(action(1));
    assert_eq!(
        c.append(retry.sealed()).unwrap_err(),
        ChainError::DuplicateAction(2)
    );
}

#[test]
fn from_metas_round_trips_and_rejects_corruption() {
    let c = action_chain(peer(1), 5);
    let copy = CommitChain::from_metas(c.metas().to_vec()).unwrap();
    assert_eq!(copy.head(), c.head());
    assert_eq!(copy.action_commit(action(4)), Some(ci(5)));

    let mut metas = c.metas().to_vec();
    metas.remove(2);
    assert!(CommitChain::from_metas(metas).is_err());
}

// ---- chain: lookup ----

#[test]
fn get_and_since() {
    let c = action_chain(peer(1), 4);
    assert!(c.get(ci(0)).is_none());
    assert_eq!(c.get(ci(1)).unwrap().commit_kind, CommitKind::InitialState);
    assert_eq!(c.get(ci(5)).unwrap().commit_index, ci(5));
    assert!(c.get(ci(6)).is_none());
    assert_eq!(c.since(ci(1)).len(), 5);
    assert_eq!(c.since(ci(4)).len(), 2);
    assert_eq!(c.since(ci(5)).len(), 1);
    assert!(c.since(ci(6)).is_empty());
    assert_eq!(c.since(ci(0)).len(), 5);
}

// ---- chain: acks and truncation ----

#[test]
fn acks_are_monotonic_and_bounded() {
    let mut c = action_chain(peer(1), 4);
    assert_eq!(c.acked_through(), None);
    c.mark_acked(ci(3));
    c.mark_acked(ci(2));
    assert_eq!(c.acked_through(), Some(ci(3)));
    c.mark_acked(ci(99));
    assert_eq!(c.acked_through(), Some(ci(3)));
}

#[test]
fn truncate_drops_tail_actions_and_clamps_acks() {
    let mut c = action_chain(peer(1), 4);
    c.mark_acked(ci(4));
    assert_eq!(c.truncate_to(ci(3)), 2);
    assert_eq!(c.head_index(), ci(3));
    assert_eq!(c.acked_through(), Some(ci(3)));
    assert_eq!(c.action_commit(action(3)), None);
    assert_eq!(c.action_commit(action(2)), Some(ci(3)));
    // The dropped action can now be committed again.
    let m = next(c.head(), CommitKind::GameAction, peer(1), T1, 3);
    c.append(m).unwrap();
    assert_eq!(c.truncate_to(ci(99)), 0);
    assert_eq!(c.truncate_to(ci(0)), 3, "initial commit is never dropped");
    assert_eq!(c.len(), 1);
}

// ---- chain: merge ----

#[test]
fn merge_appends_new_and_ignores_known() {
    let full = action_chain(peer(1), 5);
    let mut behind = CommitChain::from_metas(full.metas()[..3].to_vec()).unwrap();
    assert_eq!(behind.merge(full.since(ci(2))).unwrap(), 3);
    assert_eq!(behind, full);
    assert_eq!(behind.merge(full.metas()).unwrap(), 0);
    assert_eq!(behind.merge(&[]).unwrap(), 0);
}

#[test]
fn merge_detects_gap_divergence_and_invalid() {
    let full = action_chain(peer(1), 5);
    let mut behind = CommitChain::from_metas(full.metas()[..2].to_vec()).unwrap();
    assert_eq!(
        behind.merge(full.since(ci(4))).unwrap_err(),
        SyncError::Gap {
            have: ci(2),
            first_received: ci(4)
        }
    );

    let mut behind = CommitChain::from_metas(full.metas()[..4].to_vec()).unwrap();
    let mut fork = CommitChain::from_metas(full.metas()[..3].to_vec()).unwrap();
    let alt = next(fork.head(), CommitKind::GameAction, peer(1), T1, 77);
    fork.append(alt).unwrap();
    let before = behind.clone();
    assert_eq!(
        behind.merge(fork.since(ci(2))).unwrap_err(),
        SyncError::Diverged { at: ci(4) }
    );
    assert_eq!(behind, before, "failed merge must not change the chain");

    let mut broken = full.since(ci(4)).to_vec();
    broken[1].logical_state_hash = Hash32::from_bytes([8; 32]);
    assert_eq!(
        behind.merge(&broken).unwrap_err(),
        SyncError::Invalid(ChainError::BadHash)
    );
    assert_eq!(behind, before);
}

// ---- reconcile ----

#[test]
fn reconcile_compatible_at_head() {
    let c = action_chain(peer(1), 3);
    assert_eq!(
        reconcile(&c, peer(2), &claim_at(&c, 4, T1)),
        Reconciliation::Compatible
    );
}

#[test]
fn reconcile_behind_when_claim_is_ahead() {
    let full = action_chain(peer(1), 5);
    let local = CommitChain::from_metas(full.metas()[..3].to_vec()).unwrap();
    assert_eq!(
        reconcile(&local, peer(2), &claim_at(&full, 6, T1)),
        Reconciliation::Behind { have: ci(3) }
    );
}

#[test]
fn reconcile_hash_mismatch_is_conflict() {
    let c = action_chain(peer(1), 3);
    let mut claim = claim_at(&c, 3, T1);
    claim.base_commit_hash = Hash32::from_bytes([5; 32]);
    assert_eq!(
        reconcile(&c, peer(2), &claim),
        Reconciliation::Conflict(ConflictReason::HashMismatch { at: ci(3) })
    );
}

#[test]
fn reconcile_invalid_base_zero() {
    let c = action_chain(peer(1), 1);
    let claim = Claim {
        term: T1,
        base_commit_index: ci(0),
        base_commit_hash: Hash32::ZERO,
    };
    assert_eq!(
        reconcile(&c, peer(1), &claim),
        Reconciliation::Conflict(ConflictReason::InvalidBase)
    );
}

#[test]
fn reconcile_same_index_different_branch_is_conflict() {
    // Two partition branches that both committed at index 4.
    let a = action_chain(peer(1), 3);
    let mut b = CommitChain::from_metas(a.metas()[..3].to_vec()).unwrap();
    let m = next(b.head(), CommitKind::GameAction, peer(2), Term::new(2), 90);
    b.append(m).unwrap();
    assert_eq!(
        reconcile(&a, peer(1), &claim_at(&b, 4, Term::new(2))),
        Reconciliation::Conflict(ConflictReason::HashMismatch { at: ci(4) })
    );
}

#[test]
fn reconcile_unacked_own_tail_is_orphan() {
    let mut c = action_chain(peer(1), 4);
    c.mark_acked(ci(3));
    let claim = claim_at(&c, 3, Term::new(2));
    assert_eq!(
        reconcile(&c, peer(1), &claim),
        Reconciliation::CompatibleWithOrphanTail {
            base: ci(3),
            discarded: 2
        }
    );
    // The verdict is advisory: applying it is the caller's job.
    assert_eq!(c.head_index(), ci(5));
    c.truncate_to(ci(3));
    assert_eq!(reconcile(&c, peer(1), &claim), Reconciliation::Compatible);
}

#[test]
fn reconcile_acked_tail_is_never_discarded() {
    let mut c = action_chain(peer(1), 4);
    c.mark_acked(ci(4));
    assert_eq!(
        reconcile(&c, peer(1), &claim_at(&c, 3, Term::new(2))),
        Reconciliation::Conflict(ConflictReason::AckedTail {
            first_acked_tail: ci(4)
        })
    );
}

#[test]
fn reconcile_foreign_tail_is_conflict() {
    // peer(2) holds commits authored by peer(1) beyond the claimed base.
    let c = action_chain(peer(1), 4);
    assert_eq!(
        reconcile(&c, peer(2), &claim_at(&c, 3, Term::new(2))),
        Reconciliation::Conflict(ConflictReason::ForeignTail {
            first_foreign: ci(4)
        })
    );
}

#[test]
fn reconcile_mixed_tail_is_conflict() {
    // Own commit followed by a commit from another host: not purely mine.
    let mut c = action_chain(peer(1), 2);
    let m = next(c.head(), CommitKind::GameAction, peer(2), Term::new(2), 30);
    c.append(m).unwrap();
    assert_eq!(
        reconcile(&c, peer(1), &claim_at(&c, 2, Term::new(3))),
        Reconciliation::Conflict(ConflictReason::ForeignTail {
            first_foreign: ci(4)
        })
    );
}

#[test]
fn reconcile_rejects_stale_term_first() {
    let mut c = action_chain(peer(1), 1);
    let m = next(c.head(), CommitKind::GameAction, peer(2), Term::new(3), 30);
    c.append(m).unwrap();
    let mut claim = claim_at(&c, 2, Term::new(2));
    claim.base_commit_hash = Hash32::from_bytes([1; 32]);
    assert_eq!(
        reconcile(&c, peer(1), &claim),
        Reconciliation::StaleTerm {
            local_term: Term::new(3)
        }
    );
}

#[test]
fn reconcile_is_pure() {
    let c = action_chain(peer(1), 4);
    let before = c.clone();
    let _ = reconcile(&c, peer(1), &claim_at(&c, 2, Term::new(2)));
    assert_eq!(c, before);
}
