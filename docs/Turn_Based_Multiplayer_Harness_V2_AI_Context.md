---
document: turn_based_multiplayer_harness_v2_ai_context
version: 2.0
status: implementation_baseline
date: 2026-09-05
working_name: TurnNet
working_name_is_placeholder: true
supersedes:
  - Turn_Based_Multiplayer_Harness_V1_AI_Context.md
intended_audience:
  - coding_agents
  - software_architects
  - maintainers
supported_players:
  min: 2
  max: 8
primary_platforms:
  - android
  - ios
web_support: out_of_scope_v2
documentation_format: markdown
---

# AI Development Context - Turn-Based Multiplayer Networking Harness V2

## 0. Documentation standard

Normative project rule:

- All project documentation MUST be Markdown (`.md`).
- Architecture, implementation specs, ADRs/decision records, AI context, plans, runbooks, test plans, release notes, and design notes are Markdown.
- Do not generate DOCX/PDF project documentation unless explicitly requested as a one-off exception.
- Source code, protobuf, JSON/YAML, test fixtures, images, and build output use their natural formats.
- Architecture changes must update the affected Markdown documents in the same task.

Canonical V2 documentation:

- `Turn_Based_Multiplayer_Harness_V2_Implementation_Spec.md` - implementation-driving specification.
- `Turn_Based_Multiplayer_Harness_V2_AI_Context.md` - compact context for coding sessions.
- `Turn_Based_Multiplayer_Harness_V2_Decisions.md` - canonical architectural decisions.

If documents conflict, the Decisions file wins on architecture. The Implementation Spec wins on implementation detail when it does not contradict Decisions.

## 1. Purpose

TurnNet is an engine-agnostic multiplayer networking/session library for casual, low-bandwidth, turn-based games. It is not a Ludo library and must not contain game-specific logic.

Target games include board games, card games, chess-like games, quiz games, party games, and similar turn-based experiences.

V2 supports 2-8 players. Every participating application includes both host and client capability. One peer is authoritative at a time, but authority can migrate when the host disappears or leaves.

## 2. V2 architectural shift

V1 mixed two concerns:

1. connectivity mechanics: LAN TCP, WebRTC, ICE/STUN/TURN, manual signaling;
2. multiplayer semantics: authority, state commits, membership, retries, recovery, host migration.

V2 keeps TurnNet responsible for multiplayer semantics and delegates IP connectivity to Iroh.

Conceptually:

```text
TurnNet = multiplayer/session semantics
Iroh    = IP peer connectivity substrate
```

Do not move TurnNet's host election, state replication, membership, action idempotency, or recovery policy into Iroh-specific logic.

## 3. Three supported play/hosting profiles

### 3.1 SAME_ROOM

Use native nearby transports:

1. Google Nearby Connections - primary.
2. Wi-Fi Aware/NAN - fallback when Nearby is unavailable/fails and the platform supports it.

Properties:

- shared Wi-Fi is not required;
- Internet is not required;
- use `P2P_CLUSTER` by default for Nearby;
- `P2P_STAR` is experimental fallback only;
- Wi-Fi Aware is direct nearby P2P, not same-router LAN;
- native platform capability/permission detection belongs in Kotlin/Swift adapters;
- same TurnNet protocol/state machine runs above the native peer links.

Do not depend on Iroh `unstable-custom-transports` for this V2 profile.

### 3.2 LOCAL_LAN

Use Iroh QUIC on the same Wi-Fi/LAN with no outside Internet dependency.

Requirements:

- use Iroh QUIC for data/control transport;
- disable relay use for this profile;
- use Iroh mDNS address lookup for local endpoint discovery/address resolution;
- TurnNet performs its own small session discovery probe/advertisement over the TurnNet ALPN;
- LAN client isolation/router policy errors are connectivity diagnostics;
- optional manual invite can exist, but local discovery is the normal UX.

This profile replaces V1 mDNS/Bonjour + custom TCP.

### 3.3 INTERNET

Use Iroh QUIC over the Internet.

Requirements:

- prefer direct connectivity/hole punching;
- allow relay fallback when direct paths fail;
- relay configuration must be injectable/vendor-neutral;
- no WebRTC, ICE, STUN, TURN, offer/answer signaling, or `SIGNALING_RELAY` in TurnNet V2;
- relay infrastructure is packet forwarding/connectivity infrastructure only, never authoritative game infrastructure;
- no game-state server, matchmaking backend, account service, or database is required.

