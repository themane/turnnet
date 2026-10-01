# TurnNet (working name)

Engine-agnostic multiplayer networking/session library for casual, low-bandwidth,
turn-based games (2–8 players) on Android and iOS.

> **TurnNet** is a placeholder name.

## Status

Phase 1 ("semantics first"): Rust session core, protocol, and deterministic
simulation. See [docs/Phase1_Implementation_Plan.md](docs/Phase1_Implementation_Plan.md).

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

## License

Licensed under either of MIT or Apache-2.0 at your option.
