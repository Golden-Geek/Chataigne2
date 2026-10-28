//! One post-Present observation of the owned HWND, not a monitor history or latch timestamp.

use serde::Serialize;
use windows::Win32::Foundation::{HWND, POINT, RECT};
use windows::Win32::Graphics::Gdi::{
    ClientToScreen, DEVMODEW, DM_DISPLAYFREQUENCY, DM_PELSHEIGHT, DM_PELSWIDTH, ENUM_CURRENT_SETTINGS,
    EnumDisplaySettingsW, GetMonitorInfoW, MONITOR_DEFAULTTONULL, MONITORINFOEXW, MonitorFromWindow,
};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::{GetClientRect, GetWindowRect, MONITORINFOF_PRIMARY};
use windows::core::PCWSTR;

#[derive(Serialize)]
pub struct Report {
    pub before_qpc: u64,
    pub after_qpc: u64,
    pub window_rect_physical: [i32; 4],
    pub client_origin_physical: [i32; 2],
    pub client_extent_physical: [u32; 2],
    pub dpi: u32,
    pub monitor: Monitor,
    pub current_mode: Result<DisplayMode, String>,
    pub coordinate_context: &'static str,
    pub scope: &'static str,
}

#[derive(Serialize)]
pub struct Monitor {
    pub device_name: String,
    pub rect_physical: [i32; 4],
    pub work_rect_physical: [i32; 4],
    pub primary: bool,
}

#[derive(Serialize)]
pub struct DisplayMode {
    pub width_pixels: u32,
    pub height_pixels: u32,
    pub nominal_refresh_hz: u32,
    pub refresh_scope: &'static str,
}

pub fn observe(window: HWND) -> Result<Report, String> {
    let _dpi_scope = crate::dpi::PhysicalDpiScope::enter().map_err(|error| error.to_string())?;
    let before_qpc = crate::clock::now().map_err(|error| error.to_string())?;
    let mut window_rect = RECT::default();
    let mut client_rect = RECT::default();
    // SAFETY: this is a probe HWND; the host retains ownership through worker completion.
    unsafe {
        GetWindowRect(window, &mut window_rect).map_err(|error| format!("GetWindowRect: {error}"))?;
        GetClientRect(window, &mut client_rect).map_err(|error| format!("GetClientRect: {error}"))?;
    }
    let mut origin = POINT {
        x: client_rect.left,
        y: client_rect.top,
    };
    // SAFETY: writable point, queried under the same physical-coordinate thread context.
    unsafe { ClientToScreen(window, &mut origin) }
        .ok()
        .map_err(|error| format!("ClientToScreen: {error}"))?;
    let width = u32::try_from(i64::from(client_rect.right) - i64::from(client_rect.left))
        .map_err(|_| "invalid client width")?;
    let height = u32::try_from(i64::from(client_rect.bottom) - i64::from(client_rect.top))
        .map_err(|_| "invalid client height")?;
    // SAFETY: read-only window queries. No nearest/primary fallback for an off-desktop HWND.
    let (monitor, dpi) = unsafe {
        (
            MonitorFromWindow(window, MONITOR_DEFAULTTONULL),
            GetDpiForWindow(window),
        )
    };
    if monitor.0.is_null() {
        return Err("owned window does not intersect a monitor".into());
    }
    if dpi == 0 {
        return Err("GetDpiForWindow returned no DPI".into());
    }
    let mut info = MONITORINFOEXW::default();
    info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
    // SAFETY: MONITORINFOEXW starts with MONITORINFO; cbSize advertises the complete buffer.
    unsafe { GetMonitorInfoW(monitor, std::ptr::from_mut(&mut info).cast()) }
        .ok()
        .map_err(|error| format!("GetMonitorInfoW: {error}"))?;
    let end = info
        .szDevice
        .iter()
        .position(|value| *value == 0)
        .ok_or("unterminated monitor device name")?;
    let device_name = String::from_utf16(&info.szDevice[..end]).map_err(|_| "invalid monitor device name")?;
    if device_name.is_empty() {
        return Err("empty monitor device name".into());
    }
    let current_mode = current_mode(&info.szDevice);
    Ok(Report {
        before_qpc,
        after_qpc: crate::clock::now().map_err(|error| error.to_string())?,
        window_rect_physical: rect(window_rect),
        client_origin_physical: [origin.x, origin.y],
        client_extent_physical: [width, height],
        dpi,
        monitor: Monitor {
            device_name,
            rect_physical: rect(info.monitorInfo.rcMonitor),
            work_rect_physical: rect(info.monitorInfo.rcWork),
            primary: info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY != 0,
        },
        current_mode,
        coordinate_context: "physical_pixels_per_monitor_aware_v2_thread",
        scope: "One owned-window snapshot after final Present; not continuous monitor or display-timing evidence",
    })
}

fn current_mode(device_name: &[u16; 32]) -> Result<DisplayMode, String> {
    let mut mode = DEVMODEW {
        dmSize: std::mem::size_of::<DEVMODEW>() as u16,
        ..Default::default()
    };
    // SAFETY: GetMonitorInfoW provided this checked NUL-terminated device name; mode owns
    // the full public DEVMODEW buffer. Enumeration does not change any display setting.
    if !unsafe { EnumDisplaySettingsW(PCWSTR(device_name.as_ptr()), ENUM_CURRENT_SETTINGS, &mut mode) }.as_bool() {
        return Err("EnumDisplaySettingsW(ENUM_CURRENT_SETTINGS) returned false".into());
    }
    let required = DM_PELSWIDTH | DM_PELSHEIGHT | DM_DISPLAYFREQUENCY;
    if !mode.dmFields.contains(required) || mode.dmPelsWidth == 0 || mode.dmPelsHeight == 0 {
        return Err("current display mode omits its extent or refresh fields".into());
    }
    if mode.dmDisplayFrequency <= 1 {
        return Err(format!(
            "current mode does not report a usable nominal refresh: {}",
            mode.dmDisplayFrequency
        ));
    }
    Ok(DisplayMode {
        width_pixels: mode.dmPelsWidth,
        height_pixels: mode.dmPelsHeight,
        nominal_refresh_hz: mode.dmDisplayFrequency,
        refresh_scope: "Nominal integer Hz from DEVMODEW; not rational refresh, vblank or latch timing",
    })
}

fn rect(value: RECT) -> [i32; 4] {
    [value.left, value.top, value.right, value.bottom]
}
