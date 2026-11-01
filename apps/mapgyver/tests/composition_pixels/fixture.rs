use super::*;
use golden_io::PayloadPool;
use golden_video::native::{AdapterId, D3d11Config, InputLimits, OutputBinding, SpoutProbe};
use golden_video::plan::PlanLimits;
use golden_video::preview::protocol::*;
use golden_video::preview::{HostViewport, PreviewController, PreviewLimits, PreviewReceiver};
use golden_video::runtime::{OutcomeKind, ResourceLimits, RuntimeConfig, VideoRuntime};
use golden_video::{AlphaMode, ColorEncoding, OutputId, PixelFormat, TextureDescriptor};
use mapgyver::media::{
    capture_channels,
    rendering::{ChannelRenderer, ChannelRoute, PreviewLayout},
};
use std::num::NonZeroUsize;

const WAIT: Duration = Duration::from_secs(8);

pub struct Fixture {
    renderer: Option<ChannelRenderer>,
    names: [String; 3],
    preview: PreviewController,
    receiver: PreviewReceiver,
    session: uuid::Uuid,
    revision: u32,
    pub extent: Extent2D,
}

impl Fixture {
    pub fn new(adapter: AdapterId) -> Self {
        let extent = Extent2D::new(32, 24).unwrap();
        let names = std::array::from_fn(|i| format!("MapGyverComposition-{}-{i}", uuid::Uuid::new_v4()));
        let limits = PlanLimits {
            max_passes: 16,
            max_external_textures: 0,
            max_outputs: 3,
            max_dependencies: 128,
            max_texture_dimension: 4096,
            max_texture_bytes: 64 * 1024 * 1024,
        };
        let runtime = VideoRuntime::spawn_d3d11(
            RuntimeConfig {
                plan_limits: limits,
                resource_limits: ResourceLimits {
                    max_bytes: 64 * 1024 * 1024,
                    max_in_flight: NonZeroUsize::new(2).unwrap(),
                    max_retiring: 2,
                },
                frame_rate: golden_time::RationalRate::new(60, 1).unwrap(),
                max_outstanding: NonZeroUsize::new(8).unwrap(),
                max_input_bytes: NonZeroUsize::new(65536).unwrap(),
                max_retained_input_bytes: NonZeroUsize::new(262144).unwrap(),
                recovery_delay: Duration::from_millis(50),
                max_recovery_delay: Duration::from_secs(1),
                notifier: None,
            },
            D3d11Config {
                adapter,
                diagnostics: None,
                inputs: InputLimits {
                    max_sources: 0,
                    max_storage_bytes: 0,
                },
                outputs: names
                    .iter()
                    .enumerate()
                    .map(|(i, name)| OutputBinding::Spout {
                        id: OutputId::new(i as u64 + 1).unwrap(),
                        name: name.clone(),
                        descriptor: Self::descriptor(extent),
                    })
                    .collect(),
            },
        )
        .unwrap();
        let renderer = ChannelRenderer::new(runtime, limits);
        let deadline = Instant::now() + WAIT;
        while renderer.snapshot().resources.is_none() {
            assert!(Instant::now() < deadline, "Composition renderer startup timeout");
            std::thread::sleep(Duration::from_millis(5));
        }
        let (preview, receiver) = PreviewController::new(
            PreviewLimits {
                lease: Duration::from_secs(60),
                ..Default::default()
            },
            None,
        )
        .unwrap();
        let PreviewResponse::Connected { session, .. } = preview.handle(
            PreviewRequest::Connect {
                client: uuid::Uuid::new_v4(),
                version: PREVIEW_PROTOCOL_VERSION,
            },
            Self::viewport(extent),
        ) else {
            panic!("preview connect rejected")
        };
        Self {
            renderer: Some(renderer),
            names,
            preview,
            receiver,
            session,
            revision: 0,
            extent,
        }
    }

