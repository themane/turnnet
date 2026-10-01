---
document: turn_based_multiplayer_harness_v2_implementation_spec
version: 2.0
status: implementation_baseline
date: 2026-09-05
working_name: TurnNet
working_name_is_placeholder: true
supersedes:
  - Turn_Based_Multiplayer_Harness_V1_Implementation_Spec.md
supported_players:
  min: 2
  max: 8
primary_platforms:
  - android
  - ios
web_support: out_of_scope_v2
documentation_format: markdown
---

# TURN-BASED MULTIPLAYER NETWORKING HARNESS V2

## Implementation Specification for AI-Assisted Development

Version 2.0 | 5 September 2026  
Status: Implementation Baseline  
Supported session size: 2-8 players

| Area | V2 baseline |
|---|---|
| Core | Engine-agnostic Rust session/protocol core with Kotlin/Swift public wrappers |
| Same room | Google Nearby Connections (`P2P_CLUSTER`) + Wi-Fi Aware fallback |
| Same Wi-Fi / offline LAN | Iroh QUIC + local mDNS address lookup, relay disabled |
| Internet | Iroh QUIC + direct/hole-punched paths + configurable relay fallback |
| Authority | One authoritative host per healthy term, deterministic migration |
| State | Full logical state commits, commit hash chain, configurable visibility/recovery policy |
| Reliability | Stable logical action IDs, committed membership, recovery blobs, conflict fencing |

The working name **TurnNet** is a placeholder. Public concepts and file structure must not depend on the final product name.

# How to use this specification

This document is implementation-driving. Developers and coding agents must treat explicit invariants, APIs, protocol semantics, migration rules, security boundaries, tests, and acceptance criteria as normative unless the V2 Decisions file supersedes them.

Canonical companion documents:

- `Turn_Based_Multiplayer_Harness_V2_Decisions.md`
- `Turn_Based_Multiplayer_Harness_V2_AI_Context.md`
- `Phase1_Implementation_Plan.md` (Phase 1 design; clarifications recorded in Decisions §24)
- `protocol.md` (normative wire format and hash encodings)

When architecture changes, update all affected V2 Markdown documents in the same task.

# 0. Documentation standard

All project documentation is Markdown (`.md`). Do not create DOCX/PDF project documentation unless explicitly requested for a specific one-off artifact.

Source code, protobuf, JSON/YAML configuration, fixtures, images, binary packages, and generated build artifacts use appropriate formats.

# 1. Purpose

Build an engine-agnostic multiplayer networking/session library for casual, low-bandwidth, turn-based games.

The core library must not understand Ludo, chess, cards, quiz answers, rendering, animation, or any specific game engine.

The same harness should support:

- board games;
- card games;
- chess-like games;
- quiz/trivia games;
- party games;
- other small 2-8 player turn-based games.

Each application includes both host and client capability. Any eligible peer may host initially or after migration.

# 2. V2 design objective

V2 deliberately separates:

```text
IP connectivity mechanics           Multiplayer/session semantics
-------------------------           -----------------------------
Iroh QUIC                            TurnNet protocol
Iroh endpoint addressing            session/peer identity
hole punching                       authoritative host
relay fallback                      membership commits
path migration                      game-state commits
mDNS endpoint lookup                action idempotency
                                    hidden-state policy
                                    host migration
                                    split-brain fencing
                                    recovery
```

TurnNet is not an alternative implementation of Iroh. TurnNet is a turn-based multiplayer state/authority layer that uses Iroh for IP connectivity.

# 3. Non-negotiable product decisions

- The networking harness is a reusable library first; games consume it.
- Core session/protocol logic is Rust and game-engine independent.
- Android and iOS are primary V2 targets.
- Web/browser support is out of scope.
- Support every session size from 2 through 8.
- Gameplay is turn-based and low-bandwidth.
- Exactly one current host is authoritative in a healthy term.
- Host migration is required in V2.
- Full logical authoritative state is committed after each accepted game-state action.
- Game payloads are opaque bytes to TurnNet.
- Committed membership and transient connectivity are distinct.
- Retried logical actions are idempotent through stable `action_id`.
- Hidden-information games use a pluggable state visibility/recovery policy.
- Anti-cheat/fair-randomness protocols are not priorities.
- No cloud game-state server, matchmaking backend, or account service is required.
- Relay infrastructure may be used for Internet connectivity but is never game authority.

# 4. Three V2 play/hosting profiles

The SDK exposes three explicit V2 profiles. All profiles run the same session protocol and game-state semantics.

## 4.1 `SAME_ROOM`

Purpose: devices physically near one another, including cases with no shared router and no Internet.

Transport order:

1. Google Nearby Connections.
2. Wi-Fi Aware/NAN fallback where supported.

Requirements:

- Google Nearby is primary;
- use `P2P_CLUSTER` by default;
- retain `P2P_STAR` only as a measured fallback experiment;
- Wi-Fi Aware is attempted when Nearby is unsupported/unavailable/permission-blocked/fails;
- Wi-Fi Aware is direct nearby P2P, not LAN-over-router;
- feature-detect capabilities at runtime;
- unsupported combinations fail gracefully;
- platform APIs and permission handling remain outside the Rust session core;
- do not depend on Iroh unstable custom transports.

## 4.2 `LOCAL_LAN`

Purpose: devices share a reachable Wi-Fi/LAN but outside Internet may be absent.

Transport:

```text
Iroh QUIC
+ local direct IP paths
+ iroh mDNS address lookup
+ TurnNet session probe
+ relay disabled
```

Requirements:

- no outside Internet dependency;
- no relay dependency;
- no custom TCP protocol;
- discovery finds Iroh endpoints, then TurnNet identifies compatible/joinable sessions;
- same TurnNet ALPN and wire protocol as Internet Iroh mode;
- LAN isolation must be diagnosed separately from protocol failure.

## 4.3 `INTERNET`

Purpose: remote peers across the Internet.

Transport:

```text
Iroh QUIC
  -> direct path when possible
  -> NAT traversal / hole punching
  -> encrypted relay forwarding when needed
```

Requirements:

- no WebRTC;
- no ICE/STUN/TURN implementation in TurnNet;
- no two-way manual signaling;
- relay configuration is injectable;
- development may use Iroh defaults;
- production may use dedicated/custom relay maps;
- relay carries encrypted endpoint traffic and does not hold authoritative game state;
- initial join uses a TurnNet invite containing Iroh dialing information and a TurnNet join capability.

# 5. Iroh external baseline

V2 assumes the stable Iroh 1.1.x API family. The release implementation pins exact dependency versions in Cargo lockfiles.

Implementation-relevant Iroh concepts:

- `Endpoint` - creates/accepts peer connections;
- `EndpointId` - cryptographic transport endpoint identity;
- `EndpointAddr` - endpoint identity plus available transport addresses;
- QUIC reliable streams;
- ALPN selection;
- address lookup services;
- direct/hole-punched connectivity;
- relay fallback;
- route/path changes;
- `watch_addr()` and route updates;
- `network_change()` notification for platforms such as Android;
- application/ticket bootstrap patterns;
- local mDNS address lookup.

Do not couple TurnNet to unstable Iroh custom-transport APIs in V2.

# 6. Selected implementation architecture

## 6.1 Core language

Use Rust for:

- session state machine;
- protocol;
- identity types;
- authoritative commit model;
- membership;
- action routing/deduplication;
- host election/migration;
- state replication policy interfaces;
- recovery;
- diagnostics normalization;
- Iroh endpoint/runtime integration;
- Iroh QUIC peer-link implementation;
- simulation.