## 4. Iroh baseline

V2 assumes the stable Iroh 1.1.x API family. Pin exact dependency versions in `Cargo.lock` for a release.

Use these stable capabilities:

- `Endpoint` to accept/connect;
- `EndpointId` as cryptographic transport identity;
- `EndpointAddr` for current dialing information;
- QUIC reliable streams;
- ALPN protocol selection;
- direct connectivity/hole punching;
- configurable relay mode/maps;
- address lookup;
- `watch_addr()` / current route information;
- network-change notification;
- endpoint tickets/application-specific tickets for bootstrap patterns;
- mDNS address lookup crate for local networks.

Do not rely on unstable custom transports.

### 4.1 Iroh ownership boundary

Iroh owns:

- QUIC connection establishment;
- peer transport authentication through endpoint keys;
- NAT traversal/hole punching;
- direct vs relay path selection;
- QUIC path migration;
- stream multiplexing;
- IP-level endpoint addressing.

TurnNet owns:

- `session_id` and `peer_id`;
- admission/join authorization;
- committed membership;
- authoritative host role;
- `term`;
- `commit_index` / state versions / membership epochs;
- game action routing;
- action/message idempotency;
- state replication;
- hidden-information visibility/recovery policy;
- migration eligibility;
- host election/migration;
- reconnect semantics;
- split-brain detection/fencing;
- diagnostics exposed to the game/app.

## 5. Runtime architecture

Recommended layers:

```text
Game / GameAdapter
        |
Public MultiplayerSession API
        |
TurnNet Rust Session Core
  - authority
  - membership
  - actions
  - state replication
  - migration
  - recovery
  - diagnostics
        |
TurnNet Wire Protocol
        |
PeerLink abstraction
  |                   |
  |                   +-- NativeNearbyPeerLink
  |                         - Google Nearby
  |                         - Wi-Fi Aware
  |
  +-- IrohPeerLink
        - LOCAL_LAN
        - INTERNET

DiscoveryProvider abstraction
  - Nearby discovery
  - Wi-Fi Aware discovery
  - Iroh mDNS endpoint discovery
  - TurnNet LAN session probing
  - TurnNetInvite import

SimulatedPeerLink + SimulatedDiscovery
```

All transport events enter the common Rust session state machine before affecting public session state.

## 6. Iroh endpoint/runtime rules

- Prefer one Iroh `Endpoint` per TurnNet networking runtime/process and reuse it for all peer connections.
- TurnNet's public SDK must not expose raw Iroh types.
- Use a TurnNet-owned ALPN; V2 major protocol is conceptually `turnnet/2`.
- One peer pair should normally reuse one QUIC connection and multiplex TurnNet streams.
- `EndpointId` is transport identity, never canonical `peer_id`.
- A process restart may produce a different Iroh `EndpointId`; authenticated TurnNet reconnect can rebind routing to the existing `peer_id`.
- Persisting the Iroh private key is optional, not required by the TurnNet recovery contract.
- Current endpoint addressing is mutable routing metadata and may be refreshed without changing committed membership.

On Android:

- initialize Iroh's required Android/JNI networking context before constructing an endpoint when using the default DNS behavior;
- route OS network-change callbacks into Iroh's network-change notification;
- also notify the TurnNet lifecycle layer when the app becomes unable to maintain networking.

## 7. QUIC usage

V2 uses reliable QUIC streams. QUIC datagrams are not required.

Recommended connection use:

- one long-lived bidirectional control stream per connected peer pair for framed control messages;
- state payloads may use the control stream when small;
- implementations may use additional uni/bi streams for larger snapshots/recovery payloads so large transfers do not unnecessarily block control traffic;
- every stream payload is still bounded by TurnNet limits and validated before allocation.

QUIC segmentation is not the same as TurnNet application framing. Protocol envelopes remain length-delimited/versioned.

## 8. Internet peer mesh

For 2-8 players, maintain one lightweight Iroh peer connection between every pair when feasible.

Maximum at 8 players:

```text
8 * 7 / 2 = 28 pairwise connections
```

Gameplay remains host-authoritative. The mesh exists for:

- host-health visibility;
- deterministic migration;
- direct rerouting to the new host;
- current peer route distribution;
- recovery when the original host is gone.

The mesh is not a multi-writer game-state system.

If a peer-pair connection is missing, use current Iroh route information distributed by session peers to dial the missing peer.

