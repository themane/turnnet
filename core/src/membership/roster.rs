//! Committed roster (Implementation Spec §20, plan §4.3).
//!
//! The roster holds every member ever admitted: `Active` ones plus `Left` and
//! `Removed` tombstones, so a `join_order` is never reused. It changes only
//! through committed membership commits; connectivity never touches it.

use thiserror::Error;

use super::MemberStatus;
use crate::hash::{self, RosterEntry};
use crate::ids::{CommitIndex, Hash32, JoinOrder, PeerId};

/// Upper bound on app-owned display metadata per member.
pub const MAX_DISPLAY_METADATA: usize = 256;

/// One committed roster record.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PeerMembership {
    pub peer_id: PeerId,
    pub join_order: JoinOrder,
    pub status: MemberStatus,
    pub admission_commit_index: CommitIndex,
    /// `BLAKE3`-based verifier of the peer's reconnect secret (C-11).
    pub reconnect_verifier: Hash32,
    /// App-owned, at most [`MAX_DISPLAY_METADATA`] bytes.
    pub display_metadata: Vec<u8>,
}

impl PeerMembership {
    pub fn is_active(&self) -> bool {
        self.status == MemberStatus::Active
    }

    fn entry(&self) -> RosterEntry<'_> {
        RosterEntry {
            peer_id: self.peer_id,
            join_order: self.join_order,
            status: self.status,
            admission_commit_index: self.admission_commit_index,
            reconnect_verifier: self.reconnect_verifier,
            display_metadata: &self.display_metadata,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug, Error)]
pub enum RosterError {
    #[error("peer {0:?} is already in the roster")]
    DuplicatePeer(PeerId),
    #[error("peer {0:?} is not in the roster")]
    UnknownPeer(PeerId),
    #[error("peer {0:?} is not an active member")]
    NotActive(PeerId),
    #[error("display metadata is {len} bytes, limit is {MAX_DISPLAY_METADATA}")]
    MetadataTooLong { len: usize },
    #[error("join_order space exhausted")]
    JoinOrderExhausted,
    #[error("join_order {0} appears more than once")]
    DuplicateJoinOrder(u32),
    #[error("next_join_order {next} is not above the highest assigned join_order {highest}")]
    NextJoinOrderTooLow { next: u32, highest: u32 },
}

/// The committed membership set. Members are kept sorted by `join_order`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Roster {
    members: Vec<PeerMembership>,
    next_join_order: JoinOrder,
}

fn check_metadata(meta: &[u8]) -> Result<(), RosterError> {
    if meta.len() > MAX_DISPLAY_METADATA {
        return Err(RosterError::MetadataTooLong { len: meta.len() });
    }
    Ok(())
}

impl Roster {
    /// The roster of a newly created session: the creating host at `join_order` 0.
    pub fn new(
        host: PeerId,
        reconnect_verifier: Hash32,
        display_metadata: Vec<u8>,
        admission_commit_index: CommitIndex,
    ) -> Result<Self, RosterError> {
        check_metadata(&display_metadata)?;
        Ok(Self {
            members: vec![PeerMembership {
                peer_id: host,
                join_order: JoinOrder::FIRST_HOST,
                status: MemberStatus::Active,
                admission_commit_index,
                reconnect_verifier,
                display_metadata,
            }],
            next_join_order: JoinOrder::new(JoinOrder::FIRST_HOST.get() + 1),
        })
    }

    /// Rebuilds a roster received from the wire or a recovery blob, validating it.
    pub fn from_members(
        mut members: Vec<PeerMembership>,
        next_join_order: JoinOrder,
    ) -> Result<Self, RosterError> {
        members.sort_by_key(|m| m.join_order);
        for (i, m) in members.iter().enumerate() {
            check_metadata(&m.display_metadata)?;
            if i > 0 && members[i - 1].join_order == m.join_order {
                return Err(RosterError::DuplicateJoinOrder(m.join_order.get()));
            }
            if members[..i].iter().any(|o| o.peer_id == m.peer_id) {
                return Err(RosterError::DuplicatePeer(m.peer_id));
            }
        }
        if let Some(last) = members.last()
            && next_join_order <= last.join_order
        {
            return Err(RosterError::NextJoinOrderTooLow {
                next: next_join_order.get(),
                highest: last.join_order.get(),
            });
        }
        Ok(Self {
            members,
            next_join_order,
        })
    }