The Rust session core must contain no game-engine or UI dependency.

## 6.2 Platform code

Android:

- Kotlin public facade;
- C ABI/JNI bridge into TurnNet Rust;
- Google Nearby adapter;
- Wi-Fi Aware adapter;
- permission/lifecycle/network-change helpers;
- sample/test application.

iOS:

- Swift public facade;
- C ABI bridge into TurnNet Rust;
- Google Nearby adapter where supported by the selected SDK/platform combination;
- Wi-Fi Aware adapter where supported;
- permission/lifecycle helpers;
- sample/test application.

Iroh itself is embedded in TurnNet Rust. Consuming apps should not need to integrate `iroh-ffi` independently.

## 6.3 Layering

Use these conceptual layers:

```text
1. Game / GameAdapter
2. Public MultiplayerSession API
3. Rust Session Core
4. TurnNet Wire Protocol
5. DiscoveryProvider abstraction
6. PeerLink abstraction
7. Transport implementations
   - IrohPeerLink
   - NearbyPeerLink
   - WifiAwarePeerLink
   - SimulatedPeerLink
```

Discovery and peer-link connectivity are separate abstractions.

Examples:

- Iroh mDNS discovers local endpoints; Iroh QUIC carries bytes.
- Internet invite supplies Iroh dialing information; Iroh QUIC carries bytes.
- Nearby discovery advertises a same-room session; Nearby byte payload/link carries frames.

# 7. Core interfaces

## 7.1 `DiscoveryProvider`

Conceptual interface:

```text
DiscoveryProvider
  start_host_advertisement(SessionAdvertisement)
  stop_host_advertisement()
  start_discovery(DiscoveryFilter)
  stop_discovery()
  events() -> DiscoveryEvent stream
```

Provider events must be normalized before entering the session state machine.

## 7.2 `PeerLink`

Conceptual interface:

```text
PeerLink
  connect(RouteDescriptor, ConnectContext) -> PeerConnection
  accept() -> IncomingPeerConnection
  send_control(peer, framed_bytes)
  open_state_stream(peer) -> optional reliable stream
  close(peer, reason)
  get_capabilities()
  get_path_info(peer)
  events() -> PeerLinkEvent stream
```

Semantics expected by the core:

- reliable delivery or explicit disconnect/error;
- message framing above raw byte stream where needed;
- bounded payload/queue handling;
- peer transport binding exposed for authentication/routing;
- no direct game-state mutation.

## 7.3 Iroh-specific runtime

Recommended internal abstraction:

```text
IrohRuntime
  endpoint
  connection_manager
  route_book
  relay_config
  mdns_lookup?       // LOCAL_LAN
  network_watch
```

Prefer one Iroh `Endpoint` per TurnNet networking runtime/process.

# 8. Iroh ALPN and protocol selection

Use a TurnNet-owned major-version ALPN.

Conceptual value:

```text
turnnet/2
```

Rules:

- accept only supported TurnNet ALPNs;
- major protocol incompatibility rejects connection cleanly;
- future clients may offer additional compatible/legacy ALPNs if an explicit backward-compatibility policy is added;
- native Nearby/Wi-Fi Aware transports still carry `protocol_major` in the TurnNet envelope because they do not use QUIC ALPN.

Do not use a generic Iroh protocol ALPN that would allow unrelated application traffic to enter the TurnNet session decoder.

# 9. Iroh endpoint identity vs TurnNet identity

Iroh `EndpointId` is transport identity. TurnNet `peer_id` is session identity.

They must remain separate.

Reasons:

- an Iroh endpoint may be process-scoped or runtime-scoped;
- network routes and endpoint addresses change;
- a process-restarted client may reconnect with a new Iroh endpoint;
- TurnNet wants session-scoped pseudonymous player identity;
- game state should not depend on networking keys.

The session maintains an authenticated mutable transport binding:

```text
PeerRouteBinding
  peer_id
  route_generation
  transport_kind
  iroh_endpoint_id?       // Iroh modes
  endpoint_addr_hints?    // Iroh modes
  native_route_data?      // Same Room
  updated_at
```

A route-binding update does not itself change committed membership.

# 10. Iroh endpoint lifecycle

## 10.1 Endpoint construction

Configure:

- supported TurnNet ALPN(s);
- profile-appropriate relay mode;
- address lookup services;
- metrics/diagnostics hooks if enabled;
- platform DNS/network context.

### `LOCAL_LAN`

- relay disabled;
- enable local mDNS address lookup;
- avoid reliance on global coordination services;
- publish/filter only the local addressing information needed for the mode.

### `INTERNET`

- relay mode configurable;
- direct addresses and relay information allowed;
- production relay maps configurable/injectable;
- for high availability, production operations should normally configure multiple relays in distinct regions; this is deployment guidance, not a wire-protocol invariant;
- optional address lookup may be configured by the app/runtime, but the initial invite must be sufficient to bootstrap under the supported invite policy;
- before creating an Internet invite, wait for/validate endpoint remote dialability and current address information (normally after Iroh reports sufficient online readiness).

## 10.2 Address updates

Monitor endpoint address changes.

When relevant addressing changes:

1. update local route generation;
2. send authenticated `PEER_ROUTE_UPDATE` to appropriate session peers;
3. do not change `peer_id`, `join_order`, or `membership_epoch`;
4. update diagnostics.

## 10.3 Android networking

Before constructing the endpoint with default DNS behavior, initialize required Android/JNI networking context.

Platform network callbacks should notify Iroh of potential network changes.

The wrapper also reports lifecycle state to TurnNet so `host_capable` can change when the app can no longer reliably host.

# 11. Iroh connection manager

For every Iroh peer pair:

- prefer one active QUIC connection;
- avoid redundant application-level connections unless connection replacement is in progress;
- reuse the connection for control + state streams;
- bind remote `EndpointId` to the authenticated TurnNet peer after HELLO/JOIN/RECONNECT;
- detect remote route changes without treating them as new players.

For 8 peers a full mesh has at most 28 pairwise connections. Validate resource use on real Android/iOS devices before release.

# 12. QUIC stream plan

V2 does not need unreliable datagrams.

Recommended stream roles:

## 12.1 Control stream

One long-lived bidirectional stream per peer connection.

Carries:

- HELLO/authentication;
- join/reconnect;
- membership;
- heartbeat;
- action submit/reject;
- commit metadata;
- ACKs;
- host migration;
- route updates;
- errors/diagnostics control.

Frames are length-delimited and bounded.

## 12.2 State transfer stream

Small states may be carried directly in control messages.

For larger states/recovery payloads, open a dedicated reliable uni/bi QUIC stream identified by a small control header, e.g.:

```text
STATE_TRANSFER_BEGIN
  transfer_id
  commit_index
  representation_kind
  declared_length
  representation_hash
```

The receiver validates declared size before reading/allocating the full payload.

This avoids making a large snapshot block unrelated control frames at the application stream level.

## 12.3 No transport-level fragmentation duplication

Do not implement packet fragmentation over Iroh QUIC merely because a payload exceeds a network MTU; QUIC handles transport segmentation.

TurnNet framing remains required for message boundaries and size limits.

# 13. Local LAN discovery with Iroh mDNS

Iroh mDNS address lookup discovers endpoint addressing, not committed TurnNet membership.

Recommended flow:

