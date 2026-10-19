use std::{
    num::NonZeroUsize,
    sync::{Arc, Barrier, mpsc},
    thread,
};

use crate::{PendingDrainState, PendingSendError, bounded_pending_channel, pending_channel};

const ONE: NonZeroUsize = NonZeroUsize::MIN;
const EIGHT: NonZeroUsize = NonZeroUsize::new(8).expect("eight is nonzero");

#[test]
fn notification_observes_published_readiness_without_the_queue_lock_and_sees_last_sender_exit() {
    let slot = Arc::new(std::sync::Mutex::new(None::<crate::PendingReceiver<u32>>));
    let observer = slot.clone();
    let (seen_tx, seen_rx) = mpsc::channel();
    let (sender, receiver) = crate::notified_pending_channel(
        2,
        8,
        crate::WorkerNotifier::new(move || {
            let receiver = observer.lock().unwrap();
            let receiver = receiver.as_ref().unwrap();
            assert!(receiver.has_pending());
            let mut values = Vec::new();
            let drain = receiver.drain_into(&mut values, EIGHT);
            seen_tx.send((values, drain.state)).unwrap();
        }),
    );
    *slot.lock().unwrap() = Some(receiver);
    let producer = thread::spawn(move || {
        sender.send(7).unwrap();
        drop(sender);
    });
    assert_eq!(
        seen_rx.recv_timeout(std::time::Duration::from_secs(2)).unwrap(),
        (vec![7], PendingDrainState::Empty)
    );
    assert_eq!(
        seen_rx.recv_timeout(std::time::Duration::from_secs(2)).unwrap(),
        (vec![], PendingDrainState::Disconnected)
    );
    producer.join().unwrap();
    slot.lock().unwrap().take();
}

#[test]
fn pending_signal_tracks_accepted_events_without_polling() {
    let (sender, receiver) = pending_channel();
    assert!(!receiver.has_pending());

    sender.send(7_u8).expect("receiver is alive");
    assert!(receiver.has_pending());

    let mut events = Vec::new();
    assert_eq!(
        receiver.drain_into(&mut events, EIGHT),
        crate::PendingDrain {
            received: 1,
            state: PendingDrainState::Empty,
        }
    );
    assert_eq!(events, vec![7]);
    assert!(!receiver.has_pending());
}

#[test]
fn publication_after_enqueue_closes_the_old_store_before_send_race() {
    let (sender, receiver) = pending_channel();
    let keepalive = sender.clone();
    let producer_entered = Arc::new(Barrier::new(2));
    let resume_producer = Arc::new(Barrier::new(2));
    let producer = {
        let producer_entered = Arc::clone(&producer_entered);
        let resume_producer = Arc::clone(&resume_producer);
        thread::spawn(move || {
            sender
                .send_before_publish(9_u8, || {
                    producer_entered.wait();
                    resume_producer.wait();
                })
                .expect("receiver is alive");
        })
    };

    producer_entered.wait();
    let mut events = Vec::new();
    let first_drain = receiver.drain_into(&mut events, EIGHT);
    assert_eq!(events, vec![9], "enqueue must precede readiness publication");
    assert_eq!(first_drain.state, PendingDrainState::Empty);

    resume_producer.wait();
    producer.join().expect("producer exits cleanly");
    assert!(
        receiver.has_pending(),
        "a resumed producer must complete readiness publication"
    );

    events.clear();
    let conservative_poll = receiver.drain_into(&mut events, EIGHT);
    assert!(events.is_empty());
    assert_eq!(conservative_poll.state, PendingDrainState::Empty);
    assert!(!receiver.has_pending());
    drop(keepalive);
}

#[test]
fn enqueue_during_drain_remains_observable() {
    let (sender, receiver) = pending_channel();
    sender.send(1_u8).expect("receiver is alive");
    let keepalive = sender.clone();
    let cleared = Arc::new(Barrier::new(2));
    let sent = Arc::new(Barrier::new(2));
    let producer = {
        let cleared = Arc::clone(&cleared);
        let sent = Arc::clone(&sent);
        thread::spawn(move || {
            cleared.wait();
            sender.send(2_u8).expect("receiver is alive");
            sent.wait();
        })
    };

    let mut events = Vec::new();
    let drain = receiver.drain_into_after_clear(&mut events, EIGHT, || {
        cleared.wait();
        sent.wait();
    });
    assert_eq!(drain.state, PendingDrainState::Empty);
    assert_eq!(events, vec![1, 2]);
    assert!(
        receiver.has_pending(),
        "concurrent publication may conservatively schedule another drain"
    );

    producer.join().expect("producer exits cleanly");
    events.clear();
    receiver.drain_into(&mut events, EIGHT);
    assert!(!receiver.has_pending());
    drop(keepalive);
}

