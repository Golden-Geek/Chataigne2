use std::sync::{Arc, Mutex, atomic::Ordering};

use tauri::window::WindowBuilder;
use tauri::{Manager, RunEvent, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent};
use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Controller3;
use windows::Win32::UI::WindowsAndMessaging::{GWL_STYLE, GetWindowLongPtrW, SetWindowLongPtrW, WS_CLIPCHILDREN};
use windows::core::Interface;

use crate::ProbeResult;
use crate::presentation::{EditorActivity, ProbeLayout, ProbeStatus};
use crate::render_worker::{self, RenderReport, Shared};
use mapgyver_native_probe::workload::RunSpec;

#[tauri::command]
fn update_layout(
    window: WebviewWindow,
    layout: ProbeLayout,
    state: tauri::State<'_, Arc<Shared>>,
) -> Result<(), String> {
    // Limit admission before scheduling onto the WebView's UI thread.
    layout.physical_rects(1.0)?;
    let shared = state.inner().clone();
    window
        .with_webview(move |webview| {
            let update = || -> Result<(), String> {
                let controller = webview.controller();
                let controller3 = controller
                    .cast::<ICoreWebView2Controller3>()
                    .map_err(|error| error.to_string())?;
                let (mut raster, mut zoom) = (0.0, 0.0);
                // SAFETY: with_webview runs on the WebView STA; writable scalar outputs only.
                unsafe {
                    controller3
                        .RasterizationScale(&mut raster)
                        .map_err(|error| error.to_string())?;
                    controller.ZoomFactor(&mut zoom).map_err(|error| error.to_string())?;
                }
                let mut status = shared.status.lock().map_err(|_| "probe status poisoned")?;
                if layout.session != status.layout_session {
                    // The UI callback was queued before document replacement. It owns no view.
                    return Ok(());
                }
                status.apply_layout(&layout, raster * zoom)
            };
            if let Err(error) = update()
                && let Ok(mut status) = shared.status.lock()
            {
                status.error = Some(error);
            }
        })
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn probe_status(state: tauri::State<'_, Arc<Shared>>) -> Result<ProbeStatus, String> {
    state.editor_requests.fetch_add(1, Ordering::Relaxed);
    state
        .status
        .lock()
        .map(|status| status.clone())
        .map_err(|_| "probe status poisoned".into())
}

#[tauri::command]
fn report_editor_activity(event: EditorActivity, state: tauri::State<'_, Arc<Shared>>) -> Result<(), String> {
    if !state.editor_test_enabled.load(Ordering::Acquire) {
        return Err("editor interruption test is not enabled".into());
    }
    let status = state.status.lock().map_err(|_| "probe status poisoned")?;
    state
        .editor_signals
        .lock()
        .map_err(|_| "editor signals poisoned")?
        .record(event, status.layout_session)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn set_zoom(window: WebviewWindow, factor: f64) -> Result<(), String> {
    if !factor.is_finite() || !(0.5..=2.0).contains(&factor) {
        return Err("zoom must be between 0.5 and 2".into());
    }
    window.set_zoom(factor).map_err(|error| error.to_string())
}

pub fn run(adapter: u32, spec: RunSpec) -> ProbeResult<RenderReport> {
    let shared = Arc::new(Shared::default());
    shared
        .editor_test_enabled
        .store(spec.isolates_editor(), Ordering::Release);
    if spec.scenario.cards() {
        shared
            .status
            .lock()
            .map_err(|_| "probe status poisoned")?
            .preview_extents = Some([[640, 360], [480, 270]]);
    }
    let worker = Arc::new(Mutex::new(None));
    let supervisor = Arc::new(Mutex::new(None));
    let setup_supervisor = supervisor.clone();
    let setup_shared = shared.clone();
    let setup_worker = worker.clone();
    let app = tauri::Builder::default()
        .manage(shared.clone())
        .invoke_handler(tauri::generate_handler![
            update_layout,
            probe_status,
            set_zoom,
            report_editor_activity
        ])
        .setup(move |app| {
            let page_shared = setup_shared.clone();
            let editor = WebviewWindowBuilder::new(app, "probe", WebviewUrl::App("index.html".into()))
                .on_page_load(move |_window, payload| {
                    if payload.event() == tauri::webview::PageLoadEvent::Started
                        && let Ok(mut status) = page_shared.status.lock()
                        && let Err(error) = status.begin_document()
                    {
                        status.error = Some(error);
                        page_shared.stop.store(true, Ordering::Release);
                    }
                })
                .title("MapGyver M01 — ordinary WebView / D3D11 probe")
                .inner_size(1100.0, 760.0)
                .transparent(true)
                .focused(false)
                .background_color(tauri::window::Color(0, 0, 0, 0))
                .build()?;
            let output = WindowBuilder::new(app, "output")
                .title("MapGyver M01 — native output")
                .inner_size(640.0, 360.0)
                .focused(false)
                .visible(false)
                .build()?;
            if spec.scenario.cards() {
                let [width, height] = spec.scenario.extent();
                if let Some(monitor) = output.available_monitors()?.get(1) {
                    output.set_position(*monitor.position())?;
                }
                // Moving between monitors can rescale the client extent. Apply the physical
                // workload size after selecting the owned output window's monitor.
                output.set_size(tauri::PhysicalSize::new(width, height))?;
            }
            output.show()?;
            let editor_hwnd = editor.hwnd()?;
            // SAFETY: change clipping only on this newly created owned probe HWND, following
            // locked Wry's GPU-underlay example. Never manipulate another app's windows.
            unsafe {
                let style = GetWindowLongPtrW(editor_hwnd, GWL_STYLE);
                SetWindowLongPtrW(editor_hwnd, GWL_STYLE, style & !(WS_CLIPCHILDREN.0 as isize));
            }
            let (editor_handle, output_handle) = (editor_hwnd.0 as usize, output.hwnd()?.0 as usize);
            let state = setup_shared.clone();
            let handle = app.handle().clone();
            let render_thread = std::thread::Builder::new()
                .name("mapgyver-native-probe-render".into())
                .spawn(move || {
                    let result = std::panic::catch_unwind(|| {
                        render_worker::run(adapter, spec, editor_handle, output_handle, state.clone())
                            .map_err(|error| error.to_string())
                    })
                    .unwrap_or_else(|_| Err("render worker panicked".into()));
                    if let Err(error) = &result
                        && let Ok(mut status) = state.status.lock()
                    {
                        status.error = Some(error.clone());
                    }
                    state.finished.store(true, Ordering::Release);
                    // Request normal UI-thread window destruction only after GPU retirement.
                    // Closing the final window lets Tauri emit its ordinary exit request.
                    let teardown_handle = handle.clone();
                    if handle
                        .run_on_main_thread(move || {
                            let editor = teardown_handle
                                .get_webview_window("probe")
                                .map(|window| window.destroy());
                            let output = teardown_handle.get_window("output").map(|window| window.destroy());
                            if editor.is_none()
                                || output.is_none()
                                || editor.is_some_and(|result| result.is_err())
                                || output.is_some_and(|result| result.is_err())
                            {
                                teardown_handle.exit(1);
                            }
                        })
                        .is_err()
                    {
                        handle.exit(1);
                    }
                    result
                })?;
            *setup_worker.lock().map_err(|_| "probe worker poisoned")? = Some(render_thread);
            if spec.isolates_editor() {
                let state = setup_shared.clone();
                *setup_supervisor.lock().map_err(|_| "editor supervisor poisoned")? = Some(
                    std::thread::Builder::new()
                        .name("probe-editor-isolation".into())
                        .spawn(move || crate::editor_isolation::run(editor, state))?,
                );
            }
            Ok(())
        })
        .build(tauri::generate_context!())?;
    let event_shared = shared.clone();
    let exit_code = app.run_return(move |_handle, event| match event {
        RunEvent::WindowEvent {
            event: WindowEvent::CloseRequested { api, .. },
            ..
        } => {
            // Keep HWNDs valid until the render worker has destroyed all swapchains/SDK state.
            if !event_shared.finished.load(Ordering::Acquire) {
                api.prevent_close();
                event_shared.stop.store(true, Ordering::Release);
            }
        }
        RunEvent::ExitRequested { api, .. } if !event_shared.finished.load(Ordering::Acquire) => {
            api.prevent_exit();
            event_shared.stop.store(true, Ordering::Release);
        }
        _ => {}
    });
    let worker = worker
        .lock()
        .map_err(|_| "probe worker poisoned")?
        .take()
        .ok_or("render worker was not started")?;
    // The event loop is terminal; this cannot block the WebView's active UI thread.
    let render_result = worker.join().map_err(|_| "render worker panicked")?;
    let editor_result = supervisor
        .lock()
        .map_err(|_| "editor supervisor poisoned")?
        .take()
        .map(|thread| thread.join().map_err(|_| "editor supervisor panicked"))
        .transpose()?
        .transpose()?;
    let mut report = render_result?;
    report.editor_isolation = editor_result;
    report.shutdown_seconds = Some(report.retirement_started.elapsed().as_secs_f64());
    if exit_code != 0 {
        return Err(format!("Tauri probe exited with status {exit_code}").into());
    }
    Ok(report)
}
