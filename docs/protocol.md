---
document: turnnet_v2_protocol
version: 2.0-draft
status: normative_draft
date: 2026-10-02
source_of_truth:
  - core/proto/turnnet/v2/turnnet.proto
  - core/src/protocol/
  - core/src/hash/
---

# TurnNet V2 Wire Protocol and Canonical Encodings

This document is normative for the TurnNet V2 wire format and hash encodings. The protobuf schema is `core/proto/turnnet/v2/turnnet.proto`.

**Freeze rule:** the encodings in §4 must be frozen before the first cross-platform interop release. After the freeze, any change needs an explicit protocol minor/major decision recorded in the V2 Decisions file. The golden vectors in `core/src/hash/tests.rs` fail if any encoding changes.

## 1. Versioning

| Item | Value |
|---|---|
| Protocol major | `2` (Iroh ALPN `turnnet/2` in Phase 2) |
| Protocol minor | `0` |
| Package | `turnnet.v2` |

- A receiver rejects any envelope whose `protocol_major` differs from its own. It sends `Error{ERROR_CODE_PROTOCOL_MISMATCH}` and closes the link.
- Higher minor versions are accepted.
- Unknown fields are ignored under protobuf rules. An envelope whose `body` is unset, or is a `oneof` member this version doesn't know, is rejected as an unknown message type.

## 2. Framing

```text
frame = length:u32be || envelope_bytes[length]
```

- `length == 0` is invalid.
- `length` is checked against `max_frame_bytes` **before** the body is allocated.
- The default `max_frame_bytes` is 1 MiB + 64 KiB. The library hard cap is 16 MiB + 64 KiB. An application can lower these limits but cannot raise them.
- Messages that do not carry state are also limited by `max_control_message_bytes`: 64 KiB by default and as the hard cap.
  - In Phase 1, state travels inline, so only `JoinAccept`, `ReconnectAccept`, `StateCommit` and `StateSyncResponse` may exceed it.
  - Phase 2 moves large state to dedicated QUIC streams (`StateTransferBegin`).
- Any framing error poisons the stream decoder, and the link must be closed. Stream framing cannot be resynchronized.
- Iroh QUIC handles transport segmentation, so TurnNet does no packet fragmentation over QUIC (FRAME-003). Native nearby adapters may fragment below this layer (FRAME-004).

## 3. Envelope

| Field | Rule |
|---|---|
| `protocol_major`, `protocol_minor` | See §1 |
| `session_id` | 16 bytes, or empty before a session is known |
| `term` | Sender's current term |
| `message_id` | 16 bytes, always present, fresh per message (retries get a new one) |
| `sender_peer_id` | 16 bytes, or empty before admission |
| `commit_index`, `state_version`, `membership_epoch` | Optional context |
| `action_id` | Optional; if present, exactly 16 bytes; stable across retries |
| `body` | `oneof`; exactly one message |

### 3.1 Validation order (Implementation Spec §39.2)

| Step | Check | Layer |
|---|---|---|
| 1 | Frame length vs limits | `FrameDecoder` |
| 2 | Protobuf decode | `decode_envelope` |
| 3 | Body present and known | `decode_envelope` |
| 4 | Protocol major | `decode_envelope` |
| — | Identifier field lengths; control-message size | `decode_envelope` |
| 5 | Session id | session core (M3) |
| 6 | Sender transport binding and authentication | session core (M3) |
| 7 | Term semantics | session core |
| 8 | Membership/commit context | session core |
| 9 | Message-specific payload limits and enum validity | session core |
| 10 | `message_id` dedupe | session core |

Enum fields decode as `i32`. `0` (`*_UNSPECIFIED`) and unknown values are rejected wherever a value is required.

## 4. Canonical hash encodings

Every hash is BLAKE3 in `derive_key` mode, which gives domain separation by context string. All digests are 32 bytes.

### 4.1 Primitive encoding

| Type | Encoding |
|---|---|
| `u8`, `u32`, `u64` | Fixed-width big-endian |
| Identifier (`SessionId`, `PeerId`, `ActionId`) | 16 raw bytes |
| `Hash32` | 32 raw bytes |
| Variable bytes | `len:u64be ‖ bytes` |
| `Option<T>` | `0x00`, or `0x01 ‖ T` |
| List | `count:u32be ‖ items` in canonical order |

Enum codes are the protobuf numbers. For `CommitKind`: `InitialState=1, GameAction=2, MemberJoin=3, MemberLeave=4, MemberRemove=5, PolicyMetadataChange=6`. For `MemberStatus`: `Active=1, Left=2, Removed=3`. For `RepresentationKind`: `Full=1, PublicPrivate=2`.

### 4.2 Hashes

| Hash | Context | Input |
|---|---|---|
| `logical_state_hash` | `turnnet v2 logical_state` | `var(state)` |
| `representation_hash` | `turnnet v2 representation` | `kind:u8 ‖ var(public) ‖ opt(var(private)) ‖ opt(var(recovery))` |
| `roster_hash` | `turnnet v2 roster` | `count:u32`, then for each member in ascending `join_order`: `peer_id ‖ join_order:u32 ‖ status:u8 ‖ admission_commit_index:u64 ‖ reconnect_verifier:32 ‖ var(display_metadata)` |
| `migration_eligibility_digest` | `turnnet v2 eligibility` | `policy_id:u32 ‖ count:u32 ‖ peer_ids` (sorted ascending, de-duplicated) |
| `reconnect_verifier` | `turnnet v2 reconnect` | `session_id ‖ peer_id ‖ secret:32` |
| `commit_hash` | `turnnet v2 commit` | see §4.3 |

The roster includes tombstones (`Left`/`Removed` members), so `join_order` values are never reused.

### 4.3 `commit_hash`

```text
session_id:16
term:u64
commit_index:u64
previous_commit_hash:32        (all zero for commit_index 1)
commit_kind:u8
state_version:u64
membership_epoch:u64
host_peer_id:16
roster_hash:32
logical_state_hash:32
next_join_order:u32
migration_eligibility_digest:32
opt(policy_metadata_digest:32)
opt(committed_action_id:16)    (GameAction commits)
opt(affected_peer_id:16)       (Member* commits)
```

### 4.4 What the hashes are not

These hashes provide integrity, convergence and branch detection. They do not provide secrecy, authentication or protection against a malicious host (STATE-008).

- `representation_hash` may differ between peers for the same commit.
- `reconnect_verifier` is the only secret-derived value. It is compared in constant time.
