---
document: turnnet_v2_phase1_implementation_plan
version: 1.0
status: draft_for_approval
date: 2026-10-02
working_name: TurnNet
derived_from:
  - Turn_Based_Multiplayer_Harness_V2_Decisions.md
  - Turn_Based_Multiplayer_Harness_V2_Implementation_Spec.md
  - Turn_Based_Multiplayer_Harness_V2_AI_Context.md
scope: phase_1_semantics_first
---

# TurnNet V2 - Phase 1 Implementation Plan

This is the final pre-implementation document for **Phase 1 ("semantics first")** of the TurnNet V2 spec (Implementation Spec §64, steps 1–15). It records the clarifications agreed before coding, the concrete Rust design, the wire and hash encodings, the algorithms, and the test plan.

Precedence: the V2 Decisions file > the V2 Implementation Spec > this plan. Where this plan resolves something the spec leaves open, the item is marked **[C-n]** (clarification) and is listed in §2.

---

## 1. Phase 1 scope

### In scope

1. Rust V2 identity types.
2. `commit_index` plus the BLAKE3 commit hash chain.
3. Separation of membership from connectivity.
4. V2 protobuf envelope and all message definitions.
5. A deterministic, sans-IO session state machine.
6. A simulated peer link, discovery, deterministic clock and RNG.
7. Create/join for 2–8 peers.
8. Action routing and `action_id` dedupe.
9. Full logical state commits and ACKs.
10. Committed membership and monotonic `join_order`.
11. The state visibility/recovery policy abstraction, with trusted full replication.
12. `host_capable` and `migration_eligible`.
13. Host suspicion, election and the migration barrier.
14. Split-brain conflict fencing.
15. Reconnect, route rebinding and the recovery blob.
16. The full core test suite (Spec §55, AI Context §31) at 2, 4 and 8 peers, run over many seeds.

### Out of scope (later phases; design hooks only)

- Iroh, the ALPN, QUIC streams, the route watcher, mDNS (Phases 2–3).
- Internet invite encoding of Iroh `EndpointAddr`, and relay config (Phase 4).
- C ABI, Kotlin/Swift, Nearby, Wi-Fi Aware (Phases 5–6).
- Dedicated state-transfer streams. In Phase 1, state travels inline in control frames. The `STATE_TRANSFER_BEGIN` header is defined in the proto but not used yet.

---

## 2. Agreed clarifications

| ID | Topic | Decision |
|---|---|---|
| C-1 | Phase scope | Phase 1 only (this document). |
| C-2 | Hash | **BLAKE3**, 32-byte digests, domain-separated with `derive_key` contexts (§6). |
| C-3 | Core model | **Sans-IO** deterministic state machine. Clock and RNG are injected. No async runtime in `turnnet-core`. |
| C-4 | Toolchain | The user installs Rust and `protoc`. The build uses `prost-build` with system `protoc`. No git repo or toolchain is set up by the agent. |
| C-5 | Late join | Admission is controlled by an app-owned `joinable` flag plus `roster_size < max_players`. The core has no lobby or "game started" phase. |
| C-6 | Grace expiry | When a member stays unreachable past `reconnect_grace_period`, the core emits `PeerReconnectGraceExpired`. The peer stays a member until the app calls `remove_peer` (a `MEMBER_REMOVE` commit). |
| C-7 | Solo survivor | If no other member is reachable for migration, the peer goes `INTERRUPTED` and keeps accepting reconnects. If nobody returns within `reconnect_grace_period`, it emits `SessionEnded(NoSurvivors)`. |
| C-8 | Stale action context | `StaleActionPolicy` is configurable. The default is `Reject`: a mismatched `expected_state_version` gets `ACTION_REJECT(STALE_CONTEXT)`. The alternative is `Advisory`. |
| C-9 | Layout | Cargo workspace in `turnnet/`. Specs live in `turnnet/docs/`. |
| C-10 | History retention | Every peer keeps **all `CommitMeta` for the session** plus the full state representation for the **head only**. |
| C-11 | Reconnect auth | **Replicated verifier.** The admitting host issues a random 256-bit reconnect secret. The committed roster stores `BLAKE3(secret)`, so any future host can verify it. The secret itself lives only in the peer's memory and recovery blob. |
| C-12 | Crate metadata | Edition 2024, `MIT OR Apache-2.0`. |
| C-13 | Orphan tail | **Unacked tail discard.** A returning former host may roll back commits that **it authored** and that **no other peer ACKed** to the new term's base, emitting a `DiagnosticEvent(OrphanTailDiscarded)`. Any commit that another peer ACKed or holds is never discarded; divergence there is fenced (HM-007/HM-008). |

Derived design choices (no question needed, listed so reviewers can see them):

