use super::super::ingress::command_queue;
use crate::{AudioCommand, AudioErrorCategory, CommandSequence, QueuePressureCounters};
use std::{
    sync::{Arc, Barrier, mpsc},
    thread,
    time::{Duration, Instant},
};

#[test]
fn full_probe_admission_retries_the_same_shared_targets() {
    let (producer, mut consumer, _) = command_queue(1, QueuePressureCounters::default());
    producer.try_push(AudioCommand::SetEnabled(true)).unwrap();
    let targets = Arc::new(vec![crate::AudioDeviceTargetId::SystemDefault {
        backend: crate::BackendId::from_static("null"),
    }]);
    let pointer = Arc::as_ptr(&targets);
    let error = producer
        .try_push(AudioCommand::SetDeviceProbeInterests {
            targets: targets.clone(),
        })
        .unwrap_err();
    assert_eq!(error.category, AudioErrorCategory::QueueFull);
    assert_eq!(Arc::strong_count(&targets), 1);
    consumer.pop().unwrap();
    let sequence = producer
        .try_push(AudioCommand::SetDeviceProbeInterests { targets })
        .unwrap();
    assert_eq!(sequence.get(), 2, "rejection does not reserve an admission sequence");
    let AudioCommand::SetDeviceProbeInterests { targets } = consumer.pop().unwrap().command else {
        panic!("probe interests")
    };
    assert_eq!(Arc::as_ptr(&targets), pointer);
}

#[test]
fn command_ingress_is_bounded_explicit_and_ordered() {
    let pressure = QueuePressureCounters::default();
    let (producer, mut consumer, _) = command_queue(1, pressure.clone());
    assert_eq!(
        producer.try_push(AudioCommand::SetEnabled(true)).unwrap(),
        CommandSequence::FIRST
    );
    let error = producer.try_push(AudioCommand::SetEnabled(false)).unwrap_err();
    assert_eq!(error.category, AudioErrorCategory::QueueFull);
    assert_eq!(pressure.snapshot().command_full, 1);
    let envelope = consumer.pop().unwrap();
    assert_eq!(envelope.sequence, CommandSequence::FIRST);
    assert!(matches!(envelope.command, AudioCommand::SetEnabled(true)));
}

#[test]
fn dropping_the_last_producer_is_visible_to_the_worker() {
    let (producer, consumer, _) = command_queue(1, QueuePressureCounters::default());
    let clone = producer.clone();
    drop(producer);
    assert!(!consumer.is_abandoned());
    drop(clone);
    assert!(consumer.is_abandoned());
}

#[test]
fn full_command_admission_can_retry_the_same_immutable_configuration_without_copying() {
    let (producer, mut consumer, _) = command_queue(1, QueuePressureCounters::default());
    producer.try_push(AudioCommand::SetEnabled(true)).unwrap();
    let config = Arc::new(crate::AudioConfiguration::empty());
    let pointer = Arc::as_ptr(&config);
    let weak = Arc::downgrade(&config);
    let rejected = producer
        .try_push(AudioCommand::ApplyConfiguration {
            generation: crate::ConfigGeneration::new(1),
            config: config.clone(),
        })
        .unwrap_err();
    assert_eq!(rejected.category, AudioErrorCategory::QueueFull);
    assert_eq!(Arc::strong_count(&config), 1);
    consumer.pop().unwrap();
    assert_eq!(
        producer
            .try_push(AudioCommand::ApplyConfiguration {
                generation: crate::ConfigGeneration::new(1),
                config: config.clone(),
            })
            .unwrap()
            .get(),
        2
    );
    drop(config);
    let AudioCommand::ApplyConfiguration { config, .. } = consumer.pop().unwrap().command else {
        panic!("configuration")
    };
    assert_eq!(Arc::as_ptr(&config), pointer);
    drop(config);
    assert!(weak.upgrade().is_none());
}

