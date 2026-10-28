use super::protocol::{CompositePhase, Request, request_line};
use crate::wire::{MAX_LINE_BYTES, read_message};
use std::io::{BufReader, Cursor};

#[test]
fn fragmented_control_lines_preserve_the_following_message() {
    let mut bytes = request_line(&Request::Sample {
        phase: CompositePhase::Recovered,
    })
    .unwrap();
    bytes.extend(request_line(&Request::Finish).unwrap());
    let mut reader = BufReader::with_capacity(1, Cursor::new(bytes));
    assert!(matches!(
        read_message::<Request>(&mut reader).unwrap(),
        Some(Request::Sample {
            phase: CompositePhase::Recovered
        })
    ));
    assert!(matches!(
        read_message::<Request>(&mut reader).unwrap(),
        Some(Request::Finish)
    ));
    assert!(read_message::<Request>(&mut reader).unwrap().is_none());
}

#[test]
fn control_decoder_is_bounded_and_rejects_partial_or_unknown_commands() {
    for bytes in [
        vec![b'x'; 10_000],
        b"{\"command\":\"finish\"}".to_vec(),
        b"{\"command\":\"other\"}\n".to_vec(),
    ] {
        assert!(read_message::<Request>(&mut BufReader::new(Cursor::new(bytes))).is_err());
    }
    let bytes = request_line(&Request::Finish).unwrap();
    let mut input = BufReader::new(Cursor::new(bytes));
    assert!(matches!(
        read_message::<Request>(&mut input).unwrap(),
        Some(Request::Finish)
    ));
    assert!(read_message::<Request>(&mut input).unwrap().is_none());
}

#[test]
fn maximum_width_frame_age_words_fit_the_existing_bounded_protocol() {
    use mapgyver_native_probe::frame_age::{CHUNK_SAMPLES, Sample};
    let sample = Sample {
        submitted_qpc: u64::MAX,
        completed_qpc: u64::MAX,
        words: [[u32::MAX; 5]; 2],
    };
    let message = super::protocol::Response::FrameAgeChunk {
        offset: 216_001,
        samples: vec![sample; CHUNK_SAMPLES],
    };
    let mut bytes = serde_json::to_vec(&message).unwrap();
    bytes.push(b'\n');
    assert!(bytes.len() as u64 <= MAX_LINE_BYTES);
    assert!(
        read_message::<super::protocol::Response>(&mut BufReader::new(Cursor::new(bytes)))
            .unwrap()
            .is_some()
    );
}

#[test]
fn maximum_width_resource_chunks_fit_both_child_protocols() {
    use mapgyver_native_probe::{
        allocation::Snapshot,
        resource_log::{self, MemoryInfo, Sample},
    };
    let sample = Sample {
        elapsed_seconds: f64::MAX,
        source_phase: Some(3),
        allocations: Snapshot {
            live_bytes_by_owner: [u64::MAX; 4],
            retiring_bytes_by_owner: [u64::MAX; 4],
            live_textures: u64::MAX,
            retiring_textures: u64::MAX,
            peak_live_plus_retiring_bytes: u64::MAX,
            peak_live_plus_retiring_textures: u64::MAX,
        },
        gpu_local_nonlocal: [
            Some(MemoryInfo {
                current_usage_bytes: u64::MAX,
                budget_bytes: u64::MAX,
            }),
            Some(MemoryInfo {
                current_usage_bytes: u64::MAX,
                budget_bytes: u64::MAX,
            }),
        ],
    };
    let message = resource_log::Message::Samples {
        offset: 3620,
        samples: vec![sample; resource_log::CHUNK_SAMPLES],
    };
    let observer = super::protocol::Response::Resources { message };
    let bytes = crate::wire::line(&observer).unwrap();
    assert!(bytes.len() as u64 <= MAX_LINE_BYTES);
    let decoded: super::protocol::Response = serde_json::from_slice(&bytes).unwrap();
    if let super::protocol::Response::Resources { message } = decoded {
        let fixture = crate::fixture::protocol::Response::Resources { message };
        assert!(crate::wire::line(&fixture).unwrap().len() as u64 <= MAX_LINE_BYTES);
    } else {
        panic!("resource message changed kind");
    }
}
