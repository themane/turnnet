---
document: turn_based_multiplayer_harness_v2_decisions
version: 2.0
status: implementation_baseline
date: 2026-09-05
working_name: TurnNet
working_name_is_placeholder: true
supersedes:
  - Turn_Based_Multiplayer_Harness_V1_Decisions.md
supported_players:
  min: 2
  max: 8
primary_platforms:
  - android
  - ios
web_support: out_of_scope_v2
documentation_format: markdown
---

# Turn-Based Multiplayer Networking Harness V2 - Canonical Decisions

This file is the canonical architectural decision record for V2. Decisions marked **Required** are normative unless a later Markdown decision record explicitly supersedes them.

V2 keeps the V1 session model—authoritative host, deterministic host migration, full logical state commits, stable action identity, committed membership metadata, simulated fault testing—and replaces the V1 IP transport stack with an Iroh-first design.

## 0. Documentation policy

| ID | Decision | Status |
|---|---|---|
| DOC-001 | All project documentation is authored and maintained as Markdown (`.md`). | **Required** |
| DOC-002 | Do not create DOCX/PDF/rich-document project documentation unless explicitly requested for a specific artifact. | **Required** |
| DOC-003 | Architecture or implementation decision changes update the affected Markdown documentation in the same change. | **Required** |
| DOC-004 | `Implementation_Spec`, `AI_Context`, and `Decisions` must remain mutually consistent. The Decisions file wins on architectural conflicts; the Implementation Spec wins on implementation detail unless it contradicts Decisions. | **Required** |

## 1. Product scope

| ID | Decision | Status |
|---|---|---|
| SCOPE-001 | Build an engine-agnostic networking/session harness for casual, low-bandwidth, turn-based multiplayer games. | **Required** |
| SCOPE-002 | Support 2-8 players inclusive. Never assume exactly four players. | **Required** |
| SCOPE-003 | Android and iOS are V2 target platforms. Browser/web support is out of scope. | **Required** |
| SCOPE-004 | Every participating app contains both host and client capability. Any eligible peer may become host. | **Required** |
| SCOPE-005 | No game-state backend, account service, matchmaking service, or authoritative cloud server is required. | **Required** |
| SCOPE-006 | Host migration remains a V2 requirement. | **Required** |
| SCOPE-007 | Anti-cheat hardening and cryptographically fair randomness remain non-goals. Protocol integrity and accidental corruption protection remain required. | **Required** |

## 2. Three V2 play/hosting profiles

| ID | Profile | Decision | Status |
|---|---|---|---|
| MODE-001 | `SAME_ROOM` | Use Google Nearby Connections as the primary same-room transport and Wi-Fi Aware/NAN as fallback where supported. No shared Wi-Fi or Internet is required. | **Required** |
| MODE-002 | `LOCAL_LAN` | For devices on the same Wi-Fi/LAN, use Iroh QUIC with local address discovery and relay disabled. The mode must work with no outside Internet connection. | **Required** |
| MODE-003 | `INTERNET` | Use Iroh QUIC for Internet play, preferring direct connectivity/hole punching and allowing configurable relay fallback. | **Required** |
| MODE-004 | All | The same TurnNet session protocol, state machine, identity model, game-state rules, and host-migration semantics run above all three profiles. | **Required** |
| MODE-005 | All | V2 does not require automatic racing between native same-room transports and Iroh. Explicit profile selection is acceptable. Iroh may internally change IP paths without changing TurnNet profile. | **Required** |

## 3. Core architecture

| ID | Decision | Status |
|---|---|---|
| ARCH-001 | Portable session/protocol core is Rust. | **Required** |
| ARCH-002 | Android public wrapper and native same-room adapters use Kotlin where platform APIs require it. | **Required** |
| ARCH-003 | iOS public wrapper and native same-room adapters use Swift where platform APIs require it. | **Required** |
| ARCH-004 | Iroh is embedded inside the TurnNet Rust networking layer; consuming games do not depend directly on Iroh or `iroh-ffi`. | **Required** |
| ARCH-005 | One peer is the authoritative host for committed session/game state at any healthy point in time. | **Required** |
| ARCH-006 | Game actions and game state are opaque byte payloads to TurnNet. | **Required** |
| ARCH-007 | Discovery and peer-link connectivity remain separate abstractions even though some Iroh address-lookup facilities participate in both. | **Required** |
| ARCH-008 | All real transports and simulated transports feed a common deterministic session state machine; transport callbacks never mutate game state directly. | **Required** |
| ARCH-009 | TurnNet owns multiplayer/session semantics. Iroh owns IP connectivity mechanics. Do not move host authority, game-state replication, membership, or action idempotency into Iroh-specific code. | **Required** |

