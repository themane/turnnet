---
document: turnnet_session_api_reference
version: 0.1
status: draft
date: 2026-10-07
audience:
  - turnnet-core implementers
  - driver / transport authors
  - game developers (see Game_Integration_Guide.md for the tutorial view)
source_of_truth: Phase1_Implementation_Plan.md §4.4–4.6
---

# TurnNet Session API (reference)

Reference for the sans-IO `SessionCore`. The tutorial view is the [Game Integration Guide](Game_Integration_Guide.md) §5. This file is the precise contract: types, ordering rules, error behaviour.

> **Status: draft, nothing here is implemented yet.** Only M1 (ids, hashes, protobuf, framing) exists in `turnnet-core`. Each item is tagged with the milestone that delivers it (see [Phase 1 plan §10](Phase1_Implementation_Plan.md)). When a milestone lands, remove its *planned* tag and check the signature against the code.

| Area | Milestone |
|---|---|
| `SessionCore::create/new_idle`, `Input`/`Output`, discovery, join, heartbeats | M3 |
| `SubmitAction`, commits, ACKs, `Leave`/`RemovePeer`/`SetJoinable`/invites | M4 |
| `StateReplicationPolicy`, eligibility | M5 |
| Election, `HostChanged`, conflict fencing, `End` | M6 |
| `restore`, `export_recovery_blob`, lifecycle inputs | M7 |

---

## 1. Model

`SessionCore` is a deterministic state machine.

```
Input  ──►  SessionCore::handle(now, input, &mut game)  ──►  Vec<Output>
```

- No sockets, threads, clocks or global RNG inside the core. The driver supplies `now` and an `Env` (RNG, policy, config).
- Same `Env` seed plus same input sequence gives byte-identical outputs. This is what the simulator relies on.
- `GameAdapter` is called **only from inside `handle`** (ARCH-008). Never call `handle` re-entrantly from an adapter callback.
- One `SessionCore` is single-threaded. The driver serializes all calls.

## 2. Construction *(planned, M3 / M7)*

```rust
impl SessionCore {
    pub fn create(opts: CreateSessionOptions, env: Env) -> (Self, Vec<Output>);
    pub fn new_idle(env: Env) -> Self;
    pub fn restore(blob: &[u8], env: Env) -> Result<(Self, Vec<Output>), RecoveryError>;
}
```

| Constructor | Use | Resulting state |
|---|---|---|
| `create` | You host. Commits `InitialState` (`commit_index=1`, `state_version=0`, `term=1`, `membership_epoch=1`). Returns `Advertise`. | `Active`, you are host, `join_order=0` |
| `new_idle` | You will discover or join. No session yet. | `Idle` |
| `restore` | Relaunch after process death. Keeps `peer_id`, `join_order`, reconnect secret. Returns `Connect` outputs. | `Reconnecting` |

`CreateSessionOptions`: `play_profile`, `max_players` (2–8), `initial_game_state`, `join_policy`, `app_id`, `display_metadata` (≤ 256 B), `state_replication_policy`.

`Env { rng, policy, config }`. `rng` is any `RngCore + Send`; tests pass a seeded `ChaCha8Rng`. `policy` defaults to `TrustedFullReplication`.

## 3. Input

```rust
pub enum Input {
    LinkUp   { link: LinkId, observed_identity: TransportIdentity, inbound: bool },
    LinkDown { link: LinkId, reason: LinkDownReason },
    Frame    { link: LinkId, bytes: Bytes },
    Discovery(DiscoveryEvent),
    Lifecycle(LifecycleEvent),   // Foreground | Background | NetworkUnavailable | ...
    Tick,
    Command(Command),
}
```

| Input | Driver responsibility |
|---|---|
| `LinkUp` | Reply to an `Output::Connect`, or report an inbound connection. `LinkId` is driver-allocated and unique for the process lifetime. |
| `LinkDown` | A link closed or failed. This changes connectivity only; it never changes membership. |
| `Frame` | One complete framed message (one length-prefixed frame; see [protocol.md](protocol.md)). The driver does the byte-stream framing. Malformed frames are dropped and reported as `Diagnostic`, they do not fail the call. |
| `Tick` | Call when `now >= next_deadline()`. Extra ticks are harmless. |
| `Lifecycle` | Map OS events. `Background` may flip `host_capable` and starts `SessionInterrupted` handling. |

`handle` never returns `Result`. Bad input produces a `Diagnostic` or an event. The only fallible API is `restore`.

### 3.1 Commands

```rust
pub enum Command {
    StartDiscovery(DiscoveryFilter), StopDiscovery,
    JoinDiscovered(DiscoveredSession), JoinInvite(TurnNetInvite),
    SubmitAction { payload: Bytes, action_id: Option<ActionId> },
    SetJoinable(bool), CreateInvite(InviteOptions), RevokeInvite(InviteId),
    RemovePeer(PeerId), Leave, End(EndReason),
}
```

| Command | Who | Notes |
|---|---|---|
| `StartDiscovery` / `StopDiscovery` | idle or any | Results arrive as `SessionDiscovered`. |
| `JoinDiscovered` / `JoinInvite` | idle | Subject to `join_timeout` and `max_join_retries`. Result: `SessionJoined` or `JoinRejected{reason}` (capacity, `joinable=false`, bad capability, `app_id`/protocol mismatch). |
| `SubmitAction` | any member | `action_id: None` lets the core generate one. The core retries with the **same** `action_id` after `action_response_timeout`. Commit and reject are both asynchronous. |
| `SetJoinable` | host | App-owned admission flag (C-5). Admission also needs `roster_size < max_players`. |
| `CreateInvite` / `RevokeInvite` | host | Host-local; invalid after migration (C-16). |
| `RemovePeer` | host | Commits `MEMBER_REMOVE`. Intended response to `PeerReconnectGraceExpired` (C-6). |
| `Leave` | any | Host first hands off authority (`HOST_TRANSFER`), then commits `MEMBER_LEAVE`. |
| `End` | host | Ends the session for everyone. |

