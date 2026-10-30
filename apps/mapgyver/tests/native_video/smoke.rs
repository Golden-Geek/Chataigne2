//! Owned native-host consumer smoke. API acceptance is distinct from pixel/scanout qualification.

#[cfg(windows)]
mod scene;

#[cfg(windows)]
mod windows_smoke {
    use std::io::{self, Error};
    use std::net::TcpListener;
    use std::num::NonZeroUsize;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{Arc, mpsc};
    use std::thread;
    use std::time::{Duration, Instant};

    use golden_core::app::{
        DesktopHostConfig, LaunchArgs, NativePresentation, NativePresentationTask, ProjectLifecycle, UiAsset,
        UiServerConfig, create_new_project_engine, launch_engine_with_native_presentation,
    };
    use golden_video::native::{AdapterId, D3d11Config, D3d11Control, OutputBinding, WindowTarget, adapters};
    use golden_video::runtime::{
        LatePolicy, OutcomeKind, PlanActivation, RuntimeRetirement, RuntimeState, VideoRuntime,
    };
    use golden_video::{AlphaMode, ColorEncoding, Extent2D, PixelFormat, TextureDescriptor};
    use tauri::Manager;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::IsWindow;

    use super::scene;

    golden_core::define_node_enum!(
        pub enum SmokeNode {}
    );

