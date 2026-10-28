//! Explicit Windows host-lifecycle smoke; contains no GPU or input qualification.

#[cfg(windows)]
mod windows_smoke {
    use std::io::{self, Error};
    use std::net::TcpListener;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{Arc, mpsc};
    use std::thread::{self, ThreadId};
    use std::time::Duration;

    use golden_core::app::{
        DesktopHostConfig, LaunchArgs, NativePresentation, NativePresentationTask, ProjectLifecycle, UiAsset,
        UiServerConfig, create_new_project_engine, launch_engine_with_native_presentation,
    };
    use tauri::Manager;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{GWL_STYLE, GetWindowLongPtrW, IsWindow, WS_CLIPCHILDREN};

    golden_core::define_node_enum!(
        pub enum SmokeNode {}
    );

    impl ProjectLifecycle for SmokeNode {
        fn application_id() -> &'static str {
            "golden-native-presentation-smoke"
        }

        fn application_display_name() -> &'static str {
            "Golden native-presentation lifecycle smoke"
        }

        fn app_data_directory_name() -> &'static str {
            "GoldenNativePresentationSmoke"
        }
    }

    static ASSETS: &[UiAsset] = &[UiAsset {
        path: "/index.html",
        content_type: "text/html",
        bytes: b"<!doctype html><html><body style='background:transparent;color:white'>Native lifecycle smoke</body></html>",
    }];

    #[derive(Default)]
    struct Observations {
        hwnd: AtomicUsize,
        stop_received: AtomicBool,
        responsive_ui_during_retirement: AtomicBool,
        resource_dropped_with_live_window: AtomicBool,
        resource_dropped_on_worker: AtomicBool,
    }

    struct WorkerResource {
        observations: Arc<Observations>,
        ui_thread: ThreadId,
    }

    impl Drop for WorkerResource {
        fn drop(&mut self) {
            self.observations.resource_dropped_with_live_window.store(
                window_is_live(self.observations.hwnd.load(Ordering::Acquire)),
                Ordering::Release,
            );
            self.observations
                .resource_dropped_on_worker
                .store(thread::current().id() != self.ui_thread, Ordering::Release);
        }
    }

    fn window_is_live(hwnd: usize) -> bool {
        // SAFETY: IsWindow only inspects this smoke's recorded HWND; it does not dereference it.
        unsafe { IsWindow(Some(HWND(hwnd as *mut _))).as_bool() }
    }

    fn require(condition: bool, message: &str) -> io::Result<()> {
        if condition { Ok(()) } else { Err(Error::other(message)) }
    }

    pub fn run() -> io::Result<()> {
        require(
            std::env::var_os("GC_UI_BIND").is_none() && std::env::var_os("GC_UI_FRONTEND_URL").is_none(),
            "unset GC_UI_BIND and GC_UI_FRONTEND_URL for the isolated native smoke",
        )?;
        let ui_thread = thread::current().id();
        let observations = Arc::new(Observations::default());
        let prepared_observations = observations.clone();
        let presentation = NativePresentation::new(move |window: &tauri::WebviewWindow<tauri::Wry>| {
            require(
                thread::current().id() == ui_thread,
                "prepare did not run on the main UI thread",
            )?;
            let hwnd = window.hwnd().map_err(Error::other)?;
            prepared_observations.hwnd.store(hwnd.0 as usize, Ordering::Release);
            require(
                window_is_live(hwnd.0 as usize),
                "host-created HWND is not live during prepare",
            )?;
            // SAFETY: the host owns this live HWND and prepare runs on its UI thread.
            let style = unsafe { GetWindowLongPtrW(HWND(hwnd.0), GWL_STYLE) };
            require(
                style & WS_CLIPCHILDREN.0 as isize == 0,
                "host did not enable ordinary GPU-underlay clipping",
            )?;

            let (webview_tx, webview_rx) = mpsc::sync_channel(1);
            window
                .with_webview(move |webview| {
                    // SAFETY: with_webview runs on the WebView STA, and no COM object leaves it.
                    let controller_works = unsafe { webview.controller().CoreWebView2() }.is_ok();
                    let _ = webview_tx.send((thread::current().id(), controller_works));
                })
                .map_err(Error::other)?;
            let window = window.clone();
            Ok(NativePresentationTask::new(move |stop| {
                let (callback_thread, controller_works) = webview_rx
                    .recv_timeout(Duration::from_secs(3))
                    .map_err(|error| Error::other(format!("WebView callback unavailable: {error}")))?;
                require(
                    callback_thread == ui_thread && controller_works,
                    "WebView STA inspection failed",
                )?;
                let resource = WorkerResource {
                    observations: prepared_observations.clone(),
                    ui_thread,
                };

                window.app_handle().exit(0);
                require(
                    stop.wait_timeout(Duration::from_secs(3)),
                    "host did not signal native worker stop",
                )?;
                prepared_observations.stop_received.store(true, Ordering::Release);

                let (ui_tx, ui_rx) = mpsc::sync_channel(1);
                let hwnd = prepared_observations.hwnd.load(Ordering::Acquire);
                window
                    .run_on_main_thread(move || {
                        let _ = ui_tx.send(thread::current().id() == ui_thread && window_is_live(hwnd));
                    })
                    .map_err(Error::other)?;
                let responsive = ui_rx
                    .recv_timeout(Duration::from_secs(3))
                    .map_err(|error| Error::other(format!("UI stopped pumping during retirement: {error}")))?;
                require(responsive, "UI thread or HWND ended before worker retirement")?;
                prepared_observations
                    .responsive_ui_during_retirement
                    .store(true, Ordering::Release);
                thread::sleep(Duration::from_millis(100));
                drop(resource);
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
        context.config_mut().identifier = "com.goldengeek.mapgyver.native-presentation-smoke".into();
        let engine = create_new_project_engine::<SmokeNode>().map_err(Error::other)?;
        let (done_tx, done_rx) = mpsc::sync_channel(1);
        let watchdog = thread::spawn(move || {
            if done_rx.recv_timeout(Duration::from_secs(20)).is_err() {
                eprintln!("native presentation smoke exceeded its 20-second deadline");
                std::process::exit(1);
            }
        });
        let outcome = launch_engine_with_native_presentation(
            engine,
            LaunchArgs {
                no_remote: true,
                ..Default::default()
            },
            context,
            config,
            presentation,
        );
        let _ = done_tx.send(());
        watchdog.join().map_err(|_| Error::other("smoke watchdog panicked"))?;
        outcome?;
        require(
            observations.stop_received.load(Ordering::Acquire),
            "worker stop was not observed",
        )?;
        require(
            observations.responsive_ui_during_retirement.load(Ordering::Acquire),
            "UI responsiveness during retirement was not observed",
        )?;
        require(
            observations.resource_dropped_with_live_window.load(Ordering::Acquire)
                && observations.resource_dropped_on_worker.load(Ordering::Acquire),
            "resource did not retire on its worker while the HWND remained live",
        )?;
        require(
            !window_is_live(observations.hwnd.load(Ordering::Acquire)),
            "HWND outlived the returned host",
        )?;
        println!(
            "native presentation smoke passed: UI/COM STA, stop delivery, live HWND during worker retirement, responsive UI, worker resource drop, destroyed HWND after host return"
        );
        Ok(())
    }
}

#[cfg(windows)]
fn main() -> std::process::ExitCode {
    match windows_smoke::run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("native presentation smoke failed: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("native presentation smoke requires Windows");
    std::process::exit(1);
}
