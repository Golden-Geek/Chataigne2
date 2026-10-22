use std::collections::{HashMap, HashSet};

#[cfg(windows)]
use std::ptr;

#[cfg(windows)]
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM},
    UI::WindowsAndMessaging::{
        EnumWindows, GetWindowTextLengthW, GetWindowThreadProcessId, IsWindowVisible, SetWindowPos, ShowWindow,
        HWND_NOTOPMOST, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SW_HIDE, SW_MAXIMIZE,
        SW_MINIMIZE, SW_RESTORE, SW_SHOW,
    },
};

use crate::app::module::common::app_control::WindowAction;

use super::saturating_i32;

#[cfg(windows)]
#[derive(Clone, Debug)]
struct WindowInfo {
    hwnd: HWND,
    pid: u32,
    visible: bool,
}

#[cfg(windows)]
pub(super) fn visible_window_counts_by_pid() -> Result<HashMap<u32, usize>, String> {
    let mut counts = HashMap::new();
    for window in enumerate_windows()? {
        if window.visible {
            *counts.entry(window.pid).or_default() += 1;
        }
    }
    Ok(counts)
}

#[cfg(not(windows))]
pub(super) fn visible_window_counts_by_pid() -> Result<HashMap<u32, usize>, String> {
    Ok(HashMap::new())
}

pub(super) fn window_count_for_pids(pids: &HashSet<u32>, counts_by_pid: &HashMap<u32, usize>) -> i32 {
    saturating_i32(
        pids.iter()
            .map(|pid| counts_by_pid.get(pid).copied().unwrap_or(0))
            .sum(),
    )
}

#[cfg(windows)]
pub(super) fn control_windows_for_pids(
    pids: &[u32],
    action: WindowAction,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    always_on_top: bool,
) -> Result<usize, String> {
    let wanted = pids.iter().copied().collect::<HashSet<_>>();
    let windows = enumerate_windows()?
        .into_iter()
        .filter(|window| wanted.contains(&window.pid))
        .collect::<Vec<_>>();

    if windows.len() > 64 {
        return Err("App Control refuses a batch above 64 windows".into());
    }
    if windows.is_empty() {
        return Ok(0);
    }

    for window in &windows {
        unsafe {
            match action {
                WindowAction::Move => {
                    SetWindowPos(
                        window.hwnd,
                        ptr::null_mut(),
                        x,
                        y,
                        0,
                        0,
                        SWP_NOACTIVATE | SWP_NOSIZE | SWP_NOZORDER,
                    );
                }
                WindowAction::Resize => {
                    SetWindowPos(
                        window.hwnd,
                        ptr::null_mut(),
                        0,
                        0,
                        width.max(1),
                        height.max(1),
                        SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOZORDER,
                    );
                }
                WindowAction::Bounds => {
                    SetWindowPos(
                        window.hwnd,
                        ptr::null_mut(),
                        x,
                        y,
                        width.max(1),
                        height.max(1),
                        SWP_NOACTIVATE | SWP_NOZORDER,
                    );
                }
                WindowAction::Minimize => {
                    ShowWindow(window.hwnd, SW_MINIMIZE);
                }
                WindowAction::Maximize => {
                    ShowWindow(window.hwnd, SW_MAXIMIZE);
                }
                WindowAction::Restore => {
                    ShowWindow(window.hwnd, SW_RESTORE);
                }
                WindowAction::Tray => {
                    ShowWindow(window.hwnd, SW_HIDE);
                }
                WindowAction::Show => {
                    ShowWindow(window.hwnd, SW_SHOW);
                }
                WindowAction::AlwaysOnTop => {
                    SetWindowPos(
                        window.hwnd,
                        if always_on_top { HWND_TOPMOST } else { HWND_NOTOPMOST },
                        0,
                        0,
                        0,
                        0,
                        SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE,
                    );
                }
            }
        }
    }

    Ok(windows.len())
}

#[cfg(not(windows))]
pub(super) fn control_windows_for_pids(
    _pids: &[u32],
    _action: WindowAction,
    _x: i32,
    _y: i32,
    _width: i32,
    _height: i32,
    _always_on_top: bool,
) -> Result<usize, String> {
    Err("application window control is currently supported on Windows only".to_string())
}

#[cfg(windows)]
fn enumerate_windows() -> Result<Vec<WindowInfo>, String> {
    struct Enumeration {
        windows: Vec<WindowInfo>,
        visited: usize,
        overflow: bool,
    }
    unsafe extern "system" fn collect_window(hwnd: HWND, lparam: LPARAM) -> i32 {
        let state = &mut *(lparam as *mut Enumeration);
        if state.visited >= 4096 {
            state.overflow = true;
            return 0;
        }
        state.visited += 1;
        let mut pid = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid == 0 {
            return 1;
        }
        let visible = IsWindowVisible(hwnd) != 0;
        if visible || GetWindowTextLengthW(hwnd) > 0 {
            state.windows.push(WindowInfo { hwnd, pid, visible });
        }
        1
    }
    let mut state = Enumeration {
        windows: Vec::new(),
        visited: 0,
        overflow: false,
    };
    unsafe {
        EnumWindows(Some(collect_window), &mut state as *mut _ as LPARAM);
    }
    if state.overflow {
        Err("App Control window inventory exceeds 4096 entries".into())
    } else {
        Ok(state.windows)
    }
}
