use std::io::{self, Error};
use std::sync::{Arc, Mutex};

use tauri::{Manager, Runtime, WebviewWindow};

use super::control::{ControlState, ControlTicket};
use super::{NativePresentation, PresentationWorker};

#[derive(Default)]
struct Retirement {
    finished: bool,
    failed: bool,
    requested_exit_code: Option<i32>,
    destroy_requested: bool,
    window_released: bool,
}

#[derive(Debug, PartialEq)]
pub(crate) enum ExitAction {
    Wait,
    DestroyWindow,
    Allow,
    Redirect(i32),
}

impl Retirement {
    fn request_exit(&mut self, code: i32) -> ExitAction {
        self.requested_exit_code.get_or_insert(code);
        if !self.finished {
            return ExitAction::Wait;
        }
        if !self.window_released {
            if self.destroy_requested {
                return ExitAction::Wait;
            }
            self.destroy_requested = true;
            return ExitAction::DestroyWindow;
        }
        match self.exit_code() {
            Some(expected) if expected != code => ExitAction::Redirect(expected),
            _ => ExitAction::Allow,
        }
    }

    fn complete(&mut self, succeeded: bool) -> Option<i32> {
        self.finished = true;
        self.failed |= !succeeded;
        self.exit_code()
    }

    fn exit_code(&self) -> Option<i32> {
        self.requested_exit_code.map(|code| if self.failed { 1 } else { code })
    }
}

/// Host-owned control contains no WebView COM objects or renderer resources.
#[derive(Default)]
pub(crate) struct PresentationHost {
    control: Mutex<ControlState>,
    retirement: Mutex<Retirement>,
    worker: Mutex<Option<PresentationWorker>>,
    error: Mutex<Option<Error>>,
}

impl PresentationHost {
    pub(crate) fn attach<R: Runtime>(
        self: &Arc<Self>,
        presentation: NativePresentation<R>,
        window: &WebviewWindow<R>,
    ) -> io::Result<()> {
        configure_underlay(window)?;
        let control = presentation.control.clone();
        let task = presentation.prepare(window)?;
        self.control.lock().unwrap().attach(window.label(), control);
        let state = self.clone();
        let app = window.app_handle().clone();
        let worker = PresentationWorker::spawn(task, move |succeeded| {
            state.control.lock().unwrap().close();
            // Task locals and captures have retired on their worker before this signal.
            // AppHandle queues exit onto Tauri's event loop; no COM/window work runs here.
            let code = state.retirement.lock().unwrap().complete(succeeded);
            // A renderer ending does not authorize discarding unsaved work. Keep the
            // editor alive unless an explicit host/user exit is already waiting.
            if let Some(code) = code {
                app.exit(code);
            }
        })?;
        *self.worker.lock().unwrap() = Some(worker);
        Ok(())
    }

    pub(crate) fn exit_action(&self, code: i32) -> ExitAction {
        self.control.lock().unwrap().close();
        let action = self.retirement.lock().unwrap().request_exit(code);
        if action == ExitAction::Wait
            && let Some(worker) = self.worker.lock().unwrap().as_ref()
        {
            worker.request_stop();
        }
        action
    }

    pub(crate) fn window_destroyed(&self) -> Option<i32> {
        self.control.lock().unwrap().close();
        let early = !self.retirement.lock().unwrap().finished;
        if early {
            self.record_error(Error::other(
                "native window was destroyed before presentation retirement",
            ));
        }
        let mut retirement = self.retirement.lock().unwrap();
        retirement.window_released = true;
        retirement.exit_code()
    }

    pub(crate) fn teardown_failed(&self, error: Error) -> i32 {
        self.control.lock().unwrap().close();
        self.record_error(error);
        // The worker has retired, so an OS teardown error can leave with an explicit
        // failure instead of hanging the UI or claiming successful window destruction.
        self.retirement.lock().unwrap().window_released = true;
        1
    }

    fn record_error(&self, error: Error) {
        self.error.lock().unwrap().get_or_insert(error);
        self.retirement.lock().unwrap().failed = true;
    }

    pub(crate) fn fail_before_start(&self, error: Error) {
        self.control.lock().unwrap().close();
        self.record_error(error);
        self.retirement.lock().unwrap().complete(false);
    }

    /// Called only after run_return: joining cannot block an active WebView STA.
    pub(crate) fn finish(&self) -> io::Result<()> {
        self.control.lock().unwrap().close();
        let worker = self.worker.lock().unwrap().take();
        let result = worker.map_or(Ok(()), |worker| {
            worker.request_stop();
            worker.join()
        });
        if let Some(error) = self.error.lock().unwrap().take() {
            return Err(error);
        }
        result
    }

    pub(super) fn admit_control(&self, window: &str, request: &str) -> Result<ControlTicket, String> {
        self.control.lock().unwrap().admit(window, request)
    }
}

pub(crate) fn validate_platform(enabled: bool, headless: bool) -> io::Result<()> {
    if enabled && !headless && !cfg!(target_os = "windows") {
        return Err(Error::new(
            io::ErrorKind::Unsupported,
            "native underlay presentation is supported only on Windows",
        ));
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn configure_underlay<R: Runtime>(window: &WebviewWindow<R>) -> io::Result<()> {
    use windows::Win32::Foundation::{GetLastError, SetLastError, WIN32_ERROR};
    use windows::Win32::UI::WindowsAndMessaging::{GWL_STYLE, GetWindowLongPtrW, SetWindowLongPtrW, WS_CLIPCHILDREN};

    let hwnd = window.hwnd().map_err(Error::other)?;
    // The shared host owns this newly constructed HWND. Clearing child clipping follows
    // locked Wry's underlay route; platform WebView/COM inspection stays on this UI thread.
    unsafe {
        SetLastError(WIN32_ERROR(0));
        let style = GetWindowLongPtrW(hwnd, GWL_STYLE);
        if style == 0 && GetLastError().0 != 0 {
            return Err(Error::last_os_error());
        }
        SetLastError(WIN32_ERROR(0));
        if SetWindowLongPtrW(hwnd, GWL_STYLE, style & !(WS_CLIPCHILDREN.0 as isize)) == 0 && GetLastError().0 != 0 {
            return Err(Error::last_os_error());
        }
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn configure_underlay<R: Runtime>(_window: &WebviewWindow<R>) -> io::Result<()> {
    validate_platform(true, false)
}

#[cfg(test)]
mod tests;
