use super::*;
use crate::media::video::protocol::VideoStepDirection;
use golden_video::native::{
    D3d11Config, DiagnosticEvent, DiagnosticsConfig, GpuFrameBudget, InputLimits, OutputBinding,
    SourceSubmissionPayload, adapters,
};
use golden_video::plan::ExternalSource;
use golden_video::{AlphaMode, ColorEncoding, PixelFormat, TextureDescriptor};
use std::collections::HashMap;

#[test]
#[ignore = "explicit Intel native File source submission/channel/PTS attribution"]
fn hardware_intel_file_submissions_attribute_each_channel_before_renderer_sampling() {
    let adapter = adapters()
        .unwrap()
        .into_iter()
        .find(|adapter| adapter.vendor_id == 0x8086 && !adapter.software)
        .unwrap();
    let mut engine = engine();
    let references = ["h264.mp4", "prores.mov", "hap.mov"].map(|name| source(&mut engine, name));
    for reference in references {
        edit(
            &mut engine,
            MediaEdit::ConfigureMediaResolution {
                media: reference.media,
                size: SizePolicy::Fixed {
                    extent: Extent2D::new(16, 16).unwrap(),
                },
            },
        );
    }
    let output = TextureDescriptor {
        extent: Extent2D::new(16, 16).unwrap(),
        format: PixelFormat::Bgra8Unorm,
        color_encoding: ColorEncoding::Srgb,
        alpha_mode: AlphaMode::Opaque,
    };
    let config = config();
    let limits = config.plan_limits;
    let run = uuid::Uuid::new_v4();
    let runtime = VideoRuntime::spawn_d3d11(
        config,
        D3d11Config {
            adapter: adapter.id,
            diagnostics: Some(DiagnosticsConfig {
                capacity: NonZeroUsize::new(4096).unwrap(),
            }),
            inputs: InputLimits {
                max_sources: 8,
                max_storage_bytes: 64 * 1024 * 1024,
            },
            output_rates: vec![],
            outputs: (1..=3)
                .map(|index| OutputBinding::Spout {
                    id: OutputId::new(index).unwrap(),
                    name: format!("MapGyver submission {run} {index}"),
                    descriptor: output,
                })
                .collect(),
        },
    )
    .unwrap();
    let mut renderer = ChannelRenderer::new(runtime, limits);
    let native = renderer.native_control().unwrap();
    assert!(
        native.source_submission_sink().is_some(),
        "available before backend startup"
    );
    let budget = GpuFrameBudget::new_with_upload_accounting(64 * 1024 * 1024);
    renderer
        .enable_hardware_videos(&fixture_root(), None, adapter.id, budget.clone())
        .unwrap();
    renderer
        .publish(
            capture_channels(&engine, &references, 8).unwrap(),
            references
                .iter()
                .enumerate()
                .map(|(index, reference)| ChannelRoute {
                    output: OutputId::new(index as u64 + 1).unwrap(),
                    channel: *reference,
                })
                .collect(),
        )
        .unwrap();
    let mut bindings = HashMap::new();
    for (index, reference) in references.into_iter().enumerate() {
        steps::at(&mut engine, reference, 0.0);
        wait(|| renderer.videos.as_ref().unwrap().source(reference).is_some());
        let (_, ExternalSource::GpuFrames(stream)) = renderer.videos.as_ref().unwrap().source(reference).unwrap()
        else {
            panic!("expected native source publication")
        };
        assert!(bindings.insert(stream.id(), index).is_none());
        let next = steps::step(&mut engine, reference, VideoStepDirection::Next);
        steps::at(&mut engine, reference, next);
    }
    let retirement = renderer.video_retirement().unwrap();
    close(renderer);
    assert!(retirement.wait(Duration::from_secs(5)));
    assert_eq!(retirement.active_workers(), 0);
    assert_eq!(retirement.retained_bytes(), 0);
    assert_eq!(budget.retained_bytes(), 0);
    let mut events = Vec::new();
    native.drain_diagnostics(&mut events, NonZeroUsize::new(4096).unwrap());
    let mut counts = [0; 3];
    let mut positive_pts = [false; 3];
    let mut software_bytes = 0;
    let mut hap_bytes = 0;
    for event in events {
        let DiagnosticEvent::SourceSubmission(row) = event else {
            continue;
        };
        let index = bindings[&row.stream];
        let frame = row
            .frame
            .expect("actual File worker must provide decoded PTS/epoch/window");
        counts[index] += 1;
        positive_pts[index] |= frame.pts_nanos > 0;
        assert!(row.observed_qpc.is_some());
        assert_eq!(frame.start_nanos, frame.pts_nanos, "paused fixture has no loop offset");
        match (index, row.payload) {
            (0, SourceSubmissionPayload::Resident) => {}
            (1, SourceSubmissionPayload::Software(upload)) => software_bytes += upload.bytes,
            (2, SourceSubmissionPayload::Hap { bytes }) => hap_bytes += bytes,
            _ => panic!("payload attributed to the wrong File channel"),
        }
    }
    assert_eq!(counts, [2; 3]);
    assert_eq!(positive_pts, [true; 3]);
    let uploads = budget.snapshot().uploads.unwrap();
    assert_eq!((uploads.software.calls, uploads.software.bytes), (2, software_bytes));
    assert_eq!((uploads.hap.calls, uploads.hap.bytes), (2, hap_bytes));
    let health = native.diagnostics_snapshot();
    assert_eq!((health.queued, health.dropped_full, health.clock_failures), (0, 0, 0));
    eprintln!("Intel File source submissions: {counts:?}; RGBA={software_bytes}, HAP={hap_bytes}, retired=0");
}
