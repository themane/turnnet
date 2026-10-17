---
document: turnnet_game_integration_guide
version: 0.1
status: preview
date: 2026-10-02
audience:
  - game developers integrating TurnNet
---

# TurnNet Game Integration Guide

How to build a 2–8 player turn-based multiplayer game on TurnNet.

> **Status: preview.** TurnNet is in Phase 1. Only the protocol and identity foundations (milestone M1) are implemented. The session API in this guide is the **planned** API from [Phase1_Implementation_Plan.md](Phase1_Implementation_Plan.md) §4. Names and signatures may change before release. Code marked *planned* will not compile yet.
>
> Sections 1–4 and the checklist (§16) describe how the library works and how to design your game. They don't depend on API details and are safe to follow now.

| Capability | Status |
|---|---|
| Identity types, hashes, wire protocol, framing | ✅ M1 |
| Commit chain, roster, `reconcile()` | ✅ M2 |
| Session core (create/join/actions/commits) | Planned: M2–M4 |
| Hidden-information policies | Planned: M5 |
| Host migration, conflict fencing | Planned: M6 |
| Reconnect and recovery blob | Planned: M7 |
| In-process simulator for testing games | Planned: M3 |
| Iroh transport (`LOCAL_LAN`, `INTERNET`) | Phases 2–4 |
| Android (Kotlin) / iOS (Swift) SDKs | Phase 5 |
| Same-room (Google Nearby, Wi-Fi Aware) | Phase 6 |

---

## 1. What TurnNet does for you

TurnNet is the networking and session layer for casual turn-based games: board, card, chess-like, quiz and party games. It is engine-agnostic. It never sees your rules, rendering or engine; game actions and state are just **bytes** to it.

| Your game owns | TurnNet owns |
|---|---|
| Rules and move legality | Peer/session identity |
| Turn order | Discovery, connectivity, invites |
| Serializing actions and state to bytes | Admission and committed membership |
| Randomness (dice, shuffles) | Choosing the authoritative host |
| What is hidden from whom | Routing actions to the host, exactly-once commits |
| UI, animation, sound | Replicating state to every player |
| | Host migration when the host leaves or drops |
| | Reconnect and process-death recovery |
| | Diagnostics |

No backend server is needed: no accounts, matchmaking or cloud state. One player's device is the **host**, and if it disappears another device takes over automatically.

---

## 2. Core concepts

### 2.1 Host-authoritative commits

```text
 Player B's device                Host (Player A)                    All devices
 ─────────────────                ───────────────                    ───────────
 submit_action(bytes) ──ACTION──▶ your validate_and_apply(...)
                                   ├─ Rejected(code) ──────────────▶ B: ActionRejected
                                   └─ Accepted(new_state) ─COMMIT──▶ your on_state_committed(...)
```

- Every player, the host included, submits moves the same way.
- Only the host runs your rules (`validate_and_apply`).
- An accepted action produces a **commit**: a new, complete game state with version numbers. Every device receives it and renders from it.
- Clients never change game state locally. They render committed state, optionally with a "pending" indicator for moves they have sent.

### 2.2 Versions you will see

| Value | Meaning |
|---|---|
| `state_version` | Number of accepted game actions. Starts at 0 for the initial state. |
| `commit_index` | Position in the total history, counting both game actions and joins/leaves. |
| `membership_epoch` | Number of committed roster changes. |
| `term` | Host generation. It increases each time the host changes. |

Use `state_version` to discard stale UI updates. The other values are mainly for diagnostics.

### 2.3 Players: membership vs. connection

A player who disconnects is **still in the game**. Their phone may be in a tunnel or the app may be backgrounded. TurnNet tracks these two things separately:

- **Membership** (committed): in the roster, left, or removed. It changes only through explicit joins, leaves or removals.
- **Connectivity** (live): connected, degraded, reconnecting, interrupted or unreachable.

Your UI should show a disconnected player as "reconnecting…", not remove them. If they stay gone past the grace period (60 s by default), TurnNet tells you, and **your game decides** whether to remove them, skip their turns or wait.

