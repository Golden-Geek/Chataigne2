use std::collections::{BTreeMap, HashMap};
use std::num::NonZeroUsize;
use std::time::Duration;

use golden_time::RationalRate;

use super::{Captures, FixtureConsumer, consumer, descriptor, diagnostics};
use crate::native::{AdapterId, D3d11Config, D3d11Control, DiagnosticEvent, InputLimits, OutputBinding};
use crate::plan::{ExternalTexture, PlanLimits, PlanOutput, RenderOperation, RenderPass, RenderPlan, SampleFilter};
use crate::plan::{SpoutFrameCount, SpoutSource};
use crate::runtime::{LatePolicy, PlanActivation, ResourceLimits, RuntimeConfig, RuntimeState, VideoRuntime};
use crate::{AlphaMode, ColorEncoding, OutputId, PassId, PixelFormat, TextureId};

pub(super) struct RuntimeCapture {
    runtime: Option<VideoRuntime>,
    control: D3d11Control,
    consumer: FixtureConsumer,
    tapped: bool,
}

impl RuntimeCapture {
    pub fn new(adapter: AdapterId, sender: &str, tapped: bool) -> Self {
        let name = format!("{sender}-out");
        let bytes = descriptor().byte_len().unwrap();
        let config = RuntimeConfig {
            plan_limits: PlanLimits {
                max_passes: 1,
                max_external_textures: 1,
                max_outputs: 1,
                max_dependencies: 4,
                max_texture_dimension: 3840,
                max_texture_bytes: bytes * 3,
            },
            resource_limits: ResourceLimits {
                max_bytes: 512 * 1024 * 1024,
                max_in_flight: NonZeroUsize::new(2).unwrap(),
                max_retiring: 2,
            },
            frame_rate: RationalRate::new(60, 1).unwrap(),
            max_outstanding: NonZeroUsize::new(4).unwrap(),
            max_input_bytes: NonZeroUsize::new(65536).unwrap(),
            max_retained_input_bytes: NonZeroUsize::new(262144).unwrap(),
            recovery_delay: Duration::from_millis(100),
            max_recovery_delay: Duration::from_secs(1),
            notifier: None,
        };
        let control = D3d11Control::default();
        if tapped {
            control.copy_tap.enable();
        }
        let mut runtime = VideoRuntime::spawn_d3d11_with_control(
            config,
            D3d11Config {
                diagnostics: Some(diagnostics()),
                adapter,
                inputs: InputLimits {
                    max_sources: 1,
                    max_storage_bytes: InputLimits::source_storage_bytes(descriptor()).unwrap(),
                },
                outputs: vec![OutputBinding::Spout {
                    id: OutputId::new(1).unwrap(),
                    name: name.clone(),
                    descriptor: descriptor(),
                }],
            },
            control.clone(),
        )
        .unwrap();
        let input = TextureId::new(1).unwrap();
        let output = TextureId::new(2).unwrap();
        let internal = crate::TextureDescriptor {
            format: PixelFormat::Rgba8Unorm,
            color_encoding: ColorEncoding::Linear,
            alpha_mode: AlphaMode::Premultiplied,
            ..descriptor()
        };
        let plan = RenderPlan {
            external_textures: vec![ExternalTexture {
                id: input,
                descriptor: descriptor(),
                source: Some(
                    SpoutSource {
                        sender: sender.into(),
                        descriptor: descriptor(),
                        poll_interval: Duration::from_millis(1),
                        frame_count: SpoutFrameCount::VersionedV1,
                    }
                    .into(),
                ),
            }],
            passes: vec![RenderPass {
                id: PassId::new(1).unwrap(),
                output,
                descriptor: internal,
                operation: RenderOperation::Sample {
                    source: input,
                    filter: SampleFilter::Nearest,
                },
            }],
            outputs: vec![PlanOutput {
                id: OutputId::new(1).unwrap(),
                source: output,
            }],
        };
        runtime
            .submit_plan(
                runtime.project_epoch(),
                plan,
                PlanActivation::NextFrame(runtime.read_time().unwrap().epoch()),
                LatePolicy::Activate,
            )
            .unwrap();
        Self {
            runtime: Some(runtime),
            control,
            consumer: consumer(adapter, &name),
            tapped,
        }
    }

    pub fn tick(&mut self) {
        self.consumer.tick().unwrap();
    }
    pub fn len(&self) -> usize {
        self.consumer.snapshot().logged_frames as usize
    }

    pub fn finish(&mut self) -> Captures {
        let frames = self.consumer.finish().unwrap();
        let runtime = self.runtime.take().unwrap();
        let retirement = runtime.retirement();
        runtime.close();
        assert!(
            retirement.wait(Duration::from_secs(6)),
            "runtime did not actually retire"
        );
        let snapshot = runtime.snapshot();
        assert_eq!(snapshot.state, RuntimeState::Stopped);
        assert_eq!(snapshot.resources.unwrap().bytes_used, 0);
        let mut rows = Vec::new();
        self.control
            .drain_diagnostics(&mut rows, NonZeroUsize::new(4096).unwrap());
        assert_eq!(self.control.diagnostics_snapshot().dropped_full, 0);
        let sources = rows
            .iter()
            .filter_map(|row| match row {
                DiagnosticEvent::SourceUse(row) => Some((
                    (row.token.device, row.token.serial),
                    row.publication.and_then(|p| p.spout_image),
                )),
                _ => None,
            })
            .collect::<HashMap<_, _>>();
        let mut images = BTreeMap::new();
        let taps = (if self.tapped {
            self.control.copy_tap.take()
        } else {
            Vec::new()
        })
        .into_iter()
        .map(|frame| ((frame.token.device, frame.token.serial), frame.words))
        .collect::<HashMap<_, _>>();
        let mut stages = BTreeMap::new();
        for row in rows {
            if let DiagnosticEvent::Output(output) = row
                && let Some(image) = output.spout_image
            {
                let source = *sources.get(&(output.token.device, output.token.serial)).unwrap();
                assert!(
                    images.insert((image.shared_handle, image.frame), source).is_none(),
                    "ambiguous output copy version"
                );
                if self.tapped {
                    stages.insert(
                        (image.shared_handle, image.frame),
                        *taps.get(&(output.token.device, output.token.serial)).unwrap(),
                    );
                }
            }
        }
        Captures {
            frames,
            source_images: Some(images),
            stages: self.tapped.then_some(stages),
        }
    }
}

impl Drop for RuntimeCapture {
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            let retirement = runtime.retirement();
            runtime.close();
            // Timeout cannot authorize native cleanup. The process watchdog supplies the bound.
            while !retirement.wait(Duration::from_millis(20)) {}
        }
    }
}