1. each TurnNet runtime exposes the TurnNet ALPN on its Iroh endpoint;
2. `LOCAL_LAN` host marks its session joinable in a bounded, non-secret local advertisement strategy;
3. mDNS discovers local Iroh endpoints;
4. discovering client attempts a bounded TurnNet `SESSION_PROBE` to candidate endpoints supporting/accepting the TurnNet ALPN;
5. host responds with sanitized `SESSION_ADVERTISEMENT`;
6. app displays joinable sessions;
7. selected client sends `JOIN_REQUEST`;
8. normal TurnNet membership/authentication applies.

`SESSION_ADVERTISEMENT` may include:

```text
session_id
protocol_major/minor
optional game/application display id
current player count
max player count
joinable boolean
host display metadata supplied by app
```

It must not include:

- reconnect capability;
- private game state;
- recovery material;
- long-lived secrets.

# 14. Internet invite model

V2 Internet bootstrap is one-way from the user perspective.

## 14.1 `TurnNetInvite`

Before producing an Internet invite, the host runtime must verify that the current Iroh endpoint address contains a usable remote dialing path according to configured policy. Normally the endpoint is allowed to reach its online/relay-ready state before serializing the invite. A Local LAN host does not perform this Internet-readiness wait.

Conceptual structure:

```text
TurnNetInvite
  invite_version
  protocol_major
  protocol_minor_hint
  play_profile = INTERNET
  session_id
  host_endpoint_addr
  join_capability
  optional_expiry_unix_ms
  optional_game_id
  optional_display_metadata
  optional_flags
```

`host_endpoint_addr` may be represented internally using Iroh `EndpointAddr` or an application-specific ticket encoding containing the required Iroh dialing information.

Do not expose the raw Iroh ticket as the whole public admission object.

## 14.2 Invite presentation

The same invite may be represented as:

- QR code;
- deep link;
- copy/paste text;
- small file;
- image import containing a QR.

Applications must not assume the user can scan a QR displayed on the same phone.

## 14.3 Join capability

The invite contains an unpredictable TurnNet join capability or equivalent proof.

Host maintains invite policy such as:

- active/revoked;
- expiry time;
- max uses if the application requests it;
- joinable session state.

Iroh addressing remaining dialable does not imply TurnNet admission remains valid.

## 14.4 Privacy

Iroh dialing data can include IP addresses and relay information. Treat the invite as sensitive bearer material.

TurnNet should expose app guidance that invites can be forwarded or reused unless the host's join capability policy rejects them.

# 15. Same-room discovery and joining

## 15.1 Google Nearby

Host:

- advertises session service identifier;
- exposes sanitized session metadata;
- accepts connection request;
- completes TurnNet HELLO/JOIN after transport connection.

Client:

- discovers advertisements;
- selects session;
- connects through Nearby;
- completes TurnNet HELLO/JOIN.

Use `P2P_CLUSTER` by default for 2-8 players.

## 15.2 Wi-Fi Aware

Map platform publisher/subscriber/pairing primitives to TurnNet discovery/connect without leaking platform-specific concepts into the Rust session core.

Requirements:

- attempt after Nearby fails/unavailable;
- prefer secure pairing where supported;
- feature-detect at runtime;
- fail gracefully on unsupported devices.

# 16. Game integration contract

The network core treats game actions and game state as opaque bytes.

Conceptual game adapter:

```text
GameAdapter
  validate_and_apply(
      actor_peer_id,
      action_bytes,
      current_authoritative_state_bytes
  )
      -> Accepted(new_authoritative_state_bytes, optional_event_bytes)
      -> Rejected(reason_code)

  on_state_committed(
      peer_visible_state_bytes,
      state_version,
      commit_index
  )

  on_session_event(event)
```

The exact ABI-safe representation may differ.

The game owns:

- rules;
- move legality;
- turn rules;
- random-number policy;
- game serialization;
- hidden-information meaning;
- UI/animations.

TurnNet owns:

- peers/session identity;
- discovery/connectivity;
- admission and membership;
- host authority;
- action routing;
- action/message dedupe;
- state commit ordering;
- state replication plumbing;
- migration/recovery;
- protocol evolution;
- diagnostics.

# 17. Public session API

The SDK must expose equivalent concepts to:

```text
create_session(CreateSessionOptions) -> SessionHandle
start_discovery(PlayProfile, DiscoveryOptions) -> DiscoveryHandle
join_discovered_session(DiscoveredSession) -> SessionHandle
join_session(TurnNetInvite) -> SessionHandle
leave_session(LeaveOptions?)
end_session(EndReason?)
submit_action(action_bytes) -> ActionHandle / action_id
retry_action(action_id)     // may be internal rather than public
get_session_info()
get_peers()
get_current_host()
get_connection_state()
get_network_path_info()
create_invite(InviteOptions)
import_invite(blob)
export_recovery_blob()
restore_session(recovery_blob) -> SessionHandle / RecoveryResult
```

All APIs are asynchronous/non-blocking from the application UI thread.

## 17.1 `CreateSessionOptions`

Conceptual fields:

```text
play_profile
max_players                 // 2..8
initial_game_state_bytes
state_replication_policy
join_policy
protocol/application metadata
reliability_policy?
frame_policy?
relay_policy?               // INTERNET only
```

Do not expose Iroh `Endpoint`, `Connection`, or `EndpointAddr` directly through the game-facing SDK.

# 18. Public events

Required event concepts:

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

`NetworkPathChanged` may report Iroh direct/relay/local path changes without implying logical disconnect.

# 19. Identity model

Use separate identifiers.

## 19.1 `session_id`

- random 128-bit;
- created once at session creation;
- preserved across migration.

## 19.2 `peer_id`

- random 128-bit session participant identity;
- created before/at admission;
- stable across reconnect;
- independent of transport endpoint.

## 19.3 `join_order`

- monotonically allocated from `next_join_order`;
- assigned by the currently authoritative host that commits admission;
- immutable for that participant;
- never reused.

This fixes the V1 ambiguity that said only the initial host assigns join order.

## 19.4 `term`

Monotonic host-generation number.

- initial host starts at term 1;
- successful migration/transfer advances term;
- old-term authority messages are stale.

## 19.5 `state_version`

Monotonic game-state version.

- initial committed game state is version 0;
- accepted game action increments exactly once;
- membership-only commit does not increment it.

## 19.6 `membership_epoch`

Monotonic roster version.

- initial host membership starts at epoch 1;
- committed admission/removal increments it;
- transient disconnect does not increment it.

## 19.7 `commit_index`

Monotonic authoritative session commit sequence.

- initial session commit starts at index 1;
- every authoritative game-state or membership commit increments it;
- provides one total order across state and roster domains.

## 19.8 `message_id`

Unique protocol-message dedupe identifier.

## 19.9 `action_id`

Unique logical action identifier.

It remains unchanged across:

- retries;
- new `message_id`s;
- reconnect;
- host migration.

# 20. Membership model

Committed roster record conceptually contains:

```text
PeerMembership
  peer_id
  join_order
  member_status
  admission_commit_index
  optional display metadata owned by app
```

Connectivity is separate:

```text
PeerConnectivity
  CONNECTED
  DEGRADED
  RECONNECTING
  INTERRUPTED
  UNREACHABLE
```

A disconnected committed member remains a member until a membership change is explicitly committed.

## 20.1 Membership changes

Membership commits include:

- join;
- voluntary leave;
- explicit removal/eviction according to app/session policy.

Each membership commit:

- increments `membership_epoch`;
- increments `commit_index`;
- updates roster hash;
- chains a new `commit_hash`;
- replicates to all reachable peers.

