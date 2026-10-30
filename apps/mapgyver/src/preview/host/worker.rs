use crate::{
    app::AppNode,
    media::rendering::{ChannelRenderer, PreviewLayout},
    preview::{
        capture::{CapturedPreview, capture_preview},
        protocol::{PREVIEW_STATUS_EVENT, PreviewSource, PreviewStatus},
    },
};
use golden_core::{app::NativePresentationStop, application::ProductionRuntime};
use golden_io::WorkerNotifier;
use golden_video::native::{D3d11Config, D3d11Control, InputLimits, OutputBinding, WindowTarget, adapters};
use golden_video::preview::{HostViewport, PreviewController, PreviewReceiver};
use golden_video::runtime::{CompletionToken, GenerationInfo, OutcomeKind, VideoRuntime};
use golden_video::{AlphaMode, ColorEncoding, Extent2D, OutputId, PixelFormat, TextureDescriptor};
use std::{
    io,
    num::NonZeroUsize,
    sync::{Arc, Mutex, mpsc},
    time::{Duration, Instant},
};
use tauri::Emitter;

struct Owner {
    renderer: ChannelRenderer,
}
impl Drop for Owner {
    fn drop(&mut self) {
        let pictures = self.renderer.picture_retirement();
        self.renderer.close();
        while !self.renderer.retirement().wait(Duration::from_millis(20)) {}
        if let Some(pictures) = pictures {
            while !pictures.wait(Duration::from_millis(20)) {}
        }
    }
}

struct SessionOwner(PreviewController);
impl Drop for SessionOwner {
    fn drop(&mut self) {
        self.0.close();
    }
}

fn create(hwnd: isize, extent: Extent2D, notifier: WorkerNotifier) -> io::Result<(Owner, D3d11Control)> {
    let adapter = adapters()
        .map_err(io::Error::other)?
        .into_iter()
        .find(|adapter| !adapter.software)
        .ok_or_else(|| io::Error::other("no native preview adapter"))?
        .id;
    let config = super::config::configuration(Some(notifier.clone()));
    let limits = config.video.plan_limits;
    let runtime = VideoRuntime::spawn_d3d11(
        config.video,
        D3d11Config {
            diagnostics: None,
            adapter,
            inputs: InputLimits {
                max_sources: 8,
                max_storage_bytes: 256 * 1024 * 1024,
            },
            outputs: vec![OutputBinding::Window {
                id: OutputId::new(1).unwrap(),
                // SAFETY: Golden retains this HWND until this task and GPU retirement finish.
                window: unsafe { WindowTarget::from_raw(hwnd).map_err(io::Error::other)? },
                extent,
            }],
        },
    )
    .map_err(io::Error::other)?;
    let mut owner = Owner {
        renderer: ChannelRenderer::new(runtime, limits),
    };
    let native = owner
        .renderer
        .native_control()
        .ok_or_else(|| io::Error::other("native preview control missing"))?;
    owner
        .renderer
        .enable_pictures(&std::env::current_dir()?, config.pictures, Some(notifier))
        .map_err(io::Error::other)?;
    Ok((owner, native))
}

pub(super) struct Input {
    pub runtime: ProductionRuntime<AppNode>,
    pub window: tauri::WebviewWindow,
    pub hwnd: isize,
    pub controller: PreviewController,
    pub receiver: PreviewReceiver,
    pub viewport: Arc<Mutex<Option<HostViewport>>>,
    pub notifier: WorkerNotifier,
    pub signal: mpsc::Receiver<()>,
    pub stop: NativePresentationStop,
    pub discovery: super::discovery::Discovery,
}

