//! Property tests: malformed or adversarial bytes never panic, and framing is
//! independent of how a stream is chunked (Spec §55 test 25, partial).

use proptest::prelude::*;
use turnnet_core::protocol::{
    FrameDecoder, FrameLimits, PROTOCOL_MAJOR, decode_envelope, encode_envelope, pb,
};

fn heartbeat_envelope(term: u64, index: u64, msg: [u8; 16]) -> pb::Envelope {
    pb::Envelope {
        protocol_major: PROTOCOL_MAJOR,
        session_id: vec![1; 16],
        term,
        message_id: msg.to_vec(),
        sender_peer_id: vec![2; 16],
        body: Some(pb::envelope::Body::Heartbeat(pb::Heartbeat {
            head: Some(pb::CommitHead {
                term,
                commit_index: index,
                commit_hash: vec![3; 32],
            }),
            host_capable: true,
            migration_eligible: false,
        })),
        ..Default::default()
    }
}

/// Splits `data` at the given cut points.
fn chunks(data: &[u8], cuts: &[usize]) -> Vec<Vec<u8>> {
    let mut points: Vec<usize> = cuts.iter().map(|c| c % (data.len() + 1)).collect();
    points.sort_unstable();
    points.dedup();
    let mut out = Vec::new();
    let mut prev = 0;
    for p in points.into_iter().chain([data.len()]) {
        out.push(data[prev..p].to_vec());
        prev = p;
    }
    out
}

proptest! {
    #[test]
    fn arbitrary_bytes_never_panic(data in proptest::collection::vec(any::<u8>(), 0..4096),
                                   cuts in proptest::collection::vec(any::<usize>(), 0..16)) {
        let limits = FrameLimits::new(1024, 512);
        let mut d = FrameDecoder::new(limits);
        for c in chunks(&data, &cuts) {
            match d.feed(&c) {
                Ok(frames) => for f in frames {
                    prop_assert!(f.len() <= 1024);
                    let _ = decode_envelope(&f, &limits);
                },
                Err(_) => break,
            }
        }
    }

    #[test]
    fn arbitrary_envelope_bodies_never_panic(data in proptest::collection::vec(any::<u8>(), 0..2048)) {
        let _ = decode_envelope(&data, &FrameLimits::default());
    }

    #[test]
    fn framing_is_chunking_independent(
        msgs in proptest::collection::vec((any::<u64>(), any::<u64>(), any::<[u8; 16]>()), 1..20),
        cuts in proptest::collection::vec(any::<usize>(), 0..32),
    ) {
        let limits = FrameLimits::default();
        let envs: Vec<_> = msgs.iter().map(|(t, i, m)| heartbeat_envelope(*t, *i, *m)).collect();
        let stream: Vec<u8> = envs.iter().flat_map(|e| encode_envelope(e, &limits).unwrap()).collect();
        let mut d = FrameDecoder::new(limits);
        let mut decoded = Vec::new();
        for c in chunks(&stream, &cuts) {
            for f in d.feed(&c).unwrap() {
                decoded.push(decode_envelope(&f, &limits).unwrap());
            }
        }
        prop_assert!(d.is_idle());
        prop_assert_eq!(decoded, envs);
    }
}
