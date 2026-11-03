use super::raster as oracle;
use golden_io::PayloadPool;
use golden_video::Extent2D;
use golden_video::native::{AdapterId, D3d11Config, InputLimits, OutputBinding, SpoutProbe};
use golden_video::plan::PlanLimits;
use golden_video::preview::protocol::*;
use golden_video::preview::{HostViewport, PreviewController, PreviewLimits, PreviewReceiver};
use golden_video::runtime::{OutcomeKind, ResourceLimits, RuntimeConfig, VideoRuntime};
use golden_video::{AlphaMode, ColorEncoding, OutputId, PixelFormat, TextureDescriptor};
use mapgyver::media::{
    capture_channels_at,
    rendering::{ChannelRenderer, ChannelRoute, PreviewLayout},
};
use mapgyver::{app::AppEngine, media::protocol::ChannelReference};
use std::num::NonZeroUsize;
use std::time::{Duration, Instant};

const WAIT: Duration = Duration::from_secs(8);

pub struct Fixture {
    renderer: Option<ChannelRenderer>,
    adapter: AdapterId,
    identity_logged: bool,
    names: Vec<String>,
    preview: PreviewController,
    receiver: PreviewReceiver,
    session: uuid::Uuid,
    revision: u32,
    pub extent: Extent2D,
}

impl Fixture {
    pub fn new(adapter: AdapterId) -> Self {
        Self::with_outputs(adapter, 2)
    }
    pub fn with_outputs(adapter: AdapterId, routes: usize) -> Self {
        let extent = Extent2D::new(32, 24).unwrap();
        let names = (0..=routes)
            .map(|i| format!("MapGyverRaster-{}-{i}", uuid::Uuid::new_v4()))
            .collect::<Vec<_>>();
        let limits = PlanLimits {
            max_passes: 16,
            max_external_textures: 0,
            max_outputs: routes + 1,
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
            adapter,
            identity_logged: false,
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
        self.publish_channels(engine, &[root, root], root, Some(5));
    }
    pub fn renderer(&mut self) -> &mut ChannelRenderer {
        self.renderer.as_mut().unwrap()
    }
    pub fn sender(&self, index: usize) -> &str {
        &self.names[index]
    }
    pub fn publish_channels(
        &mut self,
        engine: &AppEngine,
        routes: &[ChannelReference],
        root: ChannelReference,
        expected_produced_targets: Option<usize>,
    ) {
        assert_eq!(routes.len() + 1, self.names.len());
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
                        overlays: Vec::new(),
                        overlay_capacity: 0,
                    }],
                },
                Self::viewport(self.extent)
            ),
            PreviewResponse::Applied { visible_views: 1, .. }
        ));
        let preview = PreviewLayout {
            output: OutputId::new(self.names.len() as u64).unwrap(),
            descriptor: TextureDescriptor {
                format: PixelFormat::Rgba8Unorm,
                color_encoding: ColorEncoding::Linear,
                alpha_mode: AlphaMode::Premultiplied,
                ..Self::descriptor(self.extent)
            },
            layout: self.receiver.snapshot(),
        };
        let live = self.renderer.as_mut().unwrap();
        let mut roots = routes.to_vec();
        roots.push(root);
        let capture = capture_channels_at(engine, &roots, 64, live.read_time().unwrap()).unwrap();
        let mut request = live
            .publish_targets(
                capture,
                routes
                    .iter()
                    .enumerate()
                    .map(|(i, channel)| ChannelRoute {
                        output: OutputId::new(i as u64 + 1).unwrap(),
                        channel: *channel,
                    })
                    .collect(),
                Some(preview),
            )
            .unwrap();
        let deadline = Instant::now() + WAIT;
        let mut installed = None;
        let mut first_frame = None;
        loop {
            live.refresh().unwrap();
            let latest = live.latest_request().unwrap();
            if request != latest {
                request = latest;
                installed = None;
                first_frame = None;
            }
            let mut outcomes = Vec::new();
            live.drain_outcomes(&mut outcomes, NonZeroUsize::new(16).unwrap());
            for outcome in outcomes {
                if outcome.request == request {
                    match outcome.result {
                        OutcomeKind::Installed { generation, .. } => {
                            installed = Some(generation.identity);
                            first_frame = None;
                        }
                        OutcomeKind::Rejected(error) => assert_eq!(
                            error.code(),
                            golden_video::runtime::FailureCode::StaleSource,
                            "native raster rejected: {error:?}"
                        ),
                        _ => {}
                    }
                }
            }
            if let Some(frame) = live.snapshot().last_frame
                && Some(frame.identity) == installed
            {
                let token = *first_frame.get_or_insert(frame.completion);
                let outputs = live.native_control().unwrap().output_snapshots();
                if outputs.len() == self.names.len()
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
        let actual = live.native_control().unwrap().snapshot();
        assert!(actual.initialized && actual.device.is_some());
        assert_eq!(actual.adapter, Some(self.adapter));
        if !self.identity_logged {
            eprintln!("Raster native adapter={:?} device={:?}", actual.adapter, actual.device);
            self.identity_logged = true;
        }
        let targets = live
            .channels()
            .iter()
            .map(|channel| channel.target)
            .collect::<std::collections::HashSet<_>>();
        // Automatic References add channel records but reuse their producer's target.
        // Retain the original production limit rather than charging virtual aliases as renders.
        if let Some(expected) = expected_produced_targets {
            assert_eq!(targets.len(), expected, "distinct produced channel target count");
        }
    }

    pub fn pixels(&self, adapter: AdapterId, expected: &oracle::Image, label: &str) {
        self.pixels_channels(adapter, &[expected, expected], expected, label);
    }
    pub fn pixels_channels(
        &self,
        adapter: AdapterId,
        channels: &[&oracle::Image],
        expected: &oracle::Image,
        label: &str,
    ) {
        assert_eq!(channels.len() + 1, self.names.len());
        let expected = expected.resized(self.extent.width() as usize, self.extent.height() as usize);
        let outputs = channels
            .iter()
            .map(|image| {
                image
                    .resized(self.extent.width() as usize, self.extent.height() as usize)
                    .output()
            })
            .collect::<Vec<_>>();
        let preview = expected.preview();
        let preview_bounds = expected.preview_bounds();
        let output_bounds = channels
            .iter()
            .map(|image| {
                image
                    .resized(self.extent.width() as usize, self.extent.height() as usize)
                    .output_bounds()
            })
            .collect::<Vec<_>>();
        let budget = PayloadPool::new("raster-finite-readback", self.names.len(), 1024 * 1024).unwrap();
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
            let is_preview = i == channels.len();
            let wanted = if is_preview { &preview } else { &outputs[i] };
            assert_eq!(image.pixels().len(), wanted.len());
            let mut maximum = 0;
            let mut maximum_nominal = 0;
            for (offset, (actual, expected)) in image.pixels().iter().zip(wanted).enumerate() {
                let nominal = actual.iter().zip(expected).map(|(a, b)| a.abs_diff(*b)).max().unwrap();
                maximum_nominal = maximum_nominal.max(nominal);
                let (low, high) = if is_preview {
                    preview_bounds[offset]
                } else {
                    output_bounds[i][offset]
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
                    "Raster {label}/{i} texel {offset}: {actual:?}, nominal {expected:?}, legal [{low:?},{high:?}], outside error {error}>2/255"
                );
            }
            eprintln!(
                "Raster {label} output={i} extent={:?} texels={} max_error={maximum}/255 nominal_error={maximum_nominal}/255",
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
