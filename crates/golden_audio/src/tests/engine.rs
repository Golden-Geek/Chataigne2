use std::{thread, time::Duration};

use crate::{
    AudioChannelId, AudioCommand, AudioConfiguration, AudioDeviceReadiness, AudioDeviceSelection, AudioDirection,
    AudioEngineBuilder, AudioErrorCategory, AudioEvent, AudioRouteId, ConfigGeneration, DirectionConfiguration, GainDb,
    NullBackend, OutputPatchRoute, PhysicalChannelKey, VirtualOutputChannel,
};

#[test]
fn engine_applies_configuration_and_shuts_down_twice() {
    let mut engine = AudioEngineBuilder::default().build().unwrap();
    let events = engine.take_event_receiver().unwrap();
    let control = engine.control();
    let generation = ConfigGeneration::new(1);
    control
        .submit(AudioCommand::ApplyConfiguration {
            generation,
            config: Box::new(AudioConfiguration::empty()),
        })
        .unwrap();

    let mut applied = false;
    while let Ok(event) = events.recv() {
        if event == (AudioEvent::ConfigurationApplied { generation }) {
            applied = true;
            break;
        }
    }
    assert!(applied);
    assert_eq!(engine.observations().latest().generation, generation);

    engine.shutdown().unwrap();
    engine.shutdown().unwrap();
    let error = control
        .submit(AudioCommand::SetEnabled(true))
        .expect_err("stopped engine must reject commands");
    assert_eq!(error.category, AudioErrorCategory::ShuttingDown);
}

#[test]
fn invalid_generation_leaves_last_valid_observation_active() {
    let mut engine = AudioEngineBuilder::default().build().unwrap();
    let events = engine.take_event_receiver().unwrap();
    let control = engine.control();
    let valid_generation = ConfigGeneration::new(4);
    control
        .submit(AudioCommand::ApplyConfiguration {
            generation: valid_generation,
            config: Box::new(AudioConfiguration::empty()),
        })
        .unwrap();

    let invalid_generation = ConfigGeneration::new(5);
    let mut invalid = AudioConfiguration::empty();
    invalid.output.enabled = true;
    control
        .submit(AudioCommand::ApplyConfiguration {
            generation: invalid_generation,
            config: Box::new(invalid),
        })
        .unwrap();

    let mut rejected = false;
    for _ in 0..8 {
        let event = events.recv().unwrap();
        if matches!(
            event,
            AudioEvent::ConfigurationRejected {
                generation,
                ..
            } if generation == invalid_generation
        ) {
            rejected = true;
            break;
        }
    }
    assert!(rejected);
    assert_eq!(engine.observations().latest().generation, valid_generation);
    engine.shutdown().unwrap();
}

#[test]
fn stale_configuration_generation_cannot_replace_the_newest_plan() {
    let mut engine = AudioEngineBuilder::default().build().unwrap();
    let events = engine.take_event_receiver().unwrap();
    let control = engine.control();
    let newest = ConfigGeneration::new(8);
    control
        .submit(AudioCommand::ApplyConfiguration {
            generation: newest,
            config: Box::new(AudioConfiguration::empty()),
        })
        .unwrap();

    loop {
        if matches!(
            events.recv().unwrap(),
            AudioEvent::ConfigurationApplied { generation } if generation == newest
        ) {
            break;
        }
    }

    let stale = ConfigGeneration::new(7);
    control
        .submit(AudioCommand::ApplyConfiguration {
            generation: stale,
            config: Box::new(AudioConfiguration::empty()),
        })
        .unwrap();
    loop {
        if matches!(
            events.recv().unwrap(),
            AudioEvent::ConfigurationRejected { generation, .. } if generation == stale
        ) {
            break;
        }
    }

    assert_eq!(engine.observations().latest().generation, newest);
    engine.shutdown().unwrap();
}

