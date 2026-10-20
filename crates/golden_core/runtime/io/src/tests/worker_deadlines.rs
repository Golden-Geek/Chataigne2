use crate::{BoundedWorker, RetirementPool, WorkerLimits, WorkerReceive};
use std::{
    num::NonZeroUsize,
    sync::mpsc,
    time::{Duration, Instant},
};

fn limits() -> WorkerLimits {
    WorkerLimits {
        command_items: 2,
        command_weight: 2,
        event_items: 1,
        event_weight: 1,
    }
}

#[test]
fn elapsed_deadline_returns_without_consuming_future_commands() {
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    let (done, result) = mpsc::channel();
    let worker = BoundedWorker::<u8, ()>::spawn("deadline", &pool, limits(), move |context| {
        let deadline = Instant::now() + Duration::from_millis(10);
        assert_eq!(context.receive_until(deadline), WorkerReceive::Deadline);
        assert!(Instant::now() >= deadline);
        done.send(()).unwrap();
        assert_eq!(
            context.receive_until(Instant::now() + Duration::from_secs(3600)),
            WorkerReceive::Command(7)
        );
    })
    .unwrap();
    result.recv_timeout(Duration::from_secs(2)).unwrap();
    worker.send_weighted(7, 1).unwrap();
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn commands_wake_long_deadlines_repeatedly_without_lost_notifications() {
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    let (reply, replies) = mpsc::channel();
    let worker = BoundedWorker::<u8, ()>::spawn("command-deadline", &pool, limits(), move |context| {
        for value in 0..100 {
            reply.send(value).unwrap();
            assert_eq!(
                context.receive_until(Instant::now() + Duration::from_secs(3600)),
                WorkerReceive::Command(value)
            );
        }
    })
    .unwrap();
    for value in 0..100 {
        assert_eq!(replies.recv_timeout(Duration::from_secs(2)).unwrap(), value);
        worker.send_weighted(value, 1).unwrap();
    }
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn cancellation_wakes_a_long_deadline() {
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    let (entered, ready) = mpsc::channel();
    let worker = BoundedWorker::<u8, ()>::spawn("cancel-deadline", &pool, limits(), move |context| {
        entered.send(()).unwrap();
        assert_eq!(
            context.receive_until(Instant::now() + Duration::from_secs(3600)),
            WorkerReceive::Disconnected
        );
    })
    .unwrap();
    ready.recv_timeout(Duration::from_secs(2)).unwrap();
    worker.cancel();
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn accepted_prefix_precedes_past_deadline_and_graceful_disconnection() {
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    let (resume, gate) = mpsc::channel();
    let mut worker = BoundedWorker::<u8, ()>::spawn("close-deadline", &pool, limits(), move |context| {
        gate.recv().unwrap();
        let past = Instant::now() - Duration::from_secs(1);
        assert_eq!(context.receive_until(past), WorkerReceive::Command(1));
        assert_eq!(context.receive_until(past), WorkerReceive::Command(2));
        assert_eq!(context.receive_until(past), WorkerReceive::Disconnected);
    })
    .unwrap();
    worker.send_weighted(1, 1).unwrap();
    worker.send_weighted(2, 1).unwrap();
    worker.finish_admission();
    resume.send(()).unwrap();
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn cancellation_abandons_queued_commands_even_at_a_past_deadline() {
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    let (resume, gate) = mpsc::channel();
    let worker = BoundedWorker::<u8, ()>::spawn("cancel-prefix-deadline", &pool, limits(), move |context| {
        gate.recv().unwrap();
        assert_eq!(context.receive_until(Instant::now()), WorkerReceive::Disconnected);
    })
    .unwrap();
    worker.send_weighted(1, 1).unwrap();
    worker.cancel();
    resume.send(()).unwrap();
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}
