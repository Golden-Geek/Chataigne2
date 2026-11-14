use super::*;
use crate::native::{D3d11Control, SourceSubmissionFrame, SourceSubmissionPayload, SourceSubmissionTrace};
use crate::scheduling::FrameWindow;
use golden_time::{PlaybackRate, ShowPosition, SystemClock, TimeAuthority};
use std::num::NonZeroUsize;

fn descriptor() -> crate::TextureDescriptor {
    crate::TextureDescriptor {
        extent: crate::Extent2D::new(2, 1).unwrap(),
        format: crate::PixelFormat::Rgba8Unorm,
        color_encoding: crate::ColorEncoding::Srgb,
        alpha_mode: crate::AlphaMode::Straight,
    }
}

#[test]
fn immutable_frame_producer_context_roundtrips_independently_from_renderer_bindings() {
    let stream = Uuid::new_v4();
    let context = DiagnosticContext::FrameProducer { stream };
    let encoded = serde_json::to_value(context).unwrap();
    assert_eq!(encoded, serde_json::json!({ "FrameProducer": { "stream": stream } }));
    assert_eq!(serde_json::from_value::<DiagnosticContext>(encoded).unwrap(), context);
    let source_token = CompletionToken {
        device: DeviceGeneration::new(Uuid::new_v4(), 1),
        serial: 17,
    };
    let timing = PublicationTiming {
        source_token,
        receive_begin_qpc: None,
        publication_qpc: None,
        spout_image: None,
    };
    let copy: PublicationTiming = serde_json::from_str(&serde_json::to_string(&timing).unwrap()).unwrap();
    assert_eq!(copy, timing);
    assert_eq!(copy.source_token.device, source_token.device);
    assert!(copy.receive_begin_qpc.is_none() && copy.spout_image.is_none());
}

#[test]
fn source_submission_frames_preserve_authority_pts_windows_and_unknown_publication() {
    let mut authority =
        TimeAuthority::new(SystemClock::new(), ShowPosition::from_nanos(0), PlaybackRate::NORMAL).unwrap();
    let epoch = authority.sample().unwrap().epoch();
    let window = FrameWindow::new(
        epoch,
        ShowPosition::from_nanos(600),
        Some(ShowPosition::from_nanos(650)),
        ShowPosition::from_nanos(100),
    )
    .unwrap();
    let frame = SourceSubmissionFrame::from(window);
    assert_eq!(frame.epoch_authority, epoch.authority());
    assert_eq!(frame.epoch_generation, epoch.generation());
    assert_eq!(
        (frame.pts_nanos, frame.start_nanos, frame.end_nanos),
        (100, 600, Some(650))
    );
    for payload in [
        SourceSubmissionPayload::Resident,
        SourceSubmissionPayload::Hap { bytes: 128 },
        SourceSubmissionPayload::Software(SoftwareUploadTrace {
            stream: Uuid::new_v4(),
            sequence: 2,
            row_pitch: 8,
            rows: 1,
            bytes: 8,
            cpu: None,
        }),
    ] {
        let row = SourceSubmissionTrace {
            stream: Uuid::new_v4(),
            token: CompletionToken {
                device: DeviceGeneration::new(Uuid::new_v4(), 1),
                serial: 3,
            },
            descriptor: descriptor(),
            frame: Some(frame),
            payload,
            observed_qpc: None,
        };
        let json = serde_json::to_string(&DiagnosticEvent::SourceSubmission(row)).unwrap();
        let DiagnosticEvent::SourceSubmission(copy) = serde_json::from_str(&json).unwrap() else {
            panic!("wrong source submission event")
        };
        assert_eq!(copy.stream, row.stream);
        assert_eq!(copy.token, row.token);
        assert_eq!(copy.frame, Some(frame));
        assert_eq!(copy.payload, payload);
        assert!(copy.observed_qpc.is_none());
    }
}

#[test]
fn source_sink_shares_existing_bounded_health_without_retaining_control_or_gpu_storage() {
    let control = D3d11Control::default();
    assert!(control.source_submission_sink().is_none());
    let recorder = Recorder::new(DiagnosticsConfig {
        capacity: NonZeroUsize::MIN,
    });
    control.install_diagnostics(Some(recorder.clone()));
    let sink = control.source_submission_sink().unwrap();
    drop(control);
    for serial in 1..=2 {
        sink.record(SourceSubmissionTrace {
            stream: Uuid::new_v4(),
            token: CompletionToken {
                device: DeviceGeneration::new(Uuid::nil(), 1),
                serial,
            },
            descriptor: descriptor(),
            frame: None,
            payload: SourceSubmissionPayload::Resident,
            observed_qpc: None,
        });
    }
    let health = sink.diagnostics_snapshot();
    assert_eq!((health.queued, health.written, health.dropped_full), (1, 1, 1));
    let mut rows = Vec::new();
    recorder.drain(&mut rows, NonZeroUsize::MIN);
    let DiagnosticEvent::SourceSubmission(row) = rows[0] else {
        panic!("missing source submission")
    };
    assert_eq!(row.token.serial, 1);
    assert!(row.frame.is_none());
    assert!(row.observed_qpc.is_some());
}