| ID | Topic | Choice |
|---|---|---|
| C-14 | Action dedupe source | The committed `action_id` set is **derived from the retained commit chain** (each `GAME_ACTION` commit binds `committed_action_id`). This satisfies ACT-002/ACT-003 across migration with no separate replicated structure. |
| C-15 | Eligibility metadata | `CommitMeta` carries the full sorted `migration_eligible_peers` list (at most 8 × 16 bytes). `migration_eligibility_digest` is its hash. Every peer can make the same election decision (PRIV-006). |
| C-16 | Invites across migration | Join capabilities are **host-local** and are not replicated. After migration, invites from the previous host are invalid. LAN and discovered-session joins use `JoinPolicy::Open` or a fresh invite from the new host. |
| C-17 | Message dedupe | Each sender has a bounded LRU of recent `message_id`s (default 4096). This is safe because correctness-critical idempotency is enforced by `action_id`, `commit_index` and `commit_hash`. |
| C-18 | Non-host reconnect target | A non-host that receives `RECONNECT_HELLO` answers `RECONNECT_REJECT{NOT_AUTHORITY}` with the current host's `peer_id`, term and route. The reconnecting peer then dials the host. |
| C-19 | Envelope body | The protobuf `oneof body` replaces the conceptual `message_type` + `payload` pair. An unset or unknown `oneof` is treated as an unknown message type. |

**Doc sync (DOC-003):** on approval, C-6, C-7, C-11, C-13, C-14 and C-16 will be added to the V2 Decisions file as `HM-011`, `REC-006`, and so on, in the first implementation change.

---

## 3. Repository layout

```text
turnnet/
  Cargo.toml                  # workspace; edition 2024; resolver 3
  LICENSE-MIT
  LICENSE-APACHE
  README.md
  docs/
    Turn_Based_Multiplayer_Harness_V2_*.md
    Phase1_Implementation_Plan.md
    protocol.md               # written during Phase 1 (wire + canonical encodings)
    state_commit_model.md
    host_migration.md
    recovery.md
  core/                       # crate: turnnet-core
    Cargo.toml
    build.rs                  # prost-build
    proto/turnnet/v2/turnnet.proto
    src/
      lib.rs
      ids/                    # SessionId, PeerId, ActionId, MessageId, Term, CommitIndex, ...
      hash/                   # BLAKE3 contexts + canonical encoders
      commit/                 # CommitMeta, CommitKind, CommitChain, reconcile()
      membership/             # Roster, PeerMembership, MemberStatus, JoinOrder alloc
      connectivity/           # PeerConnectivity, liveness tracking
      actions/                # pending actions (client), dedupe lookup, validation order
      state/                  # head state, StateRepresentation, ACK tracking
      visibility/             # StateReplicationPolicy trait + TrustedFullReplication
      game/                   # GameAdapter trait
      election/               # suspicion, candidate selection, migration barrier
      conflict/               # fencing + branch diagnostics
      recovery/               # RecoveryBlob encode/decode, restore
      routes/                 # RouteBook, PeerRouteBinding, RouteDescriptor
      invite/                 # TurnNetInvite (transport-neutral), JoinCapability store
      protocol/               # frame codec, envelope validation, prost types
      diagnostics/            # DiagnosticEvent, categories, redaction
      peer_link/              # PeerLink trait (transport contract for later runtimes)
      discovery/              # DiscoveryProvider trait, normalized events
      session/                # SessionCore: Input/Output/Command, state machine
      config/                 # ReliabilityPolicy, FramePolicy, StaleActionPolicy, JoinPolicy
    tests/                    # integration tests driving the simulator
  sim/                        # crate: turnnet-sim (dev/test only, publish = false)
    src/
      clock.rs                # virtual ms clock
      network.rs              # SimNetwork: links, faults, partitions, event queue
      discovery.rs            # SimDiscovery
      node.rs                 # SimNode = SessionCore + test GameAdapter + policy
      harness.rs              # Scenario builder, run_until, assertions
      fixtures/               # CounterGame, HiddenCardGame + PublicPrivatePolicy
```

`turnnet-sim` is a separate crate so the shipping core contains no simulation code. It depends on `turnnet-core` with no special feature flags.

### Dependencies (core)

| Crate | Purpose |
|---|---|
| `prost`, `bytes` | protobuf runtime |
| `prost-build` (build) | codegen; requires system `protoc` (C-4) |
| `blake3` | hashing |
| `rand_core` | `RngCore` trait for injected randomness |
| `thiserror` | error types |

`turnnet-sim` adds `rand_chacha` (seeded `ChaCha8Rng`). Dev-dependencies: `proptest` (codec/fuzz-style tests).

---