### 2.4 Actions are exactly-once

Every move has an `action_id`. Retries after timeouts, reconnects or host changes reuse the same id, so a move is never applied twice. TurnNet generates the id and handles the retries. Don't resubmit a move yourself after a timeout; that creates a second, different action.

### 2.5 Host migration

If the host leaves or drops, the remaining devices agree on a new host. The new host is the lowest join-order player who can host and holds the full latest state. Play pauses briefly, `HostChanged` fires, and play continues from the last committed state. Your game needs to do nothing beyond showing a "switching host…" indicator. This works because **any device can be host**: every device runs your rules code, not just the original host.

### 2.6 Play profiles

| Profile | Use when | Network needed |
|---|---|---|
| `SAME_ROOM` | Players are physically together | None (Nearby / Wi-Fi Aware) |
| `LOCAL_LAN` | Players are on the same Wi-Fi | Shared Wi-Fi, no Internet |
| `INTERNET` | Players are remote | Internet; relay fallback |

You choose the profile explicitly when creating or discovering a session. Game code is identical across all three.

---

## 3. Designing your game for TurnNet

These rules matter more than any API detail.

### 3.1 State must be complete and self-contained

The authoritative state bytes are **everything** a new host needs to continue the game. Include:

- whose turn it is, the turn number and the phase;
- every player's score, hand, position, timers and so on, keyed by TurnNet `PeerId` or by your own seat index mapped to `PeerId`;
- **RNG state or seed**, if future randomness depends on it;
- any rules configuration chosen at game start.

If a value lives only in host memory, it is lost when the host changes.

### 3.2 `validate_and_apply` must be a pure function of its inputs

```text
validate_and_apply(actor, action_bytes, current_state_bytes) -> Accepted(new_state_bytes) | Rejected(code)
```

- Base the decision only on `actor`, `action_bytes` and `current_state_bytes`.
- Don't read UI state, local settings or the wall clock.
- Never mutate anything outside the returned state. The function may be called on any device after a migration.
- If you need time, such as a turn timer, have the acting client include a timestamp in the action, or keep timers as UI-only hints.

### 3.3 Randomness

Do randomness **on the host, inside `validate_and_apply`**, and write the results into the new state. Examples: the dice value, or the deck order after a shuffle. Clients never roll dice themselves. Cryptographically fair randomness and protection against a cheating host are explicit non-goals in V2.

### 3.4 Validate everything, trust nothing

`action_bytes` come from another device. Decode defensively. Reject malformed input, out-of-turn moves and illegal moves with a **reason code** of your choosing (`u32`). Never panic or crash on bad input.

### 3.5 Keep payloads small

TurnNet is built for low bandwidth.

| Limit | Default |
|---|---|
| Action payload | 64 KiB |
| State payload | 1 MiB (hard cap 16 MiB) |

Prefer compact binary formats such as protobuf, bincode, FlatBuffers or MessagePack over JSON for large states. Every accepted action sends the **full** state to every player, so a 50 KB state in an 8-player game is roughly 400 KB per move.

### 3.6 Serialization stability

Any device may become host and re-serialize your state, so all devices running the same game version must decode each other's bytes. Version your state and action formats (for example, a leading version byte). Refuse to start or join sessions with incompatible game versions: put your game/version id in the session's app metadata.

### 3.7 Hidden information (cards, secret roles)

With the default **trusted full replication**, every device receives the full state. A modified client could read other players' hands. That is fine for casual play among friends.

If you need real secrecy, use a **public/private replication policy** (§7):

- each player receives only the public state plus their own private part;
- only peers that hold enough recovery material can become host, so TurnNet may skip some players during migration (`migration_eligible = false`);
- if no eligible peer survives, the session cannot continue.

Decide early: secrecy and resilient host migration trade off against each other.

### 3.8 Lobby and "game started"

TurnNet has no built-in lobby phase. You control admission with a **`joinable`** flag on the host:

- `joinable = true` while you want players to join (the lobby);
- `set_joinable(false)` when the game starts, if late joining isn't allowed.

