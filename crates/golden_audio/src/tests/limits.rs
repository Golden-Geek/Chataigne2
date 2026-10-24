use crate::{EngineLimits, FrameCount, SampleRate};

#[test]
fn serialized_rates_and_frame_counts_cannot_bypass_constructor_invariants() {
    for value in [0, SampleRate::MIN - 1, SampleRate::MAX + 1, u32::MAX] {
        assert!(serde_json::from_value::<SampleRate>(serde_json::json!(value)).is_err());
    }
    assert!(serde_json::from_str::<FrameCount>("0").is_err());
    for invalid in ["-1", "1.5", "null", "true", "\"48000\""] {
        assert!(serde_json::from_str::<SampleRate>(invalid).is_err());
        assert!(serde_json::from_str::<FrameCount>(invalid).is_err());
    }
}

#[test]
fn validated_scalar_wire_values_stay_transparent_and_roundtrip_at_boundaries() {
    for value in [SampleRate::MIN, 44_100, 48_000, SampleRate::MAX] {
        let rate = SampleRate::new(value).unwrap();
        let encoded = serde_json::to_value(rate).unwrap();
        assert_eq!(encoded, serde_json::json!(value));
        assert_eq!(serde_json::from_value::<SampleRate>(encoded).unwrap(), rate);
    }
    for value in [1, 128, u32::MAX] {
        let frames = FrameCount::new(value).unwrap();
        let encoded = serde_json::to_value(frames).unwrap();
        assert_eq!(encoded, serde_json::json!(value));
        assert_eq!(serde_json::from_value::<FrameCount>(encoded).unwrap(), frames);
    }
}

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
