use super::*;
use std::sync::atomic::AtomicUsize;

fn viewport() -> NativePresentationViewport {
    from_metrics([100, 80], [0, 0, 100, 80], 1.0, 1.0, false).unwrap()
}

#[test]
fn control_is_window_scoped_bounded_and_invalidates_already_queued_calls() {
    let calls = Arc::new(AtomicUsize::new(0));
    let called = calls.clone();
    let mut state = ControlState::default();
    assert!(state.admit("main", "{}").is_err());
    state.attach(
        "main",
        Some(Arc::new(move |request, metrics| {
            assert_eq!(metrics, viewport());
            called.fetch_add(1, Ordering::Relaxed);
            Ok(request.into())
        })),
    );
    assert!(state.admit("foreign", "{}").is_err());
    assert!(state.admit("main", &"x".repeat(MAX_CONTROL_BYTES + 1)).is_err());
    let ticket = state.admit("main", "{}").unwrap();
    assert_eq!(ticket.dispatch("{}", viewport()).unwrap(), "{}");
    let queued = state.admit("main", "{}").unwrap();
    state.close();
    assert!(queued.dispatch("{}", viewport()).is_err());
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    assert!(state.admit("main", "{}").is_err());
}

#[test]
fn queued_ui_metadata_is_item_bounded_and_releases_capacity_without_a_wait() {
    let mut state = ControlState::default();
    state.attach("main", Some(Arc::new(|request, _| Ok(request.into()))));
    let mut tickets = (0..MAX_PENDING_CONTROLS)
        .map(|_| state.admit("main", "{}").unwrap())
        .collect::<Vec<_>>();
    assert!(state.admit("main", "{}").is_err());
    tickets.pop();
    assert!(state.admit("main", "{}").is_ok());
    state.close();
    assert!(tickets.iter().all(|ticket| ticket.dispatch("{}", viewport()).is_err()));
}

#[test]
fn unsupported_control_and_oversized_responses_do_not_report_success() {
    let mut state = ControlState::default();
    state.attach("main", None);
    assert!(state.admit("main", "{}").is_err());
    state.attach("main", Some(Arc::new(|_, _| Ok("x".repeat(MAX_CONTROL_BYTES + 1)))));
    assert!(state.admit("main", "{}").unwrap().dispatch("{}", viewport()).is_err());
    state.attach("main", Some(Arc::new(|_, _| Err("é".repeat(MAX_CONTROL_BYTES)))));
    let error = state
        .admit("main", "{}")
        .unwrap()
        .dispatch("{}", viewport())
        .unwrap_err();
    assert!(error.len() <= MAX_CONTROL_BYTES);
}

#[test]
fn physical_metrics_convert_bounds_mode_and_html_zoom_once_with_real_webview_origin() {
    let raw = from_metrics([800, 600], [10, 20, 610, 420], 1.25, 1.2, false).unwrap();
    assert_eq!((raw.webview_origin_x, raw.webview_origin_y), (10, 20));
    assert_eq!((raw.webview_width, raw.webview_height), (600, 400));
    assert_eq!(raw.css_scale, 1.5);
    let logical = from_metrics([1000, 750], [-3, 2, 797, 602], 1.25, 1.2, true).unwrap();
    assert_eq!((logical.webview_origin_x, logical.webview_origin_y), (-4, 2));
    assert_eq!((logical.webview_width, logical.webview_height), (1001, 751));
    assert_eq!((logical.physical_width, logical.physical_height), (1000, 750));
    assert_eq!(logical.css_scale, 1.5);
    assert!(from_metrics([0, 0], [0, 0, 0, 0], 1.0, 1.0, false).is_ok());
}

#[test]
fn invalid_native_metrics_fail_instead_of_guessing_a_scale() {
    for (raster, zoom) in [(0.0, 1.0), (-1.0, 1.0), (f64::NAN, 1.0), (1.0, f64::INFINITY)] {
        assert!(from_metrics([100, 100], [0, 0, 100, 100], raster, zoom, false).is_err());
    }
    assert!(from_metrics([-1, 100], [0, 0, 100, 100], 1.0, 1.0, false).is_err());
    assert!(from_metrics([100, 100], [5, 0, 4, 100], 1.0, 1.0, false).is_err());
    assert!(from_metrics([100, 100], [0, 0, i32::MAX, 100], 2.0, 1.0, true).is_err());
}
