//! Browser demand selects the same editor sources and render plans as local preview.
use crate::{
    app::AppNode,
    media::rendering::ChannelRenderer,
    preview::{
        capture::{CapturedPreview, capture_preview_with_time},
        config,
        layout::{PreviewFrameRoute, validate_calibration},
        protocol::PreviewSource,
    },
};
use golden_core::{
    application::ProductionRuntime,
    transport::{BrowserPreviewStream, BrowserPreviews},
    ui_sync::{UiBrowserPreviewDependency, UiBrowserPreviewExtent},
};
use golden_io::{BoundedWorker, ReconnectBackoff, RetirementPool, WorkerLimits, WorkerNotifier};
#[cfg(all(windows, feature = "native-video"))]
use golden_video::native::{
    AdapterId, AdapterPreference, D3d11Config, InputLimits, OutputBinding, PhysicalAdapterId, query_adapter_memory,
    select_adapter,
};
#[cfg(not(all(windows, feature = "native-video")))]
use golden_video::wgpu_backend::{ReadbackOutput, WgpuConfig, WindowOutputs};
use golden_video::{
    AlphaMode, ColorEncoding, Extent2D, OutputId, PixelFormat, TextureDescriptor,
    browser::encode_preview_jpeg,
    readback::{ReadbackFrame, readback_channel},
    runtime::{OutcomeKind, PlanIdentity, RuntimeState, VideoRuntime},
};
use std::{
    io,
    num::NonZeroUsize,
    path::Path,
    sync::{Arc, OnceLock, mpsc::Receiver},
    time::{Duration, Instant},
};

const OUTPUT_SLOTS: usize = BrowserPreviews::SOURCE_LIMIT;
const MAX_BUDGET: u64 = 768 * 1024 * 1024;

#[cfg(not(all(windows, feature = "native-video")))]
mod desktop;
#[cfg(not(all(windows, feature = "native-video")))]
use desktop::Desktop;
#[cfg(not(all(windows, feature = "native-video")))]
pub(crate) use desktop::presentation;

struct Route {
    stream: BrowserPreviewStream,
    source: PreviewSource,
}

fn routes(previews: &BrowserPreviews, generation: u64) -> Vec<Route> {
    previews
        .active(generation)
        .into_iter()
        .filter_map(|stream| {
            let source = serde_json::from_str(stream.source()).ok()?;
            #[cfg(not(all(windows, feature = "native-video")))]
            if !matches!(source, PreviewSource::Channel { .. } | PreviewSource::Screen { .. }) {
                return None;
            }
            match source {
                PreviewSource::MatrixPlayback { .. } => return None,
                PreviewSource::Calibration { mode, extent } => validate_calibration(mode, extent).ok()?,
                _ => {}
            }
            Some(Route { stream, source })
        })
        .take(OUTPUT_SLOTS)
        .collect()
}

pub(crate) fn start(
    runtime: ProductionRuntime<AppNode>,
    previews: BrowserPreviews,
) -> io::Result<BoundedWorker<(), ()>> {
    static POOL: OnceLock<RetirementPool> = OnceLock::new();
    let pool = POOL.get_or_init(|| RetirementPool::new(NonZeroUsize::new(4).unwrap()));
    start_worker(
        runtime,
        previews,
        pool,
        #[cfg(not(all(windows, feature = "native-video")))]
        None,
    )
}

