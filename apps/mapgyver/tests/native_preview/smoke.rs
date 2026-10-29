//! Owned Svelte/host/native preview consumer. Finite shaders have separate pixel oracles.
#[cfg(windows)]
mod scene;
#[cfg(windows)]
mod windows_smoke {
    use super::scene;
    use golden_core::app::{
        DesktopHostConfig, NativePresentation, NativePresentationTask, ProjectLifecycle, UiServerConfig,
        create_new_project_engine, launch_engine_with_native_presentation,
    };
    use golden_io::WorkerNotifier;
    use golden_video::native::{D3d11Config, InputLimits, OutputBinding, WindowTarget, adapters};
    use golden_video::preview::{HostViewport, PreviewController, PreviewLimits, PreviewReceiver};
    use golden_video::runtime::{LatePolicy, OutcomeKind, PlanActivation, VideoRuntime};
    use golden_video::{AlphaMode, ColorEncoding, Extent2D, OutputId, PixelFormat, TextureDescriptor};
    use std::{
        collections::HashMap,
        io::{self, Error},
        net::TcpListener,
        num::NonZeroUsize,
        sync::{Arc, Mutex, mpsc},
        time::{Duration, Instant},
    };
    use tauri::{Listener, Manager};

    mod assets {
        include!(concat!(env!("OUT_DIR"), "/native-preview/ui_assets.rs"));
    }
    golden_core::define_node_enum!(
        pub enum SmokeNode {}
    );
    impl ProjectLifecycle for SmokeNode {
        fn application_id() -> &'static str {
            "golden-preview-smoke"
        }
        fn app_data_directory_name() -> &'static str {
            "GoldenPreviewSmoke"
        }
        fn application_display_name() -> &'static str {
            "Golden Preview Smoke"
        }
    }
    #[derive(Default)]
    struct Observations {
        requests: std::sync::atomic::AtomicUsize,
        phases: Mutex<Vec<serde_json::Value>>,
        host: Mutex<Option<HostViewport>>,
    }

    struct Lease {
        runtime: VideoRuntime,
        controller: PreviewController,
    }
    impl Drop for Lease {
        fn drop(&mut self) {
            self.controller.close();
            self.runtime.close();
            if !self.runtime.retirement().wait(Duration::from_secs(5)) {
                eprintln!("native preview failed to retire before its owned HWND could be released");
                std::process::exit(1);
            }
        }
    }
    fn require(value: bool, message: &str) -> io::Result<()> {
        if value { Ok(()) } else { Err(Error::other(message)) }
    }

    struct WorkerInput {
        window: tauri::WebviewWindow,
        hwnd: isize,
        adapter: golden_video::native::AdapterId,
        controller: PreviewController,
        receiver: PreviewReceiver,
        signal: mpsc::Receiver<()>,
        notifier: WorkerNotifier,
        observations: Arc<Observations>,
    }
    fn run_worker(input: WorkerInput, stop: golden_core::app::NativePresentationStop) -> io::Result<()> {
        let WorkerInput {
            window,
            hwnd,
            adapter,
            controller,
            receiver,
            signal,
            notifier,
            observations,
        } = input;
        let runtime = VideoRuntime::spawn_d3d11(
            scene::config(notifier),
            D3d11Config {
                diagnostics: None,
                adapter,
                inputs: InputLimits {
                    max_sources: 0,
                    max_storage_bytes: 0,
                },
                outputs: vec![
                    OutputBinding::Window {
                        id: OutputId::new(1).unwrap(),
                        window: unsafe { WindowTarget::from_raw(hwnd).map_err(Error::other)? },
                        extent: Extent2D::new(512, 320).unwrap(),
                    },
                    OutputBinding::Spout {
                        id: OutputId::new(2).unwrap(),
                        name: format!("GoldenPreviewIndependent-{}", std::process::id()),
                        descriptor: TextureDescriptor {
                            extent: Extent2D::new(8, 8).unwrap(),
                            format: PixelFormat::Bgra8Unorm,
                            color_encoding: ColorEncoding::Srgb,
                            alpha_mode: AlphaMode::Opaque,
                        },
                    },
                ],
            },
        )
        .map_err(Error::other)?;
        let mut lease = Lease { runtime, controller };
        let native = lease
            .runtime
            .d3d11_control()
            .ok_or_else(|| Error::other("native observation missing"))?;
        let mut last_generation = None;
        let mut pending = HashMap::new();
        let mut installed = None;
        let mut phases = 0;
        let started = Instant::now();
        let mut done_frame = None;
        loop {
            if started.elapsed() >= Duration::from_secs(40) {
                return Err(Error::other(format!(
                    "native preview fixture deadline: control_requests={} phases={} host={:?} frames={}",
                    observations.requests.load(std::sync::atomic::Ordering::Relaxed),
                    phases,
                    observations.host.lock().unwrap(),
                    lease.runtime.snapshot().frames_submitted
                )));
            }
            if stop.is_requested() {
                return Err(Error::other("native preview stopped before fixture completion"));
            }
            if let Some(failure) = lease.runtime.snapshot().failure {
                return Err(Error::other(failure.to_string()));
            }
            let snapshot = receiver.snapshot();
            if last_generation != Some(snapshot.generation) {
                let epoch = lease.runtime.read_time().map_err(Error::other)?.epoch();
                let request = lease
                    .runtime
                    .submit(
                        lease.runtime.project_epoch(),
                        scene::plan(&snapshot),
                        PlanActivation::NextFrame(epoch),
                        LatePolicy::Activate,
                    )
                    .map_err(Error::other)?;
                pending.insert(request, snapshot.generation);
                last_generation = Some(snapshot.generation);
                installed = None;
            }
            let mut outcomes = Vec::new();
            lease
                .runtime
                .drain_outcomes(&mut outcomes, NonZeroUsize::new(16).unwrap());
            for outcome in outcomes {
                let layout = pending.remove(&outcome.request);
                match outcome.result {
                    OutcomeKind::Installed { generation, .. } => {
                        if let Some(layout) = layout
                            && layout == snapshot.generation
                        {
                            installed = Some((
                                layout,
                                generation.identity,
                                None::<golden_video::runtime::CompletionToken>,
                            ));
                        }
                    }
                    OutcomeKind::Rejected(failure) => require(
                        matches!(
                            failure.code(),
                            golden_video::runtime::FailureCode::StaleSource
                                | golden_video::runtime::FailureCode::Superseded
                        ),
                        &failure.to_string(),
                    )?,
                    _ => {}
                }
            }
            let matching_frame = if let Some((layout, identity, first_frame)) = &mut installed {
                if let Some(frame) = lease.runtime.snapshot().last_frame
                    && frame.identity == *identity
                {
                    let first = first_frame.get_or_insert(frame.completion);
                    let accepted = native
                        .output_snapshots()
                        .iter()
                        .find(|output| output.id == OutputId::new(1).unwrap())
                        .and_then(|output| output.last_accepted)
                        .is_some_and(|token| token.device == first.device && token.serial >= first.serial);
                    accepted && receiver.mark_installed(*layout)
                } else {
                    false
                }
            } else {
                false
            };
            let messages = observations.phases.lock().unwrap().clone();
            while phases < messages.len() {
                let message = &messages[phases];
                let phase = message["phase"].as_str().unwrap_or("invalid");
                require(phase != "failed", &message.to_string())?;
                let visible = receiver.snapshot().views.len();
                let expected = match phase {
                    "initial" | "clipped" => 2,
                    "hidden" => 1,
                    "unregistered" | "empty" | "done" => 0,
                    _ => return Err(Error::other("unknown fixture phase")),
                };
                require(visible == expected, "Svelte/native visible registration mismatch")?;
                if phase == "clipped" {
                    require(
                        receiver.snapshot().views.iter().any(|view| view.source_rect.x > 0.),
                        "scroll did not crop a native source",
                    )?;
                }
                require(
                    matching_frame || phase == "done",
                    "fixture advanced before a matching installed/submitted native generation",
                )?;
                println!(
                    "native preview phase={phase} visible={visible} generation={} frames={}",
                    snapshot.generation,
                    lease.runtime.snapshot().frames_submitted
                );
                phases += 1;
                if phase == "done" {
                    done_frame = Some(lease.runtime.snapshot().frames_submitted);
                }
            }
            if let Some(frame) = done_frame
                && lease.runtime.snapshot().frames_submitted >= frame + 3
            {
                require(
                    native.snapshot().accepted_outputs > 0 && native.snapshot().failed_outputs == 0,
                    "independent native consumer failed while views were removed",
                )?;
                require(
                    receiver.snapshot().views.is_empty(),
                    "unregistered preview pixels could remain demanded",
                )?;
                println!(
                    "native preview passed LUID{:08x}:{:08x}, phases={phases}, frames={}, accepted_outputs={}, host={:?}",
                    adapter.high as u32,
                    adapter.low,
                    lease.runtime.snapshot().frames_submitted,
                    native.snapshot().accepted_outputs,
                    observations.host.lock().unwrap()
                );
                window.app_handle().exit(0);
                require(
                    stop.wait_timeout(Duration::from_secs(3)),
                    "host stop did not reach preview worker",
                )?;
                drop(lease);
                return Ok(());
            }
            let mut timeout = receiver.next_deadline().map_or(Duration::from_secs(1), |at| {
                at.saturating_duration_since(Instant::now()).min(Duration::from_secs(1))
            });
            if installed.is_some() && !matching_frame || done_frame.is_some() {
                timeout = Duration::from_millis(16);
            }
            let _ = signal.recv_timeout(timeout);
        }
    }

    pub fn run() -> io::Result<()> {
        require(
            std::env::var_os("GC_UI_BIND").is_none() && std::env::var_os("GC_UI_FRONTEND_URL").is_none(),
            "unset external UI overrides for native preview smoke",
        )?;
        let inventory = adapters().map_err(Error::other)?;
        let args = std::env::args().collect::<Vec<_>>();
        let index = args
            .windows(2)
            .find(|pair| pair[0] == "--adapter")
            .map(|pair| pair[1].parse::<usize>())
            .transpose()
            .map_err(Error::other)?
            .unwrap_or(0);
        let adapter = inventory
            .get(index)
            .filter(|adapter| !adapter.software)
            .ok_or_else(|| Error::other("hardware adapter index unavailable"))?
            .id;
        let (wake, signal) = mpsc::sync_channel(1);
        let notify_tx = wake.clone();
        let notifier = WorkerNotifier::new(move || {
            let _ = notify_tx.try_send(());
        });
        let (controller, receiver) =
            PreviewController::new(PreviewLimits::default(), Some(notifier.clone())).map_err(Error::other)?;
        let observations = Arc::new(Observations::default());
        let event_observations = observations.clone();
        let worker_observations = observations.clone();
        let worker_controller = controller.clone();
        let presentation = NativePresentation::new(move |window| {
            let hwnd = window.hwnd().map_err(Error::other)?.0 as isize;
            window.app_handle().listen("golden-preview-smoke", move |event| {
                if event.payload().len() <= 64 * 1024
                    && let Ok(message) = serde_json::from_str::<serde_json::Value>(event.payload())
                {
                    let mut phases = event_observations.phases.lock().unwrap();
                    if phases.len() < 8 {
                        phases.push(message);
                        let _ = wake.try_send(());
                    }
                }
            });
            let window = window.clone();
            Ok(NativePresentationTask::new(move |stop| {
                let app = window.app_handle().clone();
                let result = run_worker(
                    WorkerInput {
                        window,
                        hwnd,
                        adapter,
                        controller: worker_controller,
                        receiver,
                        signal,
                        notifier,
                        observations: worker_observations,
                    },
                    stop,
                );
                if let Err(error) = &result {
                    eprintln!("native preview worker failed: {error}");
                    app.exit(1);
                }
                result
            }))
        })
        .with_control(move |request, metrics| {
            let viewport = HostViewport {
                client_width: metrics.physical_width,
                client_height: metrics.physical_height,
                webview_origin_x: metrics.webview_origin_x,
                webview_origin_y: metrics.webview_origin_y,
                webview_width: metrics.webview_width,
                webview_height: metrics.webview_height,
                css_scale: metrics.css_scale,
            };
            *observations.host.lock().unwrap() = Some(viewport);
            let result = controller.control_json(request, viewport);
            let count = observations.requests.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if count < 12 {
                eprintln!("preview control #{}: {:?}", count + 1, result);
            }
            result
        });
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let address = listener.local_addr()?;
        drop(listener);
        let config = DesktopHostConfig {
            ui_server: UiServerConfig {
                bind_addr: address.to_string(),
                frontend_assets: assets::APP_UI_ASSETS,
                ..Default::default()
            },
            dev_server: None,
        };
        let mut context = tauri::generate_context!();
        context.config_mut().identifier = "com.goldengeek.mapgyver.native-preview-smoke".into();
        let engine = create_new_project_engine::<SmokeNode>().map_err(Error::other)?;
        let (done, finished) = mpsc::sync_channel(1);
        let watchdog = std::thread::spawn(move || {
            if finished.recv_timeout(Duration::from_secs(60)).is_err() {
                eprintln!("native preview owned-process watchdog expired");
                std::process::exit(1);
            }
        });
        let result = launch_engine_with_native_presentation::<SmokeNode, tauri::Wry>(
            engine,
            golden_core::app::LaunchArgs {
                no_remote: true,
                ..Default::default()
            },
            context,
            config,
            presentation,
        );
        let _ = done.send(());
        let _ = watchdog.join();
        result
    }
}
#[cfg(windows)]
fn main() -> std::process::ExitCode {
    match windows_smoke::run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("native preview smoke failed: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
#[cfg(not(windows))]
fn main() {
    eprintln!("native preview smoke requires Windows");
    std::process::exit(1);
}
