use super::*;

#[test]
fn software_source_admission_rejects_in_band_growth_before_oversized_conversion() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../crates/golden_video/src/decode/tests/fixtures/h264-growing.mp4");
    let mut decoder = Decoder::open(
        &path,
        DecodeLimits {
            max_pixels: 16384 * 16384,
            max_packet_bytes: 16 * 1024 * 1024,
            max_packets_per_frame: 256,
            software_threads: golden_video::decode::SoftwareDecodeThreads::Four,
        },
    )
    .unwrap();
    assert_eq!((decoder.info().width, decoder.info().height), (64, 48));
    assert!(decoder.info().duration.is_some_and(|duration| duration > 0));
    let budget = Arc::new(Budget::default());
    let lease = budget.admit_software(&mut decoder).unwrap();
    let mut converter = golden_video::decode::SoftwareImageConverter::default();
    let mut rejected = false;
    let mut frames = 0;
    loop {
        match decoder.next_frame() {
            Ok(Some(frame)) => {
                frames += 1;
                if frame.software_layout().unwrap().width > 64 {
                    let image = converter.convert(&frame).unwrap();
                    assert!(
                        image.pixels().len() as u64 <= lease.bytes,
                        "unadmitted converted image {} bytes exceeds CPU reservation {} bytes",
                        image.pixels().len(),
                        lease.bytes
                    );
                }
            }
            Ok(None) => break,
            Err(golden_video::decode::DecodeError::LimitExceeded | golden_video::decode::DecodeError::Ffmpeg(_)) => {
                rejected = true;
                break;
            }
            Err(error) => panic!("unexpected admission failure: {error}"),
        }
    }
    assert!(frames > 0);
    assert!(
        rejected,
        "in-band expansion must not reach conversion outside the admitted shape"
    );
    drop(decoder);
    drop(lease);
    assert_eq!(budget.snapshot().retained_bytes, 0);
}