# 21. Authoritative commit model

V2 creates one authoritative commit chain.

## 21.1 Commit types

At minimum:

```text
INITIAL_STATE
GAME_ACTION
MEMBER_JOIN
MEMBER_LEAVE
MEMBER_REMOVE
OPTIONAL_POLICY_METADATA_CHANGE
```

Host migration itself changes `term` and authority but does not need to fabricate a new game-state version. The first commit in the new term references the migration base commit.

## 21.2 Commit metadata

Conceptual structure:

```text
AuthoritativeCommitMeta
  session_id
  term
  commit_index
  previous_commit_hash
  commit_kind
  state_version
  membership_epoch
  host_peer_id
  roster_hash
  logical_state_hash
  next_join_order
  migration_eligibility_digest
  policy_metadata_digest?
  committed_action_id?
  affected_peer_id?
  commit_hash
```

## 21.3 `commit_hash`

Conceptually hash a canonical encoding of:

```text
session_id
term
commit_index
previous_commit_hash
commit_kind
state_version
membership_epoch
host_peer_id
roster_hash
logical_state_hash
next_join_order
migration_eligibility_digest / policy metadata
committed_action_id / affected peer metadata where applicable
```

Use a fast cryptographic hash such as BLAKE3 or SHA-256.

Canonical encoding must be deterministic and explicitly specified in code/protocol docs before interoperable release.

# 22. State hashes and per-peer representations

V2 has three distinct hash concepts.

## 22.1 `logical_state_hash`

Hash of canonical logical authoritative game state.

For trusted full replication, all peers receive the same state and can verify this hash.

For hidden-information policies, some peers may not possess the full logical state and therefore cannot independently recompute it. They still receive the committed hash as branch metadata.

## 22.2 `representation_hash`

Hash of the exact state representation delivered to a peer.

Examples:

```text
public_state || player_A_private_state
public_state || player_B_private_state
public_state || encrypted_recovery_material_for_C
```

Different peers may have different representation hashes for the same logical commit.

## 22.3 `commit_hash`

Hash of authoritative commit-chain metadata.

This is used to detect:

- branch divergence;
- stale base commits;
- inconsistent migration claims;
- mismatched committed history.

None of these hashes is an anti-cheat mechanism against a malicious authoritative host.

# 23. State replication policy

Define a pluggable policy surface.

Conceptually:

```text
StateReplicationPolicy
  build_public_state(authoritative_state, commit_context)
      -> bytes

  build_private_state_for(peer_id, authoritative_state, commit_context)
      -> optional bytes

  build_recovery_material_for(peer_id, authoritative_state, commit_context)
      -> optional bytes

  migration_capability_for(peer_id, commit_context)
      -> MigrationCapability

  reconstruct_authoritative_state(
      public_state,
      private_state?,
      recovery_material?,
      commit_context
  ) -> authoritative_state / failure
```

## 23.1 Default: trusted full replication

- all peers receive the same full authoritative game state;
- easiest migration/reconnect;
- `migration_eligible = true` for latest-state peers that are otherwise capable;
- no confidentiality from modified peers.

## 23.2 Public + private state

- common public state sent to all;
- recipient-specific private payload;
- host owns complete logical state;
- recovery material determines which peers can become future host.

## 23.3 Authenticated-encrypted recovery

If encrypting private/recovery material:

- use authenticated encryption (AEAD);
- do not treat state hash/checksum as authentication;
- giving a peer decryption keys gives that peer corresponding plaintext access.

## 23.4 Advanced policies

Commitment-based and threshold/secret-sharing recovery are not mandatory V2 implementations but architecture must not block them.

# 24. Migration capability metadata

`host_capable` and `migration_eligible` are separate runtime/policy values.

## 24.1 `host_capable`

Derived from:

- lifecycle/foreground viability;
- platform permissions;
- current selected transport capability;
- ability to advertise/accept/dial as required;
- resource health.

## 24.2 `migration_eligible`

Derived from the state replication policy and latest commit.

A peer is migration-eligible only when it can reconstruct/continue authoritative game state for the migration base.

## 24.3 Deterministic visibility

The committed metadata must allow peers to agree on which members are migration-eligible without revealing unauthorized private state.

One implementation option:

```text
migration_eligibility_set = sorted peer_ids eligible for this commit
```

or a canonical equivalent policy result included/bound in commit metadata.

# 25. Action submission semantics

## 25.1 Client submission

Client sends:

```text
ACTION_SUBMIT
  action_id
  expected_term
  expected_commit_index
  expected_state_version
  action_payload
```

`expected_*` fields are advisory/concurrency guards. Host may reject stale action context according to policy.

## 25.2 Host validation order

Before game rules:

1. valid protocol/session;
2. sender is authenticated/bound member;
3. term is current;
4. sender membership is active;
5. payload size is allowed;
6. `action_id` has not already committed;
7. action context is acceptable.

Then call game adapter.

## 25.3 Accepted action

Exactly once:

1. apply game action;
2. increment `state_version`;
3. increment `commit_index`;
4. add `action_id` to committed dedupe history;
5. compute state/roster/commit hashes;
6. build each peer's state representation;
7. broadcast/transfer `STATE_COMMIT`;
8. record per-peer ACK state.

## 25.4 Duplicate action

If an `action_id` already committed:

- do not call game adapter again;
- do not increment state version;
- return/replicate enough information for requester to learn the existing committed result.

# 26. Action dedupe retention

Correctness requirement: committed logical actions cannot execute twice after host migration.

Default V2 policy for small turn-based sessions:

- retain every committed `action_id` for the active session;
- replicate/make available this dedupe state to migration-eligible peers;
- include it in recovery material as needed.

If memory measurements later justify compaction, the replacement must have a proof/contract preserving idempotency. Do not silently convert to a small time-based cache.

# 27. State acknowledgement

Each peer reports its latest successfully accepted committed head.

Conceptual:

```text
STATE_ACK
  commit_index
  commit_hash
  state_version
  membership_epoch
  representation_hash?
```

Host tracks:

```text
last_acknowledged_commit_index
last_acknowledged_commit_hash
last_acknowledged_state_version
last_acknowledged_membership_epoch
```

This supports diagnostics and migration eligibility.

# 28. Host election

Host migration is mandatory.

## 28.1 Host suspicion

Heartbeat/liveness timeout moves the session to `HOST_SUSPECTED`.

Do not instantly mutate authority based on one transport callback.

## 28.2 Candidate filtering

A candidate must:

1. be in the current committed membership;
2. be reachable/available under the current migration view;
3. reconcile to the latest verifiable compatible commit head;
4. have the current membership epoch;
5. have `host_capable = true`;
6. have `migration_eligible = true`.

## 28.3 Deterministic ordering

Among equally eligible candidates, smallest `join_order` wins.

## 28.4 Term

Candidate proposes `old_term + 1`.

No candidate may use an old term to reclaim authority.

# 29. Migration barrier

Host migration must not resume gameplay until peers establish a consistent base.

Conceptual flow:

1. peers suspect host;
2. peers exchange/observe committed head metadata;
3. determine latest compatible migration base;
4. deterministically choose candidate;
5. candidate reconstructs authoritative state;
6. candidate sends `HOST_PROPOSE`:

```text
new_term
candidate_peer_id
base_commit_index
base_commit_hash
base_state_version
membership_epoch
```

