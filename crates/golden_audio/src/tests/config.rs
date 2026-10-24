use crate::{
    AudioChannelId, AudioConfiguration, AudioRouteId, EngineLimits, GainDb, InputPatchRoute, MonitorRoute,
    OutputPatchRoute, PhysicalChannelKey, VirtualInputChannel, VirtualOutputChannel,
};

#[test]
fn serialized_gain_rejects_invalid_values_from_json_and_other_serde_formats() {
    for value in [GainDb::SILENCE_DB - 0.1, GainDb::MAX_DB + 0.1, f32::MIN, f32::MAX] {
        assert!(serde_json::from_value::<GainDb>(serde_json::json!(value)).is_err());
    }
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let deserializer = serde::de::value::F32Deserializer::<serde::de::value::Error>::new(value);
        assert!(<GainDb as serde::Deserialize>::deserialize(deserializer).is_err());
    }
    for gain in [GainDb::SILENCE, GainDb::UNITY, GainDb::new(GainDb::MAX_DB).unwrap()] {
        let encoded = serde_json::to_value(gain).unwrap();
        assert_eq!(encoded, serde_json::json!(gain.get()));
        assert_eq!(serde_json::from_value::<GainDb>(encoded).unwrap(), gain);
    }
}

#[test]
fn nested_engine_configuration_fails_before_invalid_scalars_can_be_used() {
    let original = serde_json::to_value(crate::AudioEngineConfig::default()).unwrap();
    for (field, invalid) in [("sample_rate", 0), ("internal_block_frames", 0)] {
        let mut value = original.clone();
        value[field] = serde_json::json!(invalid);
        assert!(serde_json::from_value::<crate::AudioEngineConfig>(value).is_err());
    }
    let request = serde_json::json!({"SetMasterGain": {"gain": 25.0}});
    assert!(serde_json::from_value::<crate::AudioCommand>(request).is_err());
}

#[test]
fn gain_validates_and_converts_decibels() {
    assert_eq!(GainDb::SILENCE.to_linear(), 0.0);
    assert!((GainDb::UNITY.to_linear() - 1.0).abs() < f32::EPSILON);
    assert!((GainDb::new(-6.0206).unwrap().to_linear() - 0.5).abs() < 0.0001);
    assert!(GainDb::new(f32::NAN).is_err());
    assert!(GainDb::new(-120.1).is_err());
    assert!(GainDb::new(24.1).is_err());
}

#[test]
fn configuration_rejects_duplicate_and_unresolved_channels() {
    let input = AudioChannelId::new();
    let output = AudioChannelId::new();
    let mut config = AudioConfiguration::empty();
    config.virtual_inputs = vec![
        VirtualInputChannel {
            id: input,
            label: "Input 1".to_owned(),
        },
        VirtualInputChannel {
            id: input,
            label: "Duplicate".to_owned(),
        },
    ];
    assert!(config.validate(&EngineLimits::default()).is_err());

    config.virtual_inputs.truncate(1);
    config.virtual_outputs.push(VirtualOutputChannel {
        id: output,
        label: "Output 1".to_owned(),
        gain: GainDb::UNITY,
    });
    config.monitoring.push(MonitorRoute {
        id: AudioRouteId::new(),
        source: AudioChannelId::new(),
        destination: output,
        gain: GainDb::UNITY,
    });
    assert!(config.validate(&EngineLimits::default()).is_err());

    config.monitoring[0].source = input;
    config.validate(&EngineLimits::default()).unwrap();
}

#[test]
fn configuration_owns_and_validates_ordered_physical_channel_inventories() {
    let input = AudioChannelId::new();
    let output = AudioChannelId::new();
    let input_key = PhysicalChannelKey::new("input:3").unwrap();
    let output_key = PhysicalChannelKey::new("output:5").unwrap();
    let mut config = AudioConfiguration::empty();
    config.physical_inputs.push(input_key.clone());
    config.physical_outputs.push(output_key.clone());
    config.virtual_inputs.push(VirtualInputChannel {
        id: input,
        label: "Input".to_owned(),
    });
    config.virtual_outputs.push(VirtualOutputChannel {
        id: output,
        label: "Output".to_owned(),
        gain: GainDb::UNITY,
    });
    config.input_patch.push(InputPatchRoute {
        id: AudioRouteId::new(),
        source: input_key,
        destination: input,
        gain: GainDb::UNITY,
    });
    config.output_patch.push(OutputPatchRoute {
        id: AudioRouteId::new(),
        source: output,
        destination: output_key,
        gain: GainDb::UNITY,
    });
    config.validate(&EngineLimits::default()).unwrap();

    config.physical_inputs.clear();
    assert!(config.validate(&EngineLimits::default()).is_err());

    config.physical_inputs = vec![
        PhysicalChannelKey::new("input:3").unwrap(),
        PhysicalChannelKey::new("input:3").unwrap(),
    ];
    assert!(config.validate(&EngineLimits::default()).is_err());
}