## 4. Core types

### 4.1 Identifiers (`ids/`)

```rust
pub struct SessionId([u8; 16]);      // random
pub struct PeerId([u8; 16]);         // random, chosen by joining client
pub struct ActionId([u8; 16]);       // random, stable across retries/migration
pub struct MessageId([u8; 16]);      // random per message
pub struct JoinOrder(u32);           // first host = 0
pub struct Term(u64);                // initial = 1
pub struct StateVersion(u64);        // initial = 0
pub struct MembershipEpoch(u64);     // initial = 1
pub struct CommitIndex(u64);         // initial commit = 1
pub struct Hash32([u8; 32]);
pub struct RouteGeneration(u64);
```

All of them are newtypes with no arithmetic beyond checked `next()`. `Debug`/`Display` output is redacted (first 4 hex chars) unless the `unredacted-debug` feature is on.

### 4.2 Commit metadata (`commit/`)

```rust
pub enum CommitKind { InitialState=1, GameAction=2, MemberJoin=3, MemberLeave=4,
                      MemberRemove=5, PolicyMetadataChange=6 }

pub struct CommitMeta {
    session_id, term, commit_index, previous_commit_hash: Hash32, // ZERO for index 1
    commit_kind, state_version, membership_epoch, host_peer_id,
    roster_hash, logical_state_hash, next_join_order: JoinOrder,
    migration_eligible_peers: Vec<PeerId>,      // sorted (C-15)
    migration_eligibility_digest: Hash32,
    policy_metadata_digest: Option<Hash32>,
    committed_action_id: Option<ActionId>,      // GameAction only
    affected_peer_id: Option<PeerId>,           // Member* only
    commit_hash: Hash32,
}

pub struct CommitChain {               // all metadata for the session (C-10)
    metas: Vec<CommitMeta>,            // metas[i].commit_index == i+1
    action_index: HashMap<ActionId, CommitIndex>,   // C-14
    acked_by_others: BitSet,           // host-authored commits ACKed by >=1 other peer (C-13)
}
```

The roster is carried in each membership commit's payload, and peers keep the current roster alongside the chain. `roster_hash` binds it.

### 4.3 Membership vs connectivity

```rust
pub struct PeerMembership {
    peer_id, join_order, status: MemberStatus /* Active | Left | Removed */,
    admission_commit_index, reconnect_verifier: Hash32 /* C-11 */,
    display_metadata: Bytes /* app-owned, bounded 256 B */,
}
pub enum PeerConnectivity { Connected, Degraded, Reconnecting, Interrupted, Unreachable }
```

`Roster` holds only `Active` members plus tombstones for departed ones, so `join_order` is never reused. Connectivity lives in `connectivity/` and never touches `membership_epoch`.

### 4.4 Game and policy traits

```rust
pub trait GameAdapter {
    fn validate_and_apply(&mut self, actor: PeerId, action: &[u8], state: &[u8])
        -> ActionOutcome;                     // Accepted{new_state, event} | Rejected{reason_code: u32}
    fn on_state_committed(&mut self, visible: &StateRepresentation, sv: StateVersion, ci: CommitIndex);
    fn on_session_event(&mut self, event: &SessionEvent);
}

pub trait StateReplicationPolicy {
    fn policy_id(&self) -> u32;
    fn build_public_state(&self, auth: &[u8], ctx: &CommitContext) -> Bytes;
    fn build_private_state_for(&self, peer: PeerId, auth: &[u8], ctx: &CommitContext) -> Option<Bytes>;
    fn build_recovery_material_for(&self, peer: PeerId, auth: &[u8], ctx: &CommitContext) -> Option<Bytes>;
    fn migration_capability_for(&self, peer: PeerId, ctx: &CommitContext) -> bool;
    fn reconstruct_authoritative_state(&self, rep: &StateRepresentation, ctx: &CommitContext)
        -> Result<Bytes, ReconstructError>;
}
```

The library ships `TrustedFullReplication`: public = full state, everyone is eligible, and reconstruct returns the public bytes. `turnnet-sim/fixtures` ships `PublicPrivatePolicy`, which uses hidden hands and grants recovery material to a configurable subset, to cover tests 28–30. AEAD-encrypted recovery is representable because `recovery` is opaque bytes, but Phase 1 does not implement it.

### 4.5 Sans-IO session core (`session/`)

