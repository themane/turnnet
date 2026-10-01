# TurnNet (working name)

Engine-agnostic multiplayer networking/session library for casual, low-bandwidth,
turn-based games (2–8 players) on Android and iOS.

> **TurnNet** is a placeholder name.

## Status

Phase 1 ("semantics first"): Rust session core, protocol, and deterministic
simulation. See [docs/Phase1_Implementation_Plan.md](docs/Phase1_Implementation_Plan.md).

## Using TurnNet in your game

TurnNet is the networking/session layer: you provide game rules as a
`GameAdapter` working on opaque action/state bytes, and TurnNet handles hosting,
joining, exactly-once moves, state replication, host migration, reconnects, and
recovery — with no backend server.

👉 **[Game Integration Guide](docs/Game_Integration_Guide.md)**: concepts, how to
design your game state and rules, the session lifecycle, events, hidden
information, recovery, invites, configuration, testing, and a checklist.

> The session API is still being built (Phase 1, milestone M1 done). The guide
> marks planned APIs explicitly; its game-design guidance applies today.

## Building

Requires stable Rust (edition 2024) and `protoc` (protobuf compiler).

```bash
cargo test --workspace
```

## Documentation

- [V2 Decisions](docs/Turn_Based_Multiplayer_Harness_V2_Decisions.md) — canonical architecture decisions
- [V2 Implementation Spec](docs/Turn_Based_Multiplayer_Harness_V2_Implementation_Spec.md)
- [V2 AI Context](docs/Turn_Based_Multiplayer_Harness_V2_AI_Context.md)
- [Phase 1 Implementation Plan](docs/Phase1_Implementation_Plan.md)
- [Wire protocol and hash encodings](docs/protocol.md)
- [Game Integration Guide](docs/Game_Integration_Guide.md)

## License

Licensed under either of MIT or Apache-2.0 at your option.