## 4. Iroh baseline and dependency policy

| ID | Decision | Status |
|---|---|---|
| IROH-001 | V2 architecture targets the stable Iroh 1.1.x API family; the implementation lockfile pins exact versions used by a release. | **Required** |
| IROH-002 | Use one Iroh `Endpoint` per TurnNet networking runtime/process where practical and multiplex TurnNet peer connections through it. | **Required** |
| IROH-003 | Use a TurnNet-owned ALPN for protocol selection. V2 major ALPN is conceptually `turnnet/2`. | **Required** |
| IROH-004 | Use Iroh `EndpointId`/`EndpointAddr` as transport identity/addressing, not as canonical TurnNet `peer_id`. | **Required** |
| IROH-005 | Use QUIC reliable streams for TurnNet control and state traffic. QUIC datagrams are not required for V2 gameplay semantics. | **Required** |
| IROH-006 | Use Iroh direct connection establishment, path migration, and relay fallback in Internet mode rather than implementing WebRTC/ICE/STUN/TURN. | **Required** |
| IROH-007 | Use Iroh mDNS address lookup for local-LAN endpoint discovery/address resolution with no relay/outside Internet requirement. | **Required** |
| IROH-008 | Do not depend on Iroh `unstable-custom-transports` in V2. Google Nearby and Wi-Fi Aware remain native `PeerLink` adapters until the Iroh custom-transport API is stable enough for an explicit future decision. | **Required** |
| IROH-009 | Iroh path changes (direct ↔ relay, Wi-Fi ↔ cellular where the connection survives) are transport events, not TurnNet host changes or game-state changes. | **Required** |
| IROH-010 | Android wrappers notify Iroh of relevant OS network changes and initialize required Android/JNI networking context before constructing the endpoint. | **Required** |
| IROH-011 | Internet invite creation waits for/validates usable remote dialing information from the Iroh endpoint; Local LAN must not wait for Internet/relay readiness. | **Required** |

## 5. Same-room transport decisions

| ID | Decision | Status |
|---|---|---|
| ROOM-001 | Google Nearby Connections is the first same-room transport. | **Required** |
| ROOM-002 | Nearby uses `P2P_CLUSTER` by default for small 2-8 player sessions. `P2P_STAR` is fallback experimentation only if measured device/performance issues justify it. | **Required** |
| ROOM-003 | Wi-Fi Aware/NAN is attempted when Nearby is unavailable, unsupported, permission-blocked, or fails. | **Required** |
| ROOM-004 | Wi-Fi Aware must be treated as direct nearby P2P, not as same-router LAN networking. | **Required** |
| ROOM-005 | Feature-detect OS/hardware/security capabilities at runtime and fail gracefully. | **Required** |
| ROOM-006 | Android-to-Android same-room reliability has higher V2 release priority than cross-platform interoperability, while Android↔iOS should be supported where current platform SDK/device capabilities permit it. | **Required** |

## 6. Local-LAN Iroh decisions

| ID | Decision | Status |
|---|---|---|
| LAN-001 | Same-Wi-Fi/LAN gameplay uses Iroh QUIC, replacing the V1 custom TCP data transport. | **Required** |
| LAN-002 | `LOCAL_LAN` disables relay use and must not require outside Internet access. | **Required** |
| LAN-003 | Iroh mDNS address lookup is the default endpoint discovery/address-resolution mechanism on the local network. | **Required** |
| LAN-004 | TurnNet performs a small application-level session probe/advertisement over its ALPN so local endpoint discovery is not confused with session membership. | **Required** |
| LAN-005 | LAN isolation/captive/router policy failures are surfaced as connectivity diagnostics, not protocol failures. | **Required** |
| LAN-006 | Manual invite/deep-link/QR join may additionally be supported on LAN, but automatic local discovery is the normal UX. | **Required** |

