//! Length-delimited framing: a `u32` big-endian length, then one encoded
//! `Envelope`. Declared lengths are validated before any body allocation.

/// Size of the length prefix.
pub const FRAME_HEADER_LEN: usize = 4;

/// Frame and message size limits.
///
/// In Phase 1, state travels inline, so a frame may be as large as a state
/// payload. Messages that do not carry state are additionally held to
/// `max_control_message_bytes` after decoding (see `MessageType::carries_state`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameLimits {
    max_frame_bytes: usize,
    max_control_message_bytes: usize,
}

impl FrameLimits {
    /// Library hard cap for any frame body: 16 MiB state + 64 KiB overhead.
    pub const HARD_MAX_FRAME_BYTES: usize = 16 * 1024 * 1024 + 64 * 1024;
    /// Library hard cap for messages that do not carry state.
    pub const HARD_MAX_CONTROL_MESSAGE_BYTES: usize = 64 * 1024;

    /// Creates limits, clamping each value to the library hard caps so an
    /// application policy can lower but never raise them.
    pub fn new(max_frame_bytes: usize, max_control_message_bytes: usize) -> Self {
        let max_frame_bytes = max_frame_bytes.clamp(1, Self::HARD_MAX_FRAME_BYTES);
        Self {
            max_frame_bytes,
            max_control_message_bytes: max_control_message_bytes
                .clamp(1, Self::HARD_MAX_CONTROL_MESSAGE_BYTES)
                .min(max_frame_bytes),
        }
    }

    pub const fn max_frame_bytes(&self) -> usize {
        self.max_frame_bytes
    }

    pub const fn max_control_message_bytes(&self) -> usize {
        self.max_control_message_bytes
    }
}

