use super::*;
use crate::{AudioEngineBuilder, NullBackend};
use std::{sync::mpsc, thread};

fn event(value: u64) -> AudioEvent {
    AudioEvent::DeviceInventoryChanged { revision: value }
}
fn revision(event: AudioEvent) -> u64 {
    match event {
        AudioEvent::DeviceInventoryChanged { revision } => revision,
        _ => panic!("inventory event"),
    }
}

#[test]
fn best_effort_full_queue_rejects_without_replacing_its_prefix() {
    let (sender, receiver) = event_channel(1, false);
    assert!(sender.publish(event(1)));
    assert!(!sender.publish(event(2)));
    assert_eq!(revision(receiver.recv().unwrap()), 1);
    assert!(receiver.recv_timeout(Duration::ZERO).unwrap().is_none());
}

#[test]
fn ordered_queue_waits_for_capacity_and_preserves_every_occurrence() {
    let (sender, receiver) = event_channel(1, true);
    assert!(sender.publish(event(1)));
    let (started_tx, started_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    let worker = thread::spawn(move || {
        started_tx.send(()).unwrap();
        done_tx.send(sender.publish(event(2))).unwrap();
    });
    started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(done_rx.recv_timeout(Duration::from_millis(20)).is_err());
    assert_eq!(revision(receiver.recv().unwrap()), 1);
    assert!(done_rx.recv_timeout(Duration::from_secs(2)).unwrap());
    assert_eq!(revision(receiver.recv().unwrap()), 2);
    worker.join().unwrap();
    assert!(receiver.recv().is_err(), "final sender exit must wake the receiver");
}

#[test]
fn receiver_drop_cancels_a_blocked_publisher() {
    let (sender, receiver) = event_channel(1, true);
    assert!(sender.publish(event(1)));
    let (done_tx, done_rx) = mpsc::channel();
    let worker = thread::spawn(move || done_tx.send(!sender.publish(event(2))).unwrap());
    drop(receiver);
    assert!(done_rx.recv_timeout(Duration::from_secs(2)).unwrap());
    worker.join().unwrap();
}

#[test]
fn explicit_cancellation_preserves_buffered_prefix_and_unblocks_both_sides() {
    let (sender, receiver) = event_channel(1, true);
    let cancel = receiver.cancellation();
    assert!(sender.publish(event(1)));
    let (done_tx, done_rx) = mpsc::channel();
    let worker = thread::spawn(move || done_tx.send(!sender.publish(event(2))).unwrap());
    cancel.cancel_ordered();
    assert!(done_rx.recv_timeout(Duration::from_secs(2)).unwrap());
    assert_eq!(revision(receiver.recv().unwrap()), 1);
    assert!(receiver.recv().is_err());
    worker.join().unwrap();
}

#[test]
fn forced_engine_shutdown_cancels_full_ordered_delivery_with_a_live_receiver() {
    let mut builder = AudioEngineBuilder::default()
        .without_backends()
        .with_backend(NullBackend)
        .with_ordered_events();
    builder.limits.event_queue_capacity = 1;
    let mut engine = builder.build().unwrap();
    let receiver = engine.take_event_receiver().unwrap();
    let (done_tx, done_rx) = mpsc::channel();
    let worker = thread::spawn(move || done_tx.send(engine.shutdown()).unwrap());
    let finished = done_rx.recv_timeout(Duration::from_secs(3));
    drop(receiver);
    assert!(
        finished
            .expect("full event queue must not deadlock forced shutdown")
            .is_ok()
    );
    worker.join().unwrap();
}

#[test]
fn graceful_control_shutdown_delivers_terminal_event_through_a_one_slot_queue() {
    let mut builder = AudioEngineBuilder::default()
        .without_backends()
        .with_backend(NullBackend)
        .with_ordered_events();
    builder.limits.event_queue_capacity = 1;
    let mut engine = builder.build().unwrap();
    let receiver = engine.take_event_receiver().unwrap();
    engine.control().shutdown().unwrap();
    let mut terminal = false;
    while let Ok(Some(event)) = receiver.recv_timeout(Duration::from_secs(3)) {
        terminal |= matches!(event, AudioEvent::ShutdownComplete);
    }
    assert!(terminal);
    assert_eq!(engine.observations().latest().dropped_event_count, 0);
    engine.shutdown().unwrap();
}

#[test]
fn waiting_receiver_observes_final_sender_exit_even_with_an_unrepresentable_timeout() {
    let (sender, receiver) = event_channel(1, true);
    let cancellation = receiver.cancellation();
    let (done_tx, done_rx) = mpsc::channel();
    let worker = thread::spawn(move || {
        done_tx.send(receiver.recv_timeout(Duration::MAX).is_err()).unwrap();
    });
    drop(sender);
    let finished = done_rx.recv_timeout(Duration::from_secs(2));
    cancellation.cancel_ordered();
    assert!(finished.unwrap());
    worker.join().unwrap();
}

#[test]
fn byte_pressure_blocks_ordered_publication_before_the_item_limit_and_recovers() {
    let weight = event(1).retained_bytes();
    let (sender, receiver) = event_channel_with_bytes(128, true, weight);
    assert!(sender.publish(event(1)));
    let (done_tx, done_rx) = mpsc::channel();
    let worker = thread::spawn(move || {
        done_tx.send(sender.publish(event(2))).unwrap();
    });
    assert!(done_rx.recv_timeout(Duration::from_millis(20)).is_err());
    {
        let state = receiver.shared.state.lock().unwrap();
        assert_eq!(state.queue.len(), 1);
        assert_eq!(state.retained_bytes, weight);
    }
    assert_eq!(revision(receiver.recv().unwrap()), 1);
    assert!(done_rx.recv_timeout(Duration::from_secs(2)).unwrap());
    assert_eq!(revision(receiver.recv().unwrap()), 2);
    worker.join().unwrap();
    assert_eq!(receiver.shared.state.lock().unwrap().retained_bytes, 0);
}

#[test]
fn byte_pressure_keeps_best_effort_prefix_and_cancel_releases_ordered_waits() {
    let weight = event(1).retained_bytes();
    let (sender, receiver) = event_channel_with_bytes(128, false, weight);
    assert!(sender.publish(event(1)));
    assert!(!sender.publish(event(2)));
    assert_eq!(revision(receiver.recv().unwrap()), 1);
    let (sender, receiver) = event_channel_with_bytes(128, true, weight);
    assert!(sender.publish(event(1)));
    let (done_tx, done_rx) = mpsc::channel();
    let worker = thread::spawn(move || {
        done_tx.send(!sender.publish(event(2))).unwrap();
    });
    receiver.cancellation().cancel_ordered();
    assert!(done_rx.recv_timeout(Duration::from_secs(2)).unwrap());
    assert_eq!(revision(receiver.recv().unwrap()), 1);
    worker.join().unwrap();
}

#[test]
fn path_storage_is_rejected_before_command_admission_including_spare_capacity() {
    let mut engine = AudioEngineBuilder::default()
        .without_backends()
        .with_backend(NullBackend)
        .build()
        .unwrap();
    let control = engine.control();
    let mut path = std::path::PathBuf::with_capacity(crate::PlayFileRequest::MAX_PATH_BYTES + 1);
    path.push("tiny");
    let request = crate::PlayFileRequest::new(path, crate::PlaybackId::new("test").unwrap());
    let error = control.submit(crate::AudioCommand::PlayFile(request)).unwrap_err();
    assert_eq!(error.category, crate::AudioErrorCategory::CapacityExceeded);
    assert_eq!(
        control.submit(crate::AudioCommand::SetEnabled(false)).unwrap(),
        crate::CommandSequence::FIRST
    );
    engine.shutdown().unwrap();
}

#[test]
fn empty_clone_remains_connected_until_the_final_sender_exits() {
    let (sender, receiver) = event_channel(1, true);
    let other = sender.clone();
    drop(sender);
    assert!(receiver.recv_timeout(Duration::ZERO).unwrap().is_none());
    assert!(other.publish(event(1)));
    drop(other);
    assert_eq!(revision(receiver.recv().unwrap()), 1);
    assert!(receiver.recv().is_err());
}
