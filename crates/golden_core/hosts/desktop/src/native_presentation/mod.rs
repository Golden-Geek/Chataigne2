//! Optional native presentation attached to the shared desktop host.
//!
//! The host creates and retains the window. Preparation runs on its UI thread; rendering
//! resources are constructed, used and retired inside the returned worker task.

use std::io;

use tauri::{Runtime, WebviewWindow};

pub(super) mod host;
mod worker;

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
}

impl<R: Runtime> NativePresentation<R> {
    /// Registers UI-thread preparation for the host's newly created presentation window.
    pub fn new<F>(prepare: F) -> Self
    where
        F: FnOnce(&WebviewWindow<R>) -> io::Result<NativePresentationTask> + Send + 'static,
    {
        Self {
            prepare: Box::new(prepare),
        }
    }

    pub(super) fn prepare(self, window: &WebviewWindow<R>) -> io::Result<NativePresentationTask> {
        (self.prepare)(window)
    }
}

#[cfg(test)]
mod tests;
