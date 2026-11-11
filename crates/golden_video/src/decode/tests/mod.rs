use super::*;
mod parallelism;
mod profiles;

fn limits() -> DecodeLimits {
    DecodeLimits {
        max_pixels: 4096 * 2160,
        max_packet_bytes: 8 * 1024 * 1024,
        max_packets_per_frame: 128,
        software_threads: crate::decode::SoftwareDecodeThreads::One,
    }
}

fn fixture(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/decode/tests/fixtures")
        .join(name)
}

#[test]
fn invalid_admission_and_missing_paths_fail_before_opening_ffmpeg() {
    let directory = tempfile::tempdir().unwrap();
    let absent = directory.path().join("missing.mp4");
    assert!(matches!(Decoder::open(&absent, limits()), Err(DecodeError::NotAFile)));
    assert!(matches!(
        Decoder::open(
            &absent,
            DecodeLimits {
                max_pixels: 0,
                ..limits()
            }
        ),
        Err(DecodeError::InvalidLimits)
    ));
}

#[test]
fn software_storage_cannot_expose_foreign_hardware_pixel_pointers() {
    let mut frame = ffmpeg::frame::Video::empty();
    frame.set_format(ffmpeg::format::Pixel::D3D11);
    assert!(matches!(
        DecoderKind::Software.frame(frame),
        Err(DecodeError::UnsupportedHardware)
    ));
}

#[test]
fn demux_only_timing_matches_decoded_stream_units_for_all_common_fixtures() {
    for name in [
        "h264.mp4",
        "hevc.mp4",
        "hevc-main.mp4",
        "prores.mov",
        "h264-keyframes.mp4",
    ] {
        let timing = probe_timing_with_interrupt(&fixture(name), || false).unwrap();
        let decoder = Decoder::open(&fixture(name), limits()).unwrap();
        assert_eq!(timing.duration, decoder.info().duration);
        assert_eq!(timing.time_base, decoder.info().time_base);
        assert!(timing.duration.is_some_and(|duration| duration > 0));
    }
    assert!(matches!(
        probe_timing_with_interrupt(&fixture("missing.mp4"), || false),
        Err(DecodeError::NotAFile)
    ));
}

#[test]
fn unsupported_color_interpretation_reports_a_typed_conversion_failure() {
    for (name, matrix) in [("h264-bt2020.mp4", true), ("h264-pq.mp4", false)] {
        let mut decoder = Decoder::open(&fixture(name), limits()).unwrap();
        let frame = decoder.next_frame().unwrap().unwrap();
        let result = SoftwareImageConverter::default().convert(&frame);
        assert!(if matrix {
            matches!(result, Err(ConversionError::UnsupportedMatrix))
        } else {
            matches!(result, Err(ConversionError::UnsupportedTransfer))
        });
    }
}

#[test]
fn common_codecs_preserve_exact_mixed_rate_timestamps_pixels_and_delayed_frames() {
    for (name, codec, rate, pixel) in [
        ("h264.mp4", "h264", (24, 1), ffmpeg::format::Pixel::YUV420P),
        ("hevc.mp4", "hevc", (30000, 1001), ffmpeg::format::Pixel::YUV420P),
        ("prores.mov", "prores", (25, 1), ffmpeg::format::Pixel::YUV422P10LE),
    ] {
        let mut decoder = Decoder::open(&fixture(name), limits()).unwrap();
        let info = decoder.info().clone();
        assert_eq!(info.codec, codec);
        assert_eq!((info.width, info.height), (64, 48));
        let mut frames = Vec::new();
        while let Some(frame) = decoder.next_frame().unwrap() {
            assert!(frames.len() < 6, "unexpected duplicate frame: {name}");
            let index = frames.len();
            assert_eq!(
                i128::from(frame.timestamp()) * i128::from(info.time_base.numerator) * rate.0,
                index as i128 * i128::from(info.time_base.denominator) * rate.1,
                "{name}: exact original presentation timestamp"
            );
            assert_eq!(frame.software_image().unwrap().format(), pixel);
            assert_eq!(
                frame.software_image().unwrap().color_space(),
                ffmpeg::color::Space::BT709
            );
            let row = frame.software_image().unwrap().data(0);
            let level = if pixel == ffmpeg::format::Pixel::YUV422P10LE {
                u16::from_le_bytes([row[0], row[1]]) / 4
            } else {
                u16::from(row[0])
            };
            assert!(
                level.abs_diff(32 + 24 * index as u16) <= 3,
                "{name}: wrong frame pixels {index}: {level}"
            );
            frames.push(frame);
        }
        assert_eq!(frames.len(), 6, "{name}: delayed frames must drain");
        assert!(decoder.next_frame().unwrap().is_none(), "stable clean EOF");
        let first = frames[0].software_image().unwrap().data(0).to_vec();
        drop(decoder);
        assert_eq!(
            frames[0].software_image().unwrap().data(0),
            first,
            "frame owns its storage after decoder retirement"
        );
    }
}