7. reachable peers validate deterministic choice and base compatibility;
8. peers send `HOST_ACK`;
9. default casual activation requires at least two session members including candidate;
10. candidate sends `HOST_ANNOUNCE` and becomes active host;
11. candidate sends state sync to any behind peers;
12. gameplay resumes.

This is not Raft/Paxos and does not claim majority-partition safety.

# 30. Split-brain and partition fencing

Because V2 allows continuation with two peers and does not require majority quorum, some partitions can independently form authority.

V2 must detect rather than silently merge divergence.

## 30.1 Conflict examples

- same term, different host claims;
- same `commit_index`, different `commit_hash`;
- higher-term claim based on a commit branch incompatible with local committed history;
- two post-fork branches both contain accepted game actions.

## 30.2 Required behavior

On detected authority conflict:

1. transition to `AUTHORITY_CONFLICT` / `INTERRUPTED`;
2. stop new action commits;
3. stop automatic host switching;
4. retain branch diagnostics;
5. emit `AuthorityConflict` with redacted branch metadata;
6. do not automatically merge or pick a winner that would silently discard committed turns.

A future app/game recovery policy may explicitly select a branch or restart. That is outside default V2 automatic behavior.

## 30.3 Higher-term validation

A higher term is necessary but not sufficient for authority acceptance.

The new authority's base commit must be:

- identical to the local head; or
- a verifiable descendant when local is behind; or
- otherwise reconcilable without discarding a locally committed incompatible branch.

If not, fence.

# 31. Graceful host transfer

When the host intentionally leaves:

1. choose next eligible peer using normal election ordering excluding leaving host;
2. verify migration base;
3. send `HOST_TRANSFER` / migration proposal;
4. advance term after acknowledgement;
5. new host announces authority;
6. old host commits/leaves membership according to ordered transfer semantics;
7. remaining peers continue.

If no eligible replacement exists, session may end or enter interrupted state according to game policy.

# 32. Iroh Internet migration

Internet mode maintains peer control connectivity when feasible.

On host loss:

- surviving QUIC connections remain useful;
- peers exchange migration messages directly;
- clients reroute actions to the new host over existing connection if present;
- if not present, dial using latest route information;
- no QR rescan;
- no offer/answer;
- no signaling relay protocol.

Each peer should distribute sufficiently fresh `EndpointAddr`/route information to surviving peers during normal operation.

# 33. Iroh LAN migration

On Local LAN:

- existing direct QUIC peer links are reused;
- mDNS address lookup can rediscover missing routes;
- elected host updates joinable session advertisement state if new joins remain allowed;
- outside Internet and relay remain unnecessary;
- state sync occurs before gameplay resumes.

# 34. Same-room migration

On Nearby/Wi-Fi Aware:

- use common host suspicion/election logic;
- elected peer starts hosting/advertising existing `session_id` at new term;
- other peers reconnect as needed;
- route bindings update without new TurnNet identities;
- reconcile authoritative state before resume.

# 35. Reconnection semantics

Temporary transport failure must not create a new player.

Preserve:

- session ID;
- peer ID;
- join order;
- committed membership;
- last known term;
- commit head;
- state version;
- membership epoch;
- pending/retriable action IDs;
- recovery material.

## 35.1 Reconnect flow

1. enter `RECONNECTING` after real peer-link loss;
2. try current host route;
3. if host unknown/unreachable, try known session peer routes where profile permits;
4. establish peer link;
5. send `RECONNECT_HELLO` with reconnect proof and local commit head;
6. route to current authority if connected peer is not host;
7. host authenticates existing membership;
8. host may accept a new transport binding for same `peer_id`;
9. compare commit histories;
10. sync missing state/recovery material;
11. rebuild peer mesh/control links as needed;
12. mark connection restored.

# 36. Process-death recovery

Public API:

```text
export_recovery_blob() -> bytes
restore_session(recovery_blob) -> RecoveryResult
```

Recovery blob is opaque and versioned.

It may contain:

```text
session_id
peer_id
join_order
known term
commit index/hash
state_version
membership_epoch
reconnect capability
last-known host/peer routes
pending action IDs
committed-action dedupe material where required
state/recovery representation required by policy
policy version
```

It does not have to persist the Iroh endpoint private key.

After restart, the runtime may create a new Iroh endpoint and authenticate a route rebinding to the existing TurnNet peer identity.

Applications choose persistence and use platform-appropriate secure storage for secrets.

# 37. Route updates

Iroh endpoint addresses can change as network/relay conditions change.

Define authenticated route updates:

```text
PEER_ROUTE_UPDATE
  peer_id
  route_generation
  transport_kind
  iroh_endpoint_id
  endpoint_addr_data
  sent_at
```

Rules:

- sender must already authenticate as `peer_id` or be in a reconnect flow proving that identity;
- `route_generation` must increase for updates from the same logical peer runtime lineage;
- route update does not change `membership_epoch`;
- route update is not a state commit;
- do not include join/reconnect secrets;
- distribute to peers required for full mesh/migration.

# 38. Iroh path changes

Iroh may transition between relay/direct paths or react to network changes.

TurnNet behavior:

- if QUIC connection remains alive, keep the logical peer connected;
- emit `NetworkPathChanged`/diagnostics if useful;
- do not start host migration because a path changed;
- update RTT/path metrics;
- allow Iroh to manage its connectivity paths.

Only real loss of host liveness triggers host suspicion.

# 39. Protocol encoding

Use versioned binary control messages. V2 recommendation remains Protocol Buffers.

Game action/state payloads remain `bytes`.

## 39.1 Envelope

Conceptual fields:

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

## 39.2 Validation order

Before dispatch:

1. frame length/hard max;
2. Protobuf decode;
3. known/allowed message type;
4. protocol major/minor compatibility;
5. session ID;
6. sender transport binding/authentication;
7. term semantics;
8. membership/commit context;
9. message-specific payload limits;
10. dedupe by `message_id`.

Unknown optional fields are ignored according to protobuf compatibility rules.

# 40. Required V2 message categories

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

`SIGNALING_RELAY` is intentionally absent.

# 41. HELLO and transport binding

Every newly established `PeerLink` begins with a protocol hello before session messages are trusted.

Conceptual `HELLO`:

```text
protocol_major
protocol_minor
session_id?             // present for join/reconnect target where known
turnnet_peer_id?        // present for existing peer/reconnect
transport_kind
transport_identity      // e.g. Iroh EndpointId verified by connection
capabilities
nonce/challenge fields
```

The remote transport identity observed from the actual connection must match the identity claimed in protocol metadata where applicable.

HELLO does not by itself authorize session membership.

# 42. JOIN protocol

## 42.1 New peer

Client chooses a random candidate `peer_id` and sends:

```text
JOIN_REQUEST
  session_id
  candidate_peer_id
  join_capability/proof where required
  current route metadata
  app display metadata
  protocol capabilities
```

Host:

- verifies join policy/capability;
- verifies session capacity 2-8;
- ensures candidate ID is not already a different participant;
- allocates next monotonic join order;
- commits membership change;
- returns `JOIN_ACCEPT` with roster, current authority, commit head, and initial state representation.

If rejected, no membership epoch changes.

## 42.2 Join order

`next_join_order` is committed/recoverable session metadata so a migrated host can admit later players without reuse/collision.

# 43. Reconnect authentication

A reconnecting peer presents:

- `session_id`;
- existing `peer_id`;
- reconnect capability/proof;
- local commit head;
- current transport binding.

The host must not accept a reconnect merely because the requester knows `peer_id`.

Successful reconnect may update the route binding without membership change.