```rust
pub struct SessionCore { /* all state; no I/O */ }

pub enum Input {
    LinkUp   { link: LinkId, observed_identity: TransportIdentity, inbound: bool },
    LinkDown { link: LinkId, reason: LinkDownReason },
    Frame    { link: LinkId, bytes: Bytes },
    Discovery(DiscoveryEvent),
    Lifecycle(LifecycleEvent),       // Foreground | Background | NetworkUnavailable ...
    Tick,                            // driver calls at/after next_deadline()
    Command(Command),
}

pub enum Command {
    StartDiscovery(DiscoveryFilter), StopDiscovery,
    JoinDiscovered(DiscoveredSession), JoinInvite(TurnNetInvite),
    SubmitAction { payload: Bytes, action_id: Option<ActionId> },
    SetJoinable(bool), CreateInvite(InviteOptions), RevokeInvite(InviteId),
    RemovePeer(PeerId), Leave, End(EndReason),
}

pub enum Output {
    Send    { link: LinkId, frame: Bytes },
    Connect { route: RouteDescriptor, purpose: ConnectPurpose },   // driver returns LinkUp
    Close   { link: LinkId, reason: CloseReason },
    Advertise(Option<SessionAdvertisement>),
    Event(SessionEvent),
    Diagnostic(DiagnosticEvent),
}

impl SessionCore {
    pub fn create(opts: CreateSessionOptions, env: Env) -> (Self, Vec<Output>);
    pub fn new_idle(env: Env) -> Self;                       // for discover/join
    pub fn restore(blob: &[u8], env: Env) -> Result<(Self, Vec<Output>), RecoveryError>;
    pub fn handle(&mut self, now: Millis, input: Input,
                  game: &mut dyn GameAdapter) -> Vec<Output>;
    pub fn next_deadline(&self) -> Option<Millis>;
    // queries
    pub fn state(&self) -> SessionState;
    pub fn info(&self) -> SessionInfo;
    pub fn peers(&self) -> Vec<PeerView>;
    pub fn current_host(&self) -> Option<PeerId>;
    pub fn export_recovery_blob(&self) -> Bytes;
}

pub struct Env { pub rng: Box<dyn RngCore + Send>, pub policy: Box<dyn StateReplicationPolicy + Send>,
                 pub config: SessionConfig }
```

Transport callbacks only produce `Input`s. The game adapter is invoked solely from inside `handle` (ARCH-008).

`SessionState` matches Spec §45 exactly. Path state (`PathType`) is a separate per-link field.

### 4.6 Config defaults (`config/`)

| Field | Default |
|---|---|
| `heartbeat_interval` | 2 000 ms |
| `peer_suspect_after` | 6 000 ms |
| `host_election_after` | 8 000 ms |
| `election_round_timeout` | 4 000 ms (new; bounds a stalled proposal) |
| `reconnect_grace_period` | 60 000 ms |
| `action_response_timeout` | 10 000 ms |
| `join_timeout` | 20 000 ms |
| `max_join_retries` | 3 |
| `stale_action_policy` | `Reject` (C-8) |
| `join_policy` | `Open` \| `RequireCapability` |
| `max_control_frame` | 64 KiB (library hard limit, not app-raisable) |
| `max_state_payload` | 1 MiB default, hard max 16 MiB |
| `max_action_payload` | 64 KiB default |
| `max_pending_outbound_bytes` | 4 MiB per link |
| `message_dedupe_window` | 4096 per sender |
| `min_activation_members` | 2 (HM-006); fixed in Phase 1 |

---

## 5. Wire protocol (`core/proto/turnnet/v2/turnnet.proto`)

### 5.1 Framing

`u32` big-endian length followed by the encoded `Envelope`. Validation checks the length against `max_control_frame` **before** allocating. A zero or oversized length closes the link with `ERROR{FRAME_TOO_LARGE|MALFORMED}`.

### 5.2 Envelope

```proto
syntax = "proto3";
package turnnet.v2;

message Envelope {
  uint32 protocol_major = 1;           // = 2
  uint32 protocol_minor = 2;           // = 0
  bytes  session_id = 3;               // 16 bytes or empty (pre-session probe)
  uint64 term = 4;
  bytes  message_id = 5;               // 16 bytes
  bytes  sender_peer_id = 6;           // 16 bytes or empty (pre-admission)
  optional uint64 commit_index = 7;
  optional uint64 state_version = 8;
  optional uint64 membership_epoch = 9;
  optional bytes  action_id = 10;
  oneof body {                          // C-19
    Hello hello = 20;                    SessionProbe session_probe = 21;
    SessionAdvertisement session_advertisement = 22;
    JoinRequest join_request = 23;       JoinAccept join_accept = 24;
    JoinReject join_reject = 25;         ReconnectHello reconnect_hello = 26;
    ReconnectAccept reconnect_accept = 27; ReconnectReject reconnect_reject = 28;
    PeerRoster peer_roster = 29;         PeerRouteUpdate peer_route_update = 30;
    Heartbeat heartbeat = 31;            ActionSubmit action_submit = 32;
    ActionReject action_reject = 33;     StateCommit state_commit = 34;
    StateAck state_ack = 35;             StateSyncRequest state_sync_request = 36;
    StateSyncResponse state_sync_response = 37; HostSuspected host_suspected = 38;
    HostPropose host_propose = 39;       HostAck host_ack = 40;
    HostAnnounce host_announce = 41;     HostTransfer host_transfer = 42;
    AuthorityConflict authority_conflict = 43; PeerLeft peer_left = 44;
    SessionEnd session_end = 45;         Error error = 46;
    StateTransferBegin state_transfer_begin = 47;   // reserved for Phase 2
  }
}
```