fn start_worker(
    runtime: ProductionRuntime<AppNode>,
    previews: BrowserPreviews,
    pool: &RetirementPool,
    #[cfg(not(all(windows, feature = "native-video")))] mut desktop: Option<Desktop>,
) -> io::Result<BoundedWorker<(), ()>> {
    BoundedWorker::spawn(
        "mapgyver-browser-preview",
        pool,
        WorkerLimits {
            command_items: 1,
            command_weight: 1,
            event_items: 1,
            event_weight: 1,
        },
        move |context| {
            let notifier = context.wake_notifier();
            previews.set_notifier(notifier.clone());
            #[cfg(all(windows, feature = "native-video"))]
            let mut adapter = None;
            let mut owner = None;
            let mut last_error = None;
            let mut reconnect = ReconnectBackoff::new(Duration::from_millis(250), Duration::from_secs(4));
            let mut retry_at = Instant::now();
            while !context.cancellation().is_cancelled() {
                let checkpoint = context.wake_checkpoint();
                let generation = runtime.current_project_generation().get();
                let routes = routes(&previews, generation);
                let now = Instant::now();
                #[cfg(not(all(windows, feature = "native-video")))]
                let desktop_changed = match desktop
                    .as_mut()
                    .map(|desktop| desktop.refresh(&runtime, &notifier))
                    .transpose()
                {
                    Ok(changed) => changed.unwrap_or(false),
                    Err(error) => {
                        if last_error.as_ref() != Some(&error) {
                            eprintln!("[mapgyver-browser-preview] {error}");
                        }
                        last_error = Some(error);
                        context.wait_until(checkpoint, now + Duration::from_millis(250));
                        continue;
                    }
                };
                #[cfg(not(all(windows, feature = "native-video")))]
                let has_display_output = desktop.as_ref().is_some_and(Desktop::has_outputs);
                #[cfg(all(windows, feature = "native-video"))]
                let has_display_output = false;
                let next = if routes.is_empty() && !has_display_output {
                    if owner.take().is_some() {
                        eprintln!("[mapgyver-browser-preview] renderer retired");
                    }
                    reconnect.reset();
                    retry_at = now;
                    now + Duration::from_secs(10)
                } else if owner.is_none() && now < retry_at {
                    retry_at
                } else {
                    let result = (|| -> Result<(), String> {
                        if owner.is_none() {
                            #[cfg(all(windows, feature = "native-video"))]
                            let id = match adapter {
                                Some(id) => id,
                                None => {
                                    let id = preferred_adapter(&runtime)?;
                                    adapter = Some(id);
                                    id
                                }
                            };
                            owner = Some(Owner::new(
                                #[cfg(all(windows, feature = "native-video"))]
                                id,
                                #[cfg(not(all(windows, feature = "native-video")))]
                                desktop.as_ref().map(Desktop::windows),
                                notifier.clone(),
                            )?);
                        }
                        owner.as_mut().unwrap().service(
                            &runtime,
                            generation,
                            routes,
                            #[cfg(not(all(windows, feature = "native-video")))]
                            desktop.as_mut(),
                            #[cfg(not(all(windows, feature = "native-video")))]
                            desktop_changed,
                        )
                    })();
                    let error = result.err();
                    let status = owner.as_ref().map(|owner| owner.renderer.snapshot().state);
                    let next = match status {
                        None | Some(RuntimeState::Stopped) => {
                            owner.take();
                            retry_at = reconnect.schedule(now);
                            retry_at
                        }
                        state => {
                            if state == Some(RuntimeState::Running) {
                                reconnect.reset();
                            }
                            now + Duration::from_millis(if error.is_some() { 250 } else { 25 })
                        }
                    };
                    if error != last_error
                        && let Some(error) = &error
                    {
                        eprintln!("[mapgyver-browser-preview] {error}");
                    }
                    last_error = error;
                    next
                };
                context.wait_until(checkpoint, next);
            }
        },
    )
    .map_err(io::Error::other)
}

#[cfg(all(windows, feature = "native-video"))]
fn preferred_adapter(runtime: &ProductionRuntime<AppNode>) -> Result<AdapterId, String> {
    let value = runtime
        .capture_app(crate::preferences::Source::capture)
        .map_err(|error| error.to_string())??
        .value;
    let preference = if value == crate::preferences::AUTOMATIC {
        AdapterPreference::Automatic
    } else {
        AdapterPreference::Explicit(PhysicalAdapterId::from_key(&value).ok_or("invalid selected GPU identity")?)
    };
    Ok(select_adapter(preference).map_err(|error| error.to_string())?.id)
}

struct Owner {
    renderer: ChannelRenderer,
    extent: Extent2D,
    frames: Vec<Receiver<ReadbackFrame>>,
    routes: Vec<Route>,
    captured: Option<CapturedPreview>,
    project_generation: u64,
    installed: Option<Installed>,
}

struct Installed {
    plan: PlanIdentity,
    extents: Vec<Extent2D>,
    dependencies: Arc<[UiBrowserPreviewDependency]>,
}

