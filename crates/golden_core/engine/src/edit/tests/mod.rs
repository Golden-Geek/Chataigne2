use super::*;

fn set(queue: &mut EditQueue, node: u64, value: i32) {
    queue.push(Edit::SetParam {
        node: NodeId(node),
        value: ParamValue::Int(value),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
}
fn fence(queue: &mut EditQueue) {
    queue.push(Edit::CheckPreconditions {
        preconditions: EditPreconditions::new(vec![], vec![]).unwrap(),
    });
}
fn values(queue: &EditQueue) -> Vec<(NodeId, i32)> {
    queue
        .pending
        .iter()
        .filter_map(|request| match &request.edit {
            Edit::SetParam {
                node,
                value: ParamValue::Int(value),
                ..
            } => Some((*node, *value)),
            _ => None,
        })
        .collect()
}

#[test]
fn later_guarded_scalar_writes_preserve_the_earlier_prefix_and_its_completion() {
    let mut queue = EditQueue::new();
    set(&mut queue, 1, 10);
    let prefix = queue.completion();
    fence(&mut queue);
    set(&mut queue, 1, 20);
    set(&mut queue, 1, 30);
    assert_eq!(queue.pending.len(), 3);
    assert_eq!(values(&queue), [(NodeId(1), 10), (NodeId(1), 30)]);
    assert_eq!(prefix.status(), EditCompletionStatus::Pending);
}

#[test]
fn ordinary_scalar_coalescing_still_supersedes_the_observed_value_inside_one_scope() {
    let mut queue = EditQueue::new();
    set(&mut queue, 1, 10);
    let first = queue.completion();
    set(&mut queue, 1, 20);
    assert_eq!(values(&queue), [(NodeId(1), 20)]);
    assert_eq!(first.status(), EditCompletionStatus::Failed);
    fence(&mut queue);
    set(&mut queue, 2, 1);
    set(&mut queue, 1, 30);
    set(&mut queue, 2, 2);
    assert_eq!(values(&queue), [(NodeId(1), 20), (NodeId(1), 30), (NodeId(2), 2)]);
}

#[test]
fn duplicate_removals_share_completion_only_within_the_same_guarded_scope() {
    let mut queue = EditQueue::new();
    queue.push(Edit::RemoveNode { node: NodeId(1) });
    queue.push(Edit::RemoveNode { node: NodeId(1) });
    assert_eq!(queue.pending.len(), 1);
    fence(&mut queue);
    queue.push(Edit::RemoveNode { node: NodeId(1) });
    queue.push(Edit::RemoveNode { node: NodeId(1) });
    assert_eq!(queue.pending.len(), 3);
}
