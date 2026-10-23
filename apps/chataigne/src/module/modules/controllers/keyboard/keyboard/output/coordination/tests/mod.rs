use super::*;
use golden_io::RevisionSource;
use std::{sync::mpsc, thread};

fn ticket(coordinator: &Arc<Coordinator>, scope: &RevisionSource, tick: u64) -> Ticket {
    coordinator
        .capture(
            EngineTime { tick, micro: 0, seq: 0 },
            Origin::Fixture,
            [scope.reader().guard_at(0), scope.reader().guard_at(0)],
        )
        .unwrap()
}

#[test]
fn pending_time_order_and_equal_time_admission_order_precede_native_submission_order() {
    let coordinator = Coordinator::new();
    let scope = RevisionSource::default();
    let later = ticket(&coordinator, &scope, 2);
    let earlier = ticket(&coordinator, &scope, 1);
    let last = ticket(&coordinator, &scope, 2);
    let (sent, received) = mpsc::channel();
    let next_sent = sent.clone();
    let next = thread::spawn(move || {
        let _permit = later.acquire(|| false).unwrap();
        next_sent.send(2).unwrap();
        later.finish();
    });
    let tail = thread::spawn(move || {
        let _permit = last.acquire(|| false).unwrap();
        sent.send(3).unwrap();
        last.finish();
    });
    let permit = earlier.acquire(|| false).unwrap();
    assert!(received.recv_timeout(Duration::from_millis(25)).is_err());
    earlier.finish();
    // Completion releases the ordering claim, but not the active native call's exclusion.
    assert!(received.recv_timeout(Duration::from_millis(25)).is_err());
    drop(permit);
    assert_eq!(received.recv_timeout(Duration::from_secs(2)).unwrap(), 2);
    assert_eq!(received.recv_timeout(Duration::from_secs(2)).unwrap(), 3);
    next.join().unwrap();
    tail.join().unwrap();
}

#[test]
fn retired_unsubmitted_head_cannot_strand_a_peer_while_original_ticket_is_retained() {
    let coordinator = Coordinator::new();
    let first_scope = RevisionSource::default();
    let next_scope = RevisionSource::default();
    let first = ticket(&coordinator, &first_scope, 0);
    let next = ticket(&coordinator, &next_scope, 1);
    let (sent, received) = mpsc::channel();
    let worker = thread::spawn(move || {
        let _permit = next.acquire(|| false).unwrap();
        sent.send(()).unwrap();
        next.finish();
    });
    assert!(received.recv_timeout(Duration::from_millis(25)).is_err());
    drop(first_scope);
    received.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(!first.live());
    worker.join().unwrap();
}

#[test]
fn ordering_admission_is_bounded_and_completion_recovers_slots_before_ticket_disposal() {
    let coordinator = Coordinator::new();
    let scope = RevisionSource::default();
    let tickets = (0..512).map(|_| ticket(&coordinator, &scope, 0)).collect::<Vec<_>>();
    let capture = || {
        coordinator.capture(
            EngineTime {
                tick: 1,
                micro: 0,
                seq: 0,
            },
            Origin::Fixture,
            [scope.reader().guard_at(0), scope.reader().guard_at(0)],
        )
    };
    assert!(capture().is_err());
    tickets[0].finish();
    let replacement = capture().unwrap();
    assert_eq!(coordinator.state.lock().unwrap().pending.len(), 512);
    assert!(capture().is_err());
    drop((tickets, replacement));
    assert_eq!(coordinator.state.lock().unwrap().pending.len(), 0);
}
