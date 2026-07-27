use crate::{
    AudioBufferPolicy, AudioDeviceCatalogEntry, AudioDeviceDescriptor, AudioDeviceReadiness, AudioDeviceSelection,
    AudioDirection, AudioError, AudioErrorCategory, AudioInspectorError, AudioRecoveryPolicy, AudioSampleFormat,
    DeviceSupervisor, DeviceSupervisorConfig, DeviceSwitchPhase, NegotiatedStreamFormat, RetryBackoff, SampleRate,
};

use super::support::{backend_status, configuration, device, fingerprint};

fn supervisor() -> DeviceSupervisor {
    DeviceSupervisor::new(
        DeviceSupervisorConfig::default(),
        SampleRate::new(48_000).unwrap(),
        AudioBufferPolicy::Automatic,
    )
    .unwrap()
}

fn format(channels: u16) -> NegotiatedStreamFormat {
    NegotiatedStreamFormat {
        sample_rate: 48_000,
        channels,
        sample_format: AudioSampleFormat::F32,
        buffer_frames: 128,
        estimated_latency_ms: 2.666_666_7,
    }
}

fn commit_output(supervisor: &mut DeviceSupervisor, channels: u16) {
    supervisor.output.mark_primed(format(channels)).unwrap();
    supervisor.output.begin_switch().unwrap();
    supervisor.output.commit_switch().unwrap();
}

fn observe(supervisor: &mut DeviceSupervisor, now_ms: u64, devices: Vec<AudioDeviceDescriptor>) {
    let catalog = devices.iter().map(AudioDeviceCatalogEntry::from).collect();
    supervisor.observe_discovery(now_ms, vec![backend_status()], catalog, devices);
}

#[test]
fn inventory_revision_changes_only_with_catalog_or_capabilities() {
    let output = device("output", "Output", true, fingerprint("Output", 0, 2), 0, 2, false, true);
    let catalog = vec![AudioDeviceCatalogEntry::from(&output)];
    let mut supervisor = supervisor();

    supervisor.observe_discovery(0, vec![backend_status()], catalog.clone(), Vec::new());
    assert_eq!(supervisor.inspector_state().inventory_revision, 1);
    supervisor.observe_discovery(1, vec![backend_status()], catalog.clone(), Vec::new());
    assert_eq!(supervisor.inspector_state().inventory_revision, 1);
    supervisor.observe_discovery(2, Vec::new(), catalog.clone(), Vec::new());
    assert_eq!(supervisor.inspector_state().inventory_revision, 1);
    supervisor.observe_discovery(3, vec![backend_status()], catalog, vec![output]);
    assert_eq!(supervisor.inspector_state().inventory_revision, 2);
}

#[test]
fn missing_strict_selection_remains_selected_without_default_fallback() {
    let selected = device(
        "selected",
        "Selected",
        true,
        fingerprint("Selected", 0, 2),
        0,
        2,
        false,
        false,
    );
    let fallback = device(
        "default",
        "Default",
        true,
        fingerprint("Default", 0, 2),
        0,
        2,
        false,
        true,
    );
    let selection = AudioDeviceSelection::from_descriptor(&selected);
    let mut supervisor = supervisor();
    supervisor
        .output
        .configure(true, Some(selection.clone()), AudioRecoveryPolicy::WaitForSelected);

    observe(&mut supervisor, 1_000, vec![fallback]);

    let status = supervisor.output.status();
    assert_eq!(status.selected_target.as_ref(), Some(&selection.target));
    assert_eq!(status.readiness, AudioDeviceReadiness::Missing);
    assert_eq!(status.active_target, None);
    assert!(status.next_retry_ms.is_some());
}

#[test]
fn follow_default_tracks_only_the_operating_system_default() {
    let mut first = device("first", "First", true, fingerprint("First", 0, 2), 0, 2, false, false);
    let mut second = device("second", "Second", true, fingerprint("Second", 0, 2), 0, 2, false, true);
    let selection = AudioDeviceSelection::from_descriptor(&first);
    let mut supervisor = supervisor();
    supervisor
        .output
        .configure(true, Some(selection.clone()), AudioRecoveryPolicy::FollowSystemDefault);
    observe(&mut supervisor, 0, vec![first.clone(), second.clone()]);
    assert_eq!(supervisor.output.phase(), DeviceSwitchPhase::Preparing);
    commit_output(&mut supervisor, 2);
    assert_eq!(supervisor.output.status().active_target.as_ref(), Some(&second.target));
    assert_eq!(
        supervisor.output.status().selected_target.as_ref(),
        Some(&selection.target)
    );

    first.is_system_default_output = true;
    second.is_system_default_output = false;
    observe(&mut supervisor, 10, vec![second, first.clone()]);
    assert_eq!(supervisor.output.phase(), DeviceSwitchPhase::Preparing);
    commit_output(&mut supervisor, 2);
    assert_eq!(supervisor.output.status().active_target.as_ref(), Some(&first.target));
}