## 7. Internet Iroh decisions

| ID | Decision | Status |
|---|---|---|
| NET-001 | Internet gameplay uses Iroh QUIC. V1 WebRTC DataChannel, ICE, STUN, TURN, and manual offer/answer signaling are removed. | **Required** |
| NET-002 | Prefer direct connectivity. Permit encrypted relay forwarding when direct connectivity cannot be maintained. | **Required** |
| NET-003 | Relay configuration is injectable. TurnNet must not hard-code one relay vendor. | **Required** |
| NET-004 | Relay infrastructure is connectivity infrastructure only. It is not authoritative, does not store game state, and is not a matchmaking/account backend. | **Required** |
| NET-005 | Production deployments should be able to use dedicated/custom relay maps. Development may use Iroh defaults. | **Required** |
| NET-006 | For 2-8 Internet peers, maintain an Iroh control connection between every peer pair when feasible. Maximum full mesh is 28 pairwise connections at 8 players. | **Required** |
| NET-007 | Reuse one QUIC connection per peer pair and multiplex TurnNet streams rather than creating a separate transport connection for every message category. | **Required** |
| NET-008 | For production relay high availability, multiple geographically separated configured relays are recommended operationally; this is not a TurnNet wire-protocol requirement. | **Required** |

## 8. Invites and discovery

| ID | Decision | Status |
|---|---|---|
| INV-001 | `TurnNetInvite` is the public invite abstraction. Raw Iroh tickets are never the complete TurnNet authorization mechanism. | **Required** |
| INV-002 | Internet `TurnNetInvite` wraps Iroh endpoint dialing information plus TurnNet session metadata and an unpredictable join capability. | **Required** |
| INV-003 | QR, deep link, shared text, file, and imported image are presentation methods for the same invite object. | **Required** |
| INV-004 | Initial Internet join is one-way from a user perspective: host shares an invite; client dials host. There is no answer QR or manual offer/answer exchange. | **Required** |
| INV-005 | Because Iroh tickets/addresses can be reusable and contain network addressing information, TurnNet join capabilities may expire or be revoked by the host; applications must treat invites as sensitive bearer material. | **Required** |
| INV-006 | Same-room and LAN discovery advertisements must not contain reconnect secrets, private game state, or other long-lived credentials. | **Required** |

## 9. Identity, routing, and membership

| ID | Decision | Status |
|---|---|---|
| ID-001 | `session_id`: random 128-bit identifier, stable for the session across migration. | **Required** |
| ID-002 | `peer_id`: random 128-bit TurnNet session identity, stable across reconnect and independent of transport identity. | **Required** |
| ID-003 | `join_order`: assigned exactly once by the authoritative host that admits the peer, using a session-monotonic `next_join_order`; values are never reused. | **Required** |
| ID-004 | `term`: monotonically increasing host-generation number. | **Required** |
| ID-005 | `state_version`: monotonically increasing game-state version; advances once per accepted game-state commit. | **Required** |
| ID-006 | `membership_epoch`: monotonically increasing committed roster generation; advances only for committed membership changes. | **Required** |
| ID-007 | `commit_index`: monotonically increasing authoritative session-commit sequence covering game-state and membership commits, giving V2 a single total order for committed session history. | **Required** |
| ID-008 | `message_id`: unique protocol-message identifier for message-level idempotency. | **Required** |
| ID-009 | `action_id`: unique logical game-action identifier stable across retries, reconnect, and host migration. | **Required** |
| ID-010 | Connectivity/presence state is separate from committed membership. Temporary disconnect does not by itself advance `membership_epoch`. | **Required** |
| ID-011 | Current Iroh `EndpointId`/addressing or native transport route is mutable routing metadata bound to a `peer_id` by authenticated session protocol. Route changes do not by themselves change membership. | **Required** |
| ID-012 | Device name, IP address, Bluetooth address, phone number, account name, and Iroh `EndpointId` are not canonical TurnNet peer identity. | **Required** |
| ID-013 | `next_join_order` is committed/recoverable session metadata so later hosts can admit peers without reusing join-order values. | **Required** |

