use std::collections::{HashMap, HashSet};

#[cfg(windows)]
use std::ptr;

#[cfg(windows)]
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, RECT},
    UI::WindowsAndMessaging::{
        EnumWindows, GetWindowRect, GetWindowTextLengthW, GetWindowTextW,
        GetWindowThreadProcessId, IsWindowVisible, SetWindowPos, ShowWindow,
        HWND_NOTOPMOST, HWND_TOPMOST, SW_HIDE, SW_MAXIMIZE, SW_MINIMIZE, SW_RESTORE,
        SW_SHOW, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER,
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
pub(super) fn visible_window_counts_by_pid() -> HashMap<u32, usize> {
    let mut counts = HashMap::new();
    for window in enumerate_windows() {
        if window.visible {
            *counts.entry(window.pid).or_default() += 1;
        }
    }
    counts
}

#[cfg(not(windows))]
pub(super) fn visible_window_counts_by_pid() -> HashMap<u32, usize> {
    HashMap::new()
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
    let windows = enumerate_windows()
        .into_iter()
        .filter(|window| wanted.contains(&window.pid))
        .collect::<Vec<_>>();

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
                        if always_on_top {
                            HWND_TOPMOST
                        } else {
                            HWND_NOTOPMOST
                        },
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
fn enumerate_windows() -> Vec<WindowInfo> {
    unsafe extern "system" fn collect_window(hwnd: HWND, lparam: LPARAM) -> i32 {
        let windows = &mut *(lparam as *mut Vec<WindowInfo>);
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid == 0 {
            return 1;
        }

        let visible = IsWindowVisible(hwnd) != 0;
        let title = read_window_title(hwnd);
        if !visible && title.trim().is_empty() {
            return 1;
        }

        let mut rect = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        let _ = GetWindowRect(hwnd, &mut rect);
        windows.push(WindowInfo { hwnd, pid, visible });
        1
    }

    let mut windows = Vec::new();
    unsafe {
        EnumWindows(Some(collect_window), &mut windows as *mut _ as LPARAM);
    }
    windows
}

#[cfg(windows)]
fn read_window_title(hwnd: HWND) -> String {
    unsafe {
        let length = GetWindowTextLengthW(hwnd);
        if length <= 0 {
            return String::new();
        }

        let mut buffer = vec![0u16; length as usize + 1];
        let copied = GetWindowTextW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32);
        if copied <= 0 {
            return String::new();
        }

        String::from_utf16_lossy(&buffer[..copied as usize])
    }
}
