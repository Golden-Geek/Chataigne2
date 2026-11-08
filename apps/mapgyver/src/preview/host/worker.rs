use crate::{
    app::AppNode,
    media::rendering::{ChannelRenderer, PreviewLayout},
    preview::{
        capture::{CapturedPreview, capture_presentation_at, capture_presentation_with_time},
        protocol::{PREVIEW_STATUS_EVENT, PreviewSource, PreviewStatus},
    },
};
use golden_core::{app::NativePresentationStop, application::ProductionRuntime};
use golden_io::WorkerNotifier;
use golden_video::native::{
    AdapterInfo, D3d11Config, D3d11Control, InputLimits, OutputBinding, WindowTarget, query_adapter_memory,
};
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
    gpu: String,
    windows: Vec<(OutputId, Arc<super::output_windows::WindowLease>)>,
    escapes: Vec<(super::targets::Target, Arc<std::sync::atomic::AtomicBool>)>,
}
impl Drop for Owner {
    fn drop(&mut self) {
        let pictures = self.renderer.picture_retirement();
        self.renderer.close();
        while !self.renderer.retirement().wait(Duration::from_millis(20)) {}
        if let Some(pictures) = pictures {
            while !pictures.wait(Duration::from_millis(20)) {}
        }
        self.windows.clear();
    }
}