# 44. Heartbeats and liveness

Heartbeat message should expose enough metadata for migration health:

```text
HEARTBEAT
  term
  commit_index
  commit_hash
  state_version
  membership_epoch
  host_capable
  migration_eligible
  optional path/diagnostic summary
```

Not every field needs to be transmitted on every heartbeat if an equivalent reliable mechanism provides it, but peers must maintain the information required for election.

# 45. Session state machine

Canonical states:

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

## 45.1 Transport path state is separate

Examples:

```text
LOCAL_DIRECT
INTERNET_DIRECT
INTERNET_RELAYED
NATIVE_NEARBY
PATH_RECONNECTING
```

Do not encode direct/relay status as host/session authority state.

# 46. State-machine invariants

- exactly one accepted authoritative host per non-conflicted term view;
- only host commits authoritative state;
- one accepted logical action increments `state_version` once;
- every authoritative state/membership commit increments `commit_index` once;
- membership-only changes increment `membership_epoch`, not `state_version`;
- temporary disconnect changes connectivity, not membership;
- stale term cannot mutate authority/state;
- incompatible branch cannot be overwritten by term number alone;
- peer transport identity may change through authenticated reconnect without changing `peer_id`;
- transport callback cannot directly call game adapter outside session-core dispatch.

# 47. Reliability defaults

Initial defaults:

```text
heartbeat_interval:       2s
peer_suspect_after:       6s
host_election_after:      8s without host liveness
reconnect_grace_period:   60s
action_response_timeout:  10s
join_timeout:             20s
max_join_retries:         3
```

All timers are configurable.

These values apply to active foreground sessions and are not protocol constants.

# 48. Mobile lifecycle

Mobile OS behavior can suspend networking.

Requirements:

- wrappers report foreground/background/interruption to Rust;
- update `host_capable` when the device cannot reliably accept/advertise;
- surface `SessionInterrupted` when networking cannot continue;
- do not promise seamless indefinite background gameplay;
- after return, attempt reconnect/state sync;
- distinguish graceful lifecycle interruption from protocol corruption.

# 49. Transport frame and payload policy

Game serialization and preferred payload size remain application decisions.

TurnNet supplies safe policy defaults and hard limits.

Conceptual:

```text
TransportFramePolicy
  preferred_max_payload_bytes?
  preferred_max_frame_bytes?
  max_reassembly_bytes?
  max_pending_outbound_bytes?
  backpressure_policy?
  oversize_policy = REJECT | STREAM_IF_SUPPORTED | ADAPTER_FRAGMENT
```

Rules:

- Iroh QUIC uses streams/transport segmentation; no packet fragmentation layer needed;
- large logical state may use dedicated QUIC streams;
- native nearby adapters may fragment below the game adapter if necessary;
- declared frame sizes validated before allocation;
- control-message limits are library-owned;
- app policy cannot exceed transport hard limits;
- diagnostics distinguish policy rejection from transport rejection.

# 50. Security and privacy baseline

V2 is not malicious-host hardened, but must provide baseline security hygiene.

## 50.1 Iroh profiles

- preserve Iroh authenticated encrypted endpoint connections;
- verify observed transport identity against TurnNet route binding;
- TurnNet join/reconnect authorization is still mandatory;
- relay is not trusted with plaintext game payloads but may observe network metadata;
- do not claim relay anonymity.

## 50.2 Same room

- preserve platform transport authentication/security;
- do not disable secure-pairing mechanisms;
- validate TurnNet session identity after transport connection.

## 50.3 Protocol hygiene

- size-check before allocation;
- reject malformed enums/fields;
- reject stale term;
- validate branch/base commit;
- dedupe messages/actions;
- never use IP/device name as peer identity;
- no analytics/account/contact/phone-number requirement in core;
- do not log raw game state/action by default.

# 51. Invite security

Internet invite contains bearer material.

Minimum safeguards:

- unpredictable join capability;
- optional expiry;
- host-side revocation;
- optional max-use policy;
- no reconnect secret in the normal join invite;
- no private state;
- clear app guidance that copied/shared invite may be reused until invalidated.

A future signed invite format may be added, but authentication of the connected Iroh endpoint alone is not enough to decide session admission.

# 52. Hidden-information security

For per-peer private state:

- never send another peer's private plaintext unless policy explicitly permits it;
- if encrypting recovery/private payloads, use AEAD;
- `representation_hash` validates delivered representation integrity but does not replace AEAD;
- election skips peers without reconstruction authority;
- tests inspect recipient delivery boundaries.

# 53. Diagnostics and observability

Structured diagnostic event minimum:

```text
timestamp
session_id (redactable)
peer_id (redactable)
profile
transport
connection_state
path_type
term
commit_index
state_version
membership_epoch
event_code
error_category
rtt/latency where available
route_generation where applicable
relay/direct summary where applicable
```

Do not log secrets, invite capabilities, reconnect tokens, or raw game state by default.

## 53.1 Important diagnostic categories

- permission denied;
- transport unsupported;
- Nearby discovery/connect failure;
- Wi-Fi Aware availability/pairing failure;
- mDNS discovery failure;
- LAN client isolation;
- Iroh endpoint bind failure;
- address lookup failure;
- direct connectivity failure;
- relay unavailable;
- relay rate-limit/overload signal where exposed;
- QUIC connection closed;
- protocol version mismatch;
- join auth failure;
- reconnect auth failure;
- stale term;
- stale commit;
- state desync;
- action duplicate;
- authority conflict;
- migration ineligible.

# 54. Simulated transport

Implement deterministic simulation before relying on mobile transports.

Simulation must support:

- latency;
- jitter;
- loss;
- duplication;
- reordering;
- disconnect;
- reconnect;
- host death;
- process restart;
- delayed old-term traffic;
- route changes;
- peer lifecycle changes;
- network partition;
- asymmetric partition;
- conflicting same-term host claims;
- divergent commits;
- stale membership data.

Use injectable deterministic clock/randomness.

# 55. Core V2 automated tests

Run for 2, 4, and 8 peers where applicable.

1. host creates initial session and initial commit;
2. peers join and membership epoch/commit index advance correctly;
3. join after original host migration gets a new never-reused join order;
4. accepted action increments state version and commit index exactly once;
5. duplicate `message_id` ignored;
6. same `action_id` retried under different `message_id` does not double-commit;
7. same `action_id` retry after host migration does not double-commit;
8. stale-term action/control message rejected;
9. client disconnect does not remove committed membership;
10. client reconnects and syncs missing commits;
11. voluntary leave advances membership epoch and commit index;
12. host dies immediately after commit broadcast;
13. host dies while one peer is one commit behind;
14. latest compatible commit is selected as migration base;
15. deterministic lowest join order wins among eligible peers;
16. lower join order with `host_capable=false` is skipped;
17. lower join order with `migration_eligible=false` is skipped;
18. repeated migration preserves state and dedupe history;
19. old host returns with old term and follows new authority;
20. higher-term proposal on incompatible branch triggers conflict;
21. same-term different host claim triggers conflict;
22. two partition branches with accepted turns fence on reconnection;
23. no automatic merge of conflicting game states;
24. protocol major mismatch rejects cleanly;
25. malformed/oversized frame rejects safely;
26. recovery blob restores peer identity/commit context;
27. transport route rebind keeps same peer/join order;
28. hidden-state private payloads reach only authorized recipients;
29. different representation hashes are accepted for same logical commit where policy requires;
30. migration eligibility follows hidden-state recovery capability;
31. game may continue with reduced committed membership where policy permits;
32. session ends cleanly when game/policy cannot continue.