#[test]
fn concurrent_producers_assign_sequences_in_accepted_queue_order() {
    let (producer, mut consumer, _) = command_queue(4096, QueuePressureCounters::default());
    let start = Arc::new(Barrier::new(5));
    let mut threads = Vec::new();
    for _ in 0..4 {
        let producer = producer.clone();
        let start = start.clone();
        threads.push(thread::spawn(move || {
            start.wait();
            let mut accepted = Vec::new();
            let deadline = Instant::now() + Duration::from_secs(5);
            while accepted.len() < 500 {
                match producer.try_push(AudioCommand::SetEnabled(true)) {
                    Ok(sequence) => accepted.push(sequence),
                    Err(error) => {
                        assert_eq!(error.category, AudioErrorCategory::QueueFull);
                        assert!(Instant::now() < deadline);
                        thread::yield_now();
                    }
                }
            }
            accepted
        }));
    }
    start.wait();
    let mut returned: Vec<_> = threads.into_iter().flat_map(|worker| worker.join().unwrap()).collect();
    returned.sort_unstable();
    for (index, expected) in returned.into_iter().enumerate() {
        let envelope = consumer.pop().unwrap();
        assert_eq!(envelope.sequence, expected);
        assert_eq!(expected.get(), index as u64 + 1);
    }
    assert!(consumer.pop().is_err());
}

#[test]
fn failed_nonblocking_shutdown_does_not_close_or_consume_a_sequence() {
    let (producer, mut consumer, _) = command_queue(1, QueuePressureCounters::default());
    producer.try_push(AudioCommand::SetEnabled(true)).unwrap();
    assert_eq!(
        producer.try_push(AudioCommand::Shutdown).unwrap_err().category,
        AudioErrorCategory::QueueFull
    );
    assert!(producer.accepting());
    consumer.pop().unwrap();
    assert_eq!(producer.try_push(AudioCommand::SetEnabled(false)).unwrap().get(), 2);
    consumer.pop().unwrap();
    let stop = producer.try_push(AudioCommand::Shutdown).unwrap();
    assert_eq!(stop.get(), 3);
    assert_eq!(producer.try_push(AudioCommand::Shutdown).unwrap(), stop);
    assert_eq!(
        producer.try_push(AudioCommand::SetEnabled(true)).unwrap_err().category,
        AudioErrorCategory::ShuttingDown
    );
    let envelope = consumer.pop().unwrap();
    assert_eq!(envelope.sequence, stop);
    assert!(matches!(envelope.command, AudioCommand::Shutdown));
    assert!(consumer.pop().is_err());
}

#[test]
fn blocking_shutdown_closes_admission_while_its_barrier_waits_for_capacity() {
    let (producer, mut consumer, _) = command_queue(1, QueuePressureCounters::default());
    producer.try_push(AudioCommand::SetEnabled(true)).unwrap();
    let (send, receive) = mpsc::channel();
    let waiting = producer.clone();
    let worker = thread::spawn(move || send.send(waiting.shutdown_blocking()).unwrap());
    let deadline = Instant::now() + Duration::from_secs(5);
    while producer.accepting() {
        assert!(Instant::now() < deadline);
        thread::yield_now();
    }
    assert!(matches!(receive.try_recv(), Err(mpsc::TryRecvError::Empty)));
    assert_eq!(
        producer.try_push(AudioCommand::SetEnabled(false)).unwrap_err().category,
        AudioErrorCategory::ShuttingDown
    );
    let closing_sequence = loop {
        match producer.try_push(AudioCommand::Shutdown) {
            Ok(sequence) => break sequence,
            Err(error) => {
                assert_eq!(error.category, AudioErrorCategory::QueueFull);
                assert!(Instant::now() < deadline);
                thread::yield_now();
            }
        }
    };
    assert_eq!(closing_sequence.get(), 2);
    assert_eq!(consumer.pop().unwrap().sequence.get(), 1);
    let sequence = receive.recv_timeout(Duration::from_secs(2)).unwrap().unwrap();
    worker.join().unwrap();
    let envelope = consumer.pop().unwrap();
    assert_eq!(envelope.sequence, sequence);
    assert!(matches!(envelope.command, AudioCommand::Shutdown));
}

