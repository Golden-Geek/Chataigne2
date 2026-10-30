//! Ordinary app preview: shared host owns HWND lifetime; this task owns rendering/preparation.
mod worker;

use crate::app::AppNode;
use golden_core::app::{NativePresentation, NativePresentationTask};
use golden_core::application::ProductionRuntime;
use golden_io::WorkerNotifier;
use golden_video::preview::{HostViewport, PreviewController, PreviewLimits};
use std::{
    io,
    sync::{Arc, Mutex, mpsc},
};

pub(crate) fn presentation(runtime: ProductionRuntime<AppNode>) -> io::Result<NativePresentation<tauri::Wry>> {
    let (send, signal) = mpsc::sync_channel(1);
    let notifier = WorkerNotifier::new(move || {
        let _ = send.try_send(());
    });
    let (controller, receiver) = PreviewController::new(
        PreviewLimits {
            max_views: 8,
            ..Default::default()
        },
        Some(notifier.clone()),
    )
    .map_err(io::Error::other)?;
    let viewport = Arc::new(Mutex::new(None));
    let control_viewport = viewport.clone();
    let control = controller.clone();
    Ok(NativePresentation::new(move |window: &tauri::WebviewWindow| {
        let hwnd = window.hwnd().map_err(io::Error::other)?.0 as isize;
        let window = window.clone();
        Ok(NativePresentationTask::new(move |stop| {
            worker::run(worker::Input {
                runtime,
                window,
                hwnd,
                controller,
                receiver,
                viewport,
                notifier,
                signal,
                stop,
            })
        }))
    })
    .with_control(move |request, metrics| {
        let host = HostViewport {
            client_width: metrics.physical_width,
            client_height: metrics.physical_height,
            webview_origin_x: metrics.webview_origin_x,
            webview_origin_y: metrics.webview_origin_y,
            webview_width: metrics.webview_width,
            webview_height: metrics.webview_height,
            css_scale: metrics.css_scale,
        };
        *control_viewport
            .lock()
            .map_err(|_| "preview viewport lock is poisoned")? = Some(host);
        let request = serde_json::from_str(request).map_err(|error| error.to_string())?;
        serde_json::to_string(&control.handle(request, host)).map_err(|error| error.to_string())
    }))
}