## 9. Public identity and metadata

Required identifiers:

```text
session_id          random 128-bit session identity
peer_id             random 128-bit TurnNet participant identity
join_order          immutable session-monotonic admission order
term                host generation
state_version       game-state generation
membership_epoch    committed roster generation
commit_index        total ordered authoritative session commit index
message_id          protocol-message dedupe identity
action_id           logical game-action identity
```

Additional transport/runtime metadata:

```text
transport_binding
  - current Iroh EndpointId/EndpointAddr information, OR
  - native nearby route information

host_capable
migration_eligible
connectivity_state
last_acknowledged_commit_index
last_acknowledged_state_version
```

Never use device name, IP address, account name, Bluetooth address, phone number, or Iroh `EndpointId` as TurnNet `peer_id`.

## 10. Membership model

Committed membership and connectivity are separate.

A member can be:

- connected;
- reconnecting;
- temporarily unreachable;
- background-interrupted;
- intentionally leaving;
- removed from the committed roster.

`membership_epoch` advances only when roster membership is committed, e.g.:

- a peer is admitted;
- a peer intentionally leaves and removal commits;
- host/game policy commits an eviction/removal.

Temporary connection loss does not automatically change `membership_epoch`.

`join_order` is assigned once by the authoritative host that admits a peer using committed/replicated `next_join_order`. It is never reused, even after a peer leaves.

## 11. TurnNet invite model

`TurnNetInvite` is a TurnNet protocol object, not a raw Iroh ticket.

Conceptual Internet fields:

```text
TurnNetInvite
  invite_version
  protocol_major
  protocol_minor_hint
  session_id
  host_endpoint_addr_or_ticket
  join_capability
  optional_expiry
  optional_game/display metadata
  optional_relay/profile hints
```

Rules:

- the Iroh addressing component may contain endpoint ID, relay URL, and direct addresses;
- `join_capability` is an unpredictable bearer secret or equivalent challenge material controlled by TurnNet;
- invite may be QR, deep link, text, file, or imported from an image;
- one-way user flow: host shares; client imports; client dials host;
- no answer QR;
- no manual ICE candidate exchange;
- host can revoke/expire join capabilities;
- discovery advertisements must not contain reconnect secrets or private state.

Treat Internet invites as sensitive because dialing information may include IP addresses and reusable information.

## 12. Game integration contract

Game action/state payloads remain opaque bytes.

Conceptual adapter:

```text
GameAdapter
  validate_and_apply(actor_peer_id, action_bytes, current_authoritative_state)
    -> Accepted(new_authoritative_state, optional_event_bytes)
    -> Rejected(reason_code)

  on_state_committed(peer_visible_state, state_version, commit_index)

  on_session_event(event)
```

The game owns:

- rules;
- move legality;
- turn semantics;
- random-number policy;
- serialization of action/state bytes;
- hidden-information semantics;
- UI/animations.

TurnNet owns:

- networking/session identities;
- routing and connection state;
- host role and migration;
- membership;
- action routing and deduplication;
- commit ordering;
- state replication/recovery plumbing;
- transport adaptation;
- protocol compatibility;
- diagnostics.

## 13. State visibility/recovery policy

V2 requires a pluggable policy.

Conceptual surface:

```text
StateReplicationPolicy
  build_public_state(...)
  build_private_state_for(peer_id, ...)
  build_recovery_material_for(peer_id, ...)
  can_reconstruct_authoritative_state(peer_id, commit_meta) -> bool
  reconstruct_authoritative_state(...)
```

Strategies the architecture supports:

1. Trusted full replication - default/simple.
2. Public state + per-peer private state.
3. Authenticated-encrypted private/recovery envelopes.
4. Commitment-based hidden state - advanced.
5. Threshold/secret-sharing recovery - advanced.

V2 must implement the abstraction and trusted full replication. Public/private and encrypted-recovery paths must be representable.

## 14. Authoritative commit model

V2 adds a total-order `commit_index` because game-state changes and membership changes use separate domain counters.

### 14.1 Counters

- `state_version` increments once for every accepted game-state change.
- `membership_epoch` increments once for every committed roster change.
- `commit_index` increments for every authoritative session commit, whether it changes game state, membership, or both.

### 14.2 Commit metadata

Conceptual commit:

```text
CommitMeta
  session_id
  term
  commit_index
  previous_commit_hash
  state_version
  membership_epoch
  host_peer_id
  roster_hash
  logical_state_hash
  next_join_order
  migration_eligibility_digest
  committed_action_id?      // for action commits
  commit_kind
  commit_hash
```

The commit chain is used for convergence and branch/conflict detection.

### 14.3 Three hashes

Do not conflate these:

```text
logical_state_hash
  hash of canonical logical authoritative game state

representation_hash
  hash of the concrete state/recovery representation delivered to one peer

commit_hash
  hash binding authoritative commit metadata + previous commit hash
```

Trusted-full-replication peers may independently calculate the same logical-state hash.

Hidden-information peers may receive different representations and therefore different `representation_hash` values. They may know the committed `logical_state_hash` without possessing enough private state to independently recompute it.

Hashes are for integrity/convergence/branch detection, not secrecy or malicious-host prevention.

## 15. Action routing and idempotency

Client action flow:

1. game creates logical `action_id`;
2. client sends `ACTION_SUBMIT(action_id, payload, expected_state_version/commit)` to current host;
3. host validates session/term/membership/idempotency;
4. host calls game adapter;
5. rejected action emits `ACTION_REJECT`;
6. accepted action creates exactly one authoritative state commit;
7. state commit is replicated;
8. peers ACK committed head.

The same logical action always reuses the same `action_id`, even when:

- response times out;
- network reconnects;
- a new host is elected;
- the protocol message is resent with a new `message_id`.

Default V2 may retain all committed `action_id`s for the active session. Migration/recovery material must preserve enough dedupe state to prevent double commit after migration.

## 16. Host capability vs migration eligibility

These are different.

`host_capable` means the peer can technically serve as host now, considering:

- application lifecycle;
- permissions;
- transport ability to accept/connect/advertise;
- selected play profile;
- resource limits.

`migration_eligible` means the peer can reconstruct and continue from the latest authoritative state under the selected visibility/recovery policy.

Election requires both.

Example: in a hidden-card game a peer may be fully network-capable but not possess enough recovery material to reconstruct all hidden state. It is `host_capable = true`, `migration_eligible = false`.

## 17. Host election

Default election for a suspected/dead host:

1. start from committed members known in the latest membership epoch;
2. reconcile to the highest verifiable compatible committed head;
3. require candidate to possess/reconstruct that head;
4. require `host_capable = true`;
5. require `migration_eligible = true`;
6. choose the smallest `join_order` among equally eligible reachable candidates;
7. proposed new host uses `term + 1` and references the exact base commit index/hash;
8. peers acknowledge the migration barrier;
9. default casual policy may resume with at least two mutually reachable session members including the new host;
10. new host publishes `HOST_ANNOUNCE` and resumes commits.

No Raft/Paxos implementation is required.

## 18. Migration barrier and authority rules

Authority messages include at minimum:

```text
session_id
new_term
new_host_peer_id
base_commit_index
base_commit_hash
membership_epoch
```

Rules:

- old-term authority is ignored/rejected;
- a higher term cannot silently replace an incompatible committed branch;
- if local state is behind but the new host's base chain is compatible, resynchronize then accept authority;
- a returning old host must accept the newer compatible term;
- gameplay commits pause during migration.

## 19. Split-brain policy

The default casual resume rule intentionally does not require majority quorum. This allows a game to continue after friends leave, but it cannot prevent every network-partition split brain.

V2 therefore requires detection/fencing rather than pretending partitions are impossible.

Detect conflict when, for example:

- two hosts assert authority for the same term;
- same commit index has incompatible commit hashes;
- a higher-term claim is based on a branch incompatible with a locally committed head;
- two branches contain post-fork authoritative commits.

On conflict:

1. enter `AUTHORITY_CONFLICT` / `INTERRUPTED`;
2. stop accepting/committing new gameplay actions;
3. retain both diagnostic branch heads where possible;
4. surface a clear app/session event;
5. do not automatically merge divergent game state;
6. allow a future explicit recovery policy to select a branch, or end/restart the session.

Safety is preferred over silently discarding committed turns.

## 20. Transport-specific host migration

### 20.1 INTERNET / Iroh

- existing peer mesh detects host loss;
- surviving peers elect over current QUIC connections;
- new host announces new term on surviving connections;
- clients immediately reroute actions to the new host;
- missing peer links are dialed using the latest distributed Iroh route information;
- no QR rescan and no signaling renegotiation.