impl Owner {
    fn new(
        #[cfg(all(windows, feature = "native-video"))] adapter: AdapterId,
        #[cfg(not(all(windows, feature = "native-video")))] windows: Option<WindowOutputs>,
        notifier: WorkerNotifier,
    ) -> Result<Self, String> {
        #[cfg(all(windows, feature = "native-video"))]
        let admission = query_adapter_memory(adapter)
            .map_err(|error| error.to_string())?
            .admission_limit(MAX_BUDGET)
            .map_err(|error| error.to_string())?;
        #[cfg(not(all(windows, feature = "native-video")))]
        let admission = MAX_BUDGET;
        #[cfg(all(windows, feature = "native-video", feature = "video-files"))]
        let (admission, video_budget) = (
            admission - admission / 4,
            golden_video::native::GpuFrameBudget::new(admission / 4),
        );
        let (width, height) = BrowserPreviews::CAPTURE_SIZE;
        let extent = Extent2D::new(width, height).unwrap();
        let rate = golden_time::RationalRate::new(15, 1).unwrap();
        let mut config = config::configuration(admission, None);
        config.video.frame_rate = rate;
        #[cfg(not(all(windows, feature = "native-video")))]
        {
            config.video.notifier = Some(notifier.clone());
        }
        let mut frames = Vec::with_capacity(OUTPUT_SLOTS);
        let outputs = (0..OUTPUT_SLOTS)
            .map(|slot| {
                let (sender, receiver) = readback_channel();
                frames.push(receiver);
                #[cfg(all(windows, feature = "native-video"))]
                {
                    OutputBinding::Readback {
                        id: output(slot),
                        extent,
                        frames: sender,
                    }
                }
                #[cfg(not(all(windows, feature = "native-video")))]
                {
                    ReadbackOutput {
                        id: output(slot),
                        extent,
                        frames: sender,
                    }
                }
            })
            .collect();
        let limits = config.video.plan_limits;
        #[cfg(all(windows, feature = "native-video"))]
        let runtime = VideoRuntime::spawn_d3d11(
            config.video,
            D3d11Config {
                diagnostics: None,
                adapter,
                outputs,
                output_rates: Vec::new(),
                inputs: InputLimits {
                    max_sources: config::MAX_NATIVE_PRODUCERS,
                    max_storage_bytes: 128 * 1024 * 1024,
                },
            },
        )
        .map_err(|error| error.to_string())?;
        #[cfg(not(all(windows, feature = "native-video")))]
        let runtime = VideoRuntime::spawn_wgpu(config.video, WgpuConfig { outputs, windows })
            .map_err(|error| error.to_string())?;
        let mut owner = Self {
            renderer: ChannelRenderer::new(runtime, limits),
            extent,
            frames,
            routes: Vec::new(),
            captured: None,
            project_generation: 0,
            installed: None,
        };
        let root = std::env::current_dir().map_err(|error| error.to_string())?;
        #[cfg(all(windows, feature = "native-video"))]
        {
            #[cfg(feature = "video-files")]
            owner
                .renderer
                .enable_hardware_videos(&root, Some(notifier.clone()), adapter, video_budget)?;
            #[cfg(target_arch = "x86_64")]
            owner.renderer.enable_ndi(Some(notifier.clone()));
            owner.renderer.enable_shaders(&root, notifier.clone())?;
        }
        #[cfg(all(feature = "video-files", not(all(windows, feature = "native-video"))))]
        owner.renderer.enable_videos(&root, Some(notifier.clone()))?;
        owner.renderer.enable_pictures(&root, config.pictures, Some(notifier))?;
        Ok(owner)
    }

