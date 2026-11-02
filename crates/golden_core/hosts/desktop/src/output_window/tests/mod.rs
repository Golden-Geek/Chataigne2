use super::*;

#[cfg(windows)]
#[test]
fn escape_requires_focused_output_and_fresh_keydown() {
    use super::platform::keyboard_event;
    use windows::Win32::UI::WindowsAndMessaging::{WM_KEYDOWN, WM_KEYUP};
    assert_eq!(
        keyboard_event(WM_KEYDOWN, 0x1b, 1, true),
        Some(NativeOutputEvent::EscapePressed)
    );
    assert_eq!(keyboard_event(WM_KEYDOWN, 0x1b, 1, false), None);
    assert_eq!(keyboard_event(WM_KEYDOWN, 0x1b, 1 | (1 << 30), true), None);
    assert_eq!(keyboard_event(WM_KEYUP, 0x1b, 1, true), None);
    assert_eq!(keyboard_event(WM_KEYDOWN, 0x41, 1, true), None);
}

#[cfg(windows)]
#[test]
#[ignore = "creates a finite owned native output window on the interactive desktop"]
fn hardware_native_output_window_preserves_physical_bounds_and_joins_close() {
    use windows::Win32::{Foundation::HWND, UI::WindowsAndMessaging::IsWindow};
    let bounds = NativeOutputBounds::new(-80, 40, 320, 180).unwrap();
    let window = NativeOutputWindow::new("Golden output geometry check", bounds).unwrap();
    assert_eq!(window.physical_bounds().unwrap(), bounds);
    let hwnd = HWND(window.raw_handle() as *mut _);
    assert!(unsafe { IsWindow(Some(hwnd)) }.as_bool());
    window.close().unwrap();
    assert!(!unsafe { IsWindow(Some(hwnd)) }.as_bool());
}

#[cfg(windows)]
#[test]
#[ignore = "briefly focuses two finite owned output windows on the interactive desktop"]
fn hardware_focused_output_reports_escape_once_without_closing_itself() {
    use std::{sync::mpsc, time::Duration};
    use windows::Win32::{
        Foundation::{HWND, LPARAM, WPARAM},
        UI::WindowsAndMessaging::{GetForegroundWindow, IsWindow, PostMessageW, SetForegroundWindow, WM_KEYDOWN},
    };
    let original = unsafe { GetForegroundWindow() };
    let (send, events) = mpsc::sync_channel(4);
    let first = NativeOutputWindow::with_events(
        "Golden focused output Escape check",
        NativeOutputBounds::new(80, 80, 160, 90).unwrap(),
        move |event| {
            let _ = send.try_send(event);
        },
    )
    .unwrap();
    let second = NativeOutputWindow::new(
        "Golden output focus peer",
        NativeOutputBounds::new(260, 80, 160, 90).unwrap(),
    )
    .unwrap();
    assert_eq!(
        unsafe { GetForegroundWindow() },
        original,
        "opening outputs must not steal focus"
    );
    let hwnd = HWND(first.raw_handle() as *mut _);
    unsafe { PostMessageW(Some(hwnd), WM_KEYDOWN, WPARAM(0x1b), LPARAM(1)) }.unwrap();
    first.physical_bounds().unwrap();
    assert!(events.try_recv().is_err(), "unfocused key messages must be ignored");
    if !unsafe { SetForegroundWindow(hwnd) }.as_bool() {
        eprintln!("Owned focus check ready: click 'Golden focused output Escape check' within 60 seconds");
    }
    let deadline = std::time::Instant::now() + Duration::from_secs(60);
    while unsafe { GetForegroundWindow() } != hwnd {
        assert!(
            std::time::Instant::now() < deadline,
            "no interactive focus reached the owned output window"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    unsafe { PostMessageW(Some(hwnd), WM_KEYDOWN, WPARAM(0x1b), LPARAM(1)) }.unwrap();
    assert_eq!(
        events.recv_timeout(Duration::from_secs(2)).unwrap(),
        NativeOutputEvent::EscapePressed
    );
    unsafe { PostMessageW(Some(hwnd), WM_KEYDOWN, WPARAM(0x1b), LPARAM(1 | (1 << 30))) }.unwrap();
    first.physical_bounds().unwrap();
    assert!(events.try_recv().is_err(), "key repeat must not enqueue another action");
    assert!(
        unsafe { IsWindow(Some(hwnd)) }.as_bool(),
        "Escape notification must retain the borrowed HWND"
    );
    let peer = HWND(second.raw_handle() as *mut _);
    assert!(unsafe { SetForegroundWindow(peer) }.as_bool());
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    while unsafe { GetForegroundWindow() } != peer {
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    unsafe { PostMessageW(Some(hwnd), WM_KEYDOWN, WPARAM(0x1b), LPARAM(1)) }.unwrap();
    first.physical_bounds().unwrap();
    assert!(events.try_recv().is_err(), "an output that lost focus must not react");
    second.close().unwrap();
    first.close().unwrap();
    if !original.is_invalid() {
        let _ = unsafe { SetForegroundWindow(original) };
    }
}

#[test]
fn physical_output_bounds_keep_signed_origins_and_reject_overflow() {
    let bounds = NativeOutputBounds::new(-3840, -2160, 3840, 2160).unwrap();
    assert_eq!(
        (bounds.left(), bounds.top(), bounds.width(), bounds.height()),
        (-3840, -2160, 3840, 2160)
    );
    for (left, top, width, height) in [
        (0, 0, 0, 1),
        (0, 0, 1, 0),
        (i32::MAX, 0, 1, 1),
        (0, i32::MAX, 1, 1),
        (0, 0, u32::MAX, 1),
    ] {
        assert!(NativeOutputBounds::new(left, top, width, height).is_err());
    }
}