## 10. Authoritative state and commit chain

| ID | Decision | Status |
|---|---|---|
| STATE-001 | The host is the only peer allowed to commit authoritative game/session state in a healthy term. | **Required** |
| STATE-002 | Every accepted game action produces a full logical authoritative game-state commit in V2; delta-only/event-sourcing replication is not the baseline. | **Required** |
| STATE-003 | Every authoritative commit references the previous commit hash and advances `commit_index`, creating a verifiable branch chain for convergence and conflict detection. | **Required** |
| STATE-004 | `logical_state_hash` identifies the canonical logical game state for a commit. Trusted-full-replication peers can independently verify it. Hidden-information peers may only receive it as committed metadata. | **Required** |
| STATE-005 | `representation_hash` hashes the concrete public/private/recovery representation delivered to a particular peer and is distinct from `logical_state_hash`. | **Required** |
| STATE-006 | `commit_hash` binds previous commit hash, commit index, term, state version, membership epoch, roster digest, logical-state hash, host identity, `next_join_order`, migration-eligibility digest/policy metadata, and committed action/membership metadata. | **Required** |
| STATE-007 | All surviving peers retain enough latest commit metadata, state/recovery material, and action-deduplication information to determine migration eligibility under the configured policy. | **Required** |
| STATE-008 | State hashes are for integrity/convergence/branch detection; they do not provide secrecy, authentication, or malicious-host protection by themselves. | **Required** |

## 11. Action idempotency

| ID | Decision | Status |
|---|---|---|
| ACT-001 | Retrying one logical action must reuse the same `action_id`; `message_id` may change. | **Required** |
| ACT-002 | A committed `action_id` must never commit twice, including after reconnect or host migration. | **Required** |
| ACT-003 | Migration-capable recovery material retains the committed action-ID set/history required to preserve idempotency. Default V2 may retain all committed action IDs for the active session because the target is small turn-based games. | **Required** |
| ACT-004 | Any future action-ID compaction scheme must prove that replay of a pre-compaction action cannot create a second logical commit. | **Required** |

## 12. Hidden-information and migration eligibility

| ID | Decision | Status |
|---|---|---|
| PRIV-001 | The core supports a pluggable state-visibility/recovery policy. | **Required** |
| PRIV-002 | Trusted full replication remains the default/simple policy. | **Required** |
| PRIV-003 | Public + per-peer private state and authenticated-encrypted recovery envelopes must be representable. | **Required** |
| PRIV-004 | `migration_eligible` is distinct from `host_capable`: a peer may be technically able to host but unable to reconstruct the authoritative hidden state. | **Required** |
| PRIV-005 | Host election considers only peers that are both `host_capable` and `migration_eligible` for the latest commit. | **Required** |
| PRIV-006 | The committed state metadata exposes enough eligibility information for all peers to make the same deterministic election decision without revealing unauthorized private state. | **Required** |
| PRIV-007 | Commitment/threshold/secret-sharing schemes remain optional advanced strategies; V2 architecture must not preclude them. | **Required** |

## 13. Host migration and fencing

| ID | Decision | Status |
|---|---|---|
| HM-001 | Host migration is mandatory for all 2-8 player session sizes. | **Required** |
| HM-002 | Election eligibility requires committed membership, latest verifiable commit, current membership epoch, `host_capable = true`, and `migration_eligible = true`. | **Required** |
| HM-003 | Among equally eligible reachable peers, lowest `join_order` wins deterministically. | **Required** |
| HM-004 | Successful migration advances `term`. | **Required** |
| HM-005 | A new host claim references the exact base `commit_index` and `commit_hash` from which the new term resumes. | **Required** |
| HM-006 | Default casual policy may activate a new host when at least two session members including the candidate are mutually reachable/acknowledging; majority quorum is not required. | **Required** |
| HM-007 | Because the casual policy cannot prevent every network-partition split brain, V2 requires conflict detection/fencing. If incompatible authority branches are observed, stop accepting gameplay commits and surface `AuthorityConflict`/`SessionInterrupted`; do not silently merge divergent game state. | **Required** |
| HM-008 | A higher term is not sufficient to overwrite a locally known incompatible committed branch. The proposed base commit must be reconcilable with the local commit chain. See HM-012 for the unacknowledged orphan-tail exception. | **Required** |
| HM-009 | A returning former host with an old term rejoins as a non-host after synchronizing to current authority. | **Required** |
| HM-010 | Graceful host leave uses explicit authority transfer when another eligible peer exists. | **Required** |