# 56. Iroh `LOCAL_LAN` tests

Required:

1. outside Internet disabled;
2. relay disabled;
3. two endpoints discover via mDNS;
4. session probe finds joinable TurnNet host;
5. 2/4/8 player direct QUIC sessions;
6. all peers exchange route bindings;
7. action/state commits converge;
8. host dies and migration succeeds with no Internet;
9. reconnect after temporary Wi-Fi disruption;
10. mDNS rediscovery after route change;
11. client-isolated router produces diagnostic rather than protocol error;
12. optional manual invite can join over LAN if implemented.

# 57. Iroh `INTERNET` tests

Required:

1. host creates one TurnNet invite;
2. client imports and connects without answer QR;
3. direct connection when available;
4. relay-only fallback when direct path impossible;
5. relay-to-direct path improvement where Iroh supports it;
6. direct-to-relay continuation where path fails and Iroh can preserve/recover connection;
7. path change does not change TurnNet host;
8. configurable custom relay map;
9. production-style multi-region relay map where high availability is required;
10. multi-relay failure/fallback behavior when configured;
11. invite revocation/expiry rejects TurnNet join;
12. full peer mesh at 2/4/8 players;
13. host migration over surviving peer links;
14. missing host connection redial using route book;
15. endpoint address update distributes without membership change;
16. process restart with new endpoint ID rebinds same TurnNet peer;
17. stale route data recovers using newer peer-provided route where possible;
18. repeated migration across direct and relayed peers;
19. diagnostics expose direct/relay/path state without leaking secrets.

# 58. Android Iroh integration tests

- endpoint initializes only after required Android networking/JNI setup;
- application network callback calls Iroh network-change notification;
- Wi-Fi to cellular transition;
- cellular to Wi-Fi transition;
- temporary network loss;
- app background/foreground;
- host capability changes when lifecycle prevents reliable hosting;
- process kill and recovery blob restore;
- Iroh direct and relay Internet paths on real devices;
- Local LAN with outside Internet disconnected.

# 59. iOS Iroh integration tests

- endpoint creation and connection on device;
- Local LAN mDNS/direct Iroh connectivity;
- Internet direct/relay connectivity;
- foreground/background interruption behavior;
- reconnect after app returns;
- process kill/recovery flow;
- host migration when prior host becomes unavailable;
- cross-platform Android↔iOS TurnNet protocol compatibility.

# 60. Same-room transport tests

## Google Nearby

- Android↔Android first-priority baseline;
- supported Android↔iOS combinations where available;
- P2P_CLUSTER 2/4/8 peers;
- discovery/join;
- host death migration;
- intentional host transfer;
- disconnect/reconnect;
- no shared router/Internet.

## Wi-Fi Aware

- runtime availability detection;
- secure pairing where supported;
- Nearby unavailable -> Wi-Fi Aware fallback;
- join/reconnect;
- host migration;
- unsupported hardware/OS graceful failure;
- supported cross-platform matrix where available.

# 61. Performance/resource tests

V2 is low-bandwidth, but connection count and mobile battery/resource use still require measurement.

At 8 peers measure:

- 28 Iroh peer connections in Internet/full-mesh test;
- idle heartbeat traffic;
- relay vs direct traffic;
- CPU;
- memory;
- socket count;
- battery impact on representative phones;
- migration convergence time;
- reconnect time;
- large state snapshot behavior within allowed policy.

If full mesh proves too costly, changing topology is an architecture decision and must update V2 docs.

# 62. Platform deliverables

## 62.1 Android SDK

- AAR;
- Kotlin public facade;
- Rust native libraries for supported ABIs;
- embedded Iroh runtime through TurnNet Rust;
- Google Nearby adapter;
- Wi-Fi Aware adapter;
- Android network/lifecycle helpers;
- sample connection/test application;
- diagnostics console/screens.

## 62.2 iOS SDK

- XCFramework / Swift package wrapper as appropriate;
- Swift public facade;
- Rust static/native core containing Iroh integration;
- Nearby adapter where supported;
- Wi-Fi Aware adapter where supported;
- local network/lifecycle helpers;
- sample connection/test application;
- diagnostics console/screens.

# 63. Repository layout

Recommended:

```text
repo/
  core/
    src/
      ids/
      session/
      membership/
      actions/
      state/
      visibility/
      election/
      recovery/
      protocol/
      diagnostics/
      peer_link/
      discovery/
      simulated/
    proto/
    tests/

  transport/
    iroh/
      endpoint_runtime/
      connection_manager/
      peer_link/
      route_book/
      lan_mdns/
      invite/
      relay/
      diagnostics/

  platform/
    android/
      sdk/
      transports/
        nearby/
        wifi_aware/
      lifecycle/
      sample/

    ios/
      sdk/
      transports/
        nearby/
        wifi_aware/
      lifecycle/
      sample/

  bindings/
    c/

  docs/
    protocol.md
    state_commit_model.md
    transport_contract.md
    iroh_transport.md
    host_migration.md
    recovery.md
    diagnostics.md
```

# 64. Implementation order

Implement in this order.

## Phase 1 - semantics first

1. Rust V2 identity types.
2. `commit_index` + commit hash chain.
3. membership vs connectivity separation.
4. V2 protobuf control envelope/messages.
5. deterministic session state machine.
6. simulated discovery/peer links and clock.
7. 2-8 peer create/join.
8. action routing and stable `action_id` dedupe.
9. full logical state commits and ACKs.
10. committed membership and monotonic join order.
11. state visibility/recovery abstraction.
12. `host_capable` + `migration_eligible`.
13. host suspicion/election/migration barrier.
14. split-brain conflict fencing.
15. reconnect + route rebinding + recovery blob.

## Phase 2 - Iroh core transport

16. Iroh dependency integration and pinned baseline.
17. TurnNet ALPN + endpoint runtime.
18. Iroh connection manager and control streams.
19. route book/watch/update propagation.
20. full Iroh peer mesh.
21. state transfer streams.
22. Iroh diagnostics/path events.

## Phase 3 - Local LAN

23. mDNS address lookup integration.
24. TurnNet session probe/advertisement.
25. relay-disabled offline LAN tests.
26. host migration/reconnect over Local LAN.

## Phase 4 - Internet

27. TurnNet invite codec/wrapper around Iroh dialing data.
28. direct Internet connectivity tests.
29. relay policy injection and relay fallback tests.
30. path-change tests.
31. remote full-mesh migration.
32. endpoint rebinding/process recovery.

## Phase 5 - mobile SDK

33. C ABI/facade skeleton.
34. Android Iroh/JNI/network lifecycle integration.
35. iOS Iroh lifecycle integration.
36. Android/iOS Local LAN matrix.
37. Android/iOS Internet matrix.

## Phase 6 - same room

38. Google Nearby adapter (`P2P_CLUSTER`).
39. Nearby migration/reconnect.
40. Wi-Fi Aware adapter/fallback.
41. same-room cross-platform/device matrix.

## Phase 7 - hardening

42. resource/performance tests at 8 peers.
43. diagnostics UX.
44. packaging.
45. protocol compatibility tests.
46. documentation synchronization.
47. release hardening.

Do not implement WebRTC or custom LAN TCP in parallel unless a future explicit decision supersedes V2.

# 65. V2 acceptance criteria

The V2 library is acceptable when all of the following are true.

## Core/session