There is no `SIGNALING_RELAY`. Notable message bodies:

- `CommitMetaPb`: mirrors §4.2 field by field.
- `StateRepresentation { kind; public; optional private; optional recovery; representation_hash }`
- `StateCommit { CommitMetaPb meta; optional RosterPb roster /* Member* commits */; StateRepresentation rep; }`, built per recipient.
- `StateSyncResponse { repeated CommitMetaPb metas /* from requested index */; RosterPb roster; StateRepresentation head_rep; }`
- `Heartbeat { head_index; head_hash; host_capable; migration_eligible; }`
- `HostSuspected` / `HostPropose` / `HostAck` / `HostAnnounce`: `new_term, candidate_peer_id, base_commit_index, base_commit_hash, base_state_version, membership_epoch`.
- `JoinAccept { assigned_join_order; reconnect_secret /* 32 B, sent only to the joiner */; roster; commit metas (full chain, C-10); head_rep; current_host; routes; }`
- `ReconnectHello { peer_id; reconnect_secret; head_index; head_hash; route; }`

Phase 1 sends the secret over the (simulated) transport. Later Iroh/Nearby links are authenticated and encrypted at the transport.

### 5.3 Validation order (Spec §39.2)

1. Frame length.
2. Decode.
3. `body` set.
4. Major == 2, otherwise `ERROR{PROTOCOL_MISMATCH}` and close.
5. `session_id`.
6. Sender binding: the link's bound `peer_id` must equal `sender_peer_id`, and the observed transport identity must match the binding.
7. Term rules.
8. Membership/commit context.
9. Body-specific payload limits.
10. `message_id` dedupe.

Every rejection emits a categorized diagnostic. None of them mutates state.

---

## 6. Hash and canonical encoding (`hash/`)

All hashes use `blake3::Hasher::new_derive_key(CTX)`. Integers are fixed-width big-endian. An `Option<T>` is encoded as `0x00`, or `0x01` followed by T. Variable-length bytes are encoded as a `u32` length followed by the bytes.

| Hash | Context string | Input |
|---|---|---|
| `logical_state_hash` | `"turnnet v2 logical_state"` | `len‖state_bytes` |
| `representation_hash` | `"turnnet v2 representation"` | `kind:u8 ‖ len‖public ‖ opt(len‖private) ‖ opt(len‖recovery)` |
| `roster_hash` | `"turnnet v2 roster"` | `count:u8`, then per member sorted by `join_order`: `peer_id ‖ join_order:u32 ‖ status:u8 ‖ admission_ci:u64 ‖ reconnect_verifier ‖ len‖display_metadata` |
| `migration_eligibility_digest` | `"turnnet v2 eligibility"` | `policy_id:u32 ‖ count:u8 ‖ sorted peer_ids` |
| `reconnect_verifier` | `"turnnet v2 reconnect"` | `session_id ‖ peer_id ‖ secret` |
| `commit_hash` | `"turnnet v2 commit"` | `session_id ‖ term ‖ commit_index ‖ previous_commit_hash ‖ kind:u8 ‖ state_version ‖ membership_epoch ‖ host_peer_id ‖ roster_hash ‖ logical_state_hash ‖ next_join_order:u32 ‖ migration_eligibility_digest ‖ opt(policy_metadata_digest) ‖ opt(committed_action_id) ‖ opt(affected_peer_id)` |

Golden-vector tests will pin these encodings. `docs/protocol.md` will restate them as normative.

---

## 7. Algorithms

### 7.1 Create

The host generates `session_id` and its own `peer_id` and gets `join_order = 0`. It then commits `INITIAL_STATE` with `ci=1, term=1, sv=0, epoch=1, next_join_order=1`. State becomes `HOSTING`, and after the first peer joins, `CONNECTED_HOST`. A host alone stays `HOSTING`.

### 7.2 Discovery and join