impl Owner {
    fn process_input(&self, runtime: &ProductionRuntime<AppNode>) {
        use crate::screens::protocol::{SCREEN_NAMESPACE, SCREEN_OPERATION, ScreenEdit};
        use golden_core::application::ProjectTransactions;
        for (target, pressed) in &self.escapes {
            if !pressed.swap(false, std::sync::atomic::Ordering::AcqRel) {
                continue;
            }
            let edit = ScreenEdit::DisableOutput {
                project: target.project,
                screen: target.screen,
                target: target.authored.clone(),
            };
            let result = serde_json::to_value(edit)
                .map_err(|error| error.to_string())
                .and_then(|payload| {
                    runtime
                        .apply_transaction(golden_core::ui_sync::UiEditIntent::InvokeAppOperation {
                            namespace: SCREEN_NAMESPACE.into(),
                            operation: SCREEN_OPERATION.into(),
                            payload,
                        })
                        .map_err(|error| error.to_string())
                });
            if let Err(error) = result {
                eprintln!("[mapgyver-output] Escape request ignored: {error}");
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
    adapter: &AdapterInfo,
    hwnd: isize,
    extent: Extent2D,
    targets: &[super::targets::Target],
    output_windows: &mut super::output_windows::OutputWindows,
    notifier: WorkerNotifier,
) -> io::Result<(Owner, D3d11Control)> {
    // A renderer recreation follows complete old-owner retirement. Query this process on
    // the selected adapter; unknown/zero budgets fail truthfully and retry on this worker.
    let admission = query_adapter_memory(adapter.id)
        .map_err(io::Error::other)?
        .admission_limit(u64::MAX)
        .map_err(io::Error::other)?;
    let config = crate::preview::config::configuration(admission, Some(notifier.clone()));
    let limits = config.video.plan_limits;
    let mut outputs = vec![OutputBinding::Window {
        id: OutputId::new(1).unwrap(),
        // SAFETY: Golden retains this HWND until this task and GPU retirement finish.
        window: unsafe { WindowTarget::from_raw(hwnd).map_err(io::Error::other)? },
        extent,
    }];
    let mut windows = Vec::new();
    for target in targets {
        let (binding, lease) = output_windows
            .binding(target, notifier.clone())
            .map_err(io::Error::other)?;
        outputs.push(binding);
        if let Some(lease) = lease {
            windows.push((target.id, lease));
        }
    }
    let runtime = VideoRuntime::spawn_d3d11(
        config.video,
        D3d11Config {
            output_rates: std::iter::once((
                OutputId::new(1).unwrap(),
                golden_time::RationalRate::new(60, 1).unwrap(),
            ))
            .chain(targets.iter().map(|target| (target.id, target.frame_rate)))
            .collect(),
            diagnostics: None,
            adapter: adapter.id,
            inputs: InputLimits {
                max_sources: crate::preview::config::MAX_NATIVE_PRODUCERS,
                max_storage_bytes: 256 * 1024 * 1024,
            },
            outputs,
        },
    )
    .map_err(io::Error::other)?;
    let mut owner = Owner {
        renderer: ChannelRenderer::new(runtime, limits),
        gpu: adapter.description.clone(),
        escapes: super::output_windows::escape_targets(targets, &windows),
        windows,
    };
    let native = owner
        .renderer
        .native_control()
        .ok_or_else(|| io::Error::other("native preview control missing"))?;
    native.set_output_notifier(notifier.clone());
    #[cfg(feature = "video-files")]
    owner
        .renderer
        .enable_videos(&std::env::current_dir()?, Some(notifier.clone()))
        .map_err(io::Error::other)?;
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
    pub status: Arc<crate::preview::runtime_status::StatusView>,
}

pub(super) fn run(input: Input) -> io::Result<()> {
    let _session = SessionOwner(input.controller.clone());
    #[cfg(feature = "video-files")]
    let _videos = crate::media::video::metadata::Service::start(input.runtime.clone())?;
    let mut owner: Option<Owner> = None;
    // Pin even automatic selection for this presentation task. Recreating outputs must not
    // silently reconnect existing shared textures on a different GPU.
    let mut adapter = super::adapters::Selection::default();
    let mut status_sampler = crate::preview::runtime_status::StatusSampler::default();
    let mut output_sampler = super::output_status::OutputSampler::default();
    let mut native: Option<D3d11Control> = None;
    let mut capture: Option<CapturedPreview> = None;
    let mut _capture_subscriptions = Vec::new();
    let mut published = None;
    let mut installed: Option<(u64, GenerationInfo, Option<CompletionToken>)> = None;
    let mut observation_events = super::observations::Observations::default();
    let mut confirmed = None;
    let mut retry = Instant::now();
    let mut discovery = None;
    let mut sender_catalog = crate::media::sender_catalog::native::Catalog::default();
    let mut targets = super::targets::Targets::default();
    let mut topology = Vec::new();
    let mut output_windows = super::output_windows::OutputWindows::default();
    let mut last_error = None;
    let mut output_states = Vec::new();
    let mut accepted_outputs = std::collections::HashMap::new();
    let mut next_display_scan = Instant::now();
    let mut display_snapshot = crate::screens::protocol::DisplayCatalogSnapshot::default();
    let mut display_source: Option<crate::screens::catalog_runtime::Source> = None;
    let mut _display_subscription = None;
    let mut display_options = crate::screens::catalog_runtime::display_options(&[]);
    let mut display_published = None;
    let mut display_retry = Instant::now();
    let mut applications = crate::applications::driver::ApplicationDriver::default();
    let mut application_retry = Instant::now();
    let mut application_error = None;
    let mut application_observer: Option<(uuid::Uuid, crate::applications::runtime::ApplicationSubscription)> = None;
    let mut observation_retry = Instant::now();
    let mut callback_retry = observation_retry;
    let mut supervision_retry = observation_retry;
    let mut supervisor: Option<crate::applications::runtime::supervision::Supervisor> = None;
    let mut application_callbacks: Option<(
        uuid::Uuid,
        crate::applications::runtime::ApplicationCallbackSubscription,
    )> = None;
    while !input.stop.is_requested() {
        input.receiver.set_host_suspended(input.stop.is_ui_suspended());
        adapter.service(&input.runtime);
        sender_catalog.service(&input.runtime);
        if let Some(owner) = &owner {
            owner.process_input(&input.runtime);
        }
        if supervisor.as_ref().is_some_and(|owner| !owner.is_current()) {
            supervisor.take();
        }
        if supervisor.is_none() && Instant::now() >= supervision_retry {
            match input
                .runtime
                .capture_app(crate::applications::runtime::supervision::Source::capture)
                .map_err(|error| error.to_string())
                .and_then(|source| source)
                .and_then(|source| source.claim(crate::applications::protocol::ApplicationHost::Desktop))
            {
                Ok(owner) => supervisor = Some(owner),
                Err(error) => {
                    eprintln!("[mapgyver-applications] supervision: {error}");
                    supervision_retry = Instant::now() + Duration::from_secs(1);
                }
            }
        }
        if supervisor.is_some() && applications.needs_service() && Instant::now() >= application_retry {
            let result = input
                .runtime
                .capture_app(applications.service_capture())
                .map_err(|error| error.to_string())
                .and_then(|capture| capture);
            let result = result.and_then(|prepared| {
                supervisor.as_ref().unwrap().validate_project(prepared.project())?;
                applications.apply_prepared(prepared, input.notifier.clone())
            });
            if let Some(owner) = &supervisor {
                owner
                    .publisher()
                    .demand(result.as_ref().map(|_| ()).map_err(Clone::clone));
            }
            match result {
                Ok(_) => {
                    application_error = None;
                }
                Err(error) => {
                    if application_error.as_ref() != Some(&error) {
                        eprintln!("[mapgyver-applications] {error}");
                    }
                    application_error = Some(error);
                    application_retry = Instant::now() + Duration::from_millis(500);
                }
            }
        }
        let callback_source = supervisor
            .as_ref()
            .map(|owner| owner.callbacks())
            .filter(|source| source.is_current());
        if callback_source.as_ref().map(|source| source.project())
            != application_callbacks.as_ref().map(|(project, _)| *project)
        {
            application_callbacks.take();
        }
        if let Some(source) = callback_source
            && application_callbacks.is_none()
            && Instant::now() >= callback_retry
        {
            let runtime = input.runtime.clone();
            let publisher = supervisor.as_ref().unwrap().publisher();
            let attempt = publisher.callback_attempt();
            let delivery = publisher.clone();
            match source.start(move |ticket| {
                use golden_core::application::ProjectTransactions;
                let result = runtime
                    .apply_transaction(golden_core::ui_sync::UiEditIntent::InvokeAppOperation {
                        namespace: crate::media::protocol::MEDIA_NAMESPACE.into(),
                        operation: crate::media::protocol::MEDIA_OPERATION.into(),
                        payload: serde_json::to_value(crate::media::protocol::MediaEdit::Applications {
                            edit: crate::applications::protocol::ApplicationEdit::DispatchCallbacks { ticket },
                        })
                        .map_err(|error| error.to_string())?,
                    })
                    .map_err(|error| error.to_string())
                    .and_then(|ack| {
                        if ack.success {
                            Ok(())
                        } else {
                            Err(format!("lifecycle callback delivery rejected: {ack:?}"))
                        }
                    });
                delivery.callbacks(result.clone());
                result
            }) {
                Ok(subscription) => {
                    application_callbacks = Some((source.project(), subscription));
                    publisher.callbacks_started(attempt);
                }
                Err(error) => {
                    publisher.callbacks(Err(error.clone()));
                    eprintln!("[mapgyver-applications] callbacks unavailable: {error}");
                    callback_retry = Instant::now() + Duration::from_secs(1);
                }
            }
        }
        let observation_source = supervisor
            .as_ref()
            .map(|owner| owner.observations())
            .filter(|source| source.is_current());
        if observation_source.as_ref().map(|source| source.project())
            != application_observer.as_ref().map(|(project, _)| *project)
        {
            application_observer.take();
        }
        if let Some(source) = observation_source
            && application_observer
                .as_ref()
                .is_none_or(|(project, _)| *project != source.project())
            && Instant::now() >= observation_retry
        {
            let window = input.window.clone();
            match source.subscribe(move |snapshot| {
                let _ = window.emit(crate::media::protocol::APPLICATION_RUNTIME_EVENT, snapshot);
            }) {
                Ok(subscription) => application_observer = Some((source.project(), subscription)),
                Err(error) => {
                    eprintln!("[mapgyver-applications] observation unavailable: {error}");
                    observation_retry = Instant::now() + Duration::from_secs(1);
                }
            }
        }
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
                display_options = crate::screens::catalog_runtime::display_options(&display_snapshot.displays);
            }
        }
        let source_changed = display_source
            .as_ref()
            .is_none_or(|source| source.publisher.is_closed() || !source.guard.is_current());
        if Instant::now() >= display_retry && (source_changed || display_published != Some(display_snapshot.revision)) {
            let result = (|| -> Result<(), String> {
                if source_changed {
                    let source = input
                        .runtime
                        .capture_app(crate::screens::catalog_runtime::Source::capture)
                        .map_err(|error| error.to_string())??;
                    _display_subscription = Some(source.guard.subscribe(input.notifier.clone()));
                    display_source = Some(source);
                }
                let source = display_source.as_ref().ok_or("display metadata source missing")?;
                source
                    .publisher
                    .publish(source.prepare(display_snapshot.displays.clone().into(), &display_options))?;
                display_published = Some(display_snapshot.revision);
                Ok(())
            })();
            if let Err(error) = result {
                eprintln!("[mapgyver-displays] metadata delivery: {error}");
                display_retry = Instant::now() + Duration::from_secs(1);
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
                        format: PixelFormat::Rgba16Float,
                        color_encoding: ColorEncoding::Linear,
                        alpha_mode: AlphaMode::Premultiplied,
                    },
                    layout: layout.clone(),
                };
                if capture
                    .as_ref()
                    .is_some_and(|capture| capture.allocation_pending && capture.is_current())
                {
                    return Ok(());
                }
                let mut changed = published != Some(layout.routing_generation)
                    || capture.as_ref().is_none_or(|capture| !capture.is_current());
                if changed {
                    let time = owner
                        .as_ref()
                        .map(|owner| owner.renderer.read_time())
                        .transpose()
                        .map_err(|e| e.to_string())?;
                    let sources = preview.sources()?;
                    let displays = display_snapshot.displays.clone();
                    let mut next = input
                        .runtime
                        .capture_app(move |engine| {
                            capture_presentation_with_time(
                                engine,
                                &sources,
                                crate::preview::config::MAX_CAPTURE_CHANNELS,
                                128,
                                &displays,
                                time,
                            )
                        })
                        .map_err(|e| e.to_string())??;
                    next.add_revision_guard(input.displays.guard());
                    if next.allocation_pending {
                        // Pending display metadata is not target removal. Retain the visible
                        // HWND until the allocation commits; its revision wakes this capture.
                        _capture_subscriptions = next.subscribe(input.notifier.clone());
                        capture = Some(next);
                        return Ok(());
                    }
                    let desired = targets.assign(&mut next)?;
                    if desired != topology {
                        if !super::targets::same_bindings(&topology, &desired) {
                            // Immutable GPU bindings retire completely before new backing storage.
                            // Physical window leases remain visible across the replacement.
                            if let Some(previous) = owner.take() {
                                drop(previous);
                                eprintln!("[mapgyver-output] native session retired");
                            }
                            output_windows.retain(&desired);
                            native = None;
                            output_states.clear();
                            accepted_outputs.clear();
                            installed = None;
                            confirmed = None;
                        }
                        output_windows.update_bounds(&desired)?;
                        if let Some(owner) = &mut owner {
                            owner.escapes = super::output_windows::escape_targets(&desired, &owner.windows);
                        }
                        topology = desired;
                    }
                    _capture_subscriptions = next.subscribe(input.notifier.clone());
                    capture = Some(next);
                }
                if owner.is_none() {
                    if layout.views.is_empty() && topology.is_empty() {
                        let next = capture.as_ref().ok_or("presentation capture missing")?;
                        sync_applications(
                            &input,
                            &mut applications,
                            &supervisor,
                            next.channels.project,
                            next.channels.application_demand(),
                            next.channels.guards.clone(),
                        )?;
                        published = Some(layout.routing_generation);
                        observation_events.publish(layout.generation, vec![], |statuses| {
                            input
                                .window
                                .emit(PREVIEW_STATUS_EVENT, statuses)
                                .map_err(|error| error.to_string())
                        })?;
                        return Ok(());
                    }
                    let (created, control) = create(
                        adapter.adapter()?,
                        input.hwnd,
                        extent,
                        &topology,
                        &mut output_windows,
                        input.notifier.clone(),
                    )
                    .map_err(|e| e.to_string())?;
                    native = Some(control);
                    owner = Some(created);
                    // Topology discovery precedes runtime construction. Recapture timed demand
                    // against the newly created authority before publishing its first frame.
                    let time = owner
                        .as_ref()
                        .unwrap()
                        .renderer
                        .read_time()
                        .map_err(|e| e.to_string())?;
                    let sources = preview.sources()?;
                    let displays = display_snapshot.displays.clone();
                    let mut next = input
                        .runtime
                        .capture_app(move |engine| {
                            capture_presentation_at(
                                engine,
                                &sources,
                                crate::preview::config::MAX_CAPTURE_CHANNELS,
                                128,
                                &displays,
                                time,
                            )
                        })
                        .map_err(|e| e.to_string())??;
                    next.add_revision_guard(input.displays.guard());
                    if targets.assign(&mut next)? != topology {
                        return Err("output topology changed during time binding; recapture required".into());
                    }
                    _capture_subscriptions = next.subscribe(input.notifier.clone());
                    capture = Some(next);
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
                changed |= renderer.set_file_root(&root)?;
                if changed {
                    let next = capture.as_ref().ok_or("presentation capture missing")?;
                    let project = next.channels.project;
                    let demand = next.channels.application_demand();
                    if let Some(retainer) = sync_applications(
                        &input,
                        &mut applications,
                        &supervisor,
                        project,
                        demand,
                        next.channels.guards.clone(),
                    )? {
                        renderer.bind_applications(retainer);
                    }
                    renderer.publish_screen_targets(
                        next.channels.clone(),
                        Vec::new(),
                        next.screens.clone(),
                        Some(preview),
                    )?;
                    input.receiver.mark_uninstalled(layout.generation);
                    published = Some(layout.routing_generation);
                    installed = None;
                    confirmed = None;
                } else {
                    renderer.refresh()?;
                }
                let project = capture.as_ref().ok_or("presentation capture missing")?.channels.project;
                let demand = renderer.application_demand();
                if applications.needs_sync(project, &demand)
                    && let Some(retainer) = sync_applications(
                        &input,
                        &mut applications,
                        &supervisor,
                        project,
                        demand,
                        capture.as_ref().unwrap().channels.guards.clone(),
                    )?
                {
                    renderer.bind_applications(retainer);
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
                            installed = Some((layout.routing_generation, generation, None));
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
                        .filter(|done| done.device == first.device && done.serial >= first.serial);
                    if accepted
                        .and_then(|accepted| input.receiver.mark_frame_installed(*generation, accepted))
                        .is_some()
                        && confirmed != Some(*generation)
                    {
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
                for (reference, matrix) in renderer.matrix_statuses() {
                    if let Some(status) = statuses
                        .iter_mut()
                        .find(|status| status.source == PreviewSource::Channel { reference })
                    {
                        status.matrix = Some(matrix);
                    }
                }
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
                // A newly registered/reconnected preview needs the current snapshot even when
                // output metadata is unchanged; frame progression alone still emits nothing.
                observation_events.publish(layout.generation, statuses, |statuses| {
                    input
                        .window
                        .emit(PREVIEW_STATUS_EVENT, statuses)
                        .map_err(|error| error.to_string())
                })?;
                last_error = None;
                Ok(())
            })();
            if let Err(error) = result {
                if !layout.guard.is_current()
                    || capture.as_ref().is_some_and(|capture| !capture.is_current())
                    || supervisor.as_ref().is_some_and(|owner| !owner.is_current())
                {
                    // A newer edit/layout superseded preparation. Keep the displayed frame and
                    // recapture immediately instead of exposing a transient error for 500 ms.
                    // Discard the obsolete metadata so a stable recapture failure cannot keep
                    // inheriting its stale guard and bypassing the normal recovery backoff.
                    // New/Load can return a current capture after revoking its previous
                    // application supervisor; that old owner's admission failure is stale too.
                    capture = None;
                    _capture_subscriptions.clear();
                    published = None;
                    retry = Instant::now();
                    continue;
                }
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
                        matrix: None,
                        source,
                        extent: None,
                        error: Some(error.clone()),
                        message: None,
                    })
                    .collect::<Vec<_>>();
                let _ = observation_events.publish(layout.generation, statuses, |statuses| {
                    input
                        .window
                        .emit(PREVIEW_STATUS_EVENT, statuses)
                        .map_err(|error| error.to_string())
                });
                published = None;
                retry = Instant::now() + Duration::from_millis(500);
            }
        }
        let now = Instant::now();
        if status_sampler.due(now) {
            let snapshot = owner.as_ref().map(|owner| owner.renderer.snapshot());
            let observed = snapshot
                .as_ref()
                .zip(owner.as_ref())
                .map(|(snapshot, owner)| (snapshot, owner.gpu.as_str()));
            if let Some(mut status) = status_sampler.sample(now, observed, last_error.as_deref()) {
                adapter.decorate(&mut status);
                status.gpu_adapter = native
                    .as_ref()
                    .and_then(|native| native.snapshot().adapter)
                    .map(|adapter| adapter.key());
                let routes = owner
                    .as_ref()
                    .map(|_| {
                        topology
                            .iter()
                            .map(|target| (target.screen, target.id, target.descriptor.extent))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                let outputs = native.as_ref().map(D3d11Control::output_snapshots).unwrap_or_default();
                status.screens = output_sampler.sample(now, &routes, &outputs);
                input.status.publish(status.clone());
                let _ = input
                    .window
                    .emit(crate::preview::protocol::VIDEO_RUNTIME_EVENT, &status);
            }
        }
        // Authored dependency invalidation, file/GPU/geometry completions wake this worker.
        // The timeout serves coarse supervision, lease expiry and recovery deadlines only.
        let _ = input.signal.recv_timeout(Duration::from_millis(100));
    }
    input.controller.close();
    drop(owner);
    let _ = observation_events.publish(input.receiver.snapshot().generation, vec![], |statuses| {
        input
            .window
            .emit(PREVIEW_STATUS_EVENT, statuses)
            .map_err(|error| error.to_string())
    });
    let stopped = crate::preview::protocol::VideoRuntimeStatus {
        sequence: input.status.read().sequence.saturating_add(1),
        state: crate::preview::protocol::VideoRuntimeState::Stopped,
        ..Default::default()
    };
    input.status.publish(stopped.clone());
    let _ = input
        .window
        .emit(crate::preview::protocol::VIDEO_RUNTIME_EVENT, &stopped);
    Ok(())
}

fn sync_applications(
    input: &Input,
    applications: &mut crate::applications::driver::ApplicationDriver,
    supervisor: &Option<crate::applications::runtime::supervision::Supervisor>,
    project: uuid::Uuid,
    demand: Vec<crate::applications::demand::ApplicationDemand>,
    admission: std::sync::Arc<[golden_io::RevisionGuard]>,
) -> Result<Option<crate::applications::runtime::PlanRetainer>, String> {
    let owner = supervisor.as_ref().ok_or("application supervision is not attached")?;
    let publisher = owner.publisher();
    let result = input
        .runtime
        .capture_app(applications.rendered_capture(project, demand, admission))
        .map_err(|e| e.to_string())
        .and_then(|capture| capture)
        .and_then(|prepared| {
            owner.validate_project(prepared.project())?;
            applications.apply_prepared(prepared, input.notifier.clone())
        });
    publisher.demand(result.as_ref().map(|_| ()).map_err(Clone::clone));
    result
}
