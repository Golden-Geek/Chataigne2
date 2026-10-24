use super::*;
use crate::{
    edit::Edit,
    events::{CustomEvent, CustomEventRetention, EventKind},
    node::Folder,
    parameter::{ParamValue, Parameter, ParameterChangeCheck, ParameterEventBehaviour},
    process_ctx::{ExecutionPhase, ProcessCtx},
};
use std::sync::mpsc;

#[test]
fn moves_notify_the_former_parent_and_deduplicate_shared_ancestors() {
    let observer = |label| Parameter::new(label, ParamValue::Int(0), ParameterChangeCheck::ValueChange);
    let mut engine = Engine::new(observer("root"));
    engine.add_node(observer("left"), None);
    engine.add_node(observer("right"), None);
    engine.apply_edits().unwrap();
    let root = engine.root;
    let left = engine.nodes.get(root).unwrap().node_data().first_child.unwrap();
    let right = engine.nodes.get(left).unwrap().node_data().next_sibling.unwrap();
    engine.add_node(observer("moving"), Some(left));
    engine.apply_edits().unwrap();
    let child = engine.nodes.get(left).unwrap().node_data().first_child.unwrap();
    engine.edits.push(Edit::AddEventListener {
        subscriber: root,
        subscription: crate::node::EventSubscription::node(child),
    });
    engine.apply_edits().unwrap();
    engine.inbox.clear();
    engine.edits.push(Edit::MoveNode {
        node: child,
        new_parent: right,
        new_prev_sibling: None,
    });
    engine.apply_edits().unwrap();
    let routed = engine.precompute_inbox_dispatch_since(0);
    for expected in [root, left, right, child] {
        let count = routed
            .iter()
            .filter(|(node, frame)| {
                *node == expected
                    && frame
                        .iter()
                        .any(|event| matches!(event.kind, EventKind::ChildMoved { .. }))
            })
            .count();
        assert_eq!(count, 1, "moved-child notification for {expected:?}");
    }
    // A later removal must not hide the earlier departure from its owner.
    engine.inbox.clear();
    engine.edits.push(Edit::MoveNode {
        node: child,
        new_parent: left,
        new_prev_sibling: None,
    });
    engine.edits.push(Edit::RemoveNode { node: child });
    engine.apply_edits().unwrap();
    let routed = engine.precompute_inbox_dispatch_since(0);
    assert!(routed.iter().any(|(node, frame)| {
        *node == right
            && frame
                .iter()
                .any(|event| matches!(event.kind, EventKind::ChildMoved { .. }))
    }));
}

#[test]
fn typed_local_delivery_is_shallow_inbox_only_and_keeps_redo() {
    let mut engine = Engine::new(Parameter::new("root", ParamValue::Int(0), ParameterChangeCheck::None));
    let root = engine.root;
    engine.edits.push(Edit::SetParam {
        node: root,
        value: ParamValue::Int(1),
        behaviour: ParameterEventBehaviour::Append,
    });
    engine.apply_edits().unwrap();
    assert!(engine.undo().unwrap());
    let ui_count = engine.ui_event_log().len();
    engine.inbox.clear();
    let mut event = CustomEvent::local("runtime.native", Some(root), vec![7_u8; 1024]);
    let pointer = event.local_payload::<Vec<u8>>().unwrap().as_ptr();
    event.retention = CustomEventRetention::Replay;
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    ctx.emit_custom_event(event);
    engine.absorb_edits(&mut ctx).unwrap();
    engine.apply_edits().unwrap();
    assert_eq!(engine.ui_event_log().len(), ui_count);
    let EventKind::Custom(event) = &engine.inbox.events[0].kind else {
        panic!("local event")
    };
    assert_eq!(event.local_payload::<Vec<u8>>().unwrap().as_ptr(), pointer);
    assert_eq!(engine.redo_len(), 1);
    assert!(engine.redo().unwrap());
}

#[test]
fn opaque_callbacks_reserve_target_lifetimes_and_history_restore_never_reopens_old_guards() {
    let mut engine = Engine::new(Folder::new("root"));
    engine.add_node(Folder::new("target"), None);
    engine.apply_edits().unwrap();
    let root = engine.root;
    let target = engine.nodes.get(root).unwrap().node_data().first_child.unwrap();
    let (sender, receiver) = mpsc::channel();
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    ctx.call_node_mutation(root, move |_, ctx| {
        sender
            .send((ctx.node_waker().unwrap(), ctx.node_waker_for(target).unwrap()))
            .unwrap();
        assert!(ctx.node_waker_for(NodeId(u64::MAX)).is_err());
        Ok(())
    });
    engine.absorb_edits(&mut ctx).unwrap();
    engine.apply_edits().unwrap();
    let (source, old) = receiver.recv().unwrap();
    assert!(!source.is_closed() && !old.is_closed());
    let stale = engine.process_tree_snapshot();
    engine.edits.push(Edit::RemoveNode { node: target });
    engine.apply_edits().unwrap();
    assert!(old.is_closed() && !source.is_closed());
    ctx.call_node_mutation(root, move |_, ctx| {
        ctx.set_tree_snapshot(stale);
        assert!(
            ctx.node_waker_for(target).is_err(),
            "an old read model cannot revive a detached target"
        );
        Ok(())
    });
    engine.absorb_edits(&mut ctx).unwrap();
    engine.apply_edits().unwrap();
    assert!(engine.undo().unwrap());
    let (sender, receiver) = mpsc::channel();
    ctx.call_node_mutation(root, move |_, ctx| {
        sender.send(ctx.node_waker_for(target).unwrap()).unwrap();
        Ok(())
    });
    engine.absorb_edits(&mut ctx).unwrap();
    engine.apply_edits().unwrap();
    let new = receiver.recv().unwrap();
    assert_ne!(old, new);
    assert!(old.is_closed() && !new.is_closed());
}