1. Discovery yields an endpoint route.
2. The core connects and sends `HELLO`, then `SESSION_PROBE`.
3. The host replies with a sanitized `SESSION_ADVERTISEMENT`: id, protocol, app id, player count, max, joinable, host display metadata. No secrets.
4. The core emits `SessionDiscovered`.
5. On `JoinDiscovered`/`JoinInvite`, the client generates a candidate `peer_id` and sends `JOIN_REQUEST`, including the capability if required.
6. The host checks, in order: joinable (C-5), capacity < max_players, policy/capability (expiry, revocation, max uses), and that the candidate id is unused.
7. On success the host generates a reconnect secret, commits `MEMBER_JOIN` (epoch+1, ci+1, `next_join_order`+1, new roster hash, eligibility recomputed), sends `JOIN_ACCEPT` to the joiner and `STATE_COMMIT` to everyone else, and emits `PeerJoined`.
8. On rejection, nothing is committed.
9. The new member receives the roster and route book and dials every other member to complete the mesh (`ConnectPurpose::Mesh`). Each mesh link does `HELLO` plus a lightweight member-auth step: `ReconnectHello` semantics with a `mesh=true` flag, verified against the replicated verifier.

### 7.3 Action submission

Client side:

1. `SubmitAction` stores `PendingAction{action_id, payload, first_sent, attempts}`.
2. The client sends `ACTION_SUBMIT` with the expected term/ci/sv to the current host.
3. On `action_response_timeout`, or when the host changes, it resends with the **same** `action_id` and a new `message_id`.
4. A pending action resolves on a `STATE_COMMIT` whose `committed_action_id` matches, or on `ACTION_REJECT`.

Host side follows Spec §25.2 in order. If the chain's `action_index` already contains the `action_id`, the host does not call the game. It replies with a `STATE_SYNC_RESPONSE` covering the requester's gap, or re-sends that commit's meta, so the requester learns the result. When accepted, the host runs the Spec §25.3 steps in one atomic step inside `handle`. The host's own actions take the same path locally.

### 7.4 ACKs

Followers verify every received commit before adopting it:

- `previous_commit_hash` equals their head hash;
- the indices are contiguous;
- `commit_hash` recomputes;
- the `representation_hash` matches.
- Under full replication, they also check `logical_state_hash` against the state.

After adopting, they send `STATE_ACK`. A gap triggers `STATE_SYNC_REQUEST(from = head+1)`. A hash mismatch raises a `StateDesync` diagnostic and a resync. The host records per-peer ACKs and sets `acked_by_others` (C-13).

### 7.5 Liveness

- Heartbeats go out on every mesh link every `heartbeat_interval`.
- Any valid frame refreshes liveness.
- A peer with no frames for `peer_suspect_after` becomes `Degraded`/`Unreachable`. This is connectivity only.
- When the host is silent for `host_election_after`, the state becomes `HOST_SUSPECTED` and the peer broadcasts `HOST_SUSPECTED` with its head.

A single `LinkDown` does not mutate authority. It marks connectivity, starts reconnect attempts to that peer, and lets the timers decide.

### 7.6 Election and migration barrier

Each suspecting peer collects views (head, `host_capable`) from reachable members for one `heartbeat_interval`. Then:

1. **Base.** Take the highest head among compatible views. All heads must lie on one chain, which is checkable because every peer keeps all metas (C-10). If heads are incompatible, go to `AUTHORITY_CONFLICT` (§7.8).
2. **Candidates.** Reachable active members, self included, with `host_capable` that appear in `base.migration_eligible_peers`, minus the suspected host. Pick the lowest `join_order`.
3. **If I am the candidate.** If behind, fetch the missing metas and head representation from a peer holding the base (`STATE_SYNC_REQUEST`). Reconstruct the authoritative state through the policy. Send `HOST_PROPOSE{term+1, base…}`.
4. **Peers receiving a proposal.** A peer ACKs if the term is greater than its own, the base is compatible with and at least as high as its head, the proposer is eligible, and the peer knows of no lower-`join_order` eligible reachable member. Before activation, a peer may switch its ACK to a lower-`join_order` proposer for the same term.
5. **Activation.** At least one ACK from another member (≥2 including the candidate, HM-006). The candidate sends `HOST_ANNOUNCE`, becomes `CONNECTED_HOST`, sends any behind peers their missing commits, and emits `HostChanged`. Followers adopt the term, become `CONNECTED_CLIENT`, and reroute pending actions.
6. **Stall.** If there is no progress within `election_round_timeout`, mark the unresponsive candidate unreachable for this round, emit `HostMigrationFailed` (round), and recompute. If no other member is reachable, go `INTERRUPTED` and follow C-7.

Gameplay commits are paused for the whole time between `HOST_SUSPECTED` and the announcement.

### 7.7 Graceful leave and transfer

