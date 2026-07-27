use tauri::{PhysicalPosition, PhysicalSize};

use crate::window_state::{StoredMonitor, StoredPosition, StoredSize, StoredWindowState};

fn monitor(name: &str, x: i32, y: i32, width: u32, height: u32) -> StoredMonitor {
    StoredMonitor {
        name: Some(name.to_string()),
        position: StoredPosition { x, y },
        size: StoredSize { width, height },
        scale_factor: 1.0,
    }
}

#[test]
fn restore_keeps_local_position_when_saved_monitor_moved() {
    let saved_monitor = monitor("Desk", 0, 0, 1920, 1080);
    let state = StoredWindowState::from_window(
        PhysicalPosition::new(120, 80),
        PhysicalSize::new(1000, 700),
        Some(saved_monitor),
        false,
    );
    let current_monitor = monitor("Desk", 3840, 0, 1920, 1080);

    let bounds = state.restore_bounds(&[current_monitor], None).unwrap();

    assert_eq!(bounds.position, StoredPosition { x: 3960, y: 80 });
    assert_eq!(
        bounds.size,
        StoredSize {
            width: 1000,
            height: 700
        }
    );
    assert!(!bounds.maximized);
}

#[test]
fn restore_maps_local_position_to_primary_monitor_when_saved_monitor_is_missing() {
    let saved_monitor = monitor("External", 1920, 0, 1920, 1080);
    let state = StoredWindowState::from_window(
        PhysicalPosition::new(2160, 160),
        PhysicalSize::new(900, 640),
        Some(saved_monitor),
        true,
    );
    let primary_monitor = monitor("Laptop", 0, 0, 1440, 900);

    let bounds = state
        .restore_bounds(std::slice::from_ref(&primary_monitor), Some(&primary_monitor))
        .unwrap();

    assert_eq!(bounds.position, StoredPosition { x: 240, y: 160 });
    assert_eq!(
        bounds.size,
        StoredSize {
            width: 900,
            height: 640
        }
    );
    assert!(bounds.maximized);
}

#[test]
fn restore_clamps_oversized_window_to_target_monitor() {
    let saved_monitor = monitor("External", 1920, 0, 1920, 1080);
    let state = StoredWindowState::from_window(
        PhysicalPosition::new(2500, 300),
        PhysicalSize::new(2000, 1200),
        Some(saved_monitor),
        false,
    );
    let target_monitor = monitor("Laptop", 0, 0, 1440, 900);

    let bounds = state.restore_bounds(&[target_monitor], None).unwrap();

    assert_eq!(bounds.position, StoredPosition { x: 0, y: 0 });
    assert_eq!(
        bounds.size,
        StoredSize {
            width: 1440,
            height: 900
        }
    );
}