    /// Admits a new member at the next `join_order` and returns it.
    pub fn admit(
        &mut self,
        peer_id: PeerId,
        reconnect_verifier: Hash32,
        display_metadata: Vec<u8>,
        admission_commit_index: CommitIndex,
    ) -> Result<JoinOrder, RosterError> {
        check_metadata(&display_metadata)?;
        if self.get(peer_id).is_some() {
            return Err(RosterError::DuplicatePeer(peer_id));
        }
        let join_order = self.next_join_order;
        let next = join_order.next().ok_or(RosterError::JoinOrderExhausted)?;
        self.members.push(PeerMembership {
            peer_id,
            join_order,
            status: MemberStatus::Active,
            admission_commit_index,
            reconnect_verifier,
            display_metadata,
        });
        self.next_join_order = next;
        Ok(join_order)
    }

    /// Voluntary departure. The record stays as a tombstone.
    pub fn mark_left(&mut self, peer_id: PeerId) -> Result<(), RosterError> {
        self.retire(peer_id, MemberStatus::Left)
    }

    /// Forced removal. The record stays as a tombstone.
    pub fn mark_removed(&mut self, peer_id: PeerId) -> Result<(), RosterError> {
        self.retire(peer_id, MemberStatus::Removed)
    }

    fn retire(&mut self, peer_id: PeerId, status: MemberStatus) -> Result<(), RosterError> {
        let m = self
            .members
            .iter_mut()
            .find(|m| m.peer_id == peer_id)
            .ok_or(RosterError::UnknownPeer(peer_id))?;
        if !m.is_active() {
            return Err(RosterError::NotActive(peer_id));
        }
        m.status = status;
        Ok(())
    }

    pub fn get(&self, peer_id: PeerId) -> Option<&PeerMembership> {
        self.members.iter().find(|m| m.peer_id == peer_id)
    }

    pub fn is_active(&self, peer_id: PeerId) -> bool {
        self.get(peer_id).is_some_and(PeerMembership::is_active)
    }

    /// Every record including tombstones, ascending `join_order`.
    pub fn members(&self) -> &[PeerMembership] {
        &self.members
    }

    /// Active members only, ascending `join_order`.
    pub fn active(&self) -> impl Iterator<Item = &PeerMembership> {
        self.members.iter().filter(|m| m.is_active())
    }

    pub fn active_count(&self) -> usize {
        self.active().count()
    }

    /// The `join_order` the next admitted member will receive.
    pub fn next_join_order(&self) -> JoinOrder {
        self.next_join_order
    }