#[test]
fn concurrent_shutdown_never_leaves_accepted_commands_behind_the_barrier() {
    let (producer, mut consumer, _) = command_queue(256, QueuePressureCounters::default());
    let start = Arc::new(Barrier::new(4));
    let mut threads = Vec::new();
    for _ in 0..2 {
        let producer = producer.clone();
        let start = start.clone();
        threads.push(thread::spawn(move || {
            start.wait();
            let mut accepted = Vec::new();
            while accepted.len() < 100 {
                match producer.try_push(AudioCommand::SetEnabled(true)) {
                    Ok(sequence) => accepted.push(sequence),
                    Err(error) if error.category == AudioErrorCategory::QueueFull => thread::yield_now(),
                    Err(error) => {
                        assert_eq!(error.category, AudioErrorCategory::ShuttingDown);
                        break;
                    }
                }
            }
            accepted
        }));
    }
    let stopping = producer.clone();
    let gate = start.clone();
    let stop = thread::spawn(move || {
        gate.wait();
        stopping.shutdown_blocking().unwrap()
    });
    start.wait();
    let mut accepted: Vec<_> = threads.into_iter().flat_map(|thread| thread.join().unwrap()).collect();
    let shutdown = stop.join().unwrap();
    accepted.sort_unstable();
    for sequence in accepted {
        let envelope = consumer.pop().unwrap();
        assert_eq!(envelope.sequence, sequence);
        assert!(sequence < shutdown);
        assert!(!matches!(envelope.command, AudioCommand::Shutdown));
    }
    assert_eq!(consumer.pop().unwrap().sequence, shutdown);
    assert!(consumer.pop().is_err());
}

#[test]
fn sequence_exhaustion_reserves_the_final_number_for_shutdown_without_wrapping() {
    let (producer, mut consumer, _) = command_queue(2, QueuePressureCounters::default());
    producer.set_next_sequence_for_test(CommandSequence::new(u64::MAX - 1).unwrap());
    assert_eq!(
        producer.try_push(AudioCommand::SetEnabled(true)).unwrap().get(),
        u64::MAX - 1
    );
    assert_eq!(
        producer.try_push(AudioCommand::SetEnabled(true)).unwrap_err().category,
        AudioErrorCategory::InternalInvariant
    );
    assert_eq!(producer.try_push(AudioCommand::Shutdown).unwrap().get(), u64::MAX);
    assert_eq!(consumer.pop().unwrap().sequence.get(), u64::MAX - 1);
    assert_eq!(consumer.pop().unwrap().sequence.get(), u64::MAX);
    assert_eq!(producer.try_push(AudioCommand::Shutdown).unwrap().get(), u64::MAX);
}

#[test]
fn a_disconnected_consumer_rejects_commands_and_does_not_strand_blocking_shutdown() {
    let (producer, consumer, _) = command_queue(1, QueuePressureCounters::default());
    drop(consumer);
    assert_eq!(
        producer.try_push(AudioCommand::SetEnabled(true)).unwrap_err().category,
        AudioErrorCategory::ShuttingDown
    );
    assert_eq!(
        producer.shutdown_blocking().unwrap_err().category,
        AudioErrorCategory::ShuttingDown
    );
}

#[test]
fn consumer_loss_while_shutdown_is_waiting_does_not_leave_a_phantom_barrier() {
    let (producer, consumer, _) = command_queue(1, QueuePressureCounters::default());
    producer.try_push(AudioCommand::SetEnabled(true)).unwrap();
    let stopping = producer.clone();
    let (send, receive) = mpsc::channel();
    let worker = thread::spawn(move || send.send(stopping.shutdown_blocking()).unwrap());
    let deadline = Instant::now() + Duration::from_secs(5);
    while producer.accepting() {
        assert!(Instant::now() < deadline);
        thread::yield_now();
    }
    drop(consumer);
    assert_eq!(
        receive
            .recv_timeout(Duration::from_secs(2))
            .unwrap()
            .unwrap_err()
            .category,
        AudioErrorCategory::ShuttingDown
    );
    worker.join().unwrap();
    assert_eq!(
        producer.try_push(AudioCommand::Shutdown).unwrap_err().category,
        AudioErrorCategory::ShuttingDown
    );
    assert_eq!(
        producer.shutdown_blocking().unwrap_err().category,
        AudioErrorCategory::ShuttingDown
    );
}
