use crate::{
    app::AppNode,
    media::rendering::{ChannelRenderer, PreviewLayout},
    preview::{
        capture::{CapturedPreview, capture_presentation},
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
    windows: Vec<golden_core::app::NativeOutputWindow>,
}
impl Drop for Owner {
    fn drop(&mut self) {
        let pictures = self.renderer.picture_retirement();
        self.renderer.close();
        while !self.renderer.retirement().wait(Duration::from_millis(20)) {}
        if let Some(pictures) = pictures {
            while !pictures.wait(Duration::from_millis(20)) {}
        }
        for window in self.windows.drain(..) {
            if let Err(error) = window.close() {
                eprintln!("[mapgyver-output] window teardown failed: {error}");
            }
        }
    }
}

struct SessionOwner(PreviewController);
impl Drop for SessionOwner {
    fn drop(&mut self) {
        self.0.close();
    }
}

fn create(
    hwnd: isize,
    extent: Extent2D,
    targets: &[super::targets::Target],
    notifier: WorkerNotifier,
) -> io::Result<(Owner, D3d11Control)> {
    let adapter = adapters()
        .map_err(io::Error::other)?
        .into_iter()
        .find(|adapter| !adapter.software)
        .ok_or_else(|| io::Error::other("no native preview adapter"))?
        .id;
    let config = super::config::configuration(Some(notifier.clone()));
    let limits = config.video.plan_limits;
    let mut outputs = vec![OutputBinding::Window {
        id: OutputId::new(1).unwrap(),
        // SAFETY: Golden retains this HWND until this task and GPU retirement finish.
        window: unsafe { WindowTarget::from_raw(hwnd).map_err(io::Error::other)? },
        extent,
    }];
    let mut windows = Vec::new();
    for target in targets {
        outputs.push(target.binding(&mut windows).map_err(io::Error::other)?);
    }
    let runtime = VideoRuntime::spawn_d3d11(
        config.video,
        D3d11Config {
            diagnostics: None,
            adapter,
            inputs: InputLimits {
                max_sources: 8,
                max_storage_bytes: 256 * 1024 * 1024,
            },
            outputs,
        },
    )
    .map_err(io::Error::other)?;
    let mut owner = Owner {
        renderer: ChannelRenderer::new(runtime, limits),
        windows,
    };
    let native = owner
        .renderer
        .native_control()
        .ok_or_else(|| io::Error::other("native preview control missing"))?;
    native.set_output_notifier(notifier.clone());
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
    pub initial_extent: Extent2D,
    pub controller: PreviewController,
    pub receiver: PreviewReceiver,
    pub viewport: Arc<Mutex<Option<HostViewport>>>,
    pub notifier: WorkerNotifier,
    pub signal: mpsc::Receiver<()>,
    pub stop: NativePresentationStop,
    pub discovery: super::discovery::Discovery,
    pub displays: super::displays::Catalog,
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
    let mut targets = super::targets::Targets::default();
    let mut topology = Vec::new();
    let mut last_error = None;
    let mut output_states = Vec::new();
    let mut accepted_outputs = std::collections::HashMap::new();
    let mut next_display_scan = Instant::now();
    let mut display_snapshot = crate::screens::protocol::DisplayCatalogSnapshot::default();
    while !input.stop.is_requested() {
        let refresh = input.displays.take_refresh();
        if refresh || Instant::now() >= next_display_scan {
            next_display_scan = Instant::now() + Duration::from_secs(1);
            if let Some(snapshot) = input.displays.update(super::displays::scan()) {
                eprintln!(
                    "[mapgyver-displays] revision={} active={} error={:?}",
                    snapshot.revision,
                    snapshot.displays.len(),
                    snapshot.error
                );
                let _ = input
                    .window
                    .emit(crate::screens::protocol::DISPLAY_CATALOG_EVENT, &snapshot);
                display_snapshot = snapshot;
            }
        }
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
        if Instant::now() >= retry {
            let result = (|| -> Result<(), String> {
                let extent = host
                    .filter(|host| host.client_width > 0 && host.client_height > 0)
                    .map(|host| Extent2D::new(host.client_width, host.client_height))
                    .transpose()
                    .map_err(|e| e.to_string())?
                    .unwrap_or(input.initial_extent);
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
                let mut changed = published != Some(layout.generation)
                    || capture.as_ref().is_none_or(|capture| !capture.is_current());
                if changed {
                    let sources = preview.sources()?;
                    let displays = display_snapshot.displays.clone();
                    let mut next = input
                        .runtime
                        .capture_app(move |engine| capture_presentation(engine, &sources, 8, 128, &displays))
                        .map_err(|e| e.to_string())??;
                    next.add_revision_guard(input.displays.guard());
                    let desired = targets.assign(&mut next)?;
                    if desired != topology {
                        // The public native contract has immutable bindings. Complete actual GPU
                        // retirement before replacing a topology, including the same HWND/name.
                        if let Some(previous) = owner.take() {
                            drop(previous);
                            eprintln!("[mapgyver-output] native session retired");
                        }
                        native = None;
                        output_states.clear();
                        accepted_outputs.clear();
                        topology = desired;
                        installed = None;
                        confirmed = None;
                    }
                    capture = Some(next);
                }
                if owner.is_none() {
                    if layout.views.is_empty() && topology.is_empty() {
                        published = Some(layout.generation);
                        return Ok(());
                    }
                    let (created, control) =
                        create(input.hwnd, extent, &topology, input.notifier.clone()).map_err(|e| e.to_string())?;
                    native = Some(control);
                    owner = Some(created);
                    changed = true;
                }
                let renderer = &mut owner.as_mut().unwrap().renderer;
                let project = input.runtime.read_model().current_project_file();
                let root = project
                    .current_path
                    .as_deref()
                    .and_then(|path| std::path::Path::new(path).parent())
                    .map(std::path::Path::to_path_buf)
                    .unwrap_or(std::env::current_dir().map_err(|e| e.to_string())?);
                changed |= renderer.set_picture_root(&root)?;
                if changed {
                    let next = capture.as_ref().ok_or("presentation capture missing")?;
                    renderer.publish_screen_targets(
                        next.channels.clone(),
                        Vec::new(),
                        next.screens.clone(),
                        Some(preview),
                    )?;
                    input.receiver.mark_uninstalled(layout.generation);
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
                            eprintln!(
                                "[mapgyver-presentation] plan installed; channels={} screens={} outputs={}",
                                renderer.channels().len(),
                                capture.as_ref().map_or(0, |capture| capture.screens.len()),
                                topology.len()
                            );
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
                let mut statuses = capture
                    .as_ref()
                    .map(|capture| capture.statuses(renderer.channels()))
                    .unwrap_or_default();
                let observations = native.as_ref().unwrap().output_snapshots();
                for output in observations
                    .iter()
                    .filter(|output| output.id != OutputId::new(1).unwrap())
                {
                    if let Some(token) = output.last_accepted
                        && accepted_outputs.get(&output.id) != Some(&token.device)
                    {
                        eprintln!(
                            "[mapgyver-output] id={} first API acceptance serial={}",
                            output.id.get(),
                            token.serial
                        );
                        accepted_outputs.insert(output.id, token.device);
                    }
                }
                let states = observations
                    .iter()
                    .filter(|output| output.id != OutputId::new(1).unwrap())
                    .map(|output| (output.id, output.state, output.generation, output.hresult))
                    .collect::<Vec<_>>();
                if states != output_states {
                    for (id, state, generation, hresult) in &states {
                        eprintln!(
                            "[mapgyver-output] id={} state={state:?} incarnation={generation} hresult={hresult:?}",
                            id.get()
                        );
                    }
                    output_states = states;
                }
                if let Some(capture) = &capture {
                    for route in capture.screens.iter().filter(|route| route.output.is_some()) {
                        if let Some(output) = observations.iter().find(|output| Some(output.id) == route.output)
                            && let Some(status) = statuses.iter_mut().find(|status| {
                                status.source
                                    == PreviewSource::Screen {
                                        screen: route.screen.id(),
                                    }
                            })
                            && !matches!(output.state, golden_video::native::OutputState::Ready)
                        {
                            status.message = Some(format!("Screen output {:?}", output.state));
                            if let Some(hresult) = output.hresult {
                                status.error = Some(format!("Screen output error 0x{:08X}", hresult as u32));
                            }
                        }
                    }
                }
                if statuses != last_status {
                    input
                        .window
                        .emit(PREVIEW_STATUS_EVENT, &statuses)
                        .map_err(|e| e.to_string())?;
                    last_status = statuses;
                }
                last_error = None;
                Ok(())
            })();
            if let Err(error) = result {
                if last_error.as_ref() != Some(&error) {
                    eprintln!("[mapgyver-presentation] {error}");
                    last_error = Some(error.clone());
                }
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