    /// `roster_hash` over all records including tombstones.
    pub fn hash(&self) -> Hash32 {
        let entries: Vec<RosterEntry<'_>> =
            self.members.iter().map(PeerMembership::entry).collect();
        hash::roster_hash(&entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn peer(b: u8) -> PeerId {
        PeerId::from_bytes([b; 16])
    }
    fn ver(b: u8) -> Hash32 {
        Hash32::from_bytes([b; 32])
    }
    fn ci(n: u64) -> CommitIndex {
        CommitIndex::new(n)
    }

    fn roster() -> Roster {
        Roster::new(peer(1), ver(1), b"host".to_vec(), ci(1)).unwrap()
    }

    #[test]
    fn host_is_join_order_zero() {
        let r = roster();
        assert_eq!(r.get(peer(1)).unwrap().join_order, JoinOrder::FIRST_HOST);
        assert_eq!(r.next_join_order().get(), 1);
        assert_eq!(r.active_count(), 1);
    }

    #[test]
    fn admit_assigns_monotonic_join_order() {
        let mut r = roster();
        assert_eq!(r.admit(peer(2), ver(2), vec![], ci(2)).unwrap().get(), 1);
        assert_eq!(r.admit(peer(3), ver(3), vec![], ci(3)).unwrap().get(), 2);
        assert_eq!(r.next_join_order().get(), 3);
        let orders: Vec<u32> = r.members().iter().map(|m| m.join_order.get()).collect();
        assert_eq!(orders, [0, 1, 2]);
    }

    #[test]
    fn join_order_is_never_reused_after_leave() {
        let mut r = roster();
        r.admit(peer(2), ver(2), vec![], ci(2)).unwrap();
        r.mark_left(peer(2)).unwrap();
        let jo = r.admit(peer(3), ver(3), vec![], ci(4)).unwrap();
        assert_eq!(jo.get(), 2);
        assert_eq!(r.members().len(), 3);
        assert_eq!(r.active_count(), 2);
    }

    #[test]
    fn duplicate_peer_rejected_even_after_leaving() {
        let mut r = roster();
        r.admit(peer(2), ver(2), vec![], ci(2)).unwrap();
        r.mark_removed(peer(2)).unwrap();
        assert_eq!(
            r.admit(peer(2), ver(2), vec![], ci(3)),
            Err(RosterError::DuplicatePeer(peer(2)))
        );
        assert_eq!(
            r.next_join_order().get(),
            2,
            "failed admit must not consume a join_order"
        );
    }

    #[test]
    fn retire_errors() {
        let mut r = roster();
        assert_eq!(r.mark_left(peer(9)), Err(RosterError::UnknownPeer(peer(9))));
        r.admit(peer(2), ver(2), vec![], ci(2)).unwrap();
        r.mark_left(peer(2)).unwrap();
        assert_eq!(
            r.mark_removed(peer(2)),
            Err(RosterError::NotActive(peer(2)))
        );
        assert_eq!(r.get(peer(2)).unwrap().status, MemberStatus::Left);
    }

    #[test]
    fn metadata_limit_enforced() {
        let mut r = roster();
        let ok = vec![0; MAX_DISPLAY_METADATA];
        let long = vec![0; MAX_DISPLAY_METADATA + 1];
        assert!(r.admit(peer(2), ver(2), ok, ci(2)).is_ok());
        assert_eq!(
            r.admit(peer(3), ver(3), long.clone(), ci(3)),
            Err(RosterError::MetadataTooLong { len: 257 })
        );
        assert!(Roster::new(peer(1), ver(1), long, ci(1)).is_err());
    }

    #[test]
    fn hash_changes_on_every_roster_change() {
        let mut r = roster();
        let mut seen = vec![r.hash()];
        r.admit(peer(2), ver(2), vec![], ci(2)).unwrap();
        seen.push(r.hash());
        r.mark_left(peer(2)).unwrap();
        seen.push(r.hash());
        r.admit(peer(3), ver(3), vec![], ci(4)).unwrap();
        seen.push(r.hash());
        let mut dedup = seen.clone();
        dedup.sort();
        dedup.dedup();
        assert_eq!(dedup.len(), seen.len());
    }

    #[test]
    fn hash_is_independent_of_construction_path() {
        let mut a = roster();
        a.admit(peer(2), ver(2), vec![], ci(2)).unwrap();
        a.admit(peer(3), ver(3), vec![], ci(3)).unwrap();
        let mut shuffled = a.members().to_vec();
        shuffled.reverse();
        let b = Roster::from_members(shuffled, a.next_join_order()).unwrap();
        assert_eq!(a, b);
        assert_eq!(a.hash(), b.hash());
    }

    #[test]
    fn from_members_validates() {
        let a = roster();
        let m = a.members()[0].clone();
        let mut dup_order = m.clone();
        dup_order.peer_id = peer(2);
        assert_eq!(
            Roster::from_members(vec![m.clone(), dup_order], JoinOrder::new(5)),
            Err(RosterError::DuplicateJoinOrder(0))
        );
        let mut dup_peer = m.clone();
        dup_peer.join_order = JoinOrder::new(1);
        assert_eq!(
            Roster::from_members(vec![m.clone(), dup_peer], JoinOrder::new(5)),
            Err(RosterError::DuplicatePeer(peer(1)))
        );
        assert_eq!(
            Roster::from_members(vec![m], JoinOrder::new(0)),
            Err(RosterError::NextJoinOrderTooLow {
                next: 0,
                highest: 0
            })
        );
    }

    #[test]
    fn join_order_exhaustion() {
        let m = roster().members()[0].clone();
        let mut r = Roster::from_members(vec![m], JoinOrder::new(u32::MAX)).unwrap();
        assert_eq!(
            r.admit(peer(2), ver(2), vec![], ci(2)),
            Err(RosterError::JoinOrderExhausted)
        );
    }
}
