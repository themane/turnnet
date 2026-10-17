//! TurnNet V2 wire protocol: generated protobuf types, length-delimited
//! framing, and stateless envelope validation (Implementation Spec §39).
//!
//! Session-dependent validation (session id, sender binding, term,
//! membership/commit context, dedupe) belongs to the session core.

mod envelope;
mod frame;

pub use envelope::{EnvelopeError, MessageType, decode_envelope, encode_envelope};
pub use frame::{FRAME_HEADER_LEN, FrameDecoder, FrameError, FrameLimits, encode_frame};

/// Generated protobuf types for package `turnnet.v2`.
#[allow(clippy::all, missing_debug_implementations)]
pub mod pb {
    include!(concat!(env!("OUT_DIR"), "/turnnet.v2.rs"));
}

/// Protocol major version. Also carried by the Iroh ALPN `turnnet/2`.
pub const PROTOCOL_MAJOR: u32 = 2;
/// Protocol minor version of this implementation.
pub const PROTOCOL_MINOR: u32 = 0;