### 20.2 LOCAL_LAN / Iroh

- same election semantics;
- surviving Iroh connections are reused;
- mDNS can rediscover endpoint addressing if a direct route is missing;
- if the session remains joinable, new host updates session advertisement role metadata;
- no Internet or relay is required.

### 20.3 SAME_ROOM / native nearby

- detect host loss;
- elect with the common rules;
- new host begins advertising the same `session_id` under the new term;
- peers reconnect to the elected host as required by transport topology;
- state is reconciled before gameplay resumes.

## 21. Route distribution

TurnNet maintains current transport routing separately from committed membership.

For Iroh peers, route metadata may contain:

```text
iroh_endpoint_id
current endpoint addresses / relay information
route_generation
last_updated
```

Use endpoint address updates/watchers to refresh routing metadata. Route updates:

- are authenticated;
- do not increment `membership_epoch`;
- do not change `peer_id`;
- are distributed to peers needed for migration/control mesh;
- never contain reconnect secrets.

If a process restart produces a new Iroh endpoint identity, reconnect authorization can bind the new endpoint to the existing TurnNet peer identity and publish a new route generation.

## 22. Reconnect and process recovery

Reconnection preserves:

- `session_id`;
- `peer_id`;
- `join_order`;
- committed membership status;
- known term;
- latest compatible commit index/hash;
- state version;
- membership epoch;
- outstanding logical action identity;
- recovery material required by visibility policy.

Conceptual reconnect:

1. establish/re-establish a peer link to any reachable session peer/current host;
2. send `RECONNECT_HELLO` with session/peer identity and reconnect proof;
3. receiver routes or answers with current authority information;
4. current host authenticates reconnect and may rebind transport route;
5. compare commit chain/head;
6. transfer missing authoritative state/recovery material;
7. restore control links as appropriate;
8. mark peer active without creating a new participant.

Public recovery API:

```text
export_recovery_blob() -> bytes
restore_session(recovery_blob) -> SessionHandle / RecoveryResult
```

Recovery blob is opaque/versioned and may contain secrets. Applications decide persistence and use secure storage.

## 23. Protocol envelope

Recommended Protobuf control envelope:

```text
protocol_major
protocol_minor
session_id
term
message_id
sender_peer_id
message_type
commit_index? 
state_version?
membership_epoch?
action_id?
payload
```

Iroh ALPN carries major protocol compatibility for Iroh connections; the envelope still carries protocol version for common semantics across native nearby transports and future minor evolution.

Required message families include:

```text
HELLO
SESSION_PROBE
SESSION_ADVERTISEMENT
JOIN_REQUEST
JOIN_ACCEPT
JOIN_REJECT
RECONNECT_HELLO
RECONNECT_ACCEPT
RECONNECT_REJECT
PEER_ROSTER
PEER_ROUTE_UPDATE
HEARTBEAT
ACTION_SUBMIT
ACTION_REJECT
STATE_COMMIT
STATE_ACK
STATE_SYNC_REQUEST
STATE_SYNC_RESPONSE
HOST_SUSPECTED
HOST_PROPOSE
HOST_ACK
HOST_ANNOUNCE
HOST_TRANSFER
AUTHORITY_CONFLICT
PEER_LEFT
SESSION_END
ERROR
```

There is no `SIGNALING_RELAY` in V2.

## 24. Session states

Canonical high-level states:

```text
IDLE
DISCOVERING
HOSTING
JOINING
CONNECTED_CLIENT
CONNECTED_HOST
HOST_SUSPECTED
MIGRATING
RECONNECTING
INTERRUPTED
AUTHORITY_CONFLICT
LEAVING
ENDED
```

Transport path state is separate, e.g.:

```text
DIRECT
RELAYED
LOCAL_DIRECT
NATIVE_NEARBY
RECONNECTING_PATH
```

Do not overload session states with Iroh path details.

## 25. Reliability defaults

Initial V2 defaults retain V1 values until measurements justify changes:

```text
heartbeat_interval:       2s
peer_suspect_after:       6s
host_election_after:      8s without host liveness
reconnect_grace_period:   60s
action_response_timeout:  10s
join_timeout:             20s
max_join_retries:         3
```

All are configurable policy values, not wire constants.

## 26. Path migration and lifecycle

Iroh may preserve a QUIC connection while network conditions change.

If the same logical peer connection survives:

