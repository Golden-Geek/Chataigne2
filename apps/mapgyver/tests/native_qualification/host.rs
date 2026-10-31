//! Shared-host lifetime and UI-thread wiring for the owned native qualification runner.

mod preview;
mod record;
mod worker;

use std::{
    collections::VecDeque,
    io::{self, Error},
    net::TcpListener,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    time::Duration,
};

use golden_core::app::{
    DesktopHostConfig, NativeOutputBounds, NativePresentation, NativePresentationTask, ProjectLifecycle,
    UiServerConfig, create_new_project_engine, launch_engine_with_native_presentation,
};
use golden_io::WorkerNotifier;
use golden_video::Extent2D;
use golden_video::native::AdapterId;
use golden_video::preview::{HostViewport, PreviewController, PreviewLimits};
use golden_video::qualification::protocol::{QUALIFICATION_REPORT_EVENT, QualificationReport};
use serde_json::{Value, json};
use tauri::{Listener, Manager};

use super::config::Workload;

mod assets {
    include!(concat!(env!("OUT_DIR"), "/native-preview/ui_assets.rs"));
}

golden_core::define_node_enum!(
    pub enum QualificationNode {}
);
impl ProjectLifecycle for QualificationNode {
    fn application_id() -> &'static str {
        "golden-native-qualification"
    }
    fn application_display_name() -> &'static str {
        "Golden Native Qualification"
    }
    fn app_data_directory_name() -> &'static str {
        "GoldenNativeQualification"
    }
}

#[derive(Clone)]
pub struct HostConfig {
    pub warmup: Duration,
    pub duration: Duration,
    pub independence_duration: Duration,
    pub directory: PathBuf,
    pub scenario: Workload,
    pub adapter: AdapterId,
    pub source_names: [String; 2],
    pub output_name: String,
    pub memory_ceiling: u64,
    pub trace_capacity: usize,
    pub fixture_pids: Vec<u32>,
}

#[derive(Default)]
struct Observations {
    viewport: Mutex<Option<HostViewport>>,
    reports: Mutex<VecDeque<QualificationReport>>,
    report_loss: AtomicU64,
    control_requests: AtomicU64,
}

impl Observations {
    fn receive(&self, payload: &str) {
        if payload.len() > 64 * 1024 {
            self.report_loss.fetch_add(1, Ordering::Relaxed);
            return;
        }
        if let Ok(report) = serde_json::from_str::<QualificationReport>(payload) {
            let mut reports = self.reports.lock().unwrap();
            if reports.len() < 16 {
                reports.push_back(report);
            } else {
                self.report_loss.fetch_add(1, Ordering::Relaxed);
            }
        } else {
            self.report_loss.fetch_add(1, Ordering::Relaxed);
        }
    }
}