- supports all player counts 2-8;
- game engine remains outside Rust core;
- one authoritative host per healthy term;
- full logical game state commit after accepted action;
- total commit ordering through `commit_index`;
- committed membership separated from connectivity;
- stable never-reused join order;
- stable logical action identity across retries/migration;
- hidden-state policies can use per-peer representations;
- migration eligibility respects recovery capability;
- repeated host migration preserves the session;
- conflict branches are detected/fenced rather than silently merged.

## Same Room

- Nearby works without shared Wi-Fi/Internet where supported;
- `P2P_CLUSTER` is default;
- Wi-Fi Aware fallback is feature-detected and graceful;
- host migration works through the native transport family.

## Local LAN

- Iroh QUIC works with outside Internet disconnected;
- relay is disabled;
- mDNS finds endpoints/session probe finds host;
- Android/iOS can host/join;
- host migration/reconnect works without Internet.

## Internet

- host shares one TurnNet invite;
- client joins without answer QR/manual signaling;
- direct path works where available;
- relay fallback works when direct path cannot;
- relay configuration is injectable;
- no WebRTC/ICE/STUN/TURN in TurnNet;
- 8-player peer mesh validated;
- host migration requires no invite rescan;
- Iroh path changes do not alter game authority.

## Recovery/security/diagnostics

- recovery blob restores existing peer/session identity;
- endpoint route can rebind without creating new player;
- stale/malformed/duplicate messages cannot corrupt state;
- invite admission is protected by TurnNet capability, not just endpoint reachability;
- raw game payloads/secrets are not logged by default;
- diagnostics distinguish transport/path/protocol/state/authority problems.

# 66. Explicit V2 non-goals

- web/PWA/browser client;
- dedicated matchmaking service;
- account/login service;
- cloud authoritative game state;
- spectator mode unless a later game-facing API adds it;
- WebRTC DataChannel;
- ICE/STUN/TURN integration;
- custom LAN TCP transport;
- Iroh unstable custom transports;
- real-time action/FPS networking;
- delta-only/event-sourced baseline;
- rollback netcode;
- cryptographically fair randomness;
- serious malicious-host protection;
- automatic racing between native nearby and Iroh transport families;
- uninterrupted networking after indefinite mobile suspension/force-kill;
- voice/video.

# 67. V2 decision registry summary

| ID | Decision | Status |
|---|---|---|
| D2-001 | Three profiles: Same Room, Local LAN, Internet | Accepted |
| D2-002 | Same Room = Nearby primary + Wi-Fi Aware fallback | Accepted |
| D2-003 | Local LAN = Iroh QUIC + mDNS + relay disabled | Accepted |
| D2-004 | Internet = Iroh QUIC direct/hole punch + configurable relay | Accepted |
| D2-005 | Remove V1 WebRTC/ICE/STUN/TURN/manual answer signaling | Accepted |
| D2-006 | Rust TurnNet core embeds Iroh; app-facing SDK hides Iroh | Accepted |
| D2-007 | Iroh `EndpointId` is transport identity, not `peer_id` | Accepted |
| D2-008 | TurnNet major protocol selected by Iroh ALPN | Accepted |
| D2-009 | Iroh full peer mesh where feasible for 2-8 Internet peers | Accepted |
| D2-010 | Commit chain adds `commit_index` + `commit_hash` | Accepted |
| D2-011 | Separate logical-state, per-peer representation, and commit hashes | Accepted |
| D2-012 | Membership and connectivity are separate | Accepted |
| D2-013 | Join order assigned once by current authoritative admitting host | Accepted |
| D2-014 | Election requires `host_capable` and `migration_eligible` | Accepted |
| D2-015 | Incompatible authority branches are fenced, not auto-merged | Accepted |
| D2-016 | TurnNet invite wraps Iroh dialing data + TurnNet join capability | Accepted |
| D2-017 | Recovery may rebind new transport endpoint to existing peer identity | Accepted |
| D2-018 | Do not depend on Iroh unstable custom transports in V2 | Accepted |

# 68. Instructions for coding AIs

When generating/modifying this project:

- do not introduce game-specific rules into TurnNet core;
- do not add a game-engine dependency;
- do not assume exactly four players;
- do not reintroduce WebRTC, ICE, STUN, TURN, or custom LAN TCP;
- do not expose Iroh as the public game SDK contract;
- do not make Iroh EndpointId the canonical peer ID;
- do not use unstable Iroh custom transports for Same Room;
- do not make transient disconnect change committed membership;
- do not reuse join orders;
- do not retry the same logical action with a new action ID;
- do not accept old-term authority;
- do not accept a higher-term incompatible branch based only on term number;
- do not elect peers lacking host capability or migration eligibility;
- do not silently merge divergent split-brain commits;
- do not assume hidden-information peers receive identical state bytes or hashes;
- do not put reconnect credentials/private state into mDNS/Nearby advertisements;
- do not make relay infrastructure authoritative;
- keep relay configuration injectable;
- keep Local LAN operational without outside Internet;
- keep deterministic simulation as the correctness baseline;
- update all affected V2 Markdown docs when architecture changes.

# 69. Release invariant checklist

Before V2 release confirm:

- [ ] Player-count logic works for 2,3,4,5,6,7,8.
- [ ] Core has no game-engine dependency.
- [ ] Same session protocol runs above all three profiles.
- [ ] Iroh ALPN major protocol is enforced.
- [ ] Iroh EndpointId is not TurnNet peer ID.
- [ ] Local LAN works with relay and outside Internet disabled.
- [ ] Internet invite is one-way and has TurnNet admission capability.
- [ ] No WebRTC/ICE/STUN/TURN dependency remains.
- [ ] Internet direct + relay paths are tested.
- [ ] Iroh route changes do not mutate membership/authority.
- [ ] 8-peer Iroh full mesh is device-tested.
- [ ] Temporary disconnect does not increment membership epoch.
- [ ] Join order remains monotonic after host migration.
- [ ] Every accepted action advances state version exactly once.
- [ ] Every authoritative state/membership commit advances commit index exactly once.
- [ ] Every commit chains to previous commit hash.
- [ ] `action_id` survives retries/reconnect/migration.
- [ ] Election filters both host capability and migration eligibility.
- [ ] Hidden-state representations can differ per peer.
- [ ] Authority conflict fences commits instead of silently merging.
- [ ] Recovery can reconnect existing peer with a rebound transport route.
- [ ] Same-room migration works on supported native transports.
- [ ] Simulated fault suite passes before transport release tests.
- [ ] Diagnostics redact identities/secrets and distinguish failure layers.

# 70. External Iroh implementation references

V2 architecture was aligned to the current stable Iroh surface as of 2026-09-05.

- Iroh 1.1.0 core: https://docs.rs/iroh/1.1.0/iroh/
- Iroh Endpoint: https://docs.rs/iroh/1.1.0/iroh/endpoint/struct.Endpoint.html
- Iroh EndpointAddr: https://docs.rs/iroh/1.1.0/iroh/struct.EndpointAddr.html
- Iroh tickets concept: https://docs.iroh.computer/concepts/tickets
- iroh-tickets 1.0.0: https://docs.rs/iroh-tickets/1.0.0/iroh_tickets/
- Iroh mDNS address lookup: https://docs.rs/iroh-mdns-address-lookup/0.4.0/iroh_mdns_address_lookup/
- Iroh relay deployment guidance: https://docs.iroh.computer/iroh-services/relays/managed

These external references describe the connectivity substrate only. TurnNet's authoritative session semantics are defined by this specification and the V2 Decisions file.