- do not emit TurnNet `ConnectionLost`/reconnect solely because traffic changed direct↔relay or Wi-Fi↔cellular;
- emit diagnostic/path-change events if useful;
- keep the same session/peer/host state.

If the QUIC connection actually fails:

- enter TurnNet reconnect behavior;
- retry using current route information/address lookup;
- host-migration logic runs only when authority liveness is actually lost, not merely because a path changed.

Mobile suspension is platform-dependent. If the OS prevents reliable networking, surface `SessionInterrupted`. Do not promise background continuity after indefinite suspension/force-kill.

## 27. Payload/framing policy

Game payload size remains application-defined.

TurnNet provides a safety policy such as:

```text
TransportFramePolicy
  preferred_max_payload_bytes?
  preferred_max_frame_bytes?
  max_reassembly_bytes?
  max_pending_outbound_bytes?
  backpressure_policy?
```

Iroh QUIC streams already segment transport bytes; do not build unnecessary network-level fragmentation on top of QUIC.

Native nearby adapters may require TurnNet-level fragmentation/reassembly depending on platform message limits.

Always:

- validate size before allocation;
- bound queued outbound bytes;
- reject malformed length prefixes;
- keep TurnNet control-message limits library-owned;
- distinguish application-policy rejection from transport hard-limit rejection.

## 28. Security/privacy baseline

V2 is not anti-cheat hardened, but requires:

- Iroh's authenticated encrypted peer transport for Iroh profiles;
- preservation of native Nearby/Wi-Fi Aware security capabilities;
- unpredictable join/reconnect capabilities;
- sender/route binding validation;
- stale term rejection;
- commit-chain compatibility checks;
- payload and enum validation;
- size limits before allocation;
- no raw game state/action logging by default;
- no persistent personal identity requirement;
- no account/contact/phone-number collection in the core;
- no use of IP/device name as game identity.

Relay infrastructure may observe networking metadata but cannot be treated as a trusted game authority. Do not claim a relay provides anonymity.

## 29. Diagnostics

Structured diagnostic fields should include:

```text
timestamp
session_id (redactable)
peer_id (redactable)
profile
transport
connection_state
path_type (direct/relay/local/native)
iroh_endpoint_id (redactable, where applicable)
term
commit_index
state_version
membership_epoch
event_code
error_category
rtt/latency where available
relay/home relay info where appropriate and redacted
```

Required categories should distinguish at least:

- discovery failure;
- permission failure;
- Nearby unavailable;
- Wi-Fi Aware unavailable;
- LAN isolation;
- no local route;
- Iroh address resolution failure;
- direct-path failure;
- relay unavailable/rate-limited;
- protocol mismatch;
- join authorization failure;
- timeout;
- reconnect failure;
- state desynchronization;
- stale term;
- authority conflict;
- hidden-state migration ineligibility.

## 30. Simulated transport requirement

Implement simulation before relying on phones.

Simulation must support deterministic injection of:

- latency;
- jitter;
- loss;
- duplication;
- reordering;
- disconnect;
- reconnect;
- host death;
- process restart;
- network partition;
- delayed old-term messages;
- route changes;
- conflicting host claims;
- peer lifecycle/host-capability changes.

The session correctness suite must run entirely in-process without Iroh or mobile SDKs.

## 31. Required core tests

At minimum cover 2, 4, and 8 peers where applicable:

1. host creates session; peers join;
2. state/action commits converge;
3. duplicate `message_id` ignored;
4. duplicate/retried `action_id` never double-commits;
5. stale term rejected;
6. temporary client disconnect resynchronizes without roster removal;
7. committed leave advances membership epoch;
8. join after original host migrated receives a new monotonic never-reused join order;
9. host dies after commit broadcast;
10. host dies while a peer is behind;
11. deterministic eligible host selected;
12. lower-join-order peer with `host_capable=false` is skipped;
13. lower-join-order peer with `migration_eligible=false` is skipped;
14. repeated host migration preserves state and action dedupe;
15. old host returns and accepts newer compatible authority;
16. two partitions create conflicting branches and the session fences instead of merging;
17. incompatible higher-term branch is not silently accepted;
18. recovery blob restores peer identity and reconnect attempt;
19. endpoint/route rebinding does not create a new peer;
20. hidden-information recipients get only authorized representations;
21. per-peer representation hashes may differ while commit metadata remains consistent;
22. malformed/oversized payloads fail safely;
23. protocol major mismatch rejects cleanly;
24. session continues with reduced membership where game permits.

