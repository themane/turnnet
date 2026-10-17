//! Envelope encoding and stateless validation (Implementation Spec §39.2,
//! steps 1–4 plus identifier-length and control-size checks).

use prost::Message;

use super::PROTOCOL_MAJOR;
use super::frame::{FrameError, FrameLimits, encode_frame};
use super::pb::{self, envelope::Body};

/// Message categories of Implementation Spec §40 (no `SIGNALING_RELAY`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MessageType {
    Hello,
    SessionProbe,
    SessionAdvertisement,
    JoinRequest,
    JoinAccept,
    JoinReject,
    ReconnectHello,
    ReconnectAccept,
    ReconnectReject,
    PeerRoster,
    PeerRouteUpdate,
    Heartbeat,
    ActionSubmit,
    ActionReject,
    StateCommit,
    StateAck,
    StateSyncRequest,
    StateSyncResponse,
    HostSuspected,
    HostPropose,
    HostAck,
    HostAnnounce,
    HostTransfer,
    AuthorityConflict,
    PeerLeft,
    SessionEnd,
    Error,
    StateTransferBegin,
}

impl MessageType {
    pub fn of(body: &Body) -> Self {
        match body {
            Body::Hello(_) => Self::Hello,
            Body::SessionProbe(_) => Self::SessionProbe,
            Body::SessionAdvertisement(_) => Self::SessionAdvertisement,
            Body::JoinRequest(_) => Self::JoinRequest,
            Body::JoinAccept(_) => Self::JoinAccept,
            Body::JoinReject(_) => Self::JoinReject,
            Body::ReconnectHello(_) => Self::ReconnectHello,
            Body::ReconnectAccept(_) => Self::ReconnectAccept,
            Body::ReconnectReject(_) => Self::ReconnectReject,
            Body::PeerRoster(_) => Self::PeerRoster,
            Body::PeerRouteUpdate(_) => Self::PeerRouteUpdate,
            Body::Heartbeat(_) => Self::Heartbeat,
            Body::ActionSubmit(_) => Self::ActionSubmit,
            Body::ActionReject(_) => Self::ActionReject,
            Body::StateCommit(_) => Self::StateCommit,
            Body::StateAck(_) => Self::StateAck,
            Body::StateSyncRequest(_) => Self::StateSyncRequest,
            Body::StateSyncResponse(_) => Self::StateSyncResponse,
            Body::HostSuspected(_) => Self::HostSuspected,
            Body::HostPropose(_) => Self::HostPropose,
            Body::HostAck(_) => Self::HostAck,
            Body::HostAnnounce(_) => Self::HostAnnounce,
            Body::HostTransfer(_) => Self::HostTransfer,
            Body::AuthorityConflict(_) => Self::AuthorityConflict,
            Body::PeerLeft(_) => Self::PeerLeft,
            Body::SessionEnd(_) => Self::SessionEnd,
            Body::Error(_) => Self::Error,
            Body::StateTransferBegin(_) => Self::StateTransferBegin,
        }
    }

