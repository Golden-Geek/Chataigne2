use std::path::Path;
use windows::{
    Win32::{
        Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM},
        System::LibraryLoader::GetModuleHandleW,
        UI::WindowsAndMessaging::*,
    },
    core::w,
};

pub fn run(report: &Path) {
    unsafe {
        let instance = HINSTANCE(GetModuleHandleW(None).unwrap().0);
        let class = WNDCLASSW {
            lpfnWndProc: Some(procedure),
            hInstance: instance,
            lpszClassName: w!("GoldenIoOwnedFixture"),
            ..Default::default()
        };
        assert_ne!(RegisterClassW(&class), 0);
        // Never show/activate this window; only the owned-process close primitive targets it.
        let window = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class.lpszClassName,
            w!("Golden IO owned fixture"),
            WS_OVERLAPPED,
            0,
            0,
            40,
            40,
            None,
            None,
            Some(instance),
            None,
        )
        .unwrap();
        SetTimer(Some(window), 1, 20_000, None);
        std::fs::write(report, b"owned hidden window ready").unwrap();
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