A command that is invalid for the current state (for example `SetJoinable` on a non-host) is ignored and produces `Diagnostic(CommandRejected)`. *Open: see §8, item 3.*

## 4. Output

```rust
pub enum Output {
    Send    { link: LinkId, frame: Bytes },
    Connect { route: RouteDescriptor, purpose: ConnectPurpose },
    Close   { link: LinkId, reason: CloseReason },
    Advertise(Option<SessionAdvertisement>),
    Event(SessionEvent),
    Diagnostic(DiagnosticEvent),
}
```

Execute outputs **in order**. Order matters: for example `Send` of a `COMMIT` precedes the `Event(StateCommitted)` that depends on it.

| Output | Driver action |
|---|---|
| `Send` | Write to the link. If the link is gone, drop silently; the core recovers through heartbeats and sync. |
| `Connect` | Dial. Report success as `LinkUp{inbound:false}`, failure as `LinkDown`. The core owns retry policy. |
| `Close` | Close the link. |
| `Advertise(Some)` / `Advertise(None)` | Start, update or stop discovery advertising. |
| `Event` | Hand to the UI. |
| `Diagnostic` | Developer logging only. |

## 5. Events

Canonical list is Guide §8. Contract notes:

- `StateCommitted` is delivered to **every** member, host included, in strict `commit_index` order, with no gaps and no repeats.
- Every `SubmitAction` ends in exactly one of: a `StateCommitted` whose `committed_action_id` equals the action, or `ActionRejected{action_id, reason}`. Retries never create a second terminal outcome.
- `HostMigrationStarted` is always followed by `HostChanged`, `HostMigrationFailed`, `AuthorityConflict` or `SessionEnded`.
- `SessionEnded` is terminal. After it the core accepts only queries.
- `SessionDiscovered` can repeat for the same session with a changed advertisement. De-duplicate by `session_id`.

*Event enum fields are not specified anywhere yet. See §8, item 1.*

## 6. Game and policy traits

```rust
pub trait GameAdapter {
    fn validate_and_apply(&mut self, actor: PeerId, action: &[u8], state: &[u8]) -> ActionOutcome;
    fn on_state_committed(&mut self, visible: &StateRepresentation, sv: StateVersion, ci: CommitIndex);
    fn on_session_event(&mut self, event: &SessionEvent);
}
pub enum ActionOutcome { Accepted { new_state: Bytes, event: Option<Bytes> }, Rejected { reason_code: u32 } }
```

Contract:

1. `validate_and_apply` runs only on the current host, and must be pure (Guide §3.2). It may be called again for the same action after a retry or migration.
2. `reason_code` is app-owned. `0` is reserved. TurnNet's own `STALE_CONTEXT` is a separate reject reason, not a game code.
3. `on_state_committed` and `on_session_event` run on all peers. They must not block, and must not call back into the core.
4. A panic in an adapter is a bug in the game. The core does not catch it.

`StateReplicationPolicy` is specified in Plan §4.4. `TrustedFullReplication` ships in the core. Other policies are for hidden-information games and land in M5.

## 7. Queries and timing

```rust
pub fn next_deadline(&self) -> Option<Millis>;
pub fn state(&self) -> SessionState;
pub fn info(&self) -> SessionInfo;
pub fn peers(&self) -> Vec<PeerView>;
pub fn current_host(&self) -> Option<PeerId>;
pub fn export_recovery_blob(&self) -> Bytes;
```

- Queries take `&self` and never change state.
- `next_deadline()` is only valid until the next `handle`. Recompute after every call.
- `now: Millis` must be non-decreasing across calls. A decrease is clamped and reported as `Diagnostic(ClockWentBackwards)`.
- `export_recovery_blob` contains the reconnect secret. Treat as a password. Persist after each `StateCommitted` and on `Background`.

`SessionState` follows Spec §45. Per-peer `PeerConnectivity` is separate from membership (Plan §4.3).

## 8. Gaps to settle before M3 coding

These are not specified in the plan or the guide, and the first game will hit them.

1. **`SessionEvent` / `DiagnosticEvent` / `Reason` enums.** Fields are not defined (Guide §8 lists names only). Needed: `JoinRejected` reasons, `ActionRejected` reason shape (game code vs `STALE_CONTEXT`), `SessionEnded` reasons (`NoSurvivors`, `HostEnded`, `Left`).
2. **Module paths.** The Guide imports `turnnet_core::{game, config, visibility, session}`. `lib.rs` has none of them. Confirm the layout, or fix the Guide imports.
3. **Invalid-command policy.** Ignore with `Diagnostic`, or return an `Event`? Games need a way to know `SetJoinable` was refused.
4. **Observer / non-playing members.** The tic tac toe design wants a third roster member with no game seat. This is game-level (reject with code 3), so no core change is needed. Just confirm `max_players=3` is acceptable.
5. **Who calls `SetJoinable(false)` on `Start`?** The host app does this from `on_state_committed`. Because `on_state_committed` cannot call `handle`, the driver must queue the command. Document that pattern in Guide §3.8 and §5.5.
6. **`Tick` on an idle core.** Define `next_deadline()` for `Idle` (discovery timeouts only).

## 9. Compatibility rules

- Everything in this file is pre-1.0 and may change before M8.
- The wire protocol is versioned separately ([protocol.md](protocol.md)). A driver and core of the same crate version always interoperate.
- Canonical encodings are frozen at the first cross-platform release (Plan §11).