- **Client leave.** The client sends `PEER_LEFT`. The host commits `MEMBER_LEAVE`.
- **Host leave.** The host picks a successor with the §7.6 ordering, excluding itself, and sends `HOST_TRANSFER{term+1, base = head}`. After the successor's ACK, the successor announces and then commits `MEMBER_LEAVE(old host)` as the first commit of the new term. With no eligible successor, the host emits `SessionEnded` and sends `SESSION_END`.

### 7.8 Authority acceptance and conflict fencing

`reconcile(local_chain, claim{term, base_ci, base_hash})` returns one of:

- **Compatible**: `local[base_ci].hash == base_hash` and the local head is at or below `base_ci`.
- **Behind**: local head < `base_ci`. Fetch metas and verify that the `previous_hash` linkage reaches the local head.
- **CompatibleWithOrphanTail**: matches at `base_ci`, but local has commits beyond it **authored by me as host and not `acked_by_others`**. Truncate to base and emit `OrphanTailDiscarded` (C-13).
- **Conflict**: anything else, or a tail that someone ACKed.

Conflict triggers (Spec §30.1) are: two `HOST_ANNOUNCE`s for the same term with different hosts; the same `commit_index` with different hashes; a Conflict result from `reconcile`.

On conflict:

1. Enter `AUTHORITY_CONFLICT`.
2. Reject all actions and host switching.
3. Store both branch heads.
4. Emit `AuthorityConflict` with redacted heads.
5. Send `AUTHORITY_CONFLICT` to peers.
6. Never merge.

Only `End` leaves this state in Phase 1. Stale-term authority messages (term below local) are rejected with a `StaleTerm` diagnostic.

### 7.9 Reconnect and route rebinding

1. A real link loss enters `RECONNECTING` and dials the host route, then other members' routes.
2. On link up, the peer sends `RECONNECT_HELLO` with its secret.
3. A non-host replies `NOT_AUTHORITY` with host info (C-18).
4. The host verifies `reconnect_verifier` and accepts the new transport identity for the existing `peer_id`: `route_generation`+1, `PeerRouteChanged`, and no membership change. It reconciles the heads (§7.8) and sends a sync.
5. The peer re-meshes, and the core emits `ConnectionRestored`.

Knowing a `peer_id` alone never authenticates (Spec §43). `PEER_ROUTE_UPDATE` is accepted only from a link already bound to that `peer_id` and only with an increasing generation.

### 7.10 Recovery blob

A versioned protobuf `RecoveryBlobV1` contains:

- `session_id`, `peer_id`, `join_order`, `reconnect_secret`;
- the full `CommitMeta` chain, the roster, and the head `StateRepresentation`;
- the route book, pending actions, `policy_id`, and the config fingerprint.

The blob is not encrypted by TurnNet. The app stores it securely (REC-003).

`restore()` validates the version and the chain hashes, rebuilds state, enters `RECONNECTING`, and emits `Connect` outputs to the known routes. In the simulator, a restarted node gets a **new** transport identity, which exercises rebinding (REC-004).

---

## 8. Simulator (`turnnet-sim`)

- **Clock:** a virtual `Millis`. The event queue is a `BinaryHeap<(deliver_at, seq)>`. `run_until(t)` and `run_until(predicate, max_t)` drive it.
- **Determinism:** one `ChaCha8Rng` seed drives network faults, each node's RNG (derived per node), and jitter. Same seed means byte-identical traces.
- **Links:** each `SimLink` has a per-direction fault profile: latency, jitter, loss, duplication, reorder probability, and bandwidth cap.
  - Reliable mode honors the `PeerLink` contract: in-order delivery or an explicit `LinkDown`.
  - Adversarial mode adds raw loss, duplication and reordering to test robustness.
- **Controls:**
  - `partition(groups)`, `heal()`, asymmetric `block(a→b)`;
  - `kill(node)`, `restart_from_blob(node)` (new transport identity);
  - `set_lifecycle(node, Background)`;
  - `delay_matching(pred, ms)`, for example to delay old-term messages;
  - `inject_frame(link, bytes)` for malformed or oversized frames;
  - `change_route(node)`.
- **SimDiscovery:** hosts register as discoverable routes. Probing goes over real sim links (§7.2).
- **Fixtures:**
  - `CounterGame`: any member may increment; full replication.
  - `TurnGame`: strict turn order, used to exercise rejections.
  - `HiddenCardGame` with `PublicPrivatePolicy`.
  - A trace recorder that captures every delivered envelope per recipient, for delivery-boundary assertions.
- **Invariant checker**, run after every step:
  - at most one active host per non-conflicted term;
  - per-peer `commit_index`, `state_version` and `epoch` are monotonic;
  - every adopted chain is a prefix of the authoritative chain or the session is fenced;
  - no `action_id` appears twice in any chain;
  - `join_order` values are unique.