pub fn run(config: HostConfig) -> io::Result<()> {
    if std::env::var_os("GC_UI_BIND").is_some() || std::env::var_os("GC_UI_FRONTEND_URL").is_some() {
        return Err(Error::other(
            "unset external UI overrides for the owned qualification host",
        ));
    }
    std::fs::create_dir_all(&config.directory)?;
    let (wake, signal) = mpsc::sync_channel(1);
    let notification = wake.clone();
    let notifier = WorkerNotifier::new(move || {
        let _ = notification.try_send(());
    });
    let (controller, receiver) = PreviewController::new(
        PreviewLimits {
            max_views: 2,
            ..Default::default()
        },
        Some(notifier.clone()),
    )
    .map_err(Error::other)?;
    let observations = Arc::new(Observations::default());
    let worker_observations = observations.clone();
    let control_observations = observations.clone();
    let worker_controller = controller.clone();
    let socket = TcpListener::bind("127.0.0.1:0")?;
    let bind_addr = socket.local_addr()?.to_string();
    drop(socket);
    let fixture_url: tauri::Url = format!("http://{bind_addr}/?mode=qualification")
        .parse()
        .map_err(Error::other)?;
    let presentation = NativePresentation::new(move |editor| {
        editor
            .set_size(tauri::PhysicalSize::new(1280, 720))
            .map_err(Error::other)?;
        let editor_size = editor.inner_size().map_err(Error::other)?;
        let backing = Extent2D::new(editor_size.width.min(1536), editor_size.height.min(900)).map_err(Error::other)?;
        let (width, height) = config.scenario.extent();
        let output_extent = Extent2D::new(width, height).map_err(Error::other)?;
        let (output_bounds, mut window_metadata) = output_request(editor, output_extent)?;
        let editor_hwnd = editor.hwnd().map_err(Error::other)?.0 as isize;
        let event_observations = worker_observations.clone();
        editor.app_handle().listen(QUALIFICATION_REPORT_EVENT, move |event| {
            event_observations.receive(event.payload());
            let _ = wake.try_send(());
        });
        // Creation starts WebView navigation asynchronously. Its current Source can still be
        // about:blank here; select the known listener origin instead of modifying that Source.
        window_metadata["initial_editor_url"] = json!(editor.url().ok().map(|url| url.to_string()));
        window_metadata["fixture_url"] = json!(fixture_url.as_str());
        editor.navigate(fixture_url).map_err(Error::other)?;
        let _ = editor.set_focus();
        let editor = editor.clone();
        Ok(NativePresentationTask::new(move |stop| {
            let app = editor.app_handle().clone();
            let result = worker::run(
                worker::Input {
                    config,
                    editor,
                    output_bounds,
                    editor_hwnd,
                    output_extent,
                    editor_backing: backing,
                    window_metadata,
                    controller: worker_controller,
                    receiver,
                    observations: worker_observations,
                    notifier,
                    signal,
                },
                stop,
            );
            if let Err(error) = &result {
                eprintln!("qualification renderer: {error}");
            }
            app.exit(if result.is_ok() { 0 } else { 1 });
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
        *control_observations.viewport.lock().unwrap() = Some(viewport);
        control_observations.control_requests.fetch_add(1, Ordering::Relaxed);
        controller.control_json(request, viewport)
    });
    let host = DesktopHostConfig {
        ui_server: UiServerConfig {
            bind_addr,
            frontend_assets: assets::APP_UI_ASSETS,
            ..Default::default()
        },
        dev_server: None,
    };
    let mut context = tauri::generate_context!();
    context.config_mut().identifier = "com.goldengeek.mapgyver.native-qualification".into();
    let engine = create_new_project_engine::<QualificationNode>().map_err(Error::other)?;
    launch_engine_with_native_presentation::<QualificationNode, tauri::Wry>(
        engine,
        golden_core::app::LaunchArgs {
            no_remote: true,
            ..Default::default()
        },
        context,
        host,
        move |_| Ok(presentation),
    )
}

fn output_request(editor: &tauri::WebviewWindow, extent: Extent2D) -> io::Result<(NativeOutputBounds, Value)> {
    let current = editor.current_monitor().map_err(Error::other)?;
    let monitors = editor.available_monitors().map_err(Error::other)?;
    let chosen = monitors
        .iter()
        .find(|monitor| {
            current
                .as_ref()
                .is_some_and(|current| current.position() != monitor.position())
        })
        .or(monitors.first())
        .ok_or_else(|| Error::other("qualification requires an active desktop monitor"))?;
    let bounds = NativeOutputBounds::new(
        chosen.position().x,
        chosen.position().y,
        extent.width(),
        extent.height(),
    )?;
    let metadata = json!({
        "backing": [extent.width(), extent.height()],
        "native_window_owner": "golden_host_desktop::NativeOutputWindow",
        "selected_monitor": {
            "name": chosen.name(), "origin": [chosen.position().x, chosen.position().y],
            "size": [chosen.size().width, chosen.size().height], "scale": chosen.scale_factor(),
        },
        "monitor_count": monitors.len(), "physical_display_qualification": false,
    });
    Ok((bounds, metadata))
}
