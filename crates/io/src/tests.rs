use std::{
    sync::mpsc::TryRecvError,
    thread,
    time::{Duration, Instant},
};

use crate::testkit::{TestTransportSendError, test_transport_pair};
use crate::{BoundedQueue, ReconnectBackoff, WorkerTask, pending_channel};

#[test]
fn pending_signal_tracks_worker_events_without_polling() {
    let (sender, receiver) = pending_channel();
    assert!(!receiver.has_pending());

    sender.send(7_u8).expect("receiver is alive");
    assert!(receiver.has_pending());

    receiver.clear_pending();
    assert_eq!(receiver.try_recv(), Ok(7));
    assert!(!receiver.has_pending());
    assert_eq!(receiver.try_recv(), Err(TryRecvError::Empty));
}

#[test]
fn event_arriving_during_a_drain_remains_observable() {
    let (sender, receiver) = pending_channel();
    sender.send(1_u8).expect("receiver is alive");

    receiver.clear_pending();
    assert_eq!(receiver.try_recv(), Ok(1));

    thread::spawn(move || sender.send(2_u8).expect("receiver is alive"))
        .join()
        .expect("worker exits cleanly");

    assert!(receiver.has_pending());
    receiver.clear_pending();
    assert_eq!(receiver.try_recv(), Ok(2));
}

#[test]
fn reconnect_backoff_is_capped_and_resets_after_success() {
    let start = Instant::now();
    let mut backoff = ReconnectBackoff::new(Duration::from_millis(250), Duration::from_secs(1));

    assert_eq!(backoff.schedule(start), start + Duration::from_millis(250));
    assert_eq!(backoff.schedule(start), start + Duration::from_millis(500));
    assert_eq!(backoff.schedule(start), start + Duration::from_secs(1));
    assert_eq!(backoff.schedule(start), start + Duration::from_secs(1));

    backoff.reset();
    assert_eq!(backoff.next_delay(), Duration::from_millis(250));
}

#[test]
fn bounded_queue_enforces_item_and_weight_limits_without_losing_ownership() {
    let mut queue = BoundedQueue::new(2, 5);
    queue.try_push("one", 2).expect("first item fits");
    queue.try_push("two", 3).expect("second item fits exactly");

    let rejected = queue.try_push("three", 1).expect_err("item limit is observable");
    assert_eq!(rejected.into_inner(), "three");
    assert_eq!(queue.len(), 2);
    assert_eq!(queue.total_weight(), 5);

    assert_eq!(queue.pop_front(), Some("one"));
    let rejected = queue.try_push("heavy", 4).expect_err("weight limit is observable");
    assert_eq!(rejected.into_inner(), "heavy");
    assert_eq!(queue.pop_front(), Some("two"));
    assert!(queue.is_empty());
    assert_eq!(queue.total_weight(), 0);
}

#[test]
fn worker_task_owns_command_channel_and_orderly_join() {
    enum Command {
        Value(u8),
        Stop,
    }

    let (observed_tx, observed_rx) = std::sync::mpsc::channel();
    let mut worker = WorkerTask::spawn("golden-io-test", move |commands| {
        while let Ok(command) = commands.recv() {
            match command {
                Command::Value(value) => observed_tx.send(value).expect("observer is alive"),
                Command::Stop => break,
            }
        }
    })
    .expect("test worker starts");

    worker.send(Command::Value(9)).expect("worker is alive");
    assert_eq!(observed_rx.recv(), Ok(9));
    worker.stop(Command::Stop);
    assert!(!worker.is_running());
}

#[test]
fn test_transport_models_bounded_loopback_and_disconnects() {
    let (left, right) = test_transport_pair(1, 4);
    left.send("ping", 4).expect("first frame fits");
    assert_eq!(right.try_receive(), Some("ping"));

    left.send("full", 4).expect("queue is empty again");
    assert_eq!(left.send("overflow", 1), Err(TestTransportSendError::Full("overflow")));
    assert_eq!(right.try_receive(), Some("full"));

    right.disconnect();
    assert!(!left.is_connected());
    assert_eq!(
        left.send("offline", 1),
        Err(TestTransportSendError::Disconnected("offline"))
    );
}