#[test]
fn input_and_output_selection_and_switching_are_independent() {
    let input = device("input", "Input", true, fingerprint("Input", 2, 0), 2, 0, true, false);
    let output = device("output", "Output", true, fingerprint("Output", 0, 4), 0, 4, false, true);
    let mut supervisor = supervisor();
    supervisor.input.configure(
        true,
        Some(AudioDeviceSelection::from_descriptor(&input)),
        AudioRecoveryPolicy::WaitForSelected,
    );
    supervisor.output.configure(
        true,
        Some(AudioDeviceSelection::from_descriptor(&output)),
        AudioRecoveryPolicy::WaitForSelected,
    );
    observe(&mut supervisor, 0, vec![output.clone(), input.clone()]);
    supervisor.input.mark_primed(format(2)).unwrap();
    supervisor.input.begin_switch().unwrap();
    supervisor.input.commit_switch().unwrap();
    commit_output(&mut supervisor, 4);

    assert_eq!(supervisor.input.status().active_target.as_ref(), Some(&input.target));
    assert_eq!(supervisor.output.status().active_target.as_ref(), Some(&output.target));
}

#[test]
fn format_change_reenters_prepare_and_recovery_backoff_is_deterministic() {
    let mut output = device("output", "Output", true, fingerprint("Output", 0, 2), 0, 2, false, true);
    let mut supervisor = supervisor();
    supervisor.output.configure(
        true,
        Some(AudioDeviceSelection::from_descriptor(&output)),
        AudioRecoveryPolicy::WaitForSelected,
    );
    observe(&mut supervisor, 0, vec![output.clone()]);
    commit_output(&mut supervisor, 2);

    output.supported_configurations = vec![configuration(
        AudioDirection::Output,
        2,
        AudioSampleFormat::F32,
        48_000,
        192_000,
        256,
    )];
    observe(&mut supervisor, 10, vec![output]);
    assert_eq!(supervisor.output.phase(), DeviceSwitchPhase::Preparing);

    supervisor
        .output
        .report_open_error(1_000, AudioError::new(AudioErrorCategory::DeviceBusy, "busy"));
    assert_eq!(supervisor.output.status().readiness, AudioDeviceReadiness::Busy);
    let retry_at = supervisor.output.status().next_retry_ms.unwrap();
    assert!(!supervisor.tick(retry_at - 1));
    assert!(supervisor.tick(retry_at));
    assert_eq!(supervisor.output.status().readiness, AudioDeviceReadiness::Recovering);

    let backoff = RetryBackoff::default();
    assert_eq!(
        backoff.delay_ms(3, AudioDirection::Output),
        backoff.delay_ms(3, AudioDirection::Output)
    );
    assert_ne!(
        backoff.delay_ms(3, AudioDirection::Input),
        backoff.delay_ms(3, AudioDirection::Output)
    );
}

#[test]
fn permission_denial_is_structured_and_selected_device_is_preserved() {
    let output = device("output", "Output", true, fingerprint("Output", 0, 2), 0, 2, false, true);
    let selection = AudioDeviceSelection::from_descriptor(&output);
    let mut supervisor = supervisor();
    supervisor
        .output
        .configure(true, Some(selection.clone()), AudioRecoveryPolicy::WaitForSelected);
    supervisor
        .output
        .report_open_error(0, AudioError::new(AudioErrorCategory::PermissionDenied, "denied"));

    assert_eq!(
        supervisor.output.status().readiness,
        AudioDeviceReadiness::PermissionDenied
    );
    assert_eq!(
        supervisor.output.status().selected_target.as_ref(),
        Some(&selection.target)
    );
}

#[test]
fn active_stream_failure_retires_the_device_and_enters_retry_backoff() {
    let output = device("output", "Output", true, fingerprint("Output", 0, 2), 0, 2, false, true);
    let selection = AudioDeviceSelection::from_descriptor(&output);
    let mut supervisor = supervisor();
    supervisor
        .output
        .configure(true, Some(selection.clone()), AudioRecoveryPolicy::WaitForSelected);
    observe(&mut supervisor, 0, vec![output]);
    commit_output(&mut supervisor, 2);
    let mut runtime = supervisor.output.status().clone();
    runtime.readiness = AudioDeviceReadiness::Recovering;
    runtime.error = Some(AudioInspectorError {
        category: AudioErrorCategory::StreamNegotiationFailed,
        message: "stream invalidated".to_owned(),
        technical_detail: None,
    });

    assert!(supervisor.output.report_runtime_status(100, &runtime));
    assert_eq!(supervisor.output.phase(), DeviceSwitchPhase::RetryWaiting);
    assert_eq!(
        supervisor.output.status().selected_target.as_ref(),
        Some(&selection.target)
    );
    assert!(supervisor.output.status().active_target.is_none());
    assert!(supervisor.output.status().format.is_none());
    assert_eq!(
        supervisor
            .output
            .status()
            .error
            .as_ref()
            .map(|error| error.message.as_str()),
        Some("stream invalidated")
    );
    assert!(supervisor.output.status().next_retry_ms.is_some());
}
