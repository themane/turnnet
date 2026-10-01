//! Hash encoding tests.
//!
//! `*_layout` tests rebuild the exact byte layout from docs/protocol.md by hand
//! and hash it with `blake3::derive_key`, independently of the encoder.
//! `golden_*` tests pin the digests so any encoding change fails loudly.

use super::*;

fn sid() -> SessionId {
    SessionId::from_bytes([0x11; 16])
}
fn peer(b: u8) -> PeerId {
    PeerId::from_bytes([b; 16])
}

fn sample_commit() -> CommitHashInput {
    CommitHashInput {
        session_id: sid(),
        term: Term::new(3),
        commit_index: CommitIndex::new(42),
        previous_commit_hash: Hash32::from_bytes([0x22; 32]),
        commit_kind: CommitKind::GameAction,
        state_version: StateVersion::new(40),
        membership_epoch: MembershipEpoch::new(2),
        host_peer_id: peer(0x33),
        roster_hash: Hash32::from_bytes([0x44; 32]),
        logical_state_hash: Hash32::from_bytes([0x55; 32]),
        next_join_order: JoinOrder::new(4),
        migration_eligibility_digest: Hash32::from_bytes([0x66; 32]),
        policy_metadata_digest: None,
        committed_action_id: Some(ActionId::from_bytes([0x77; 16])),
        affected_peer_id: None,
    }
}

fn derive(ctx: &str, buf: &[u8]) -> Hash32 {
    Hash32::from_bytes(blake3::derive_key(ctx, buf))
}

#[test]
fn logical_state_layout() {
    let mut buf = 3u64.to_be_bytes().to_vec();
    buf.extend_from_slice(b"abc");
    assert_eq!(logical_state_hash(b"abc"), derive(ctx::LOGICAL_STATE, &buf));
}

#[test]
fn representation_layout() {
    let mut buf = vec![2u8];
    buf.extend_from_slice(&2u64.to_be_bytes());
    buf.extend_from_slice(b"pu");
    buf.push(1);
    buf.extend_from_slice(&1u64.to_be_bytes());
    buf.extend_from_slice(b"p");
    buf.push(0);
    assert_eq!(
        representation_hash(RepresentationKind::PublicPrivate, b"pu", Some(b"p"), None),
        derive(ctx::REPRESENTATION, &buf)
    );
}

#[test]
fn commit_layout() {
    let c = sample_commit();
    let mut buf = Vec::new();
    buf.extend_from_slice(&[0x11; 16]);
    buf.extend_from_slice(&3u64.to_be_bytes());
    buf.extend_from_slice(&42u64.to_be_bytes());
    buf.extend_from_slice(&[0x22; 32]);
    buf.push(2);
    buf.extend_from_slice(&40u64.to_be_bytes());
    buf.extend_from_slice(&2u64.to_be_bytes());
    buf.extend_from_slice(&[0x33; 16]);
    buf.extend_from_slice(&[0x44; 32]);
    buf.extend_from_slice(&[0x55; 32]);
    buf.extend_from_slice(&4u32.to_be_bytes());
    buf.extend_from_slice(&[0x66; 32]);
    buf.push(0);
    buf.push(1);
    buf.extend_from_slice(&[0x77; 16]);
    buf.push(0);
    assert_eq!(commit_hash(&c), derive(ctx::COMMIT, &buf));
}

#[test]
fn roster_layout_and_order_independence() {
    let a = RosterEntry {
        peer_id: peer(0xa0),
        join_order: JoinOrder::new(0),
        status: MemberStatus::Active,
        admission_commit_index: CommitIndex::new(1),
        reconnect_verifier: Hash32::from_bytes([0xa1; 32]),
        display_metadata: b"host",
    };
    let b = RosterEntry {
        peer_id: peer(0xb0),
        join_order: JoinOrder::new(1),
        status: MemberStatus::Left,
        admission_commit_index: CommitIndex::new(2),
        reconnect_verifier: Hash32::from_bytes([0xb1; 32]),
        display_metadata: b"",
    };
    let mut buf = 2u32.to_be_bytes().to_vec();
    for (e, status) in [(&a, 1u8), (&b, 2u8)] {
        buf.extend_from_slice(e.peer_id.as_bytes());
        buf.extend_from_slice(&e.join_order.get().to_be_bytes());
        buf.push(status);
        buf.extend_from_slice(&e.admission_commit_index.get().to_be_bytes());
        buf.extend_from_slice(e.reconnect_verifier.as_bytes());
        buf.extend_from_slice(&(e.display_metadata.len() as u64).to_be_bytes());
        buf.extend_from_slice(e.display_metadata);
    }
    let expected = derive(ctx::ROSTER, &buf);
    assert_eq!(roster_hash(&[a, b]), expected);
    assert_eq!(roster_hash(&[b, a]), expected);
}

#[test]
fn eligibility_layout_sorts_and_dedups() {
    let mut buf = Vec::new();
    buf.extend_from_slice(&9u32.to_be_bytes());
    buf.extend_from_slice(&2u32.to_be_bytes());
    buf.extend_from_slice(&[0x01; 16]);
    buf.extend_from_slice(&[0x02; 16]);
    let expected = derive(ctx::ELIGIBILITY, &buf);
    assert_eq!(
        eligibility_digest(9, &[peer(2), peer(1), peer(2)]),
        expected
    );
    assert_ne!(eligibility_digest(8, &[peer(1), peer(2)]), expected);
}