## 32. Iroh-specific integration tests

### LOCAL_LAN

- works with outside Internet unavailable;
- relay disabled;
- mDNS finds local endpoints;
- TurnNet session probe identifies joinable host;
- direct QUIC connects;
- host migration works without Internet;
- route/address refresh after Wi-Fi network change;
- LAN client isolation produces useful diagnostics.

### INTERNET

- initial join from one TurnNet invite;
- no answer/offer exchange;
- direct connection path;
- relay-only fallback path;
- relay→direct upgrade when possible;
- direct→relay fallback when direct path fails and QUIC survives;
- configurable custom relay map;
- production-style multi-relay configuration across distinct regions where high availability is required;
- failure of one relay with another configured;
- peer mesh reaches expected connectivity at 8 players;
- host migration uses surviving Iroh connections and/or route dialing;
- process restart with new endpoint identity authenticates and rebinds existing peer;
- stale/revoked invite is rejected by TurnNet even if the Iroh endpoint remains dialable.

### Mobile

- Android network-change notification integration;
- Android JNI/DNS initialization before endpoint creation;
- Android Wi-Fi↔cellular changes;
- iOS foreground/background interruption/recovery;
- process kill/relaunch via recovery blob;
- repeated reconnect and migration on real devices.

## 33. Same-room integration tests

- Nearby Android↔Android baseline;
- supported cross-platform Nearby combinations;
- `P2P_CLUSTER` at 2/4/8 peers;
- fallback to Wi-Fi Aware when Nearby fails/unavailable;
- secure pairing/capability detection where supported;
- host death and migration;
- intentional host leave/transfer;
- unsupported-device graceful failure;
- no shared Wi-Fi / no Internet case.

## 34. Public SDK concepts

Conceptually expose:

```text
create_session(CreateSessionOptions) -> SessionHandle
start_discovery(PlayProfile) -> DiscoveryHandle / event stream
join_discovered_session(DiscoveredSession) -> SessionHandle
join_session(TurnNetInvite) -> SessionHandle
leave_session()
end_session()
submit_action(bytes, action_id?)
get_session_info()
get_peers()
get_current_host()
get_connection_state()
get_network_path_info()
create_invite(representation/options)
import_invite(blob)
export_recovery_blob()
restore_session(recovery_blob)
```

Game SDK may generate `action_id` automatically, but retries must preserve it internally.

Required events:

```text
SessionDiscovered
PeerJoined
PeerLeft
PeerConnectionChanged
PeerRouteChanged
NetworkPathChanged
HostMigrationStarted
HostChanged
HostMigrationFailed
AuthorityConflict
StateCommitted
ActionRejected
ConnectionLost
ConnectionRestored
SessionInterrupted
SessionEnded
DiagnosticEvent
```

All public APIs must be asynchronous/non-blocking from the application UI thread.

## 35. Repository direction

Recommended V2 structure:

```text
repo/
  core/
    src/
      session/
      protocol/
      state/
      membership/
      election/
      recovery/
      diagnostics/
      peer_link/
      discovery/
      simulated/
    proto/
    tests/

  transport/
    iroh/
      endpoint_runtime/
      quic_peer_link/
      lan_mdns/
      invite/
      relay_config/

  platform/
    android/
      sdk/
      nearby/
      wifi_aware/
      lifecycle/
      sample/
    ios/
      sdk/
      nearby/
      wifi_aware/
      lifecycle/
      sample/

  bindings/
    c/

  docs/
```

Iroh remains inside the Rust transport layer. Platform wrappers should not independently implement Internet/LAN protocols.

## 36. Implementation order

Implement V2 in this order:

1. V2 IDs and commit model: `commit_index`, hash chain, membership separation.
2. V2 Protobuf envelope/message definitions and deterministic session state machine.
3. Simulated peer link/discovery + deterministic clock.
4. 2-8 peer create/join/action/state commits in simulation.
5. action-id idempotency and full-state replication.
6. committed membership + monotonic join order.
7. hidden-state policy abstraction + `migration_eligible`.
8. host timeout/election/migration + conflict fencing.
9. reconnect + recovery blob + route rebinding semantics.
10. Rust Iroh runtime with TurnNet ALPN and peer-link connection manager.
11. Iroh peer mesh + route updates.
12. LOCAL_LAN Iroh mDNS discovery with relay disabled.
13. INTERNET TurnNetInvite + direct/relay connectivity.
14. Iroh path diagnostics/network-change handling.
15. C ABI/public Kotlin/Swift facade skeletons.
16. Android/iOS Iroh integration and real-device LAN/Internet tests.
17. Google Nearby same-room adapter.
18. Wi-Fi Aware same-room adapter.
19. cross-platform matrix, packaging, performance, diagnostics, release hardening.

