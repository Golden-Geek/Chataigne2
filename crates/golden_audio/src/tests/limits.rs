use crate::EngineLimits;

#[test]
fn default_limits_are_valid() {
    EngineLimits::default().validate().unwrap();
}

#[test]
fn limits_reject_zero_and_invalid_cache_or_fft_shapes() {
    let limits = EngineLimits {
        command_queue_capacity: 0,
        ..EngineLimits::default()
    };
    assert!(limits.validate().is_err());

    let defaults = EngineLimits::default();
    let limits = EngineLimits {
        resident_asset_threshold_bytes: defaults.resident_cache_budget_bytes + 1,
        ..defaults
    };
    assert!(limits.validate().is_err());

    let limits = EngineLimits {
        max_fft_frames: 1_000,
        ..EngineLimits::default()
    };
    assert!(limits.validate().is_err());
}