#[test]
fn partial_drain_rearms_without_another_producer() {
    let (sender, receiver) = pending_channel();
    for value in 1_u8..=3 {
        sender.send(value).expect("receiver is alive");
    }

    let mut events = Vec::new();
    assert_eq!(
        receiver.drain_into(&mut events, ONE).state,
        PendingDrainState::BudgetExhausted
    );
    assert_eq!(events, vec![1]);
    assert!(receiver.has_pending());

    events.clear();
    assert_eq!(receiver.drain_into(&mut events, EIGHT).state, PendingDrainState::Empty);
    assert_eq!(events, vec![2, 3]);
    assert!(!receiver.has_pending());
}

#[test]
fn concurrent_producers_publish_every_accepted_item() {
    let (sender, receiver) = pending_channel();
    let start = Arc::new(Barrier::new(5));
    let (complete_tx, complete_rx) = mpsc::channel();
    let mut producers = Vec::new();
    for value in 0_u8..4 {
        let sender = sender.clone();
        let start = Arc::clone(&start);
        let complete_tx = complete_tx.clone();
        producers.push(thread::spawn(move || {
            start.wait();
            sender.send(value).expect("receiver is alive");
            complete_tx.send(()).expect("observer is alive");
        }));
    }
    drop(sender);
    drop(complete_tx);

    start.wait();
    for _ in 0..4 {
        complete_rx.recv().expect("each send completes");
        assert!(receiver.has_pending());
    }

    let mut events = Vec::new();
    let drain = receiver.drain_into(&mut events, EIGHT);
    assert_eq!(drain.received, 4);
    assert_eq!(drain.state, PendingDrainState::Disconnected);
    events.sort_unstable();
    assert_eq!(events, vec![0, 1, 2, 3]);

    for producer in producers {
        producer.join().expect("producer exits cleanly");
    }
}

#[test]
fn disconnect_is_distinct_from_empty() {
    let (sender, receiver) = pending_channel::<u8>();
    let mut events = Vec::new();
    assert_eq!(receiver.drain_into(&mut events, EIGHT).state, PendingDrainState::Empty);

    sender.send(3).expect("receiver is alive");
    drop(sender);
    assert_eq!(
        receiver.drain_into(&mut events, EIGHT),
        crate::PendingDrain {
            received: 1,
            state: PendingDrainState::Disconnected,
        }
    );
    assert_eq!(events, vec![3]);
    assert!(!receiver.has_pending());
}

#[test]
fn admission_is_bounded_by_items_and_retained_weight() {
    let (sender, receiver) = bounded_pending_channel(2, 5);
    sender.send_weighted("one", 3).unwrap();
    assert_eq!(
        sender.send_weighted("too-heavy", 3),
        Err(PendingSendError::Full("too-heavy"))
    );
    sender.send_weighted("two", 2).unwrap();
    assert_eq!(sender.send("too-many"), Err(PendingSendError::Full("too-many")));

    let metrics = receiver.metrics();
    assert_eq!(metrics.depth, 2);
    assert_eq!(metrics.retained_weight, 5);
    assert_eq!(metrics.rejected, 2);
    assert!(metrics.ready);

    let mut events = Vec::new();
    assert_eq!(
        receiver.drain_into(&mut events, ONE).state,
        PendingDrainState::BudgetExhausted
    );
    assert_eq!(receiver.metrics().retained_weight, 2);
    assert_eq!(events, vec!["one"]);
}

#[test]
fn producer_gets_its_event_back_after_receiver_disconnects() {
    let (sender, receiver) = bounded_pending_channel(1, 1);
    assert!(!sender.is_disconnected());
    drop(receiver);
    assert!(sender.is_disconnected());

    assert_eq!(sender.send(7), Err(PendingSendError::Disconnected(7)));
}