## 14. Transport-specific migration

| ID | Decision | Status |
|---|---|---|
| MIG-001 | In Iroh modes, surviving peer-to-peer QUIC control connections are reused for election and rerouting; no QR rescan or WebRTC renegotiation is required. | **Required** |
| MIG-002 | In `INTERNET`, peers distribute current Iroh route information to the roster so surviving peers can dial one another if an existing QUIC connection is absent. | **Required** |
| MIG-003 | In `LOCAL_LAN`, mDNS continues to provide local endpoint addressing; the elected host updates TurnNet session-advertisement role metadata if the session remains joinable. | **Required** |
| MIG-004 | In `SAME_ROOM`, the elected host advertises the existing `session_id` under the new term and peers reconnect as required by Nearby/Wi-Fi Aware. | **Required** |

## 15. Protocol

| ID | Decision | Status |
|---|---|---|
| PROTO-001 | Use a versioned binary TurnNet control protocol; Protocol Buffers remains the V2 recommendation. | **Required** |
| PROTO-002 | TurnNet major version is also represented by Iroh ALPN for Iroh transports. | **Required** |
| PROTO-003 | Unknown optional fields are ignored where compatible; incompatible major versions reject cleanly. | **Required** |
| PROTO-004 | Every inbound message is size-checked before allocation/decoding and validated for session, sender binding, term, membership, and state/commit context as applicable. | **Required** |
| PROTO-005 | Iroh transport authentication of `EndpointId` does not replace TurnNet join/reconnect authorization. | **Required** |
| PROTO-006 | V1 `SIGNALING_RELAY` is removed. Iroh owns Internet dialing/path establishment. | **Required** |

## 16. Recovery

| ID | Decision | Status |
|---|---|---|
| REC-001 | Public SDK exposes `export_recovery_blob()` and `restore_session(...)` or equivalent. | **Required** |
| REC-002 | Recovery blob is opaque, versioned, and may contain session/peer IDs, join order, current known term/commit metadata, reconnect capability, last-known peer routes, dedupe material, and visibility-policy recovery material. | **Required** |
| REC-003 | The application chooses persistence and must use secure platform storage when the blob contains secrets. | **Required** |
| REC-004 | Canonical TurnNet identity is recoverable even if a process restart creates a new Iroh `EndpointId`; authenticated reconnect may rebind transport routing to the existing `peer_id`. | **Required** |
| REC-005 | Persisting an Iroh endpoint private key is optional, not required by TurnNet V2. | **Required** |

## 17. Reliability and lifecycle

| ID | Decision | Status |
|---|---|---|
| REL-001 | All timers are configurable; V1 defaults remain the initial V2 baseline unless measurements justify changes. | **Required** |
| REL-002 | Temporary connection loss pauses/reroutes/resynchronizes rather than immediately ending a turn-based session. | **Required** |
| REL-003 | Mobile backgrounding/OS suspension can interrupt connectivity; surface `SessionInterrupted` and never promise indefinite background networking. | **Required** |
| REL-004 | Iroh path migration that preserves a QUIC connection is not treated as session reconnect. It may emit diagnostics such as direct/relay/path changes. | **Required** |

## 18. Framing and payload policy

| ID | Decision | Status |
|---|---|---|
| FRAME-001 | Game action/state serialization and logical payload size remain application-defined. | **Required** |
| FRAME-002 | TurnNet enforces safe hard limits for control messages and exposes configurable limits/backpressure for game payloads. | **Required** |
| FRAME-003 | Iroh QUIC handles transport segmentation; TurnNet still length-frames protocol envelopes on streams and validates declared sizes before allocation. | **Required** |
| FRAME-004 | Native Nearby/Wi-Fi Aware adapters obey their platform hard limits and may use TurnNet fragmentation/reassembly below the game adapter where required. | **Required** |

