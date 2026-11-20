//! MapGyver routes browser Channel demand through the same render plans as local preview.
use crate::{
    app::AppNode,
    media::{
        CapturedChannels, capture_channels_at,
        protocol::ChannelReference,
        rendering::{ChannelRenderer, ChannelRoute},
    },
    preview::{config, protocol::PreviewSource},
};
use golden_core::{
    application::ProductionRuntime,
    transport::{BrowserPreviewStream, BrowserPreviews},
};
use golden_io::{BoundedWorker, RetirementPool, WorkerLimits, WorkerNotifier};
use golden_video::{
    Extent2D, OutputId,
    browser::encode_preview_jpeg,
    native::{
        AdapterId, AdapterPreference, D3d11Config, InputLimits, OutputBinding, PhysicalAdapterId, ReadbackFrame,
        query_adapter_memory, readback_channel, select_adapter,
    },
    runtime::{OutcomeKind, PlanIdentity, VideoRuntime},
};
use std::{
    io,
    num::NonZeroUsize,
    path::Path,
    sync::{OnceLock, mpsc::Receiver},
    time::{Duration, Instant},
};

const OUTPUT_SLOTS: usize = BrowserPreviews::SOURCE_LIMIT;
const MAX_BUDGET: u64 = 768 * 1024 * 1024;

struct Route {
    stream: BrowserPreviewStream,
    reference: ChannelReference,
}

fn routes(previews: &BrowserPreviews, generation: u64) -> Vec<Route> {
    previews
        .active(generation)
        .into_iter()
        .filter_map(|stream| {
            let PreviewSource::Channel { reference } = serde_json::from_str(stream.source()).ok()? else {
                return None;
            };
            Some(Route { stream, reference })
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
            let mut adapter = None;
            let mut owner = None;
            let mut last_error = None;
            while !context.cancellation().is_cancelled() {
                let checkpoint = context.wake_checkpoint();
                let generation = runtime.current_project_generation().get();
                let routes = routes(&previews, generation);
                let interval = if routes.is_empty() {
                    if owner.take().is_some() {
                        eprintln!("[mapgyver-browser-preview] renderer retired");
                    }
                    Duration::from_secs(10)
                } else {
                    let result = (|| -> Result<(), String> {
                        if owner.is_none() {
                            let id = match adapter {
                                Some(id) => id,
                                None => {
                                    let id = preferred_adapter(&runtime)?;
                                    adapter = Some(id);
                                    id
                                }
                            };
                            owner = Some(Owner::new(id, notifier.clone())?);
                        }
                        owner.as_mut().unwrap().service(&runtime, generation, routes)
                    })();
                    let error = result.err();
                    let interval = Duration::from_millis(if error.is_some() { 250 } else { 25 });
                    if error != last_error
                        && let Some(error) = &error
                    {
                        eprintln!("[mapgyver-browser-preview] {error}");
                    }
                    last_error = error;
                    interval
                };
                context.wait_until(checkpoint, Instant::now() + interval);
            }
        },
    )
    .map_err(io::Error::other)
}

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
    captured: Option<CapturedChannels>,
    project_generation: u64,
    installed: Option<PlanIdentity>,
}

impl Owner {
    fn new(adapter: AdapterId, notifier: WorkerNotifier) -> Result<Self, String> {
        let admission = query_adapter_memory(adapter)
            .map_err(|error| error.to_string())?
            .admission_limit(MAX_BUDGET)
            .map_err(|error| error.to_string())?;
        #[cfg(feature = "video-files")]
        let (admission, video_budget) = (
            admission - admission / 4,
            golden_video::native::GpuFrameBudget::new(admission / 4),
        );
        let (width, height) = BrowserPreviews::CAPTURE_SIZE;
        let extent = Extent2D::new(width, height).unwrap();
        let rate = golden_time::RationalRate::new(15, 1).unwrap();
        let mut config = config::configuration(admission, None);
        config.video.frame_rate = rate;
        let mut frames = Vec::with_capacity(OUTPUT_SLOTS);
        let outputs = (0..OUTPUT_SLOTS)
            .map(|slot| {
                let (sender, receiver) = readback_channel();
                frames.push(receiver);
                OutputBinding::Readback {
                    id: output(slot),
                    extent,
                    frames: sender,
                }
            })
            .collect();
        let limits = config.video.plan_limits;
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
        #[cfg(feature = "video-files")]
        owner
            .renderer
            .enable_hardware_videos(&root, Some(notifier.clone()), adapter, video_budget)?;
        #[cfg(target_arch = "x86_64")]
        owner.renderer.enable_ndi(Some(notifier.clone()));
        owner.renderer.enable_shaders(&root, notifier.clone())?;
        owner.renderer.enable_pictures(&root, config.pictures, Some(notifier))?;
        Ok(owner)
    }

    fn service(
        &mut self,
        runtime: &ProductionRuntime<AppNode>,
        generation: u64,
        routes: Vec<Route>,
    ) -> Result<(), String> {
        let mut outcomes = Vec::new();
        self.renderer
            .drain_outcomes(&mut outcomes, NonZeroUsize::new(8).unwrap());
        for outcome in outcomes {
            if Some(outcome.request) == self.renderer.latest_request() {
                match outcome.result {
                    OutcomeKind::Installed { generation, .. } => self.installed = Some(generation.identity),
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
        if routes_changed || root_changed || !self.captured.as_ref().is_some_and(CapturedChannels::is_current) {
            let references = routes.iter().map(|route| route.reference).collect::<Vec<_>>();
            let time = self.renderer.read_time().map_err(|error| error.to_string())?;
            let capture = runtime
                .capture_app(move |engine| capture_channels_at(engine, &references, config::MAX_CAPTURE_CHANNELS, time))
                .map_err(|error| error.to_string())??;
            if runtime.current_project_generation().get() != generation {
                return Ok(());
            }
            let outputs = routes
                .iter()
                .enumerate()
                .map(|(slot, route)| ChannelRoute {
                    output: output(slot),
                    channel: route.reference,
                })
                .collect();
            self.renderer.publish(capture.clone(), outputs)?;
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
            let Some(route) = self.routes.get(slot).filter(|_| self.installed == Some(frame.plan)) else {
                continue;
            };
            let content = self
                .renderer
                .channels()
                .iter()
                .find(|channel| channel.reference == route.reference)
                .and_then(|channel| channel.resolution.extent)
                .unwrap_or(self.extent);
            route
                .stream
                .publish_frame(|encoding| {
                    let limit = Extent2D::new(encoding.width, encoding.height).expect("valid browser preview preset");
                    encode_preview_jpeg(&frame.pixels, self.extent, content, limit, encoding.jpeg_quality)
                })
                .map_err(|error| error.to_string())?;
        }
        Ok(())
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