    fn descriptor(extent: Extent2D) -> TextureDescriptor {
        TextureDescriptor {
            extent,
            format: PixelFormat::Bgra8Unorm,
            color_encoding: ColorEncoding::Srgb,
            alpha_mode: AlphaMode::Straight,
        }
    }
    fn viewport(extent: Extent2D) -> HostViewport {
        HostViewport {
            client_width: extent.width(),
            client_height: extent.height(),
            webview_origin_x: 0,
            webview_origin_y: 0,
            webview_width: extent.width(),
            webview_height: extent.height(),
            css_scale: 1.0,
        }
    }
    pub fn publish(&mut self, engine: &AppEngine, root: ChannelReference) {
        self.revision += 1;
        let rect = CssRect {
            x: 0.0,
            y: 0.0,
            width: self.extent.width() as f64,
            height: self.extent.height() as f64,
        };
        assert!(matches!(
            self.preview.handle(
                PreviewRequest::Layout {
                    session: self.session,
                    revision: self.revision,
                    views: vec![PreviewView {
                        id: uuid::Uuid::new_v4(),
                        source: serde_json::to_string(&mapgyver::preview::protocol::PreviewSource::Channel {
                            reference: root
                        })
                        .unwrap(),
                        rect,
                        clip: rect,
                        visible: true,
                    }],
                },
                Self::viewport(self.extent)
            ),
            PreviewResponse::Applied { visible_views: 1, .. }
        ));
        let preview = PreviewLayout {
            output: OutputId::new(3).unwrap(),
            descriptor: TextureDescriptor {
                format: PixelFormat::Rgba8Unorm,
                color_encoding: ColorEncoding::Linear,
                alpha_mode: AlphaMode::Premultiplied,
                ..Self::descriptor(self.extent)
            },
            layout: self.receiver.snapshot(),
        };
        let live = self.renderer.as_mut().unwrap();
        let request = live
            .publish_targets(
                capture_channels(engine, &[root], 16).unwrap(),
                [1, 2]
                    .map(|id| ChannelRoute {
                        output: OutputId::new(id).unwrap(),
                        channel: root,
                    })
                    .to_vec(),
                Some(preview),
            )
            .unwrap();
        let deadline = Instant::now() + WAIT;
        let mut installed = None;
        let mut first_frame = None;
        loop {
            live.refresh().unwrap();
            let mut outcomes = Vec::new();
            live.drain_outcomes(&mut outcomes, NonZeroUsize::new(16).unwrap());
            for outcome in outcomes {
                if outcome.request == request {
                    match outcome.result {
                        OutcomeKind::Installed { generation, .. } => installed = Some(generation.identity),
                        OutcomeKind::Rejected(error) => panic!("Composition rejected: {error:?}"),
                        _ => {}
                    }
                }
            }
            if let Some(frame) = live.snapshot().last_frame
                && Some(frame.identity) == installed
            {
                let token = *first_frame.get_or_insert(frame.completion);
                let outputs = live.native_control().unwrap().output_snapshots();
                if outputs.len() == 3
                    && outputs.iter().all(|output| {
                        output
                            .last_accepted
                            .is_some_and(|accepted| accepted.device == token.device && accepted.serial >= token.serial)
                    })
                {
                    break;
                }
            }
            assert!(
                Instant::now() < deadline,
                "Composition publication timeout: {:?}",
                live.channels()
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(
            live.channels().len(),
            5,
            "shared inputs and three consumers retain five channel productions"
        );
        let targets = live
            .channels()
            .iter()
            .map(|channel| channel.target)
            .collect::<std::collections::HashSet<_>>();
        assert_eq!(targets.len(), 5);
    }

    pub fn pixels(&self, adapter: AdapterId, expected: &oracle::Image, label: &str) {
        let expected = expected.resized(self.extent.width() as usize, self.extent.height() as usize);
        let output = expected.output();
        let preview = expected.preview();
        let preview_bounds = expected.preview_bounds();
        let budget = PayloadPool::new("composition-finite-readback", 3, 1024 * 1024).unwrap();
        for (i, name) in self.names.iter().enumerate() {
            let mut probe = SpoutProbe::new(name, adapter, budget.clone()).unwrap();
            let deadline = Instant::now() + WAIT;
            let image = loop {
                if let Some(image) = probe.capture_pixels().unwrap() {
                    break image;
                }
                assert!(Instant::now() < deadline, "Composition readback timeout {label}/{i}");
                std::thread::sleep(Duration::from_millis(5));
            };
            assert_eq!(image.extent(), self.extent);
            let wanted = if i == 2 { &preview } else { &output };
            assert_eq!(image.pixels().len(), wanted.len());
            let mut maximum = 0;
            let mut maximum_nominal = 0;
            for (offset, (actual, expected)) in image.pixels().iter().zip(wanted).enumerate() {
                let nominal = actual.iter().zip(expected).map(|(a, b)| a.abs_diff(*b)).max().unwrap();
                maximum_nominal = maximum_nominal.max(nominal);
                let (low, high) = if i == 2 {
                    preview_bounds[offset]
                } else {
                    (*expected, *expected)
                };
                let error = actual
                    .iter()
                    .enumerate()
                    .map(|(c, a)| low[c].saturating_sub(*a).max(a.saturating_sub(high[c])))
                    .max()
                    .unwrap();
                maximum = maximum.max(error);
                assert!(
                    error <= 2,
                    "Composition {label}/{i} texel {offset}: {actual:?}, nominal {expected:?}, legal [{low:?},{high:?}], outside error {error}>2/255"
                );
            }
            eprintln!(
                "Composition {label} output={i} extent={:?} texels={} max_error={maximum}/255 nominal_error={maximum_nominal}/255",
                self.extent,
                wanted.len()
            );
        }
        assert!(budget.wait_for_idle(Duration::from_secs(1)));
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.preview.close();
        if let Some(mut renderer) = self.renderer.take() {
            let control = renderer.native_control().unwrap().clone();
            let retired = renderer.retirement();
            renderer.close();
            drop(renderer);
            while !retired.wait(Duration::from_millis(20)) {}
            let final_inventory = control.allocations_snapshot();
            assert_eq!(final_inventory.plan.live, 0);
            assert_eq!(final_inventory.plan.created, final_inventory.plan.released);
            assert!(final_inventory.outputs.is_empty());
            assert!(final_inventory.sources.is_empty());
        }
    }
}
