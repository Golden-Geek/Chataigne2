use std::num::NonZeroUsize;
use std::time::{Duration, Instant};

use crate::testkit::{TestTransportSendError, test_transport_pair};
use crate::{BoundedQueue, ReconnectBackoff, RetirementPool, WorkerTask};

mod pending;

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

    queue.try_push("four", 1).expect("queue accepts after drain");
    queue.try_push("five", 1).expect("second item fits");
    assert_eq!(queue.take_all(), vec!["four", "five"]);
    assert_eq!(queue.total_weight(), 0);

    queue.try_push("six", 1).expect("queue accepts before clear");
    queue.clear();
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
fn worker_task_rejects_overload_without_blocking_the_producer() {
    use std::sync::{Arc, Barrier, mpsc::TrySendError};

    let start = Arc::new(Barrier::new(2));
    let worker_start = Arc::clone(&start);
    let mut worker = WorkerTask::spawn_with_capacity("golden-io-bounded-test", 1, move |commands| {
        worker_start.wait();
        let _ = commands.recv();
    })
    .expect("test worker starts");

    worker.send(1_u8).expect("first command fits");
    assert_eq!(worker.send(2_u8), Err(TrySendError::Full(2)));
    start.wait();
    worker.join();
}

#[test]
fn retirement_pool_rejects_saturation_without_losing_the_resource() {
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    let (entered_tx, entered_rx) = std::sync::mpsc::sync_channel(1);
    let (release_tx, release_rx) = std::sync::mpsc::sync_channel(1);
    pool.try_retire("golden-io-retirement-blocked", 1_u8, move |value| {
        entered_tx.send(value).expect("test observes active retirement");
        release_rx.recv().expect("test releases active retirement");
    })
    .expect("first retirement is admitted");
    assert_eq!(entered_rx.recv(), Ok(1));

    let rejected = pool
        .try_retire("golden-io-retirement-rejected", 2_u8, |_| {})
        .expect_err("a blocked retirement consumes the fixed capacity");
    assert_eq!(rejected.into_value(), 2);
    assert_eq!(
        pool.metrics(),
        crate::RetirementMetricsSnapshot {
            capacity: 1,
            active: 1,
            peak: 1,
            rejected: 1,
        }
    );

    release_tx.send(()).expect("release blocked retirement");
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
    let (recovered_tx, recovered_rx) = std::sync::mpsc::sync_channel(1);
    pool.try_retire("golden-io-retirement-recovered", 3_u8, move |value| {
        recovered_tx.send(value).expect("test observes recovered retirement");
    })
    .expect("capacity recovers after cleanup");
    assert_eq!(recovered_rx.recv(), Ok(3));
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn unused_retirement_reservation_releases_its_capacity() {
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    let permit = pool.try_reserve().expect("slot is available");
    assert!(pool.try_reserve().is_err());

    drop(permit);
    assert!(pool.try_reserve().is_ok());
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