#[test]
fn extent_packet_and_per_call_work_limits_are_enforced() {
    let file = fixture("h264.mp4");
    assert!(
        Decoder::open(
            &file,
            DecodeLimits {
                max_pixels: 16,
                ..limits()
            }
        )
        .is_err()
    );
    let mut decoder = Decoder::open(
        &file,
        DecodeLimits {
            max_packet_bytes: 1,
            ..limits()
        },
    )
    .unwrap();
    assert!(matches!(decoder.next_frame(), Err(DecodeError::LimitExceeded)));
    assert!(matches!(decoder.next_frame(), Err(DecodeError::ReopenRequired)));
    let mut decoder = Decoder::open(
        &file,
        DecodeLimits {
            max_packets_per_frame: 1,
            software_threads: crate::decode::SoftwareDecodeThreads::One,
            ..limits()
        },
    )
    .unwrap();
    assert!(matches!(decoder.next_frame(), Err(DecodeError::LimitExceeded)));
}

#[test]
fn standard_main_hevc_fixture_decodes_hd_ramp_with_exact_timestamps() {
    let mut decoder = Decoder::open(&fixture("hevc-main.mp4"), limits()).unwrap();
    let base = decoder.info().time_base;
    assert_eq!((decoder.info().width, decoder.info().height), (1920, 1080));
    let mut count = 0;
    while let Some(frame) = decoder.next_frame().unwrap() {
        assert!(count < 6);
        assert_eq!(
            i128::from(frame.timestamp()) * i128::from(base.numerator) * 30000,
            count as i128 * i128::from(base.denominator) * 1001
        );
        let image = frame.software_image().unwrap();
        assert_eq!(image.format(), ffmpeg::format::Pixel::YUV420P);
        assert!(image.data(0)[0].abs_diff(32 + count * 24) <= 3);
        count += 1;
    }
    assert_eq!(count, 6);
}

#[test]
fn malformed_files_are_errors_and_replaced_files_can_be_reopened() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("relinked.mp4");
    std::fs::write(&path, b"not a movie").unwrap();
    assert!(matches!(Decoder::open(&path, limits()), Err(DecodeError::Ffmpeg(_))));
    std::fs::copy(fixture("h264.mp4"), &path).unwrap();
    let mut reopened = Decoder::open(&path, limits()).unwrap();
    assert_eq!(reopened.next_frame().unwrap().unwrap().timestamp(), 0);
}

#[test]
fn untimed_streams_do_not_invent_timestamps_from_frame_rate() {
    let mut decoder = Decoder::open(&fixture("untimed.h264"), limits()).unwrap();
    assert!(matches!(decoder.next_frame(), Err(DecodeError::MissingTimestamp)));
    assert!(matches!(decoder.next_frame(), Err(DecodeError::ReopenRequired)));
}