    fn service(
        &mut self,
        runtime: &ProductionRuntime<AppNode>,
        generation: u64,
        routes: Vec<Route>,
        #[cfg(not(all(windows, feature = "native-video")))] desktop: Option<&mut Desktop>,
        #[cfg(not(all(windows, feature = "native-video")))] desktop_changed: bool,
    ) -> Result<(), String> {
        let mut outcomes = Vec::new();
        self.renderer
            .drain_outcomes(&mut outcomes, NonZeroUsize::new(8).unwrap());
        for outcome in outcomes {
            if Some(outcome.request) == self.renderer.latest_request() {
                match outcome.result {
                    OutcomeKind::Installed { generation, .. } => {
                        self.installed = Some(Installed {
                            plan: generation.identity,
                            extents: self.source_extents(),
                            dependencies: self.source_dependencies(),
                        });
                    }
                    OutcomeKind::Rejected(error) => {
                        self.captured = None;
                        return Err(error.to_string());
                    }
                    _ => {}
                }
            }
        }

        let project = runtime.read_model().current_project_file();
        let root = project
            .current_path
            .as_deref()
            .and_then(|path| Path::new(path).parent())
            .map(Path::to_path_buf)
            .map_or_else(std::env::current_dir, Ok)
            .map_err(|error| error.to_string())?;
        let root_changed = self.renderer.set_file_root(&root)?;
        let routes_changed = self.project_generation != generation
            || !self
                .routes
                .iter()
                .map(|route| route.stream.id())
                .eq(routes.iter().map(|route| route.stream.id()));
        #[cfg(all(windows, feature = "native-video"))]
        let desktop_changed = false;
        if desktop_changed {
            self.captured = None;
        }
        if routes_changed || root_changed || !self.captured.as_ref().is_some_and(CapturedPreview::is_current) {
            let sources = routes.iter().map(|route| route.source).collect::<Vec<_>>();
            #[cfg(not(all(windows, feature = "native-video")))]
            let sources = {
                let mut sources = sources;
                if let Some(desktop) = desktop.as_ref() {
                    sources.extend(desktop.sources());
                }
                sources
            };
            let time = self.renderer.read_time().map_err(|error| error.to_string())?;
            let capture = runtime
                .capture_app(move |engine| {
                    capture_preview_with_time(engine, &sources, config::MAX_CAPTURE_CHANNELS, 128, Some(time))
                })
                .map_err(|error| error.to_string())??;
            if runtime.current_project_generation().get() != generation {
                return Ok(());
            }
            #[cfg(not(all(windows, feature = "native-video")))]
            let capture = if let Some(desktop) = desktop {
                let mut capture = capture;
                desktop.bind(&mut capture)?;
                capture
            } else {
                capture
            };
            let outputs = routes
                .iter()
                .enumerate()
                .map(|(slot, route)| PreviewFrameRoute {
                    output: output(slot),
                    source: route.source,
                })
                .collect();
            self.renderer.publish_preview_frames(
                capture.clone(),
                outputs,
                TextureDescriptor {
                    extent: self.extent,
                    format: PixelFormat::Rgba16Float,
                    color_encoding: ColorEncoding::Linear,
                    alpha_mode: AlphaMode::Premultiplied,
                },
            )?;
            self.captured = Some(capture);
            self.routes = routes;
            self.project_generation = generation;
            if routes_changed {
                self.installed = None;
            }
        } else {
            self.renderer.refresh()?;
        }

        for (slot, frames) in self.frames.iter().enumerate() {
            let Ok(frame) = frames.try_recv() else { continue };
            let Some(installed) = self.installed.as_ref().filter(|state| state.plan == frame.plan) else {
                continue;
            };
            let Some((route, content)) = self.routes.get(slot).zip(installed.extents.get(slot).copied()) else {
                continue;
            };
            route
                .stream
                .publish_frame(browser_extent(content), &installed.dependencies, |encoding| {
                    let limit = Extent2D::new(encoding.width, encoding.height).expect("valid browser preview preset");
                    encode_preview_jpeg(&frame.pixels, self.extent, content, limit, encoding.jpeg_quality)
                })
                .map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    fn source_extents(&self) -> Vec<Extent2D> {
        self.routes
            .iter()
            .map(|route| {
                match route.source {
                    PreviewSource::Calibration { extent, .. } => Some(extent),
                    PreviewSource::Screen { screen } => self.captured.as_ref().and_then(|capture| {
                        capture
                            .screens
                            .iter()
                            .find(|route| route.screen.id() == screen)
                            .map(|route| route.extent)
                    }),
                    source => source.reference().and_then(|reference| {
                        self.renderer
                            .channels()
                            .iter()
                            .find(|channel| channel.reference == reference)
                            .and_then(|channel| channel.resolution.extent)
                    }),
                }
                .unwrap_or(self.extent)
            })
            .collect()
    }

    fn source_dependencies(&self) -> Arc<[UiBrowserPreviewDependency]> {
        let mut dependencies = self
            .renderer
            .channels()
            .iter()
            .filter_map(|channel| {
                let extent = browser_extent(channel.resolution.extent?);
                let source = serde_json::to_string(&PreviewSource::Channel {
                    reference: channel.reference,
                })
                .ok()?;
                Some(UiBrowserPreviewDependency { source, extent })
            })
            .collect::<Vec<_>>();
        dependencies.sort_unstable_by(|left, right| left.source.cmp(&right.source));
        dependencies.into()
    }
}

fn browser_extent(extent: Extent2D) -> UiBrowserPreviewExtent {
    UiBrowserPreviewExtent {
        width: extent.width(),
        height: extent.height(),
    }
}

impl Drop for Owner {
    fn drop(&mut self) {
        let pictures = self.renderer.picture_retirement();
        #[cfg(feature = "video-files")]
        let videos = self.renderer.video_retirement();
        self.renderer.close();
        let delay = Duration::from_millis(20);
        while !self.renderer.retirement().wait(delay) {}
        if let Some(pictures) = pictures {
            while !pictures.wait(delay) {}
        }
        #[cfg(feature = "video-files")]
        if let Some(videos) = videos {
            while !videos.wait(delay) {}
        }
    }
}

fn output(slot: usize) -> OutputId {
    OutputId::new(slot as u64 + 1).unwrap()
}
