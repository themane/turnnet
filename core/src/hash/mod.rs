//! Canonical hash encodings (plan §6, normative in `docs/protocol.md`).
//!
//! Every hash is BLAKE3 in `derive_key` mode with a per-purpose context string,
//! so digests of different kinds can never collide by construction. Inputs are
//! encoded deterministically:
//!
//! - integers are fixed-width big-endian;
//! - variable-length bytes are a `u64` big-endian length followed by the bytes;
//! - `Option<T>` is `0x00`, or `0x01` followed by `T`;
//! - lists are a `u32` big-endian count followed by the canonically sorted items.
//!
//! Changing anything here changes commit hashes and breaks interoperability.

use crate::commit::CommitKind;
use crate::ids::{
    ActionId, CommitIndex, Hash32, JoinOrder, MembershipEpoch, PeerId, ReconnectSecret, SessionId,
    StateVersion, Term,
};
use crate::membership::MemberStatus;
use crate::state::RepresentationKind;

/// BLAKE3 `derive_key` context strings.
pub mod ctx {
    pub const LOGICAL_STATE: &str = "turnnet v2 logical_state";
    pub const REPRESENTATION: &str = "turnnet v2 representation";
    pub const ROSTER: &str = "turnnet v2 roster";
    pub const ELIGIBILITY: &str = "turnnet v2 eligibility";
    pub const RECONNECT: &str = "turnnet v2 reconnect";
    pub const COMMIT: &str = "turnnet v2 commit";
}

/// Canonical field writer over a BLAKE3 hasher.
struct Canonical(blake3::Hasher);

impl Canonical {
    fn new(context: &str) -> Self {
        Self(blake3::Hasher::new_derive_key(context))
    }

    fn u8(&mut self, v: u8) -> &mut Self {
        self.0.update(&[v]);
        self
    }

    fn u32(&mut self, v: u32) -> &mut Self {
        self.0.update(&v.to_be_bytes());
        self
    }

    fn u64(&mut self, v: u64) -> &mut Self {
        self.0.update(&v.to_be_bytes());
        self
    }

    /// Fixed-width field (identifier or hash); no length prefix.
    fn fixed(&mut self, bytes: &[u8]) -> &mut Self {
        self.0.update(bytes);
        self
    }

    fn var(&mut self, bytes: &[u8]) -> &mut Self {
        self.u64(bytes.len() as u64);
        self.0.update(bytes);
        self
    }

    fn opt_fixed(&mut self, bytes: Option<&[u8]>) -> &mut Self {
        match bytes {
            None => self.u8(0),
            Some(b) => self.u8(1).fixed(b),
        }
    }

    fn opt_var(&mut self, bytes: Option<&[u8]>) -> &mut Self {
        match bytes {
            None => self.u8(0),
            Some(b) => self.u8(1).var(b),
        }
    }

    fn finish(&self) -> Hash32 {
        self.0.finalize().into()
    }
}

/// Hash of the canonical logical authoritative game state.
pub fn logical_state_hash(state: &[u8]) -> Hash32 {
    Canonical::new(ctx::LOGICAL_STATE).var(state).finish()
}

/// Hash of the exact representation delivered to one peer.
pub fn representation_hash(
    kind: RepresentationKind,
    public_state: &[u8],
    private_state: Option<&[u8]>,
    recovery_material: Option<&[u8]>,
) -> Hash32 {
    Canonical::new(ctx::REPRESENTATION)
        .u8(kind.code())
        .var(public_state)
        .opt_var(private_state)
        .opt_var(recovery_material)
        .finish()
}

/// Borrowed view of one roster member, as hashed.
#[derive(Clone, Copy, Debug)]
pub struct RosterEntry<'a> {
    pub peer_id: PeerId,
    pub join_order: JoinOrder,
    pub status: MemberStatus,
    pub admission_commit_index: CommitIndex,
    pub reconnect_verifier: Hash32,
    pub display_metadata: &'a [u8],
}

/// Hash of the committed roster, including departed-member tombstones.
/// Entries are hashed in ascending `join_order` regardless of input order.
pub fn roster_hash(entries: &[RosterEntry<'_>]) -> Hash32 {
    let mut sorted: Vec<&RosterEntry<'_>> = entries.iter().collect();
    sorted.sort_by_key(|e| e.join_order);
    let mut h = Canonical::new(ctx::ROSTER);
    h.u32(sorted.len() as u32);
    for e in sorted {
        h.fixed(e.peer_id.as_bytes())
            .u32(e.join_order.get())
            .u8(e.status.code())
            .u64(e.admission_commit_index.get())
            .fixed(e.reconnect_verifier.as_bytes())
            .var(e.display_metadata);
    }
    h.finish()
}

/// Digest of the migration-eligible peer set for a commit under `policy_id`.
/// The set is sorted and de-duplicated before hashing.
pub fn eligibility_digest(policy_id: u32, eligible: &[PeerId]) -> Hash32 {
    let mut peers = eligible.to_vec();
    peers.sort_unstable();
    peers.dedup();
    let mut h = Canonical::new(ctx::ELIGIBILITY);
    h.u32(policy_id).u32(peers.len() as u32);
    for p in &peers {
        h.fixed(p.as_bytes());
    }
    h.finish()
}

/// Verifier stored in the committed roster for a peer's reconnect secret.
pub fn reconnect_verifier(
    session_id: SessionId,
    peer_id: PeerId,
    secret: &ReconnectSecret,
) -> Hash32 {
    Canonical::new(ctx::RECONNECT)
        .fixed(session_id.as_bytes())
        .fixed(peer_id.as_bytes())
        .fixed(secret.expose())
        .finish()
}

/// Fields bound by `commit_hash` (Implementation Spec §21.3).
#[derive(Clone, Copy, Debug)]
pub struct CommitHashInput {
    pub session_id: SessionId,
    pub term: Term,
    pub commit_index: CommitIndex,
    pub previous_commit_hash: Hash32,
    pub commit_kind: CommitKind,
    pub state_version: StateVersion,
    pub membership_epoch: MembershipEpoch,
    pub host_peer_id: PeerId,
    pub roster_hash: Hash32,
    pub logical_state_hash: Hash32,
    pub next_join_order: JoinOrder,
    pub migration_eligibility_digest: Hash32,
    pub policy_metadata_digest: Option<Hash32>,
    pub committed_action_id: Option<ActionId>,
    pub affected_peer_id: Option<PeerId>,
}

/// Chained hash of authoritative commit metadata.
pub fn commit_hash(c: &CommitHashInput) -> Hash32 {
    Canonical::new(ctx::COMMIT)
        .fixed(c.session_id.as_bytes())
        .u64(c.term.get())
        .u64(c.commit_index.get())
        .fixed(c.previous_commit_hash.as_bytes())
        .u8(c.commit_kind.code())
        .u64(c.state_version.get())
        .u64(c.membership_epoch.get())
        .fixed(c.host_peer_id.as_bytes())
        .fixed(c.roster_hash.as_bytes())
        .fixed(c.logical_state_hash.as_bytes())
        .u32(c.next_join_order.get())
        .fixed(c.migration_eligibility_digest.as_bytes())
        .opt_fixed(c.policy_metadata_digest.as_ref().map(|h| &h.as_bytes()[..]))
        .opt_fixed(c.committed_action_id.as_ref().map(|a| &a.as_bytes()[..]))
        .opt_fixed(c.affected_peer_id.as_ref().map(|p| &p.as_bytes()[..]))
        .finish()
}

#[cfg(test)]
mod tests;