#[test]
fn original_timestamp_seeks_flush_old_frames_and_rewind_after_clean_eof() {
    for name in ["h264.mp4", "hevc.mp4", "prores.mov"] {
        let mut decoder = Decoder::open(&fixture(name), limits()).unwrap();
        let mut timestamps = Vec::new();
        while let Some(frame) = decoder.next_frame().unwrap() {
            timestamps.push(frame.timestamp());
        }
        assert_eq!(timestamps.len(), 6);
        for index in [5, 0, 4, 2, 1] {
            decoder.seek_before(timestamps[index]).unwrap();
            let mut found = false;
            for _ in 0..6 {
                let frame = decoder.next_frame().unwrap().unwrap();
                assert!(
                    frame.timestamp() <= timestamps[index],
                    "seek skipped exact frame: {name}, target index {index}, wanted {}, got {}",
                    timestamps[index],
                    frame.timestamp()
                );
                if frame.timestamp() == timestamps[index] {
                    let data = frame.software_image().unwrap().data(0);
                    let level = if name == "prores.mov" {
                        u16::from_le_bytes([data[0], data[1]]) / 4
                    } else {
                        u16::from(data[0])
                    };
                    assert!(level.abs_diff(32 + 24 * index as u16) <= 3, "stale seek pixels: {name}");
                    found = true;
                    break;
                }
            }
            assert!(found, "exact timestamp not reached: {name}");
        }
    }
}

#[test]
fn warm_seeks_keep_the_codec_and_retained_pixels_without_stale_reordered_frames() {
    for name in ["h264.mp4", "hevc.mp4", "prores.mov"] {
        let mut reference = Decoder::open(&fixture(name), limits()).unwrap();
        let mut expected = Vec::new();
        while let Some(frame) = reference.next_frame().unwrap() {
            expected.push((frame.timestamp(), frame.software_image().unwrap().data(0).to_vec()));
        }
        let mut decoder = Decoder::open(&fixture(name), limits()).unwrap();
        let retained = decoder.next_frame().unwrap().unwrap();
        let mut warm_seeks = 0;
        for index in [2, 0, 3, 1, 4, 0, 2] {
            let warm = decoder.state == DecodeState::Reading;
            let owner = unsafe { decoder.decoder.as_ref().unwrap().as_ptr() };
            decoder.seek_before(expected[index].0).unwrap();
            if warm {
                warm_seeks += 1;
                assert_eq!(
                    unsafe { decoder.decoder.as_ref().unwrap().as_ptr() },
                    owner,
                    "warm seek must retain the admitted codec: {name}"
                );
            }
            loop {
                let frame = decoder.next_frame().unwrap().unwrap();
                assert!(frame.timestamp() <= expected[index].0, "stale reorder state: {name}");
                if frame.timestamp() == expected[index].0 {
                    assert_eq!(frame.software_image().unwrap().data(0), expected[index].1);
                    break;
                }
            }
            assert_eq!(retained.software_image().unwrap().data(0), expected[0].1);
        }
        // The six-frame HEVC fixture may enter draining before its first output.
        if name != "hevc.mp4" {
            assert!(warm_seeks > 0, "fixture must exercise warm reuse: {name}");
        }
    }
}

#[test]
fn invalid_seek_is_atomic_and_poisoned_decoders_cannot_hide_errors_by_seeking() {
    let mut decoder = Decoder::open(&fixture("h264.mp4"), limits()).unwrap();
    assert!(matches!(decoder.seek_before(i64::MAX), Err(DecodeError::OutsideStream)));
    assert_eq!(decoder.next_frame().unwrap().unwrap().timestamp(), 0);
    let mut decoder = Decoder::open(
        &fixture("h264.mp4"),
        DecodeLimits {
            max_packet_bytes: 1,
            ..limits()
        },
    )
    .unwrap();
    assert!(matches!(decoder.next_frame(), Err(DecodeError::LimitExceeded)));
    assert!(matches!(decoder.seek_before(0), Err(DecodeError::ReopenRequired)));
}