pub(super) fn run(input: Input) -> io::Result<()> {
    let _session = SessionOwner(input.controller.clone());
    let mut owner: Option<Owner> = None;
    let mut native: Option<D3d11Control> = None;
    let mut capture: Option<CapturedPreview> = None;
    let mut published = None;
    let mut installed: Option<(u64, GenerationInfo, Option<CompletionToken>)> = None;
    let mut last_status = Vec::new();
    let mut confirmed = None;
    let mut retry = Instant::now();
    let mut discovery = None;
    while !input.stop.is_requested() {
        if let Some(revision) = input.discovery.take() {
            let result = (|| {
                if discovery.is_none() {
                    discovery = Some(golden_video::native::SpoutDiscovery::new().map_err(|error| error.to_string())?);
                }
                let catalog = discovery
                    .as_mut()
                    .unwrap()
                    .snapshot(NonZeroUsize::new(super::discovery::MAX_NAMES).unwrap())
                    .map_err(|error| error.to_string())?;
                Ok((catalog.names, catalog.truncated))
            })();
            if result.is_err() {
                discovery = None;
            }
            if let Some(snapshot) = input.discovery.complete(revision, result) {
                let _ = input
                    .window
                    .emit(crate::media::protocol::SENDER_DISCOVERY_EVENT, snapshot);
            }
        }
        let layout = input.receiver.snapshot();
        let host = *input
            .viewport
            .lock()
            .map_err(|_| io::Error::other("preview viewport lock poisoned"))?;
        if let Some(host) =
            host.filter(|host| host.client_width > 0 && host.client_height > 0 && Instant::now() >= retry)
        {
            let result = (|| -> Result<(), String> {
                let extent = Extent2D::new(host.client_width, host.client_height).map_err(|e| e.to_string())?;
                if owner.is_none() {
                    if layout.views.is_empty() || Instant::now() < retry {
                        return Ok(());
                    }
                    let (created, control) =
                        create(input.hwnd, extent, input.notifier.clone()).map_err(|e| e.to_string())?;
                    native = Some(control);
                    owner = Some(created);
                }
                let renderer = &mut owner.as_mut().unwrap().renderer;
                let project = input.runtime.read_model().current_project_file();
                let root = project
                    .current_path
                    .as_deref()
                    .and_then(|path| std::path::Path::new(path).parent())
                    .map(std::path::Path::to_path_buf)
                    .unwrap_or(std::env::current_dir().map_err(|e| e.to_string())?);
                let root_changed = renderer.set_picture_root(&root)?;
                let preview = PreviewLayout {
                    output: OutputId::new(1).unwrap(),
                    descriptor: TextureDescriptor {
                        extent,
                        format: PixelFormat::Rgba8Unorm,
                        color_encoding: ColorEncoding::Linear,
                        alpha_mode: AlphaMode::Premultiplied,
                    },
                    layout: layout.clone(),
                };
                if root_changed
                    || published != Some(layout.generation)
                    || capture.as_ref().is_none_or(|capture| !capture.is_current())
                {
                    let sources = preview.sources()?;
                    let next = input
                        .runtime
                        .capture_app(move |engine| capture_preview(engine, &sources, 8, 128))
                        .map_err(|e| e.to_string())??;
                    renderer.publish_screen_targets(
                        next.channels.clone(),
                        Vec::new(),
                        next.screens.clone(),
                        Some(preview),
                    )?;
                    input.receiver.mark_uninstalled(layout.generation);
                    capture = Some(next);
                    published = Some(layout.generation);
                    installed = None;
                    confirmed = None;
                } else {
                    renderer.refresh()?;
                }
                let mut outcomes = Vec::new();
                renderer.drain_outcomes(&mut outcomes, NonZeroUsize::new(8).unwrap());
                for outcome in outcomes {
                    match outcome.result {
                        OutcomeKind::Installed { generation, .. }
                            if Some(outcome.request) == renderer.latest_request() =>
                        {
                            installed = Some((layout.generation, generation, None));
                        }
                        OutcomeKind::Rejected(failure) if Some(outcome.request) == renderer.latest_request() => {
                            return Err(failure.to_string());
                        }
                        _ => {}
                    }
                }
                if let Some((generation, plan, first)) = &mut installed
                    && let Some(frame) = renderer
                        .snapshot()
                        .last_frame
                        .filter(|frame| frame.identity == plan.identity)
                {
                    let first = first.get_or_insert(frame.completion);
                    let accepted = native
                        .as_ref()
                        .unwrap()
                        .output_snapshots()
                        .iter()
                        .find(|out| out.id == OutputId::new(1).unwrap())
                        .and_then(|out| out.last_accepted)
                        .is_some_and(|done| done.device == first.device && done.serial >= first.serial);
                    if accepted && input.receiver.mark_installed(*generation) && confirmed != Some(*generation) {
                        eprintln!(
                            "[mapgyver-preview] native layout {generation} installed; channels={}",
                            renderer.channels().len()
                        );
                        confirmed = Some(*generation);
                    }
                }
                let statuses = capture
                    .as_ref()
                    .map(|capture| capture.statuses(renderer.channels()))
                    .unwrap_or_default();
                if statuses != last_status {
                    input
                        .window
                        .emit(PREVIEW_STATUS_EVENT, &statuses)
                        .map_err(|e| e.to_string())?;
                    last_status = statuses;
                }
                Ok(())
            })();
            if let Err(error) = result {
                input.receiver.mark_uninstalled(layout.generation);
                let statuses = layout
                    .views
                    .iter()
                    .filter_map(|view| serde_json::from_str::<PreviewSource>(&view.source).ok())
                    .map(|source| PreviewStatus {
                        source,
                        extent: None,
                        error: Some(error.clone()),
                        message: None,
                    })
                    .collect::<Vec<_>>();
                if statuses != last_status {
                    eprintln!("[mapgyver-preview] {error}");
                    let _ = input.window.emit(PREVIEW_STATUS_EVENT, &statuses);
                    last_status = statuses;
                }
                published = None;
                retry = Instant::now() + Duration::from_millis(500);
            }
        }
        // Coarse worker wake catches authored guard invalidation; the actor is contacted only
        // when metadata changed. File/GPU/geometry completions also wake this worker directly.
        let _ = input.signal.recv_timeout(Duration::from_millis(100));
    }
    input.controller.close();
    drop(owner);
    Ok(())
}