#[test]
fn reconnect_verifier_layout() {
    let secret = ReconnectSecret::from_bytes([0x99; 32]);
    let mut buf = vec![0x11; 16];
    buf.extend_from_slice(&[0x03; 16]);
    buf.extend_from_slice(&[0x99; 32]);
    assert_eq!(
        reconnect_verifier(sid(), peer(3), &secret),
        derive(ctx::RECONNECT, &buf)
    );
}

#[test]
fn every_commit_field_is_bound() {
    let base = commit_hash(&sample_commit());
    const H: Hash32 = Hash32::from_bytes([0xee; 32]);
    let mutations: [fn(&mut CommitHashInput); 15] = [
        |c| c.session_id = SessionId::from_bytes([0; 16]),
        |c| c.term = Term::new(4),
        |c| c.commit_index = CommitIndex::new(43),
        |c| c.previous_commit_hash = H,
        |c| c.commit_kind = CommitKind::MemberJoin,
        |c| c.state_version = StateVersion::new(41),
        |c| c.membership_epoch = MembershipEpoch::new(3),
        |c| c.host_peer_id = peer(0),
        |c| c.roster_hash = H,
        |c| c.logical_state_hash = H,
        |c| c.next_join_order = JoinOrder::new(5),
        |c| c.migration_eligibility_digest = H,
        |c| c.policy_metadata_digest = Some(Hash32::ZERO),
        |c| c.committed_action_id = None,
        |c| c.affected_peer_id = Some(peer(0)),
    ];
    for (i, m) in mutations.iter().enumerate() {
        let mut c = sample_commit();
        m(&mut c);
        assert_ne!(
            commit_hash(&c),
            base,
            "mutation {i} not bound by commit_hash"
        );
    }
}

#[test]
fn optional_presence_is_distinguished() {
    // None vs Some(empty) must differ for variable-length optionals.
    assert_ne!(
        representation_hash(RepresentationKind::Full, b"", None, None),
        representation_hash(RepresentationKind::Full, b"", Some(b""), None)
    );
    // Private vs recovery placement must differ.
    assert_ne!(
        representation_hash(RepresentationKind::PublicPrivate, b"", Some(b"x"), None),
        representation_hash(RepresentationKind::PublicPrivate, b"", None, Some(b"x"))
    );
}

#[test]
fn contexts_are_domain_separated() {
    // Same input bytes under different contexts must not collide.
    let a = logical_state_hash(b"");
    let b = derive(ctx::REPRESENTATION, &0u64.to_be_bytes());
    assert_ne!(a, b);
}

#[test]
fn golden_vectors() {
    let secret = ReconnectSecret::from_bytes([0x99; 32]);
    let roster = [RosterEntry {
        peer_id: peer(0xa0),
        join_order: JoinOrder::new(0),
        status: MemberStatus::Active,
        admission_commit_index: CommitIndex::new(1),
        reconnect_verifier: Hash32::from_bytes([0xa1; 32]),
        display_metadata: b"host",
    }];
    let cases = [
        ("logical_state", logical_state_hash(b"turnnet")),
        (
            "representation",
            representation_hash(RepresentationKind::Full, b"turnnet", None, None),
        ),
        ("roster", roster_hash(&roster)),
        ("eligibility", eligibility_digest(1, &[peer(1), peer(2)])),
        ("reconnect", reconnect_verifier(sid(), peer(3), &secret)),
        ("commit", commit_hash(&sample_commit())),
    ];
    let mismatches: Vec<String> = cases
        .iter()
        .zip(GOLDEN.iter())
        .filter(|((_, got), (_, want))| got.to_hex() != *want)
        .map(|((name, got), _)| format!("    (\"{name}\", \"{}\"),", got.to_hex()))
        .collect();
    assert!(
        mismatches.is_empty(),
        "golden vectors changed; this breaks wire compatibility. Actual values:\n{}",
        mismatches.join("\n")
    );
    for ((name, _), (ename, _)) in cases.iter().zip(GOLDEN.iter()) {
        assert_eq!(name, ename);
    }
}

// Pinned 2026-10-02. The `*_layout` tests above independently confirm the
// byte layouts these digests are computed over.
const GOLDEN: [(&str, &str); 6] = [
    (
        "logical_state",
        "d130fbf66d6b29da35389a1b9648bde549b664b4c795ebb41325ebf3e9bdbdb3",
    ),
    (
        "representation",
        "d8ebb13f7cd743f045d341f4a42eea9eff9b397d9aa46221cdd76b75aab96063",
    ),
    (
        "roster",
        "e2699507d27f053c8e7ad18df768713dc4be70183c9b1df4bcfb09627ca81f42",
    ),
    (
        "eligibility",
        "1bccf1727a74a80e3aaa0c627b39d2fa38513802652164facd4b0f9595569be5",
    ),
    (
        "reconnect",
        "f3cd46ff6b3e5dc285cf1f7712a25e007f3e573ce10b2e78c6c0fc0989e797bb",
    ),
    (
        "commit",
        "ff5124814a869df6824221aad5613c0daf8a68a7f696ac629711ac2ce4772c87",
    ),
];
