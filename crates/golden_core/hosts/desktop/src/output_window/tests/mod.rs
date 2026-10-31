use super::*;

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
