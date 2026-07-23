use golden_audio::{
    AudioBufferPolicy, AudioDeviceInspectorState, DeviceSupervisor, DeviceSupervisorConfig, SampleRate,
};

#[test]
fn canonical_device_inspector_projection_is_data_only() {
    let supervisor = DeviceSupervisor::new(
        DeviceSupervisorConfig::default(),
        SampleRate::new(48_000).unwrap(),
        AudioBufferPolicy::Automatic,
    )
    .unwrap();
    let state: AudioDeviceInspectorState = supervisor.inspector_state();
    let value = serde_json::to_value(state).unwrap();
    let object = value.as_object().unwrap();

    assert!(object.contains_key("input"));
    assert!(object.contains_key("output"));
    assert!(object.contains_key("devices"));
    assert!(object.keys().all(|key| {
        !key.starts_with("set") && !key.starts_with("select") && !key.starts_with("refresh") && !key.starts_with("open")
    }));
}