Do not implement WebRTC/TCP fallback unless a future explicit decision changes V2.

## 37. V2 acceptance criteria

V2 is acceptable when:

- one Rust session core supports 2-8 players;
- game integration remains engine-independent;
- all three profiles use the same session semantics;
- Same Room works through Nearby and supported Wi-Fi Aware fallback without shared Wi-Fi/Internet;
- Local LAN works through Iroh QUIC with outside Internet/relays disabled;
- Internet play uses one-way TurnNet invites and Iroh direct/relay connectivity with no WebRTC signaling;
- Android/iOS can both host and join Iroh sessions;
- full Iroh mesh is validated at 8 players on representative devices;
- Iroh direct/relay path changes do not incorrectly alter host authority;
- accepted actions commit once and exactly once by `action_id`;
- committed membership is separate from transient connectivity;
- state, membership, and total commit ordering remain consistent;
- hidden-state games can use per-peer representations and migration eligibility;
- host migration works repeatedly in all three transport families;
- conflicting authority branches are detected and fenced;
- process recovery can rebind transport routing without creating a new participant;
- malformed/stale/duplicate traffic cannot corrupt session state;
- deterministic simulated fault tests pass for 2, 4, and 8 peers;
- structured diagnostics distinguish transport, path, protocol, state, and authority failures.

## 38. Explicit V2 non-goals

- Browser client.
- Matchmaking/account backend.
- Cloud game-state persistence.
- Server-authoritative cloud gameplay.
- WebRTC/ICE/STUN/TURN.
- Custom LAN TCP.
- Iroh unstable custom transports.
- Real-time FPS/action netcode.
- Delta-only replication/event sourcing.
- Rollback netcode.
- Malicious-host prevention/fair dice.
- Automatic cross-family transport racing.
- Guaranteed background networking after OS suspension/force-kill.
- Voice/video chat.

## 39. Coding-agent guardrails

When modifying V2:

- preserve the TurnNet/Iroh ownership boundary;
- keep the core game-engine neutral;
- never assume four players;
- do not reintroduce V1 WebRTC/ICE/STUN/TURN or custom LAN TCP;
- do not use Iroh endpoint identity as TurnNet peer identity;
- do not put Nearby/Wi-Fi Aware behind unstable Iroh custom transports in V2;
- keep temporary connection state separate from membership;
- keep `action_id` stable across retries/migration;
- propagate `commit_index`, `state_version`, `membership_epoch`, and commit hash correctly;
- require both host capability and migration eligibility during election;
- do not silently accept incompatible higher-term branches;
- do not merge split-brain state automatically;
- do not assume all hidden-information peers have the same representation hash;
- do not leak secrets through discovery advertisements;
- keep relay configuration injectable and non-authoritative;
- keep Local LAN usable with no Internet;
- preserve deterministic simulated tests before transport-specific debugging;
- update Decisions + Implementation Spec + AI Context together for architecture changes.

## 40. External Iroh reference baseline

Verified baseline for V2 architecture as of 2026-09-05:

- Iroh 1.1.0 stable core connectivity API.
- Peer-to-peer encrypted QUIC connections with direct connectivity/hole punching and relay fallback.
- ALPN-selected application protocols and multiplexed QUIC streams.
- `EndpointId`/`EndpointAddr` addressing.
- address-lookup support.
- endpoint tickets/application tickets for bootstrap patterns.
- mDNS address lookup crate for local-network endpoint discovery without relay/outside Internet.
- relay maps can be disabled/default/custom.
- custom transports remain unstable and are excluded from the V2 contract.

References:

- https://docs.rs/iroh/1.1.0/iroh/
- https://docs.rs/iroh/1.1.0/iroh/endpoint/struct.Endpoint.html
- https://docs.iroh.computer/concepts/tickets
- https://docs.rs/iroh-tickets/1.0.0/iroh_tickets/
- https://docs.rs/iroh-mdns-address-lookup/0.4.0/iroh_mdns_address_lookup/
- https://docs.iroh.computer/iroh-services/relays/managed

