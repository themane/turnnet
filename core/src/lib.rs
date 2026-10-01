//! TurnNet V2 session/protocol core.
//!
//! Engine-agnostic, sans-IO core for casual turn-based multiplayer sessions of
//! 2–8 players. Game actions and state are opaque bytes. Transports (Iroh,
//! Nearby, Wi-Fi Aware, simulation) live outside this crate and feed it
//! normalized inputs.
//!
//! See `docs/Phase1_Implementation_Plan.md` for the design and
//! `docs/protocol.md` for the normative wire and hash encodings.

pub mod commit;
pub mod hash;
pub mod ids;
pub mod membership;
pub mod protocol;
pub mod state;

/// Smallest supported session size.
pub const MIN_PLAYERS: u32 = 2;
/// Largest supported session size.
pub const MAX_PLAYERS: u32 = 8;