Represent "waiting for players" vs. "playing" inside **your** game state, for example with a `StartGame` action that only the host's player may submit.

---

## 4. Choosing session parameters

| Parameter | Guidance |
|---|---|
| `max_players` | 2–8. TurnNet never assumes 4. |
| `initial_game_state` | Your serialized initial state, committed as `state_version = 0`. |
| `play_profile` | `SAME_ROOM`, `LOCAL_LAN` or `INTERNET`. |
| `join_policy` | `Open` (anyone who discovers the session) or `RequireCapability` (only with an invite). Use invites for Internet play. |
| `state_replication_policy` | `TrustedFullReplication` (default) or a custom policy (§7). |
| `stale_action_policy` | `Reject` (default): a move made against an outdated state is rejected so the player re-decides. `Advisory`: your rules decide. |
| Display metadata | Small (≤ 256 B) app-defined bytes per player, such as a nickname or avatar id. **Visible to all players and in advertisements; never put secrets here.** |

---

## 5. Integrating from Rust (planned API)

The Rust core is **sans-IO**: it never opens sockets, spawns threads or reads clocks. A *driver* feeds it inputs (received bytes, timer ticks, your commands) and executes its outputs (send bytes, connect, raise events).

- **Mobile games:** the Kotlin/Swift SDKs (Phase 5) will include the driver and transports. You will only see async methods and an event stream (§10).
- **Rust games and tests:** the in-process simulator (`turnnet-sim`) is a ready-made driver.

### 5.1 Implement `GameAdapter` *(planned)*

```rust
use turnnet_core::game::{ActionOutcome, GameAdapter};
use turnnet_core::ids::{CommitIndex, PeerId, StateVersion};
use turnnet_core::session::SessionEvent;
use turnnet_core::state::StateRepresentation;

struct TicTacToe {
    /// Latest committed state, decoded, for rendering.
    view: Option<Board>,
}

impl GameAdapter for TicTacToe {
    // Runs ONLY on the current host. Must be pure (see §3.2).
    fn validate_and_apply(&mut self, actor: PeerId, action: &[u8], state: &[u8]) -> ActionOutcome {
        let Ok(mut board) = Board::decode(state) else {
            return ActionOutcome::Rejected { reason_code: 1 };
        };
        let Ok(mv) = Move::decode(action) else {
            return ActionOutcome::Rejected { reason_code: 2 }; // malformed
        };
        if board.player_to_move() != Some(actor) {
            return ActionOutcome::Rejected { reason_code: 3 }; // not your turn
        }
        if !board.is_legal(mv) {
            return ActionOutcome::Rejected { reason_code: 4 };
        }
        board.apply(actor, mv);
        ActionOutcome::Accepted { new_state: board.encode().into(), event: None }
    }

    // Runs on EVERY device, including the host, after each commit.
    fn on_state_committed(&mut self, visible: &StateRepresentation, _sv: StateVersion, _ci: CommitIndex) {
        self.view = Board::decode(&visible.public_state).ok();
        // trigger a re-render here
    }

    fn on_session_event(&mut self, event: &SessionEvent) {
        // PeerJoined / HostChanged / ConnectionLost ... update UI (see §8)
    }
}
```

### 5.2 Host a session *(planned)*

```rust
use turnnet_core::config::{CreateSessionOptions, JoinPolicy, PlayProfile};
use turnnet_core::session::{Command, Env, SessionCore};
use turnnet_core::visibility::TrustedFullReplication;

let env = Env {
    rng: Box::new(rand::rngs::OsRng),                 // any RngCore
    policy: Box::new(TrustedFullReplication),
    config: Default::default(),                        // timers/limits, see §11
};
let opts = CreateSessionOptions {
    play_profile: PlayProfile::LocalLan,
    max_players: 4,
    initial_game_state: Board::new_lobby().encode().into(),
    join_policy: JoinPolicy::Open,
    app_id: b"com.example.tictactoe/v1".to_vec(),
    display_metadata: b"Alice".to_vec(),
    ..Default::default()
};
let (mut session, outputs) = SessionCore::create(opts, env);
driver.execute(outputs);           // starts advertising, etc.
```

