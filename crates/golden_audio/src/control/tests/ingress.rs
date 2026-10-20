use crate::{AudioCommand, AudioErrorCategory, CommandSequence, QueuePressureCounters};

use super::super::ingress::{CommandEnvelope, command_queue};

#[test]
fn full_probe_admission_retries_the_same_shared_targets() {
    use std::sync::Arc;
    let (producer, mut consumer, _) = command_queue(1, QueuePressureCounters::default());
    producer
        .try_push(CommandEnvelope {
            sequence: CommandSequence::FIRST,
            command: AudioCommand::SetEnabled(true),
        })
        .unwrap();
    let targets = Arc::new(vec![crate::AudioDeviceTargetId::SystemDefault {
        backend: crate::BackendId::from_static("null"),
    }]);
    let pointer = Arc::as_ptr(&targets);
    let error = producer
        .try_push(CommandEnvelope {
            sequence: CommandSequence::new(2).unwrap(),
            command: AudioCommand::SetDeviceProbeInterests {
                targets: targets.clone(),
            },
        })
        .unwrap_err();
    assert_eq!(error.category, AudioErrorCategory::QueueFull);
    assert_eq!(Arc::strong_count(&targets), 1);
    consumer.pop().unwrap();
    producer
        .try_push(CommandEnvelope {
            sequence: CommandSequence::new(3).unwrap(),
            command: AudioCommand::SetDeviceProbeInterests { targets },
        })
        .unwrap();
    let AudioCommand::SetDeviceProbeInterests { targets } = consumer.pop().unwrap().command else {
        panic!("probe interests")
    };
    assert_eq!(Arc::as_ptr(&targets), pointer);
}

#[test]
fn command_ingress_is_bounded_explicit_and_ordered() {
    let pressure = QueuePressureCounters::default();
    let (producer, mut consumer, _worker) = command_queue(1, pressure.clone());
    producer
        .try_push(CommandEnvelope {
            sequence: CommandSequence::new(1).unwrap(),
            command: AudioCommand::SetEnabled(true),
        })
        .unwrap();
    let error = producer
        .try_push(CommandEnvelope {
            sequence: CommandSequence::new(2).unwrap(),
            command: AudioCommand::SetEnabled(false),
        })
        .unwrap_err();

    assert_eq!(error.category, AudioErrorCategory::QueueFull);
    assert_eq!(pressure.snapshot().command_full, 1);
    let envelope = consumer.pop().unwrap();
    assert_eq!(envelope.sequence, CommandSequence::new(1).unwrap());
    assert!(matches!(envelope.command, AudioCommand::SetEnabled(true)));
}

#[test]
fn dropping_the_last_producer_is_visible_to_the_worker() {
    let (producer, consumer, _worker) = command_queue(1, QueuePressureCounters::default());
    let clone = producer.clone();
    drop(producer);
    assert!(!consumer.is_abandoned());
    drop(clone);
    assert!(consumer.is_abandoned());
}

#[test]
fn full_command_admission_can_retry_the_same_immutable_configuration_without_copying() {
    use std::sync::Arc;
    let (producer, mut consumer, _) = command_queue(1, QueuePressureCounters::default());
    producer
        .try_push(CommandEnvelope {
            sequence: CommandSequence::FIRST,
            command: AudioCommand::SetEnabled(true),
        })
        .unwrap();
    let config = Arc::new(crate::AudioConfiguration::empty());
    let pointer = Arc::as_ptr(&config);
    let weak = Arc::downgrade(&config);
    let rejected = producer
        .try_push(CommandEnvelope {
            sequence: CommandSequence::new(2).unwrap(),
            command: AudioCommand::ApplyConfiguration {
                generation: crate::ConfigGeneration::new(1),
                config: config.clone(),
            },
        })
        .unwrap_err();
    assert_eq!(rejected.category, AudioErrorCategory::QueueFull);
    assert_eq!(Arc::as_ptr(&config), pointer);
    assert_eq!(Arc::strong_count(&config), 1);
    consumer.pop().unwrap();
    producer
        .try_push(CommandEnvelope {
            sequence: CommandSequence::new(3).unwrap(),
            command: AudioCommand::ApplyConfiguration {
                generation: crate::ConfigGeneration::new(1),
                config: config.clone(),
            },
        })
        .unwrap();
    drop(config);
    let admitted = consumer.pop().unwrap();
    let AudioCommand::ApplyConfiguration { config, .. } = admitted.command else {
        panic!("configuration");
    };
    assert_eq!(Arc::as_ptr(&config), pointer);
    drop(config);
    assert!(weak.upgrade().is_none());
}
