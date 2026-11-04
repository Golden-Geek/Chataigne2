//! Bounded live-sender isolation using the existing source owner and telemetry.
use super::*;
use crate::native::timing::{DiagnosticEvent, DiagnosticsConfig, Recorder};
use crate::native::{D3d11Control, InputLimits, adapters};
use crate::plan::{ExternalTexture, PlanLimits, RenderPlan, SpoutFrameCount, SpoutMetadata, SpoutSource};
use std::num::NonZeroUsize;
use std::time::{Duration, Instant};

#[test]
#[ignore = "explicit live sender cadence isolation; set GOLDEN_LIVE_SPOUT_SENDER"]
fn hardware_live_unversioned_source_cadence() {
    let name = std::env::var("GOLDEN_LIVE_SPOUT_SENDER").expect("exact live sender name");
    let descriptor = TextureDescriptor {
        extent: Extent2D::new(8192, 8192).unwrap(),
        format: PixelFormat::Bgra8Unorm,
        color_encoding: ColorEncoding::Srgb,
        alpha_mode: AlphaMode::Straight,
    };
    let id = TextureId::new(1).unwrap();
    let plan = RenderPlan {
        external_textures: vec![ExternalTexture {
            id,
            descriptor: TextureDescriptor {
                extent: Extent2D::new(1, 1).unwrap(),
                ..descriptor
            },
            source: Some(
                SpoutSource {
                    sender: name,
                    descriptor,
                    metadata: SpoutMetadata::Discover,
                    poll_interval: Duration::from_millis(16),
                    frame_count: SpoutFrameCount::Unversioned,
                }
                .into(),
            ),
        }],
        ..Default::default()
    }
    .compile_with_source_demands(
        &PlanLimits {
            max_passes: 0,
            max_external_textures: 1,
            max_outputs: 0,
            max_dependencies: 0,
            max_texture_dimension: 8192,
            max_texture_bytes: 512 * 1024 * 1024,
        },
        &[id],
    )
    .unwrap();
    let mut streaming = false;
    for adapter in adapters().unwrap().into_iter().filter(|adapter| !adapter.software) {
        let control = D3d11Control::default();
        let recorder = Recorder::new(DiagnosticsConfig {
            capacity: NonZeroUsize::new(4096).unwrap(),
        });
        *control.diagnostics.lock().unwrap() = Some(recorder.clone());
        let mut pool = SourcePool::new(
            adapter.id,
            InputLimits {
                max_sources: 1,
                max_storage_bytes: 192 * 1024 * 1024,
            },
            &control,
            None,
            DeviceGeneration::new(Uuid::new_v4(), 0),
        );
        pool.ensure(&plan).unwrap();
        let shared = pool.inputs[&id].shared.clone();
        let demand = shared.acquire_demand();
        let started = Instant::now();
        while started.elapsed() < Duration::from_secs(3) {
            pool.retire(0);
            std::thread::sleep(Duration::from_millis(10));
        }
        let state = *shared.status.lock().unwrap();
        streaming |= state.receive_copies > 0;
        eprintln!(
            "LIVE source={} adapter={} elapsed={:?} state={state:?}",
            plan.external_textures()[0].id.get(),
            adapter.description,
            started.elapsed()
        );
        let mut events = Vec::new();
        recorder.drain(&mut events, NonZeroUsize::new(4096).unwrap());
        let source_events = events
            .into_iter()
            .filter(|event| matches!(event, DiagnosticEvent::SourceCpu(_)))
            .collect::<Vec<_>>();
        eprintln!(
            "LIVE source_cpu_events={} first={}",
            source_events.len(),
            source_events
                .first()
                .map(|event| serde_json::to_string(event).unwrap())
                .unwrap_or_default()
        );
        if let Some(descriptor) = state.observed_descriptor {
            use crate::native::{completion::EventCompletion, device::Device, texture::Texture};
            use crate::runtime::CompletionToken;
            let device = Device::new(adapter.id).unwrap();
            let target = Texture::new(&device.device, descriptor).unwrap();
            let mut completion = EventCompletion::new(&device).unwrap();
            let mut previous = CopyFrame::default();
            let mut serial = 0;
            let consumed = Instant::now();
            while consumed.elapsed() < Duration::from_secs(3) {
                let completed = completion.completed(&device, pool.device).unwrap();
                pool.retire(completed.serial);
                if completion.can_submit() {
                    serial += 1;
                    let _submission = completion.begin(
                        &device,
                        CompletionToken {
                            device: pool.device,
                            serial,
                        },
                    );
                    if let Some(frame) = pool.copy(id, &device, &target, serial, previous).unwrap() {
                        previous = frame;
                    }
                }
                std::thread::sleep(Duration::from_millis(16));
            }
            while completion.completed(&device, pool.device).unwrap().serial < serial {
                std::thread::sleep(Duration::from_millis(1));
            }
            pool.retire(serial);
            eprintln!(
                "LIVE with_renderer adapter={} elapsed={:?} renderer_submissions={serial} last_sequence={} state={:?}",
                adapter.description,
                consumed.elapsed(),
                previous.sequence,
                *shared.status.lock().unwrap()
            );
        }
        drop(demand);
        drop(pool);
    }
    assert!(streaming, "no compatible live sender was copied");
}