### 5.3 Discover and join *(planned)*

```rust
let mut session = SessionCore::new_idle(env);
let out = session.handle(now, Input::Command(Command::StartDiscovery(filter)), &mut game);
driver.execute(out);
// ... later, SessionEvent::SessionDiscovered(d) arrives; show it in a list ...
let out = session.handle(now, Input::Command(Command::JoinDiscovered(d)), &mut game);
```

For Internet play the host creates an invite (`Command::CreateInvite`) and shares it as a QR code, link or text. The joiner calls `Command::JoinInvite(invite)`. There is no answer step: the host shares once and the joiner connects.

### 5.4 Submit moves *(planned)*

```rust
let out = session.handle(now, Input::Command(Command::SubmitAction {
    payload: Move::new(1, 1).encode().into(),
    action_id: None,              // let TurnNet generate it; retries reuse it
}), &mut game);
```

The result arrives later, either as a commit (`on_state_committed`, where `committed_action_id` matches) or as `ActionRejected { action_id, reason }`.

### 5.5 The driver loop *(planned)*

```rust
loop {
    let input = wait_for_next(
        transport_events,                  // LinkUp / LinkDown / Frame
        ui_commands,                       // Command::*
        session.next_deadline(),           // -> Input::Tick
    );
    let outputs = session.handle(clock.now(), input, &mut game);
    for o in outputs {
        match o {
            Output::Send { link, frame }   => transport.send(link, frame),
            Output::Connect { route, .. }  => transport.connect(route),
            Output::Close { link, .. }     => transport.close(link),
            Output::Advertise(ad)          => discovery.advertise(ad),
            Output::Event(e)               => ui.post(e),
            Output::Diagnostic(d)          => diagnostics.record(d),
        }
    }
}
```

All calls into one `SessionCore` must be serialized on one task or thread. Never call `handle` from inside `GameAdapter` callbacks.

### 5.6 Leave and end *(planned)*

- `Command::Leave`: leave gracefully. If you are the host, authority is handed to the next eligible player first.
- `Command::End(reason)`: host only; ends the session for everyone.
- `Command::RemovePeer(peer)`: host only; removes a member, for example after the grace period expires.

---

## 6. Mobile SDKs (Phase 5, preview of shape)

The Kotlin and Swift facades will expose the same concepts with idiomatic async APIs. A conceptual Kotlin example:

```kotlin
val session = TurnNet.createSession(
    CreateSessionOptions(
        playProfile = PlayProfile.LOCAL_LAN,
        maxPlayers = 4,
        initialGameState = board.encode(),
        joinPolicy = JoinPolicy.OPEN,
    ),
    game = myGameAdapter,
)
session.events.collect { event -> /* see §8 */ }
session.submitAction(move.encode())
```

All SDK calls are non-blocking from the UI thread. Platform permissions are handled by helpers in the SDK: nearby devices, local network, and Wi-Fi Aware.

---

## 7. Hidden information with a custom replication policy *(planned)*

Implement `StateReplicationPolicy` to control what each device receives:

```rust
impl StateReplicationPolicy for MyCardPolicy {
    fn policy_id(&self) -> u32 { 0x4341_5244 }   // stable id for your policy

    fn build_public_state(&self, auth: &[u8], _: &CommitContext) -> Bytes {
        public_part(auth)                         // table, discard pile, hand sizes
    }
    fn build_private_state_for(&self, peer: PeerId, auth: &[u8], _: &CommitContext) -> Option<Bytes> {
        Some(hand_of(auth, peer))                 // only this player's cards
    }
    fn build_recovery_material_for(&self, peer: PeerId, auth: &[u8], ctx: &CommitContext) -> Option<Bytes> {
        // What lets `peer` rebuild the FULL state if it becomes host.
        // None => that peer can never become host.
        if self.is_trusted_backup(peer, ctx) { Some(full_state_encrypted_for(peer, auth)) } else { None }
    }
    fn migration_capability_for(&self, peer: PeerId, ctx: &CommitContext) -> bool {
        self.is_trusted_backup(peer, ctx)
    }
    fn reconstruct_authoritative_state(&self, rep: &StateRepresentation, _: &CommitContext)
        -> Result<Bytes, ReconstructError> {
        decrypt_recovery(rep.recovery_material.as_deref().ok_or(ReconstructError::NoMaterial)?)
    }
}
```