---

## 9. Test plan

Every scenario runs at **2, 4 and 8 peers** where applicable and over a seed sweep: 32 seeds by default and 1 000 under `--ignored` soak. File names are indicative.

| # | Spec §55 test | File |
|---|---|---|
| 1–2 | create + join, epoch/ci advance | `join.rs` |
| 3 | join after migration gets fresh join order | `join.rs` |
| 4 | accepted action: sv and ci exactly +1 | `actions.rs` |
| 5 | duplicate `message_id` ignored | `dedupe.rs` |
| 6 | same `action_id`, new `message_id`: no double commit | `dedupe.rs` |
| 7 | same `action_id` after migration: no double commit | `dedupe.rs` |
| 8 | stale-term action/control rejected | `term.rs` |
| 9–10 | disconnect keeps membership; reconnect syncs | `reconnect.rs` |
| 11 | voluntary leave advances epoch + ci | `membership.rs` |
| 12 | host dies right after commit broadcast | `migration.rs` |
| 13 | host dies while a peer is one behind | `migration.rs` |
| 14 | latest compatible base selected | `migration.rs` |
| 15–17 | lowest join order; skip `!host_capable`; skip `!migration_eligible` | `election.rs` |
| 18 | repeated migration preserves state + dedupe | `migration.rs` |
| 19 | old host returns and follows (incl. orphan tail, C-13) | `migration.rs` |
| 20–23 | incompatible higher-term, same-term dual host, partition branches fence, no merge | `conflict.rs` |
| 24 | protocol major mismatch | `protocol.rs` |
| 25 | malformed/oversized frames (+ proptest fuzz of decoder) | `protocol.rs` |
| 26 | recovery blob restores identity/context | `recovery.rs` |
| 27 | route rebind keeps peer/join order | `recovery.rs` |
| 28–30 | hidden state: delivery boundaries, differing representation hashes, eligibility | `hidden_state.rs` |
| 31 | continue with reduced membership | `membership.rs` |
| 32 | clean end when policy cannot continue (incl. C-7 solo survivor) | `lifecycle.rs` |

Extra tests:

- golden vectors for every hash in §6;
- the `max_players` boundary (a ninth join is rejected);
- every player count from 2 to 8 for create/join (release checklist);
- C-6 grace-expiry event;
- C-8 stale-context reject and advisory modes;
- invite expiry, revocation and max-uses;
- graceful host transfer;
- a lifecycle background that flips `host_capable`.

**Definition of done for Phase 1:**

- `cargo test --workspace` passes;
- the invariant checker never fires across the seed sweep;
- `cargo clippy -D warnings` is clean;
- `docs/protocol.md`, `state_commit_model.md`, `host_migration.md` and `recovery.md` are written;
- the V2 Decisions file is updated with the C-items (DOC-003).

---

## 10. Milestones

| M | Content | Exit check |
|---|---|---|
| M1 | Workspace, ids, hash encoders + golden vectors, proto + codegen, frame codec | codec/hash unit tests |
| M2 | CommitChain, roster, `reconcile()` (pure, unit-tested exhaustively) | chain/reconcile tests |
| M3 | SessionCore skeleton, sim crate, create/discover/join, mesh, heartbeats | tests 1–2, every count from 2 to 8 |
| M4 | Actions, dedupe, commits, ACK, sync, leave/remove, joinable, invites | tests 4–6, 9–11, 31 |
| M5 | Visibility policy, eligibility, hidden-state fixture | tests 28–30 |
| M6 | Election, barrier, transfer, conflict fencing, orphan tail | tests 3, 7, 12–23, 32 |
| M7 | Reconnect auth, route rebinding, recovery blob, lifecycle | tests 8, 24–27 |
| M8 | Seed sweep, invariant checker, docs, Decisions update | definition of done |

---

## 11. Risks and items to revisit

- **Election view divergence** under asymmetric partitions can stall rounds. This is mitigated by the round timeout and the ACK-switching rule, and the soak seeds will measure it.
- **Canonical encodings** (§6) must be frozen before the first cross-platform interop release. After that, changing them needs a protocol minor/major decision.
- **Full chain retention (C-10)** grows linearly with the number of turns. This is fine for casual sessions (~250 B per commit). Revisit if a game exceeds ~10⁵ commits, and any compaction must satisfy ACT-004.
- **Invites are host-local (C-16).** If apps need invites to survive migration, Phase 4 can replicate invite verifiers in policy metadata.
- **Reconnect secret over the sim transport** is fine in Phase 1. Real transports must be encrypted (Iroh is; Nearby/Wi-Fi Aware must use secure modes, per §50.2).
