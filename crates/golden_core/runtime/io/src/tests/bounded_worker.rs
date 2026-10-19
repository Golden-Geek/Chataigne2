use crate::{BoundedWorker, PendingSendError, RetirementPool, WorkerLimits, WorkerStartError};
use std::num::NonZeroUsize;
use std::sync::mpsc;
use std::time::Duration;

fn limits() -> WorkerLimits {
    WorkerLimits {
        command_items: 2,
        command_weight: 5,
        event_items: 1,
        event_weight: 5,
    }
}

#[test]
fn bounded_command_turns_preserve_fifo_through_graceful_close() {
    let pool = pool(1);
    let (turn_tx, turn_rx) = mpsc::channel();
    let (resume_tx, resume_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    let mut worker = BoundedWorker::<u32, ()>::spawn("command-turns", &pool, limits(), move |context| {
        let mut commands = Vec::new();
        assert_eq!(
            context.drain_commands(&mut commands, NonZeroUsize::MIN).state,
            crate::PendingDrainState::Empty
        );
        turn_tx.send(()).unwrap();
        resume_rx.recv().unwrap();
        let first = context.drain_commands(&mut commands, NonZeroUsize::MIN);
        assert_eq!(first.received, 1);
        assert_eq!(first.state, crate::PendingDrainState::BudgetExhausted);
        let last = context.drain_commands(&mut commands, NonZeroUsize::MIN);
        assert_eq!(last.received, 1);
        assert_eq!(last.state, crate::PendingDrainState::Disconnected);
        assert_eq!(commands, [1, 2]);
        done_tx.send(()).unwrap();
    })
    .unwrap();
    turn_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    worker.send_weighted(1, 1).unwrap();
    worker.send_weighted(2, 1).unwrap();
    worker.finish_admission();
    assert!(!worker.is_accepting_commands());
    resume_tx.send(()).unwrap();
    done_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn explicit_cancellation_ends_nonblocking_command_service() {
    let pool = pool(1);
    let (entered_tx, entered_rx) = mpsc::channel();
    let (resume_tx, resume_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    let worker = BoundedWorker::<u32, ()>::spawn("cancel-command-turn", &pool, limits(), move |context| {
        entered_tx.send(()).unwrap();
        resume_rx.recv().unwrap();
        let mut commands = Vec::new();
        let turn = context.drain_commands(&mut commands, NonZeroUsize::MIN);
        assert_eq!(turn.received, 0);
        assert_eq!(turn.state, crate::PendingDrainState::Disconnected);
        assert!(commands.is_empty());
        done_tx.send(()).unwrap();
    })
    .unwrap();
    entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    worker.send_weighted(1, 1).unwrap();
    worker.cancel();
    assert!(!worker.is_accepting_commands());
    resume_tx.send(()).unwrap();
    done_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}

fn pool(capacity: usize) -> RetirementPool {
    RetirementPool::new(NonZeroUsize::new(capacity).unwrap())
}

#[test]
fn notified_worker_publishes_readiness_after_reply_and_after_native_cleanup() {
    let pool = pool(1);
    let (ready_tx, ready_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (cleaned_tx, cleaned_rx) = mpsc::channel();
    struct Native(mpsc::Sender<()>);
    impl Drop for Native {
        fn drop(&mut self) {
            let _ = self.0.send(());
        }
    }
    let worker = BoundedWorker::<(), u8>::spawn_notified(
        "notified-worker",
        &pool,
        limits(),
        crate::WorkerNotifier::new(move || {
            let _ = ready_tx.send(());
        }),
        move |context| {
            let _native = Native(cleaned_tx);
            context.publish_weighted(7, 1).unwrap();
            release_rx.recv().unwrap();
        },
    )
    .unwrap();
    ready_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(worker.has_pending());
    let mut events = Vec::new();
    assert_eq!(worker.drain_events(&mut events, NonZeroUsize::MIN).received, 1);
    assert_eq!(events, [7]);
    assert!(!worker.has_pending());
    release_tx.send(()).unwrap();
    ready_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    cleaned_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(worker.has_pending());
    assert_eq!(
        worker.drain_events(&mut events, NonZeroUsize::MIN).state,
        crate::PendingDrainState::Disconnected
    );
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn a_panicking_worker_notifies_its_owner_of_disconnection_without_a_reply() {
    let pool = pool(1);
    let (ready_tx, ready_rx) = mpsc::channel();
    let worker = BoundedWorker::<(), ()>::spawn_notified(
        "panic-notification",
        &pool,
        limits(),
        crate::WorkerNotifier::new(move || {
            let _ = ready_tx.send(());
        }),
        |_context| panic!("controlled worker failure"),
    )
    .unwrap();
    ready_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(worker.has_pending());
    assert_eq!(
        worker.drain_events(&mut Vec::new(), NonZeroUsize::MIN).state,
        crate::PendingDrainState::Disconnected
    );
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn command_admission_rejects_items_and_weight_with_original_ownership() {
    let pool = pool(1);
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let worker = BoundedWorker::<String, ()>::spawn("weighted-commands", &pool, limits(), move |context| {
        entered_tx.send(()).unwrap();
        release_rx.recv().unwrap();
        assert_eq!(context.receive().as_deref(), Some("first"));
        assert_eq!(context.receive().as_deref(), Some("second"));
    })
    .unwrap();
    entered_rx.recv().unwrap();
    worker.send_weighted("first".into(), 4).unwrap();
    assert_eq!(
        worker.send_weighted("too heavy".into(), 2),
        Err(PendingSendError::Full("too heavy".into()))
    );
    worker.send_weighted("second".into(), 1).unwrap();
    assert_eq!(
        worker.send_weighted("too many".into(), 0),
        Err(PendingSendError::Full("too many".into()))
    );
    assert!(
        worker.is_accepting_commands(),
        "capacity pressure must remain distinct from closure"
    );
    release_tx.send(()).unwrap();
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
    assert_eq!(
        worker.send_weighted("disconnected".into(), 1),
        Err(PendingSendError::Disconnected("disconnected".into()))
    );
    assert!(!worker.is_accepting_commands());
}

#[test]
fn full_replies_apply_backpressure_and_resume_in_order() {
    let pool = pool(1);
    let (published_tx, published_rx) = mpsc::channel();
    let worker = BoundedWorker::<(), u32>::spawn("ordered-replies", &pool, limits(), move |context| {
        context.publish_weighted(1, 5).unwrap();
        published_tx.send(()).unwrap();
        context.publish_weighted(2, 5).unwrap();
        published_tx.send(()).unwrap();
    })
    .unwrap();
    published_rx.recv().unwrap();
    assert!(published_rx.try_recv().is_err());
    let mut replies = Vec::new();
    assert_eq!(worker.drain_events(&mut replies, NonZeroUsize::MIN).received, 1);
    published_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(worker.drain_events(&mut replies, NonZeroUsize::MIN).received, 1);
    assert_eq!(replies, [1, 2]);
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn cancellation_releases_full_mailboxes_and_retains_native_cleanup_capacity() {
    let pool = pool(1);
    let (waiting_tx, waiting_rx) = mpsc::channel();
    let (cleanup_tx, cleanup_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let worker = BoundedWorker::<u32, u32>::spawn("cancel-full", &pool, limits(), move |context| {
        context.publish_weighted(1, 5).unwrap();
        waiting_tx.send(()).unwrap();
        assert_eq!(context.publish_weighted(2, 5), Err(PendingSendError::Disconnected(2)));
        cleanup_tx.send(()).unwrap();
        release_rx.recv().unwrap();
    })
    .unwrap();
    waiting_rx.recv().unwrap();
    worker.send_weighted(1, 1).unwrap();
    worker.send_weighted(2, 1).unwrap();
    worker.cancel();
    assert_eq!(worker.send_weighted(3, 1), Err(PendingSendError::Disconnected(3)));
    drop(worker);
    cleanup_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(matches!(
        BoundedWorker::<(), ()>::spawn("rejected-replacement", &pool, limits(), |_| {}),
        Err(WorkerStartError::Capacity(_))
    ));
    release_tx.send(()).unwrap();
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
    let replacement = BoundedWorker::<(), ()>::spawn("replacement", &pool, limits(), |_| {}).unwrap();
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
    drop(replacement);
    assert_eq!(pool.metrics().peak, 1);
}

#[test]
fn cancellation_interrupts_retry_wait_and_idle_receive() {
    for retry in [false, true] {
        let pool = pool(1);
        let (entered_tx, entered_rx) = mpsc::channel();
        let worker = BoundedWorker::<(), ()>::spawn("cancel-wait", &pool, limits(), move |context| {
            entered_tx.send(()).unwrap();
            if retry {
                assert!(context.cancellation().wait(Duration::from_secs(60)));
            } else {
                assert!(context.receive().is_none());
            }
        })
        .unwrap();
        entered_rx.recv().unwrap();
        drop(worker);
        assert!(pool.wait_for_idle(Duration::from_secs(2)));
    }
}

#[test]
fn oversized_reply_is_rejected_without_waiting_and_worker_failure_releases_its_slot() {
    let pool = pool(1);
    let worker = BoundedWorker::<(), u32>::spawn("oversized-reply", &pool, limits(), |context| {
        assert_eq!(context.publish_weighted(7, 6), Err(PendingSendError::Full(7)));
        panic!("injected worker failure");
    })
    .unwrap();
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
    assert!(
        worker.has_pending(),
        "a dead worker must wake its event-driven owner even without a reply"
    );
    assert_eq!(worker.send_weighted((), 1), Err(PendingSendError::Disconnected(())));
    drop(worker);
    let next = BoundedWorker::<(), ()>::spawn("after-failure", &pool, limits(), |_| {}).unwrap();
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
    drop(next);
}

#[test]
fn graceful_admission_close_preserves_full_commands_and_replies_until_drained() {
    let pool = pool(1);
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (published_tx, published_rx) = mpsc::channel();
    let mut worker = BoundedWorker::<u32, u32>::spawn("graceful-full", &pool, limits(), move |context| {
        entered_tx.send(()).unwrap();
        release_rx.recv().unwrap();
        while let Some(command) = context.receive() {
            assert!(!context.cancellation().is_cancelled());
            context.publish_weighted(command, 5).unwrap();
            published_tx.send(()).unwrap();
        }
    })
    .unwrap();
    entered_rx.recv().unwrap();
    worker.send_weighted(1, 2).unwrap();
    worker.send_weighted(2, 3).unwrap();
    worker.finish_admission();
    worker.finish_admission();
    assert_eq!(worker.send_weighted(3, 1), Err(PendingSendError::Disconnected(3)));
    release_tx.send(()).unwrap();
    published_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(pool.metrics().active, 1);
    let mut replies = Vec::new();
    worker.drain_events(&mut replies, NonZeroUsize::MIN);
    published_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
    worker.drain_events(&mut replies, NonZeroUsize::MIN);
    assert_eq!(replies, [1, 2]);
}

#[test]
fn worker_can_close_failed_admission_and_resolve_every_previously_accepted_command() {
    let pool = pool(1);
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (closed_tx, closed_rx) = mpsc::channel();
    let worker = BoundedWorker::<u32, u32>::spawn("close-and-resolve", &pool, limits(), move |context| {
        entered_tx.send(()).unwrap();
        release_rx.recv().unwrap();
        context.close_command_admission();
        closed_tx.send(()).unwrap();
        while let Some(command) = context.receive() {
            context.publish_weighted(command, 1).unwrap();
        }
    })
    .unwrap();
    entered_rx.recv().unwrap();
    worker.send_weighted(1, 1).unwrap();
    worker.send_weighted(2, 1).unwrap();
    release_tx.send(()).unwrap();
    closed_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(
        !worker.is_accepting_commands(),
        "native closure must be observable before draining replies"
    );
    assert_eq!(worker.send_weighted(3, 1), Err(PendingSendError::Disconnected(3)));
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    let mut replies = Vec::new();
    while replies.len() < 2 && std::time::Instant::now() < deadline {
        worker.drain_events(&mut replies, NonZeroUsize::MIN);
        std::thread::yield_now();
    }
    assert_eq!(replies, [1, 2]);
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}
