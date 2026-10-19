use super::super::super::{output_plan, tests::snapshot::nested_snapshot};
use super::*;

#[test]
fn cold_admission_does_not_prepare_on_the_caller_and_invalid_plans_resolve_once_after_setup() {
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    let (entered_tx, entered_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::sync_channel(1);
    let mut handle = UdpStreamingTransportHandle::spawn_with_setup(config(), &pool, notifier(), move || {
        entered_tx.send(()).unwrap();
        release_rx.recv().unwrap();
        UdpSocket::bind("127.0.0.1:0").map_err(|error| error.to_string())
    })
    .unwrap();
    entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    let snapshot = Arc::new(nested_snapshot(20_000, &"x".repeat(1025), 1));
    let (command, candidate) =
        output_plan::command(&mut None, &snapshot, golden_core::node::NodeId(1), vec![1], false).unwrap();
    assert!(
        candidate.ticket.result().is_none(),
        "caller admission must not inspect an invalid hostname deep in the tree"
    );
    handle.send(command).unwrap();
    assert!(
        candidate.ticket.result().is_none(),
        "blocked native setup must retain unprepared work"
    );
    handle.finish_admission();
    release_tx.send(()).unwrap();
    assert!(pool.wait_for_idle(Duration::from_secs(3)));
    let error = candidate.ticket.result().unwrap().as_ref().unwrap_err();
    assert!(!error.retryable);
    assert!(error.message.contains("1024"));
    let mut events = Vec::new();
    while handle.drain_events(&mut events).state != PendingDrainState::Disconnected {}
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, StreamingWorkerEvent::Error(_)))
            .count(),
        1
    );
}

#[test]
fn accepted_cold_plan_uses_its_captured_view_after_a_later_endpoint_configuration() {
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    let (release_tx, release_rx) = mpsc::sync_channel(1);
    let mut handle = UdpStreamingTransportHandle::spawn_with_setup(config(), &pool, notifier(), move || {
        release_rx.recv().unwrap();
        UdpSocket::bind("127.0.0.1:0").map_err(|error| error.to_string())
    })
    .unwrap();
    let old_receiver = UdpSocket::bind("127.0.0.1:0").unwrap();
    let new_receiver = UdpSocket::bind("127.0.0.1:0").unwrap();
    old_receiver.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    new_receiver.set_read_timeout(Some(Duration::from_millis(100))).unwrap();
    let old = Arc::new(nested_snapshot(
        20_000,
        "127.0.0.1",
        old_receiver.local_addr().unwrap().port(),
    ));
    let (command, candidate) =
        output_plan::command(&mut None, &old, golden_core::node::NodeId(1), vec![7], false).unwrap();
    handle.send(command).unwrap();
    let accepted_ticket = candidate.ticket.clone();
    let mut cache = Some(candidate);
    let new = Arc::new(nested_snapshot(
        20_000,
        "127.0.0.1",
        new_receiver.local_addr().unwrap().port(),
    ));
    assert_ne!(
        old.subtree_revision(golden_core::node::NodeId(1)),
        new.subtree_revision(golden_core::node::NodeId(1))
    );
    let (next, rejected_candidate) =
        output_plan::command(&mut cache, &new, golden_core::node::NodeId(1), vec![8], false).unwrap();
    assert!(handle
        .send(next)
        .unwrap_err()
        .contains("whole command was not admitted"));
    drop(rejected_candidate);
    assert!(
        cache.is_none(),
        "a rejected replacement must not install an ownerless pending ticket"
    );
    handle.finish_admission();
    release_tx.send(()).unwrap();
    assert!(pool.wait_for_idle(Duration::from_secs(3)));
    assert!(accepted_ticket.result().unwrap().is_ok());
    let mut bytes = [0; 8];
    assert_eq!(old_receiver.recv(&mut bytes).unwrap(), 1);
    assert_eq!(bytes[0], 7);
    assert!(new_receiver.recv(&mut bytes).is_err());
}