Rules:

- Your game renders from `public_state` plus `private_state`.
- Each player's private part must contain only what that player may see.
- If you encrypt recovery material, use authenticated encryption (AEAD). A hash is not encryption.
- Whoever can decrypt recovery material can see everything. Choose backups deliberately.
- Peers with no recovery material are skipped during host election. Make sure enough eligible peers usually remain.

---

## 8. Events and how to react

| Event | Typical UI reaction |
|---|---|
| `SessionDiscovered` | Add to the "join a game" list. |
| `PeerJoined` / `PeerLeft` | Update the player list. |
| `PeerConnectionChanged` | Show or hide a "reconnecting…" badge on that player. |
| `PeerReconnectGraceExpired` | Ask the host's player whether to remove, skip or wait (§2.3). |
| `StateCommitted` | Re-render from the new state (also delivered via `on_state_committed`). |
| `ActionRejected` | Undo the optimistic UI and show why (your reason code). `STALE_CONTEXT` means the state changed; let the player re-decide. |
| `HostMigrationStarted` | Show "switching host…" and disable move input. |
| `HostChanged` | Hide the indicator and re-enable input. |
| `HostMigrationFailed` | Keep waiting; it is retried automatically. |
| `ConnectionLost` / `ConnectionRestored` | Show or hide a connection banner for this device. |
| `SessionInterrupted` | Networking is paused (for example, the app is backgrounded). Show "paused". |
| `AuthorityConflict` | The group split and both halves kept playing. TurnNet stops play rather than silently losing moves. Offer "end game" or "start a new game". |
| `SessionEnded` | Go to the results or menu screen. |
| `NetworkPathChanged` / `DiagnosticEvent` | Developer diagnostics only; no user action. |

---

## 9. Disconnects, backgrounding, and recovery

### 9.1 Backgrounding

Mobile operating systems may suspend networking. TurnNet reports `SessionInterrupted` and reconnects when the app returns. If the host is backgrounded for long, another player may become host. That is expected.

### 9.2 Process death and app restart *(planned)*

Save a recovery blob regularly, for example after every `StateCommitted` and when the app goes to the background:

```rust
let blob = session.export_recovery_blob();   // opaque bytes; contains a reconnect SECRET
secure_storage.put("turnnet.recovery", blob); // Android Keystore / iOS Keychain-backed storage
```

On relaunch:

```rust
if let Some(blob) = secure_storage.get("turnnet.recovery") {
    let (session, outputs) = SessionCore::restore(&blob, env)?;   // same player, same seat
    driver.execute(outputs);                                      // reconnects automatically
}
```

- The restored player keeps their identity and seat. They are **not** a new player.
- Delete the blob when the session ends.
- **Never** log, upload or share the blob. It works like a password for that seat.

---

## 10. Invites (Internet play)

- An invite is bearer material, like a password. Anyone holding it can join until it expires or is revoked.
- Set an expiry and, optionally, a maximum number of uses. Revoke invites when the game starts.
- An invite can be shown as a QR code, deep link, copyable text or file. Don't assume the joining player can scan a QR code shown on their own phone.
- Invites are tied to the host that created them. After a host change, have the new host create a fresh one.
- Internet invites contain network addresses, which can include IP addresses. Treat them as private.

---

## 11. Configuration reference

| Setting | Default | Notes |
|---|---|---|
| `heartbeat_interval` | 2 s | |
| `peer_suspect_after` | 6 s | Player marked degraded/unreachable |
| `host_election_after` | 8 s | Without host liveness, migration starts |
| `reconnect_grace_period` | 60 s | Then `PeerReconnectGraceExpired` |
| `action_response_timeout` | 10 s | Automatic retry with the same `action_id` |
| `join_timeout` / `max_join_retries` | 20 s / 3 | |
| `stale_action_policy` | `Reject` | Or `Advisory` |
| Max action payload | 64 KiB | |
| Max state payload | 1 MiB | Hard cap 16 MiB |
| Max control message | 64 KiB | Library-owned; cannot be raised |

