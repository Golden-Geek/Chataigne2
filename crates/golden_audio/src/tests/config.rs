use crate::{
    AudioChannelId, AudioConfiguration, AudioRouteId, EngineLimits, GainDb, InputPatchRoute, MonitorRoute,
    OutputPatchRoute, PhysicalChannelKey, VirtualInputChannel, VirtualOutputChannel,
};

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