impl Default for FrameLimits {
    /// 1 MiB state payload + 64 KiB envelope overhead; 64 KiB control messages.
    fn default() -> Self {
        Self::new(
            1024 * 1024 + 64 * 1024,
            Self::HARD_MAX_CONTROL_MESSAGE_BYTES,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum FrameError {
    #[error("zero-length frame")]
    Empty,
    #[error("frame of {declared} bytes exceeds limit of {limit}")]
    TooLarge { declared: usize, limit: usize },
    #[error("decoder is poisoned by an earlier framing error")]
    Poisoned,
}

/// Prefixes an already-encoded envelope with its length.
pub fn encode_frame(body: &[u8], limits: &FrameLimits) -> Result<Vec<u8>, FrameError> {
    if body.is_empty() {
        return Err(FrameError::Empty);
    }
    if body.len() > limits.max_frame_bytes {
        return Err(FrameError::TooLarge {
            declared: body.len(),
            limit: limits.max_frame_bytes,
        });
    }
    let mut out = Vec::with_capacity(FRAME_HEADER_LEN + body.len());
    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
    out.extend_from_slice(body);
    Ok(out)
}

/// Incremental frame decoder for a reliable byte stream.
///
/// Memory is bounded by one header plus one validated body. After any error
/// the decoder is poisoned and the link must be closed: stream framing cannot
/// be resynchronized.
#[derive(Debug)]
pub struct FrameDecoder {
    limits: FrameLimits,
    header: [u8; FRAME_HEADER_LEN],
    header_filled: usize,
    body: Option<Body>,
    poisoned: bool,
}

#[derive(Debug)]
struct Body {
    declared: usize,
    bytes: Vec<u8>,
}

impl FrameDecoder {
    pub fn new(limits: FrameLimits) -> Self {
        Self {
            limits,
            header: [0; FRAME_HEADER_LEN],
            header_filled: 0,
            body: None,
            poisoned: false,
        }
    }

    /// Consumes `data` and returns every frame body it completes, in order.
    pub fn feed(&mut self, mut data: &[u8]) -> Result<Vec<Vec<u8>>, FrameError> {
        if self.poisoned {
            return Err(FrameError::Poisoned);
        }
        let mut frames = Vec::new();
        while !data.is_empty() {
            match &mut self.body {
                None => {
                    let take = (FRAME_HEADER_LEN - self.header_filled).min(data.len());
                    self.header[self.header_filled..self.header_filled + take]
                        .copy_from_slice(&data[..take]);
                    self.header_filled += take;
                    data = &data[take..];
                    if self.header_filled == FRAME_HEADER_LEN {
                        let declared = u32::from_be_bytes(self.header) as usize;
                        self.header_filled = 0;
                        if let Err(e) = self.check_declared(declared) {
                            self.poisoned = true;
                            return Err(e);
                        }
                        self.body = Some(Body {
                            declared,
                            bytes: Vec::with_capacity(declared),
                        });
                    }
                }
                Some(body) => {
                    let take = (body.declared - body.bytes.len()).min(data.len());
                    body.bytes.extend_from_slice(&data[..take]);
                    data = &data[take..];
                    if body.bytes.len() == body.declared {
                        frames.push(self.body.take().map(|b| b.bytes).unwrap_or_default());
                    }
                }
            }
        }
        Ok(frames)
    }

    /// True when no partial frame is buffered.
    pub fn is_idle(&self) -> bool {
        self.header_filled == 0 && self.body.is_none()
    }

    fn check_declared(&self, declared: usize) -> Result<(), FrameError> {
        if declared == 0 {
            Err(FrameError::Empty)
        } else if declared > self.limits.max_frame_bytes {
            Err(FrameError::TooLarge {
                declared,
                limit: self.limits.max_frame_bytes,
            })
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(body: &[u8]) -> Vec<u8> {
        encode_frame(body, &FrameLimits::default()).unwrap()
    }

    #[test]
    fn decodes_byte_at_a_time() {
        let mut stream = frame(b"hello");
        stream.extend(frame(b"x"));
        let mut d = FrameDecoder::new(FrameLimits::default());
        let mut out = Vec::new();
        for b in &stream {
            out.extend(d.feed(std::slice::from_ref(b)).unwrap());
        }
        assert_eq!(out, vec![b"hello".to_vec(), b"x".to_vec()]);
        assert!(d.is_idle());
    }

    #[test]
    fn decodes_many_frames_in_one_chunk() {
        let stream: Vec<u8> = (1..=5u8)
            .flat_map(|n| frame(&vec![n; n as usize]))
            .collect();
        let mut d = FrameDecoder::new(FrameLimits::default());
        let out = d.feed(&stream).unwrap();
        assert_eq!(out.len(), 5);
        assert_eq!(out[4], vec![5; 5]);
    }

    #[test]
    fn rejects_zero_length_and_poisons() {
        let mut d = FrameDecoder::new(FrameLimits::default());
        assert_eq!(d.feed(&[0, 0, 0, 0]), Err(FrameError::Empty));
        assert_eq!(d.feed(&frame(b"ok")), Err(FrameError::Poisoned));
    }

    #[test]
    fn rejects_oversized_declaration_before_allocating() {
        let limits = FrameLimits::new(16, 16);
        let mut d = FrameDecoder::new(limits);
        // Declares 4 GiB - 1; must fail on the header alone.
        let err = d.feed(&[0xff, 0xff, 0xff, 0xff]).unwrap_err();
        assert_eq!(
            err,
            FrameError::TooLarge {
                declared: u32::MAX as usize,
                limit: 16
            }
        );
    }

    #[test]
    fn encode_enforces_limits() {
        let limits = FrameLimits::new(4, 4);
        assert_eq!(encode_frame(b"", &limits), Err(FrameError::Empty));
        assert!(matches!(
            encode_frame(b"12345", &limits),
            Err(FrameError::TooLarge { .. })
        ));
    }

    #[test]
    fn limits_clamp_to_hard_caps() {
        let l = FrameLimits::new(usize::MAX, usize::MAX);
        assert_eq!(l.max_frame_bytes(), FrameLimits::HARD_MAX_FRAME_BYTES);
        assert_eq!(
            l.max_control_message_bytes(),
            FrameLimits::HARD_MAX_CONTROL_MESSAGE_BYTES
        );
        let small = FrameLimits::new(100, 1000);
        assert_eq!(small.max_control_message_bytes(), 100);
    }
}