All timers are policy values and are not part of the wire protocol. Tune them for your game's pace.

---

## 12. Testing your game *(planned, M3+)*

Use `turnnet-sim` to run whole multiplayer sessions in one process, deterministically, with no phones:

```rust
let mut sim = Scenario::new(seed)
    .players(4)
    .game(|| TicTacToe::default())
    .build();
sim.create_and_join_all();
sim.submit(peer(1), Move::new(0, 0).encode());
sim.kill_host();                         // host migration
sim.run_until_stable();
sim.assert_all_states_equal();
```

The simulator can inject latency, packet loss, duplication, reordering, partitions, app restarts and host death. Test at least 2, 4 and 8 players. Bugs in your `validate_and_apply` are much easier to find here than on devices.

---

## 13. Privacy and security checklist

- No accounts, phone numbers or contacts are needed. Player identity is a random per-session `PeerId`.
- Don't use device names, IP addresses or account names as player identity.
- Don't log raw action/state bytes, invites or recovery blobs in production.
- Display metadata and session advertisements are visible to nearby or other players. Keep them non-sensitive.
- TurnNet does not protect against a cheating host or a modified client (V2 non-goal). Don't use it for wagering or competitive play that needs anti-cheat.

---

## 14. Using what exists today (M1)

The currently implemented crate, `turnnet-core`, provides the low-level building blocks. They are mostly internal to the upcoming session core:

```rust
use turnnet_core::ids::{PeerId, SessionId};
use turnnet_core::hash::logical_state_hash;
use turnnet_core::protocol::{FrameDecoder, FrameLimits, decode_envelope};

let peer = PeerId::random(&mut rng);
let h = logical_state_hash(&state_bytes);          // BLAKE3, see docs/protocol.md
let mut decoder = FrameDecoder::new(FrameLimits::default());
for frame in decoder.feed(&bytes_from_stream)? {
    let envelope = decode_envelope(&frame, &FrameLimits::default())?;
}
```

Game developers should wait for the session API (M3–M4) before integrating. The design guidance in §3 can be applied to your game's state and action formats now.

---

## 15. FAQ

**Does the host player have an advantage or a different code path?** No. The host's own moves go through the same `submit_action` path. Only `validate_and_apply` runs on the host, and every device has that code.

**Can two players move at the same time?** Yes. The host processes actions one at a time in arrival order. Your rules decide whether the second one is still legal.

**What if two players disconnect and the game is for exactly N players?** TurnNet keeps them as members while they reconnect. If they don't return, your game decides whether to continue with fewer players, using `RemovePeer`, or to end.

**Can someone join mid-game?** Only while the host keeps `joinable = true` and there's room. Your state decides what a late joiner does: spectate until next round, take an empty seat, and so on.

**Spectators?** Not supported in V2.

**Real-time games?** No. TurnNet targets turn-based, low-bandwidth play. It has no rollback or real-time netcode.

---

## 16. Integration checklist

- [ ] State bytes contain everything needed to continue the game on another device (§3.1).
- [ ] `validate_and_apply` is pure, defensive and never panics (§3.2, §3.4).
- [ ] Randomness happens on the host and is stored in state (§3.3).
- [ ] State and action formats are versioned; the game version is in app metadata (§3.6).
- [ ] Hidden-information needs are decided, and the replication policy is chosen (§3.7, §7).
- [ ] Lobby/start logic lives in your state, and `joinable` is toggled at game start (§3.8).
- [ ] The UI handles every event in §8, especially host migration and authority conflict.
- [ ] The recovery blob is stored in secure storage and deleted when the session ends (§9.2).
- [ ] Invites expire or are revoked and treated as private (§10).
- [ ] Simulator tests cover 2, 4 and 8 players with host death and disconnects (§12).
