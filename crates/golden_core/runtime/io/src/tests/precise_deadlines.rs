use crate::{BoundedWorker, RetirementPool, WorkerLimits, WorkerReceive, WorkerWake};
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
fn precise_wait_rearms_without_early_deadlines_or_lost_command_wakes() {
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    let (ready, turns) = mpsc::channel();
    let worker = BoundedWorker::<u8, ()>::spawn("precise-command", &pool, limits(), move |mut context| {
        context.enable_precise_deadlines().unwrap();
        context.enable_precise_deadlines().unwrap();
        for value in 0..100 {
            let deadline = Instant::now() + Duration::from_millis(1);
            assert_eq!(context.receive_until(deadline), WorkerReceive::Deadline);
            assert!(Instant::now() >= deadline);
            ready.send(value).unwrap();
            assert_eq!(
                context.receive_until(Instant::now() + Duration::from_secs(3600)),
                WorkerReceive::Command(value)
            );
        }
    })
    .unwrap();
    for value in 0..100 {
        assert_eq!(turns.recv_timeout(Duration::from_secs(2)).unwrap(), value);
        worker.send_weighted(value, 1).unwrap();
    }
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn precise_notification_preserves_checkpoint_and_cancellation() {
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    let (ready, turns) = mpsc::channel();
    let (resume, resumed) = mpsc::channel();
    let worker = BoundedWorker::<(), ()>::spawn("precise-notification", &pool, limits(), move |mut context| {
        context.enable_precise_deadlines().unwrap();
        for _ in 0..100 {
            let checkpoint = context.wake_checkpoint();
            ready.send(()).unwrap();
            resumed.recv_timeout(Duration::from_secs(2)).unwrap();
            assert_eq!(
                context.wait_until(checkpoint, Instant::now() + Duration::from_secs(3600)),
                WorkerWake::Notified
            );
        }
        ready.send(()).unwrap();
        assert_eq!(
            context.receive_until(Instant::now() + Duration::from_secs(3600)),
            WorkerReceive::Disconnected
        );
    })
    .unwrap();
    let notifier = worker.wake_notifier();
    for _ in 0..100 {
        turns.recv_timeout(Duration::from_secs(2)).unwrap();
        notifier.notify();
        resume.send(()).unwrap();
    }
    turns.recv_timeout(Duration::from_secs(2)).unwrap();
    worker.cancel();
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
    notifier.notify();
}

#[test]
fn precise_wait_keeps_command_prefix_and_cancellation_priority() {
    for cancel in [false, true] {
        let pool = RetirementPool::new(NonZeroUsize::MIN);
        let (resume, resumed) = mpsc::channel();
        let mut worker = BoundedWorker::<u8, ()>::spawn("precise-priority", &pool, limits(), move |mut context| {
            context.enable_precise_deadlines().unwrap();
            resumed.recv_timeout(Duration::from_secs(2)).unwrap();
            let past = Instant::now() - Duration::from_secs(1);
            if !cancel {
                assert_eq!(context.receive_until(past), WorkerReceive::Command(1));
                assert_eq!(context.receive_until(past), WorkerReceive::Command(2));
            }
            assert_eq!(context.receive_until(past), WorkerReceive::Disconnected);
        })
        .unwrap();
        worker.send_weighted(1, 1).unwrap();
        worker.send_weighted(2, 1).unwrap();
        if cancel {
            worker.cancel();
        } else {
            worker.finish_admission();
        }
        resume.send(()).unwrap();
        assert!(pool.wait_for_idle(Duration::from_secs(2)));
    }
}
