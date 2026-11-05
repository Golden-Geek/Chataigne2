use std::path::Path;
use windows::{
    Win32::{
        Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM},
        System::LibraryLoader::GetModuleHandleW,
        System::Threading::{GetStartupInfoW, STARTUPINFOW},
        UI::WindowsAndMessaging::*,
    },
    core::w,
};

pub fn run(report: &Path, show_on_launch: bool) {
    unsafe {
        let instance = HINSTANCE(GetModuleHandleW(None).unwrap().0);
        let class = WNDCLASSW {
            lpfnWndProc: Some(procedure),
            hInstance: instance,
            lpszClassName: w!("GoldenIoOwnedFixture"),
            ..Default::default()
        };
        assert_ne!(RegisterClassW(&class), 0);
        // The launch case honors the controller's actual STARTUPINFO show policy.
        // The original close-only case remains hidden throughout.
        let window = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class.lpszClassName,
            w!("Golden IO owned fixture"),
            if show_on_launch {
                WS_OVERLAPPEDWINDOW
            } else {
                WS_OVERLAPPED
            },
            0,
            0,
            if show_on_launch { 320 } else { 40 },
            if show_on_launch { 180 } else { 40 },
            None,
            None,
            Some(instance),
            None,
        )
        .unwrap();
        SetTimer(Some(window), 1, 20_000, None);
        if show_on_launch {
            let mut startup = STARTUPINFOW::default();
            GetStartupInfoW(&mut startup);
            let _ = ShowWindow(window, SW_SHOWDEFAULT);
            std::fs::write(
                report,
                format!(
                    "pid={}\nvisible={}\nstartup_show={}\nstartup_flags={}\n",
                    std::process::id(),
                    IsWindowVisible(window).as_bool(),
                    startup.wShowWindow,
                    startup.dwFlags.0,
                ),
            )
            .unwrap();
        } else {
            std::fs::write(report, b"owned hidden window ready").unwrap();
        }
        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}
unsafe extern "system" fn procedure(window: HWND, message: u32, word: WPARAM, parameter: LPARAM) -> LRESULT {
    match message {
        WM_CLOSE | WM_TIMER => {
            let _ = unsafe { DestroyWindow(window) };
            LRESULT(0)
        }
        WM_DESTROY => {
            unsafe {
                PostQuitMessage(0);
            }
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(window, message, word, parameter) },
    }
}
