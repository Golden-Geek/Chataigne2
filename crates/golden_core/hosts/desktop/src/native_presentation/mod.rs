//! Optional native presentation attached to the shared desktop host.
//!
//! The host creates and retains the window. Preparation runs on its UI thread; rendering
//! resources are constructed, used and retired inside the returned worker task.

use std::io;
use std::sync::Arc;

use tauri::{Runtime, WebviewWindow};

pub(super) mod control;
pub(super) mod host;
mod worker;

pub use control::NativePresentationViewport;
pub(super) use worker::PresentationWorker;
pub use worker::{NativePresentationStop, NativePresentationTask};

type Prepare<R> = Box<dyn FnOnce(&WebviewWindow<R>) -> io::Result<NativePresentationTask> + Send + 'static>;

/// An optional attachment to a host-created native window.
///
/// The preparation callback runs on the window's UI thread. Use scoped platform-WebView
/// access for STA-bound observations; do not move WebView COM objects into the task. Capture
/// lightweight configuration and renderer-safe handles, then construct GPU resources inside
/// [`NativePresentationTask`]. Preparation must not wait for rendering or worker retirement.
/// Inspect or subscribe to the supplied host-owned window; do not create or destroy windows
/// from the callback. The host coordinates their lifetime with the task's retirement.
pub struct NativePresentation<R: Runtime> {
    prepare: Prepare<R>,
    control: Option<Arc<control::ControlHandler>>,
}

impl<R: Runtime> NativePresentation<R> {
    /// Registers UI-thread preparation for the host's newly created presentation window.
    pub fn new<F>(prepare: F) -> Self
    where
        F: FnOnce(&WebviewWindow<R>) -> io::Result<NativePresentationTask> + Send + 'static,
    {
        Self {
            prepare: Box::new(prepare),
            control: None,
        }
    }

    /// Adds bounded metadata control for the retained presentation window. The callback runs
    /// on its UI thread with fresh host/WebView metrics. It must only validate/coalesce metadata:
    /// no filesystem/GPU work, renderer waits or native-resource ownership belongs here.
    /// The host rejects new calls after presentation cancellation or completion.
    pub fn with_control<F>(mut self, handler: F) -> Self
    where
        F: Fn(&str, NativePresentationViewport) -> Result<String, String> + Send + Sync + 'static,
    {
        self.control = Some(Arc::new(handler));
        self
    }

    pub(super) fn prepare(self, window: &WebviewWindow<R>) -> io::Result<NativePresentationTask> {
        (self.prepare)(window)
    }
}

#[cfg(test)]
mod tests;
