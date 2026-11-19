use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::w;

use crate::native::WindowTarget;

pub(in crate::native) struct OwnedWindow(HWND);

impl OwnedWindow {
    pub(in crate::native) fn new() -> Self {
        // SAFETY: built-in window class, no borrowed creation data. This test owns destruction.
        Self(
            unsafe {
                CreateWindowExW(
                    WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
                    w!("STATIC"),
                    w!("Golden window pixel test"),
                    WS_POPUP | WS_VISIBLE,
                    30,
                    30,
                    64,
                    64,
                    None,
                    None,
                    None,
                    None,
                )
            }
            .unwrap(),
        )
    }

    pub(in crate::native) fn target(&self) -> WindowTarget {
        // SAFETY: the guard outlives this test's endpoint and its actual GPU retirement.
        unsafe { WindowTarget::from_raw(self.0.0 as isize) }.unwrap()
    }

    #[cfg(feature = "qualification")]
    pub(in crate::native) fn set_visible(&self, visible: bool) {
        // SAFETY: the window is owned by this test and this method runs on its creating thread.
        let _ = unsafe { ShowWindow(self.0, if visible { SW_SHOWNOACTIVATE } else { SW_HIDE }) };
        pump();
    }
}

impl Drop for OwnedWindow {
    fn drop(&mut self) {
        // SAFETY: only this guard owns the test HWND, on its creating thread.
        unsafe { DestroyWindow(self.0) }.unwrap();
    }
}

pub(in crate::native) fn pump() {
    let mut message = MSG::default();
    // SAFETY: this test thread's message queue and initialized message storage.
    unsafe {
        while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}