#[test]
fn applied_configuration_drives_null_device_readiness_and_channel_observations() {
    let mut engine = AudioEngineBuilder::default().build().unwrap();
    let events = engine.take_event_receiver().unwrap();
    let first = AudioChannelId::new();
    let second = AudioChannelId::new();
    let mut configuration = AudioConfiguration::empty();
    configuration.output = DirectionConfiguration {
        enabled: true,
        device: Some(AudioDeviceSelection::follow_system_default(
            NullBackend::backend_id(),
            AudioDirection::Output,
        )),
        recovery_policy: crate::AudioRecoveryPolicy::WaitForSelected,
        buffer_policy: crate::AudioBufferPolicy::Automatic,
    };
    configuration.physical_outputs = vec![
        PhysicalChannelKey::new("output:0").unwrap(),
        PhysicalChannelKey::new("output:1").unwrap(),
    ];
    configuration.virtual_outputs = vec![
        VirtualOutputChannel {
            id: first,
            label: "Left".to_owned(),
            gain: GainDb::UNITY,
        },
        VirtualOutputChannel {
            id: second,
            label: "Right".to_owned(),
            gain: GainDb::UNITY,
        },
    ];
    configuration.output_patch = vec![OutputPatchRoute {
        id: AudioRouteId::new(),
        source: first,
        destination: PhysicalChannelKey::new("output:0").unwrap(),
        gain: GainDb::UNITY,
    }];
    let generation = ConfigGeneration::new(1);
    engine
        .control()
        .submit(AudioCommand::ApplyConfiguration {
            generation,
            config: Box::new(configuration.clone()),
        })
        .unwrap();

    let mut output_readiness = Vec::new();
    loop {
        match events.recv_timeout(Duration::from_secs(1)).unwrap() {
            Some(AudioEvent::DeviceStatusChanged(status)) if status.direction == AudioDirection::Output => {
                output_readiness.push(status.readiness);
            }
            Some(AudioEvent::ConfigurationApplied { generation: applied }) if applied == generation => break,
            Some(_) => {}
            None => panic!("timed out waiting for the audio configuration to be applied"),
        }
    }
    let discovering = output_readiness
        .iter()
        .position(|readiness| *readiness == AudioDeviceReadiness::Discovering)
        .expect("device discovery should be observable before native driver work");
    let preparing = output_readiness
        .iter()
        .position(|readiness| *readiness == AudioDeviceReadiness::Preparing)
        .expect("device preparation should be observable before the stream is opened");
    let ready = output_readiness
        .iter()
        .position(|readiness| *readiness == AudioDeviceReadiness::Ready)
        .expect("ready device status");
    assert!(discovering < preparing);
    assert!(preparing < ready);

    let mut latest = engine.observations().latest();
    for _ in 0..100 {
        latest = engine.observations().latest();
        if latest.generation == generation && latest.device.output.readiness == AudioDeviceReadiness::Ready {
            break;
        }
        thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(latest.generation, generation);
    assert_eq!(latest.device.output.readiness, AudioDeviceReadiness::Ready);
    assert_eq!(
        latest.outputs.iter().map(|channel| channel.channel).collect::<Vec<_>>(),
        [first, second]
    );

    let reapplied = ConfigGeneration::new(2);
    engine
        .control()
        .submit(AudioCommand::ApplyConfiguration {
            generation: reapplied,
            config: Box::new(configuration),
        })
        .unwrap();
    let mut reapplied_statuses = Vec::new();
    loop {
        match events.recv_timeout(Duration::from_secs(1)).unwrap() {
            Some(AudioEvent::DeviceStatusChanged(status)) if status.direction == AudioDirection::Output => {
                reapplied_statuses.push(status.readiness);
            }
            Some(AudioEvent::ConfigurationApplied { generation: applied }) if applied == reapplied => {
                break;
            }
            Some(_) => {}
            None => panic!("timed out waiting for the repeated audio configuration"),
        }
    }
    assert!(
        reapplied_statuses.is_empty(),
        "an unchanged device stream contract must not reconnect: {reapplied_statuses:?}",
    );
    engine.shutdown().unwrap();
}

#[test]
fn gain_commands_are_processed_without_recompiling_configuration() {
    let mut engine = AudioEngineBuilder::default().build().unwrap();
    let events = engine.take_event_receiver().unwrap();
    let output = AudioChannelId::new();
    let mut configuration = AudioConfiguration::empty();
    configuration.virtual_outputs.push(VirtualOutputChannel {
        id: output,
        label: "Output".to_owned(),
        gain: GainDb::UNITY,
    });
    let generation = ConfigGeneration::new(1);
    let control = engine.control();
    control
        .submit(AudioCommand::ApplyConfiguration {
            generation,
            config: Box::new(configuration),
        })
        .unwrap();
    loop {
        if matches!(
            events.recv().unwrap(),
            AudioEvent::ConfigurationApplied {
                generation: applied
            } if applied == generation
        ) {
            break;
        }
    }

    control
        .submit(AudioCommand::SetMasterGain {
            gain: GainDb::new(-6.0).unwrap(),
        })
        .unwrap();
    control
        .submit(AudioCommand::SetChannelGain {
            channel: output,
            gain: GainDb::new(-3.0).unwrap(),
        })
        .unwrap();
    control
        .submit(AudioCommand::SetChannelGain {
            channel: AudioChannelId::new(),
            gain: GainDb::UNITY,
        })
        .unwrap();

    let diagnostic = loop {
        if let AudioEvent::Diagnostic(diagnostic) = events.recv().unwrap() {
            break diagnostic;
        }
    };
    assert_eq!(diagnostic.code, "audio_command_failed");
    assert_eq!(
        diagnostic.context.get("operation").map(String::as_str),
        Some("set_channel_gain")
    );
    assert_eq!(engine.observations().latest().generation, generation);
    engine.shutdown().unwrap();
}
