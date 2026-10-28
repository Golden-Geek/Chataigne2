use super::*;

fn window() -> Window {
    Window {
        start_qpc: 1000,
        frequency: 1000,
        warmup_seconds: 1,
        maximum_seconds: 10,
    }
}

#[test]
fn sampling_uses_an_inclusive_begin_and_exclusive_end() {
    let window = window();
    window.validate().unwrap();
    assert_eq!(window.sample_begin_qpc(), 2000);
    assert!(!window.collecting(999));
    assert!(!window.collecting(1999));
    assert!(window.collecting(2000));
    assert!(window.collecting(10_999));
    assert!(!window.collecting(11_000));
    assert_eq!(window.capacity(), 9 * 60 + 1);
}

#[test]
fn zero_warmup_starts_collecting_at_the_original_clock_stamp() {
    let window = Window {
        warmup_seconds: 0,
        ..window()
    };
    window.validate().unwrap();
    assert_eq!(window.sample_begin_qpc(), window.start_qpc);
    assert!(window.collecting(window.start_qpc));
    assert!(!window.collecting(window.start_qpc - 1));
}

#[test]
fn admission_rejects_empty_unbounded_and_overflowing_clock_windows() {
    for invalid in [
        Window {
            start_qpc: 0,
            ..window()
        },
        Window {
            frequency: 0,
            ..window()
        },
        Window {
            maximum_seconds: 0,
            ..window()
        },
        Window {
            maximum_seconds: 3601,
            ..window()
        },
        Window {
            warmup_seconds: 10,
            ..window()
        },
        Window {
            warmup_seconds: 11,
            ..window()
        },
        Window {
            frequency: u64::MAX,
            ..window()
        },
        Window {
            start_qpc: u64::MAX - 1,
            ..window()
        },
    ] {
        assert!(invalid.validate().is_err(), "{invalid:?}");
    }
}

#[test]
fn admitted_duration_bounds_the_sixty_hertz_log() {
    let full = Window {
        maximum_seconds: 3600,
        warmup_seconds: 0,
        ..window()
    };
    full.validate().unwrap();
    assert_eq!(full.capacity(), MAX_SAMPLES);
    let one_second = Window {
        maximum_seconds: 3600,
        warmup_seconds: 3599,
        ..window()
    };
    one_second.validate().unwrap();
    assert_eq!(one_second.capacity(), 61);
}

#[test]
fn large_clock_identities_remain_exact_across_the_json_boundary() {
    let window = Window {
        start_qpc: (1_u64 << 53) + 1,
        frequency: 10_000_000,
        warmup_seconds: 30,
        maximum_seconds: 150,
    };
    window.validate().unwrap();
    assert_eq!(window.sample_begin_qpc(), window.start_qpc + 300_000_000);
    let encoded = serde_json::to_value(window).unwrap();
    assert_eq!(encoded.as_object().unwrap().len(), 4);
    assert_eq!(encoded["start_qpc"].as_u64(), Some(window.start_qpc));
    assert_eq!(serde_json::from_value::<Window>(encoded).unwrap(), window);
}
