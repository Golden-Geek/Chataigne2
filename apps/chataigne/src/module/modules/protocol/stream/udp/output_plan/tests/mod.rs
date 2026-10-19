use super::super::tests::snapshot::nested_snapshot;
use super::*;

#[test]
fn native_preparation_yields_after_a_bounded_number_of_edges() {
    let snapshot = Arc::new(nested_snapshot(20_000, "127.0.0.1", 9000));
    let (mut send, _) = command(&mut None, &snapshot, NodeId(1), vec![1], false).unwrap();
    let visits = std::cell::Cell::new(0);
    assert!(!send.prepare_turn(|| { visits.set(visits.get() + 1); false }).unwrap());
    assert_eq!(visits.get(), PREPARATION_EDGES_PER_TURN);
    assert!(send.prepared_count().is_none());
    send.prepare(|| false).unwrap();
    assert_eq!(send.len(), 1);
}

#[test]
fn deep_folder_preparation_uses_no_recursive_stack_or_output_id_accumulation() {
    let snapshot = nested_snapshot(20_000, "127.0.0.1", 9000);
    let plan = prepare(&snapshot, NodeId(1), UDP_PLAN_BYTES, || false).unwrap();
    assert_eq!(plan.len(), 1);
    assert_eq!(plan.target(0).remote_host, "127.0.0.1");
}

#[test]
fn pending_native_owner_is_reused_and_invalid_hosts_fail_without_publishing_a_prefix() {
    let snapshot = Arc::new(nested_snapshot(2, "127.0.0.1", 9000));
    let mut cache = None;
    let (mut first, candidate) = command(&mut cache, &snapshot, NodeId(1), vec![1], false).unwrap();
    assert!(first.prepared_count().is_none());
    cache = Some(candidate);
    let (mut second, _) = command(&mut cache, &snapshot, NodeId(1), vec![2], false).unwrap();
    first.prepare(|| false).unwrap();
    second.prepare(|| false).unwrap();
    assert!(std::ptr::eq(first.target(0), second.target(0)));
    let invalid = Arc::new(nested_snapshot(2, &"x".repeat(1025), 9000));
    let (mut rejected, candidate) = command(&mut cache, &invalid, NodeId(1), vec![3], false).unwrap();
    assert!(rejected.prepare(|| false).is_err());
    cache = Some(candidate);
    assert!(command(&mut cache, &invalid, NodeId(1), vec![4], false).is_err());
    let (mut recovered, _) = command(&mut cache, &snapshot, NodeId(1), vec![5], false).unwrap();
    recovered.prepare(|| false).unwrap();
    assert_eq!(recovered.target(0).remote_host, "127.0.0.1");
}

#[test]
fn cancellation_interrupts_a_deep_folder_walk_and_transient_preparation_can_retry() {
    let snapshot = Arc::new(nested_snapshot(20_000, "127.0.0.1", 9000));
    let visits = std::cell::Cell::new(0);
    let result = prepare(&snapshot, NodeId(1), UDP_PLAN_BYTES, || {
        let count = visits.get() + 1;
        visits.set(count);
        count == 64
    });
    assert!(result.is_err());
    assert_eq!(visits.get(), 64);
    let ticket = PlanTicket::pending(UDP_PLAN_BYTES);
    ticket.fail("temporary pool pressure".into());
    let mut cache = Some(OutputPlanCache {
        root: NodeId(1),
        revision: snapshot.subtree_revision(NodeId(1)).unwrap(),
        ticket,
    });
    let (mut retry, _) = command(&mut cache, &snapshot, NodeId(1), vec![7], false).unwrap();
    assert!(retry.prepared_count().is_none());
    retry.prepare(|| false).unwrap();
    assert_eq!(retry.len(), 1);
}
