use super::deadline;
use golden_io::{BoundedWorker, RetirementPool, WorkerLimits, WorkerWake};
use std::{
    num::NonZeroUsize,
    sync::mpsc,
    time::{Duration, Instant},
};

#[test]
fn overdue_decode_turn_does_not_add_a_sleep_and_paused_turn_stays_wakeable() {
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    let (ready, turns) = mpsc::channel();
    let (resume, resumed) = mpsc::channel();
    let worker = BoundedWorker::<(), ()>::spawn(
        "video-file-pacing",
        &pool,
        WorkerLimits {
            command_items: 1,
            command_weight: 1,
            event_items: 1,
            event_weight: 1,
        },
        move |mut context| {
            context.enable_precise_deadlines().unwrap();
            // Model a decode/conversion that already exceeded both active polling intervals.
            let turn_started = Instant::now() - Duration::from_millis(20);
            for (playing, pending) in [(true, false), (true, true), (false, true)] {
                let checkpoint = context.wake_checkpoint();
                let due = deadline(turn_started, playing, pending);
                assert!(due < Instant::now());
                assert_eq!(context.wait_until(checkpoint, due), WorkerWake::Deadline);
            }
            let checkpoint = context.wake_checkpoint();
            let due = deadline(Instant::now(), false, false);
            assert!(due > Instant::now() + Duration::from_secs(3500));
            ready.send(()).unwrap();
            resumed.recv_timeout(Duration::from_secs(2)).unwrap();
            assert_eq!(context.wait_until(checkpoint, due), WorkerWake::Notified);
            let checkpoint = context.wake_checkpoint();
            ready.send(()).unwrap();
            assert_eq!(context.wait_until(checkpoint, due), WorkerWake::Disconnected);
        },
    )
    .unwrap();
    turns.recv_timeout(Duration::from_secs(2)).unwrap();
    worker.wake_notifier().notify();
    resume.send(()).unwrap();
    turns.recv_timeout(Duration::from_secs(2)).unwrap();
    worker.cancel();
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}
