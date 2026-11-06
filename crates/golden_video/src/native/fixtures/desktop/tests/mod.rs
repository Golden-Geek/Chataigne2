use super::*;

fn config(region: [i32; 4]) -> DesktopCaptureConfig {
    DesktopCaptureConfig {
        adapter: AdapterId { high: 0, low: 1 },
        output: 0,
        hwnd: 1,
        pid: 1,
        region,
    }
}

#[test]
fn owned_region_admission_bounds_memory_without_requiring_a_window_or_gpu() {
    assert_eq!(dimensions(config([-1920, -100, -1664, -68])).unwrap(), (256, 32));
    for bounds in [[0, 0, 0, 1], [0, 0, 257, 1], [0, 0, 1, 33], [i32::MIN, 0, i32::MAX, 1]] {
        assert!(dimensions(config(bounds)).is_err());
    }
    assert!(
        dimensions(DesktopCaptureConfig {
            hwnd: 0,
            ..config([0, 0, 1, 1])
        })
        .is_err()
    );
    assert!(
        dimensions(DesktopCaptureConfig {
            pid: 0,
            ..config([0, 0, 1, 1])
        })
        .is_err()
    );
}

#[test]
fn region_must_stay_within_the_exact_client_and_output_including_negative_origins() {
    assert!(window::contains([-1920, -100, 0, 980], [-1910, 0, -1782, 8]));
    assert!(!window::contains([-1920, -100, 0, 980], [-1921, 0, -1782, 8]));
    assert!(!window::contains([-1920, -100, 0, 980], [-1910, 0, 1, 8]));
}
