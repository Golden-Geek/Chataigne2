use crate::{AudioCommand, AudioErrorCategory, CommandSequence, QueuePressureCounters};

use super::super::ingress::{CommandEnvelope, command_queue};

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