    impl ProjectLifecycle for SmokeNode {
        fn application_id() -> &'static str {
            "golden-native-video-smoke"
        }
        fn application_display_name() -> &'static str {
            "Golden native video smoke"
        }
        fn app_data_directory_name() -> &'static str {
            "GoldenNativeVideoSmoke"
        }
    }

    static ASSETS: &[UiAsset] = &[UiAsset {
        path: "/index.html",
        content_type: "text/html",
        bytes: b"<!doctype html><html><body style='background:transparent;color:white;font-family:sans-serif'><p>Golden native video smoke: fixed backing texture, transparent WebView</p></body></html>",
    }];

    #[derive(Default)]
    struct Observations {
        hwnd: AtomicUsize,
        frames_verified: AtomicBool,
        ui_responsive: AtomicBool,
        retired_with_live_window: AtomicBool,
    }

    /// Created only inside the outer presentation worker. Every unwind/error must retain the
    /// host's HWND lease through the inner renderer's actual retirement, not just close/drop.
    struct RenderLease {
        runtime: Option<VideoRuntime>,
        retirement: RuntimeRetirement,
        window: tauri::WebviewWindow,
        observations: Arc<Observations>,
        succeeded: bool,
    }

    impl Drop for RenderLease {
        fn drop(&mut self) {
            self.window.app_handle().exit(if self.succeeded { 0 } else { 1 });
            if let Some(runtime) = self.runtime.take() {
                runtime.close();
                drop(runtime);
            }
            // A timeout cannot release the HWND. The executable watchdog independently fails
            // the smoke if native retirement stops progressing; it never reports retirement.
            while !self.retirement.wait(Duration::from_millis(20)) {}
            self.observations.retired_with_live_window.store(
                window_is_live(self.observations.hwnd.load(Ordering::Acquire)),
                Ordering::Release,
            );
        }
    }

    fn require(condition: bool, message: &str) -> io::Result<()> {
        if condition { Ok(()) } else { Err(Error::other(message)) }
    }

    fn window_is_live(hwnd: usize) -> bool {
        // SAFETY: IsWindow only inspects this smoke's host-owned HWND.
        unsafe { IsWindow(Some(HWND(hwnd as *mut _))).as_bool() }
    }

    fn wait_until(label: &str, mut check: impl FnMut() -> io::Result<bool>) -> io::Result<()> {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if check()? {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(Error::other(format!("timed out waiting for {label}")));
            }
            thread::sleep(Duration::from_millis(10));
        }
    }

    fn run_phase(runtime: &mut VideoRuntime, native: &D3d11Control, outputs: &[u64], label: &str) -> io::Result<()> {
        let epoch = runtime.read_time().map_err(Error::other)?.epoch();
        let request = runtime
            .submit_plan(
                runtime.project_epoch(),
                scene::plan(outputs),
                PlanActivation::NextFrame(epoch),
                LatePolicy::Activate,
            )
            .map_err(Error::other)?;
        let mut installed = None;
        let mut outcomes = Vec::new();
        wait_until(label, || {
            runtime.drain_outcomes(&mut outcomes, NonZeroUsize::new(4).unwrap());
            for outcome in outcomes.drain(..) {
                require(outcome.request == request, "unexpected smoke request outcome")?;
                match outcome.result {
                    OutcomeKind::Installed { generation, .. } => installed = Some(generation),
                    outcome => return Err(Error::other(format!("{label}: {outcome:?}"))),
                }
            }
            if let Some(failure) = runtime.snapshot().failure {
                return Err(Error::other(format!("{label}: {failure}")));
            }
            Ok(installed.is_some())
        })?;
        let installed = installed.unwrap();
        require(
            installed.output_count == outputs.len(),
            "installed output demand differs from smoke plan",
        )?;
        let before = native.snapshot();
        let before_frames = runtime.snapshot().frames_submitted;
        wait_until(label, || {
            let observed = native.snapshot();
            let snapshot = runtime.snapshot();
            require(
                observed.failed_outputs == 0 && observed.failure.is_none(),
                "native output API failed",
            )?;
            if let Some(failure) = snapshot.failure {
                return Err(Error::other(format!("{label}: {failure}")));
            }
            Ok(observed.accepted_outputs >= before.accepted_outputs + 4
                && snapshot.frames_submitted >= before_frames + 4
                && snapshot
                    .last_frame
                    .is_some_and(|frame| frame.identity == installed.identity))
        })?;
        let observed = native.snapshot();
        let snapshot = runtime.snapshot();
        let resources = snapshot
            .resources
            .ok_or_else(|| Error::other("native resource observations missing"))?;
        println!(
            "phase={label} outputs={} batches={} api_accepted={} busy={} unavailable={} peak_reserved_bytes={}",
            outputs.len(),
            observed.submitted,
            observed.accepted_outputs,
            observed.busy_outputs,
            observed.unavailable_outputs,
            resources.peak_bytes,
        );
        Ok(())
    }

    fn selected_adapter() -> io::Result<AdapterId> {
        let found = adapters().map_err(Error::other)?;
        for (index, adapter) in found.iter().enumerate() {
            println!(
                "adapter={index} luid={:08x}:{:08x} software={} {}",
                adapter.id.high as u32, adapter.id.low, adapter.software, adapter.description
            );
        }
        let args = std::env::args().skip(1).collect::<Vec<_>>();
        require(
            args.len() == 2 && args[0] == "--adapter",
            "select an observed adapter explicitly: --adapter <index>",
        )?;
        let index: usize = args[1].parse().map_err(Error::other)?;
        let selected = found
            .get(index)
            .ok_or_else(|| Error::other("adapter index is out of range"))?;
        require(
            !selected.software,
            "native smoke requires an explicitly selected hardware adapter",
        )?;
        Ok(selected.id)
    }

    pub fn run() -> io::Result<()> {
        require(
            std::env::var_os("GC_UI_BIND").is_none() && std::env::var_os("GC_UI_FRONTEND_URL").is_none(),
            "unset GC_UI_BIND and GC_UI_FRONTEND_URL for the isolated native smoke",
        )?;
        let adapter = selected_adapter()?;
        let ui_thread = thread::current().id();
        let observations = Arc::new(Observations::default());
        let prepared_observations = observations.clone();
        let presentation = NativePresentation::new(move |window: &tauri::WebviewWindow<tauri::Wry>| {
            require(thread::current().id() == ui_thread, "native prepare left the UI thread")?;
            let hwnd = window.hwnd().map_err(Error::other)?.0 as usize;
            require(window_is_live(hwnd), "native prepare received an invalid host HWND")?;
            prepared_observations.hwnd.store(hwnd, Ordering::Release);
            // SAFETY: the host owns this window, and RenderLease prevents task completion until
            // actual VideoRuntime retirement. No other renderer attaches to this smoke window.
            let target = unsafe { WindowTarget::from_raw(hwnd as isize) }.map_err(Error::other)?;
            let window = window.clone();
            Ok(NativePresentationTask::new(move |stop| {
                let config = D3d11Config {
                    diagnostics: None,
                    adapter,
                    inputs: Default::default(),
                    outputs: vec![
                        OutputBinding::Window {
                            id: scene::output(1),
                            window: target,
                            extent: Extent2D::new(640, 360).unwrap(),
                        },
                        OutputBinding::Spout {
                            id: scene::output(2),
                            name: format!("GoldenVideoSmoke-{}", std::process::id()),
                            descriptor: TextureDescriptor {
                                extent: Extent2D::new(320, 180).unwrap(),
                                format: PixelFormat::Rgba8Unorm,
                                color_encoding: ColorEncoding::Srgb,
                                alpha_mode: AlphaMode::Straight,
                            },
                        },
                    ],
                };
                let runtime = VideoRuntime::spawn_d3d11(scene::config(), config).map_err(|error| {
                    window.app_handle().exit(1);
                    Error::other(error)
                })?;
                let mut lease = RenderLease {
                    retirement: runtime.retirement(),
                    runtime: Some(runtime),
                    window: window.clone(),
                    observations: prepared_observations.clone(),
                    succeeded: false,
                };
                let runtime = lease.runtime.as_mut().unwrap();
                let native = runtime
                    .d3d11_control()
                    .ok_or_else(|| Error::other("native control missing"))?;
                wait_until("native initialization", || {
                    let snapshot = runtime.snapshot();
                    if let Some(failure) = snapshot.failure {
                        return Err(Error::other(failure.to_string()));
                    }
                    Ok(snapshot.state == RuntimeState::Running && native.snapshot().initialized)
                })?;
                require(
                    native.snapshot().adapter == Some(adapter),
                    "renderer changed the explicitly selected adapter",
                )?;
                run_phase(runtime, &native, &[1], "window-only")?;
                run_phase(runtime, &native, &[2], "spout-only")?;

                let (resize_tx, resize_rx) = mpsc::sync_channel(1);
                let resized_window = window.clone();
                window
                    .run_on_main_thread(move || {
                        let result = resized_window
                            .set_size(tauri::PhysicalSize::new(800, 500))
                            .map_err(|error| error.to_string());
                        let _ = resize_tx.send(result);
                    })
                    .map_err(Error::other)?;
                resize_rx
                    .recv_timeout(Duration::from_secs(3))
                    .map_err(Error::other)?
                    .map_err(Error::other)?;
                run_phase(runtime, &native, &[1, 2], "shared-producer-after-client-resize")?;
                prepared_observations.frames_verified.store(true, Ordering::Release);

                window.app_handle().exit(0);
                require(
                    stop.wait_timeout(Duration::from_secs(3)),
                    "host stop did not reach the presentation worker",
                )?;
                runtime.close();
                let (ui_tx, ui_rx) = mpsc::sync_channel(1);
                window
                    .run_on_main_thread(move || {
                        let _ = ui_tx.send(thread::current().id() == ui_thread && window_is_live(hwnd));
                    })
                    .map_err(Error::other)?;
                require(
                    ui_rx.recv_timeout(Duration::from_secs(3)).map_err(Error::other)?,
                    "UI/owned HWND ended before task retirement",
                )?;
                prepared_observations.ui_responsive.store(true, Ordering::Release);
                require(
                    lease.retirement.wait(Duration::from_secs(5)),
                    "native renderer did not finish its completion-based retirement",
                )?;
                let stopped = runtime.snapshot();
                require(
                    stopped.state == RuntimeState::Stopped
                        && stopped.resources.is_some_and(|resources| resources.bytes_used == 0)
                        && !native.snapshot().initialized,
                    "retired native resources still appear live in final observations",
                )?;
                lease.succeeded = true;
                drop(lease);
                Ok(())
            }))
        });

        let listener = TcpListener::bind("127.0.0.1:0")?;
        let address = listener.local_addr()?;
        drop(listener);
        let config = DesktopHostConfig {
            ui_server: UiServerConfig {
                bind_addr: address.to_string(),
                frontend_assets: ASSETS,
                ..Default::default()
            },
            dev_server: None,
        };
        let mut context = tauri::generate_context!();
        context.config_mut().identifier = "com.goldengeek.mapgyver.native-video-smoke".into();
        let engine = create_new_project_engine::<SmokeNode>().map_err(Error::other)?;
        let (done_tx, done_rx) = mpsc::sync_channel(1);
        let watchdog = thread::spawn(move || {
            if done_rx.recv_timeout(Duration::from_secs(30)).is_err() {
                eprintln!("native video smoke exceeded its 30-second deadline");
                std::process::exit(1);
            }
        });
        let result = launch_engine_with_native_presentation(
            engine,
            LaunchArgs {
                no_remote: true,
                ..Default::default()
            },
            context,
            config,
            move |_| Ok(presentation),
        );
        let _ = done_tx.send(());
        watchdog.join().map_err(|_| Error::other("smoke watchdog panicked"))?;
        result?;
        require(
            observations.frames_verified.load(Ordering::Acquire),
            "native output phases did not complete",
        )?;
        require(
            observations.ui_responsive.load(Ordering::Acquire),
            "UI was not responsive during native retirement",
        )?;
        require(
            observations.retired_with_live_window.load(Ordering::Acquire),
            "HWND ended before actual renderer retirement",
        )?;
        require(
            !window_is_live(observations.hwnd.load(Ordering::Acquire)),
            "HWND survived returned host",
        )?;
        println!(
            "native video smoke passed: DAG/all four blends, window and Spout API acceptance, shared producer, fixed backing size during client resize, live HWND through retirement, destroyed HWND after host return"
        );
        println!(
            "qualification limits: no pixel/scanout/cadence budget pass; Spout SDK may wait up to 67 ms and does not expose mutex timeout drops"
        );
        Ok(())
    }
}

#[cfg(windows)]
fn main() -> std::process::ExitCode {
    match windows_smoke::run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("native video smoke failed: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("native video smoke requires Windows");
    std::process::exit(1);
}