## 19. Simulated transport and tests

| ID | Decision | Status |
|---|---|---|
| TEST-001 | Deterministic `SimulatedPeerLink`/discovery infrastructure remains a V2 prerequisite before production transport work. | **Required** |
| TEST-002 | Simulation covers latency, jitter, loss, duplication, reordering, disconnect, reconnect, host death, delayed old-term traffic, process recovery, and network partitions. | **Required** |
| TEST-003 | V2 adds explicit split-brain/conflicting-branch tests and hidden-state migration-eligibility tests. | **Required** |
| TEST-004 | Iroh test matrix covers no-Internet LAN, direct Internet path, relay-only fallback, direct↔relay path changes, network changes, endpoint route refresh, process restart/rebind, and 8-player full mesh. | **Required** |
| TEST-005 | Same-room test matrix covers Nearby and Wi-Fi Aware join/reconnect/migration and graceful unsupported-device behavior. | **Required** |

## 20. Explicit V2 non-goals

- Browser/web client.
- Dedicated matchmaking backend.
- Account/login service.
- Cloud-authoritative game state.
- WebRTC, ICE, STUN, or TURN implementation inside TurnNet.
- Event sourcing/delta-only replication as the default state model.
- Rollback netcode.
- Real-time action/FPS latency semantics.
- Serious malicious-host cheat prevention.
- Cryptographically fair randomness protocol.
- Automatic cross-family racing between Nearby/Wi-Fi Aware and Iroh.
- Dependence on Iroh unstable custom transports.
- Guaranteed uninterrupted networking while mobile apps are suspended/force-killed.
- Voice/video chat.

## 21. V1 decisions explicitly superseded

| V1 decision | V2 replacement |
|---|---|
| LAN mDNS/Bonjour + custom TCP | Iroh QUIC + Iroh mDNS address lookup in `LOCAL_LAN`. |
| WebRTC DataChannel Internet transport | Iroh QUIC in `INTERNET`. |
| ICE/STUN/TURN policy | Iroh direct connectivity/hole punching + configurable relay fallback. |
| Manual non-trickle offer/answer | One-way `TurnNetInvite` bootstrap wrapping Iroh dialing information and a TurnNet join capability. |
| WebRTC `SIGNALING_RELAY` peer-mesh construction | Direct Iroh peer dialing using distributed route information; `SIGNALING_RELAY` removed. |
| State/hash language that implied one hash for all peer representations | Separate `logical_state_hash`, per-peer `representation_hash`, and chained `commit_hash`. |
| Election based on latest state + membership + host capability | Adds `migration_eligible`, `commit_index`, `commit_hash`, and branch compatibility. |
| `join_order` assigned by initial host | Assigned once by whichever authoritative host admits the peer, from committed monotonic `next_join_order`. |

## 22. AI implementation guardrails

A coding agent working on V2 must obey these rules unless a newer decision record changes them:

1. Do not add game-specific rules to the networking core.
2. Do not require a particular game engine.
3. Do not reintroduce WebRTC, STUN, TURN, or custom LAN TCP as parallel V2 transports without an explicit architecture change.
4. Do not expose raw Iroh APIs as the public TurnNet SDK contract.
5. Do not use Iroh `EndpointId` as TurnNet `peer_id`.
6. Do not depend on Iroh unstable custom transports for Same Room mode.
7. Do not treat Iroh path migration as TurnNet host migration.
8. Do not let temporary disconnect mutate committed roster membership.
9. Do not reuse `join_order` values.
10. Do not retry a logical game action with a fresh `action_id`.
11. Do not accept stale-term authority.
12. Do not accept a higher-term authority claim if its base commit conflicts with the known commit chain.
13. Do not elect a peer unless `host_capable && migration_eligible` and it has/reconciles to the latest committed head.
14. Do not silently merge divergent split-brain game state.
15. Do not assume identical peer payload hashes in hidden-information games.
16. Do not put reconnect secrets or private state into discovery advertisements.
17. Do not make relay infrastructure authoritative or store game state there.
18. Keep relay configuration vendor-neutral and injectable.
19. Keep `LOCAL_LAN` functional with relay and outside Internet disabled.
20. Keep Same Room functional without a shared Wi-Fi network where the selected native technology supports it.
21. Preserve the deterministic simulated fault suite as the primary correctness gate for session semantics.
22. Update all affected Markdown V2 documents when architecture changes.

