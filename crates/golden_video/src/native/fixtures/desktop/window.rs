use std::{marker::PhantomData, rc::Rc};
use windows::Win32::Foundation::{HWND, POINT, RECT};
use windows::Win32::Graphics::Gdi::ClientToScreen;
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForWindow, SetThreadDpiAwarenessContext,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetClientRect, GetForegroundWindow, GetWindowRect, GetWindowThreadProcessId,
};
use windows::core::Result;

use super::{DesktopCaptureConfig, invalid};

// DPI context restoration must happen on the entering thread, even if COM permits moving a handle.
pub(super) struct DpiScope(DPI_AWARENESS_CONTEXT, PhantomData<Rc<()>>);
impl DpiScope {
    pub fn enter() -> Result<Self> {
        // SAFETY: changes only this capture worker's coordinate interpretation, never OS display settings.
        let old = unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
        if old.0.is_null() {
            return Err(invalid("cannot authenticate physical capture coordinates"));
        }
        Ok(Self(old, PhantomData))
    }
}
impl Drop for DpiScope {
    fn drop(&mut self) {
        // SAFETY: restores the calling worker's previously observed DPI context.
        unsafe {
            SetThreadDpiAwarenessContext(self.0);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
/// Physical window/client bounds and actual window DPI from one authenticated observation.
pub struct DesktopGeometry {
    /// Physical desktop rectangle of the exact owned window.
    pub window: [i32; 4],
    /// Physical desktop client rectangle; every exported pixel must be inside it.
    pub client: [i32; 4],
    /// Actual DPI of the same native window.
    pub dpi: u32,
}

pub(super) fn contains(outer: [i32; 4], inner: [i32; 4]) -> bool {
    inner[0] >= outer[0] && inner[1] >= outer[1] && inner[2] <= outer[2] && inner[3] <= outer[3]
}

pub(super) fn geometry(config: DesktopCaptureConfig) -> Result<DesktopGeometry> {
    let hwnd = HWND(config.hwnd as *mut _);
    let mut owner = 0;
    let mut window = RECT::default();
    let mut client = RECT::default();
    let mut origin = POINT::default();
    // SAFETY: borrowed observed HWND, initialized scalar/rectangle outputs; no input or window mutation.
    let dpi = unsafe {
        GetWindowThreadProcessId(hwnd, Some(&mut owner));
        if owner != config.pid || GetForegroundWindow() != hwnd {
            return Err(invalid("capture requires the exact owned foreground HWND/PID"));
        }
        GetWindowRect(hwnd, &mut window)?;
        GetClientRect(hwnd, &mut client)?;
        ClientToScreen(hwnd, &mut origin).ok()?;
        GetDpiForWindow(hwnd)
    };
    let client = [
        origin.x,
        origin.y,
        origin
            .x
            .checked_add(client.right)
            .ok_or_else(|| invalid("client x overflow"))?,
        origin
            .y
            .checked_add(client.bottom)
            .ok_or_else(|| invalid("client y overflow"))?,
    ];
    if dpi == 0 || !contains(client, config.region) {
        return Err(invalid("capture region is outside the owned client"));
    }
    Ok(DesktopGeometry {
        window: [window.left, window.top, window.right, window.bottom],
        client,
        dpi,
    })
}