    /// Messages that may carry a state representation and so may exceed the
    /// control-message limit (up to the frame limit).
    pub const fn carries_state(self) -> bool {
        matches!(
            self,
            Self::JoinAccept | Self::ReconnectAccept | Self::StateCommit | Self::StateSyncResponse
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum EnvelopeError {
    #[error(transparent)]
    Frame(#[from] FrameError),
    #[error("protobuf decode failed")]
    Malformed,
    #[error("envelope has no recognized message body")]
    UnknownMessageType,
    #[error("protocol major {major} is not supported")]
    ProtocolMismatch { major: u32 },
    #[error("field `{field}` has {actual} bytes, expected {expected}")]
    InvalidFieldLength {
        field: &'static str,
        expected: &'static str,
        actual: usize,
    },
    #[error("{message_type:?} of {size} bytes exceeds control-message limit {limit}")]
    ControlMessageTooLarge {
        message_type: MessageType,
        size: usize,
        limit: usize,
    },
}

impl EnvelopeError {
    /// Wire error code to report to the remote before closing the link.
    pub fn wire_code(&self) -> pb::ErrorCode {
        match self {
            Self::Frame(_) | Self::ControlMessageTooLarge { .. } => pb::ErrorCode::FrameTooLarge,
            Self::ProtocolMismatch { .. } => pb::ErrorCode::ProtocolMismatch,
            Self::Malformed | Self::UnknownMessageType | Self::InvalidFieldLength { .. } => {
                pb::ErrorCode::Malformed
            }
        }
    }
}

/// Validates and encodes `env` as a complete length-prefixed frame.
pub fn encode_envelope(env: &pb::Envelope, limits: &FrameLimits) -> Result<Vec<u8>, EnvelopeError> {
    let body = env.encode_to_vec();
    check(env, body.len(), limits)?;
    Ok(encode_frame(&body, limits)?)
}

/// Decodes one frame body (without its length prefix) and applies stateless
/// validation. The caller has already bounded `frame` via `FrameDecoder`.
pub fn decode_envelope(frame: &[u8], limits: &FrameLimits) -> Result<pb::Envelope, EnvelopeError> {
    if frame.len() > limits.max_frame_bytes() {
        return Err(FrameError::TooLarge {
            declared: frame.len(),
            limit: limits.max_frame_bytes(),
        }
        .into());
    }
    let env = pb::Envelope::decode(frame).map_err(|_| EnvelopeError::Malformed)?;
    check(&env, frame.len(), limits)?;
    Ok(env)
}

fn check(env: &pb::Envelope, size: usize, limits: &FrameLimits) -> Result<(), EnvelopeError> {
    let body = env.body.as_ref().ok_or(EnvelopeError::UnknownMessageType)?;
    if env.protocol_major != PROTOCOL_MAJOR {
        return Err(EnvelopeError::ProtocolMismatch {
            major: env.protocol_major,
        });
    }
    id_len("session_id", &env.session_id, true)?;
    id_len("message_id", &env.message_id, false)?;
    id_len("sender_peer_id", &env.sender_peer_id, true)?;
    if let Some(action_id) = &env.action_id {
        id_len("action_id", action_id, false)?;
    }
    let message_type = MessageType::of(body);
    if !message_type.carries_state() && size > limits.max_control_message_bytes() {
        return Err(EnvelopeError::ControlMessageTooLarge {
            message_type,
            size,
            limit: limits.max_control_message_bytes(),
        });
    }
    Ok(())
}

fn id_len(field: &'static str, bytes: &[u8], may_be_empty: bool) -> Result<(), EnvelopeError> {
    match bytes.len() {
        16 => Ok(()),
        0 if may_be_empty => Ok(()),
        actual => Err(EnvelopeError::InvalidFieldLength {
            field,
            expected: if may_be_empty { "0 or 16" } else { "16" },
            actual,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{FrameDecoder, PROTOCOL_MINOR};

    fn env(body: Body) -> pb::Envelope {
        pb::Envelope {
            protocol_major: PROTOCOL_MAJOR,
            protocol_minor: PROTOCOL_MINOR,
            session_id: vec![1; 16],
            term: 1,
            message_id: vec![2; 16],
            sender_peer_id: vec![3; 16],
            body: Some(body),
            ..Default::default()
        }
    }

    fn heartbeat() -> Body {
        Body::Heartbeat(pb::Heartbeat {
            head: Some(pb::CommitHead {
                term: 1,
                commit_index: 7,
                commit_hash: vec![9; 32],
            }),
            host_capable: true,
            migration_eligible: true,
        })
    }

    fn roundtrip(e: &pb::Envelope) -> Result<pb::Envelope, EnvelopeError> {
        let limits = FrameLimits::default();
        let frame = encode_envelope(e, &limits)?;
        let mut d = FrameDecoder::new(limits);
        let mut frames = d.feed(&frame)?;
        assert_eq!(frames.len(), 1);
        decode_envelope(&frames.remove(0), &limits)
    }

    #[test]
    fn roundtrips_through_frame_codec() {
        let mut e = env(heartbeat());
        e.commit_index = Some(7);
        e.action_id = Some(vec![4; 16]);
        assert_eq!(roundtrip(&e).unwrap(), e);
    }

    #[test]
    fn rejects_missing_body() {
        let mut e = env(heartbeat());
        e.body = None;
        let bytes = e.encode_to_vec();
        assert_eq!(
            decode_envelope(&bytes, &FrameLimits::default()),
            Err(EnvelopeError::UnknownMessageType)
        );
    }

    #[test]
    fn unknown_body_field_is_unknown_message_type() {
        // Field 99, varint 1: a future message type this version does not know.
        let mut e = env(heartbeat());
        e.body = None;
        let bytes = [e.encode_to_vec(), vec![0x98, 0x06, 0x01]].concat();
        assert_eq!(
            decode_envelope(&bytes, &FrameLimits::default()),
            Err(EnvelopeError::UnknownMessageType)
        );
    }

    #[test]
    fn rejects_major_mismatch_cleanly() {
        let mut e = env(heartbeat());
        e.protocol_major = 3;
        let bytes = e.encode_to_vec();
        let err = decode_envelope(&bytes, &FrameLimits::default()).unwrap_err();
        assert_eq!(err, EnvelopeError::ProtocolMismatch { major: 3 });
        assert_eq!(err.wire_code(), pb::ErrorCode::ProtocolMismatch);
    }

    #[test]
    fn accepts_newer_minor() {
        let mut e = env(heartbeat());
        e.protocol_minor = PROTOCOL_MINOR + 5;
        assert!(roundtrip(&e).is_ok());
    }

    #[test]
    fn rejects_bad_identifier_lengths() {
        let mut e = env(heartbeat());
        e.message_id = vec![];
        assert!(matches!(
            roundtrip(&e),
            Err(EnvelopeError::InvalidFieldLength {
                field: "message_id",
                ..
            })
        ));
        let mut e = env(heartbeat());
        e.session_id = vec![1; 15];
        assert!(matches!(
            roundtrip(&e),
            Err(EnvelopeError::InvalidFieldLength {
                field: "session_id",
                ..
            })
        ));
        let mut e = env(heartbeat());
        e.action_id = Some(vec![]);
        assert!(matches!(
            roundtrip(&e),
            Err(EnvelopeError::InvalidFieldLength {
                field: "action_id",
                ..
            })
        ));
        let mut e = env(heartbeat());
        e.session_id = vec![];
        e.sender_peer_id = vec![];
        assert!(roundtrip(&e).is_ok());
    }

    #[test]
    fn control_limit_applies_only_to_non_state_messages() {
        let limits = FrameLimits::new(200_000, 1_000);
        let big = vec![0u8; 10_000];
        let probe = env(Body::SessionProbe(pb::SessionProbe {
            nonce: big.clone(),
            app_id: vec![],
        }));
        assert!(matches!(
            encode_envelope(&probe, &limits),
            Err(EnvelopeError::ControlMessageTooLarge {
                message_type: MessageType::SessionProbe,
                ..
            })
        ));
        let commit = env(Body::StateCommit(pb::StateCommit {
            state: Some(pb::StateRepresentation {
                kind: 1,
                public_state: big,
                ..Default::default()
            }),
            ..Default::default()
        }));
        let frame = encode_envelope(&commit, &limits).unwrap();
        assert!(decode_envelope(&frame[4..], &limits).is_ok());
    }

    #[test]
    fn garbage_is_malformed() {
        assert_eq!(
            decode_envelope(&[0xff, 0xff, 0xff], &FrameLimits::default()),
            Err(EnvelopeError::Malformed)
        );
    }
}