## 23. External Iroh baseline references

These references document the external networking substrate assumed by this V2 decision set. They are implementation references, not substitutes for TurnNet's own protocol specification.

- Iroh crate / connection model: https://docs.rs/iroh/1.1.0/iroh/
- Iroh `Endpoint`: https://docs.rs/iroh/1.1.0/iroh/endpoint/struct.Endpoint.html
- Iroh `EndpointAddr`: https://docs.rs/iroh/1.1.0/iroh/struct.EndpointAddr.html
- Iroh tickets: https://docs.iroh.computer/concepts/tickets
- `iroh-tickets`: https://docs.rs/iroh-tickets/1.0.0/iroh_tickets/
- Iroh mDNS address lookup: https://docs.rs/iroh-mdns-address-lookup/0.4.0/iroh_mdns_address_lookup/
- Iroh relay deployment guidance: https://docs.iroh.computer/iroh-services/relays/managed

## 24. Phase 1 implementation clarifications (2026-10-02)

These decisions resolve points the V2 baseline left open. They were agreed before Phase 1 implementation; rationale and detail are in `Phase1_Implementation_Plan.md` §2 (plan IDs in brackets). Wire and hash encodings are normative in `protocol.md`.

| ID | Decision | Status |
|---|---|---|
| PROTO-007 | All TurnNet hashes (`logical_state_hash`, `representation_hash`, `roster_hash`, `migration_eligibility_digest`, `reconnect_verifier`, `commit_hash`) use BLAKE3 `derive_key` with per-purpose context strings and the canonical encodings in `protocol.md`. [C-2] | **Required** |
| PROTO-008 | The envelope's message type is the protobuf `oneof body`; an unset or unknown body is an unknown message type. [C-19] | **Required** |
| ARCH-010 | The Rust session core is a sans-IO deterministic state machine with injected clock and randomness; async runtimes live only in runtime/transport crates. [C-3] | **Required** |
| JOIN-001 | Admission is gated by an app-controlled `joinable` flag and `roster_size < max_players`; the core has no lobby/game-started phase. [C-5] | **Required** |
| REL-005 | When a member stays unreachable past `reconnect_grace_period`, the core emits an event only; removal requires an explicit app-initiated `MEMBER_REMOVE` commit. [C-6] | **Required** |
| HM-011 | If no other member is reachable for migration, the peer enters `INTERRUPTED`, keeps accepting reconnects, and ends the session (`NO_SURVIVORS`) if nobody returns within `reconnect_grace_period`. [C-7] | **Required** |
| HM-012 | A returning former host may discard commits that it authored and that no other peer acknowledged, rolling back to the new term's base, and must emit a diagnostic. Commits acknowledged or held by any other peer are never discarded; divergence there is fenced per HM-007/HM-008. [C-13] | **Required** |
| ACT-005 | Mismatched `expected_state_version` in `ACTION_SUBMIT` is handled by a configurable policy whose default is reject (`STALE_CONTEXT`). [C-8] | **Required** |
| ACT-006 | The committed `action_id` set is derived from the retained commit chain (each `GAME_ACTION` commit binds its `committed_action_id`). [C-14] | **Required** |
| STATE-009 | Every peer retains all commit metadata for the session and the full state representation for the head commit only. [C-10] | **Required** |
| PRIV-008 | Commit metadata carries the sorted migration-eligible peer list; `migration_eligibility_digest` is its hash. [C-15] | **Required** |
| REC-006 | The admitting host issues each peer a random 256-bit reconnect secret; the committed roster stores only its `reconnect_verifier`, so any later host can authenticate reconnects. [C-11] | **Required** |
| INV-007 | Join capabilities are host-local and are not replicated; invites issued by a previous host are invalid after migration. [C-16] | **Required** |
