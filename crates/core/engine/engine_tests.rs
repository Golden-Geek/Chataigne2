use super::*;
use crate::edit::Edit;
use crate::events::{CustomEvent, EventKind};
use crate::node::{Container, Manager, NodeId};
use crate::parameter::{ParamValue, Parameter, ParameterChangeCheck};
use crate::process_ctx::ExecutionPhase;

#[test]
fn absorb_edits_reports_node_type_mismatch() {
    let root = Container::new("root".to_string());
    let mut engine = Engine::new(root);
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);

    ctx.add_child(NodeId(0), Manager::new("manager".to_string()), None);

    let result = engine.absorb_edits(&mut ctx);
    assert!(matches!(
        result,
        Err(EngineEditError::NodeTypeMismatch {
            operation: "AddNode",
            ..
        })
    ));
}

#[test]
fn absorb_edits_accepts_matching_node_type() {
    let root = Container::new("root".to_string());
    let mut engine = Engine::new(root);
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);

    ctx.add_child(NodeId(0), Container::new("child".to_string()), None);

    let result = engine.absorb_edits(&mut ctx);
    assert!(result.is_ok());
    assert_eq!(engine.edits.pending.len(), 1);
}

#[test]
fn apply_edits_adds_children_in_call_order() {
    let mut engine = Engine::new(Container::new("root".to_string()));
    engine.add_node(Container::new("child_a".to_string()), None);
    engine.add_node(Container::new("child_b".to_string()), None);

    engine.apply_edits().expect("apply_edits should succeed");

    let root_data = engine
        .nodes
        .get(engine.root)
        .expect("root should exist")
        .node_data();
    let first = root_data.first_child.expect("first child should exist");
    let second = engine
        .nodes
        .get(first)
        .and_then(|node| node.node_data().next_sibling)
        .expect("second child should exist");

    assert_eq!(
        engine
            .nodes
            .get(first)
            .expect("first node should exist")
            .node_data()
            .meta
            .label,
        "child_a"
    );
    assert_eq!(
        engine
            .nodes
            .get(second)
            .expect("second node should exist")
            .node_data()
            .meta
            .label,
        "child_b"
    );
    assert_eq!(
        engine
            .nodes
            .get(second)
            .and_then(|node| node.node_data().next_sibling),
        None
    );
}

#[test]
fn apply_edits_move_reorders_children() {
    let mut engine = Engine::new(Container::new("root".to_string()));
    engine.add_node(Container::new("child_a".to_string()), None);
    engine.add_node(Container::new("child_b".to_string()), None);
    engine
        .apply_edits()
        .expect("initial apply_edits should succeed");

    let child_a = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("child_a should exist");
    let child_b = engine
        .nodes
        .get(child_a)
        .and_then(|child| child.node_data().next_sibling)
        .expect("child_b should exist");

    engine.edits.push(Edit::MoveNode {
        node: child_a,
        new_parent: engine.root,
        new_prev_sibling: Some(child_b),
    });
    engine.apply_edits().expect("move should succeed");

    let root_data = engine
        .nodes
        .get(engine.root)
        .expect("root should exist")
        .node_data();
    assert_eq!(root_data.first_child, Some(child_b));
    assert_eq!(
        engine
            .nodes
            .get(child_b)
            .and_then(|node| node.node_data().next_sibling),
        Some(child_a)
    );
    assert_eq!(
        engine
            .nodes
            .get(child_a)
            .and_then(|node| node.node_data().next_sibling),
        None
    );
    assert!(
        matches!(
            engine.inbox.events.last().map(|event| &event.kind),
            Some(EventKind::ChildReordered { parent, child }) if *parent == engine.root && *child == child_a
        ),
        "last event should report child reordering",
    );
}

#[test]
fn apply_edits_rejects_cycle_move() {
    let mut engine = Engine::new(Container::new("root".to_string()));
    engine.add_node(Container::new("parent".to_string()), None);
    engine.apply_edits().expect("initial apply should succeed");

    let parent = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("parent should exist");

    engine.add_node(Container::new("child".to_string()), Some(parent));
    engine.apply_edits().expect("second apply should succeed");

    let child = engine
        .nodes
        .get(parent)
        .and_then(|node| node.node_data().first_child)
        .expect("child should exist");

    engine.edits.push(Edit::MoveNode {
        node: parent,
        new_parent: child,
        new_prev_sibling: None,
    });

    let result = engine.apply_edits();
    assert!(matches!(
        result,
        Err(EngineEditError::CycleDetected {
            operation: "MoveNode",
            ..
        })
    ));
}

#[test]
fn apply_edits_set_param_rejects_non_parameter_node() {
    let mut engine = Engine::new(Container::new("root".to_string()));
    engine.edits.push(Edit::SetParam {
        node: engine.root,
        value: ParamValue::Int(12),
    });

    let result = engine.apply_edits();
    assert!(matches!(
        result,
        Err(EngineEditError::ParamEditTargetMismatch { .. })
    ));
}

#[test]
fn apply_edits_set_param_updates_parameter_node() {
    let root = Parameter::new("root_param", ParamValue::Int(10), ParameterChangeCheck::None);
    let mut engine = Engine::new(root);

    engine.edits.push(Edit::SetParam {
        node: engine.root,
        value: ParamValue::Int(42),
    });

    engine.apply_edits().expect("set param should succeed");

    let node = engine
        .nodes
        .get(engine.root)
        .expect("root parameter should exist");
    assert_eq!(node.value, ParamValue::Int(42));
    assert!(
        matches!(
            engine.inbox.events.last().map(|event| &event.kind),
            Some(EventKind::ParamChanged {
                param,
                old_value: ParamValue::Int(10),
            }) if *param == engine.root
        ),
        "last event should report previous parameter value",
    );
}

#[test]
fn emit_custom_event_uses_edit_pipeline() {
    let mut engine = Engine::new(Container::new("root".to_string()));
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);

    ctx.emit_custom_event(CustomEvent::new(
        "transport.play",
        Some(engine.root),
        serde_json::Value::Null,
    ));

    assert!(ctx.events.is_empty(), "custom event should not be injected directly into ctx events");
    assert_eq!(ctx.edits.pending.len(), 1, "custom event should enqueue one edit request");

    engine
        .absorb_edits(&mut ctx)
        .expect("absorb_edits should accept custom event edits");
    engine
        .apply_edits()
        .expect("apply_edits should convert custom event edit into engine event");

    assert!(
        matches!(
            engine.inbox.events.last().map(|event| &event.kind),
            Some(EventKind::Custom(event))
                if event.topic == "transport.play" && event.origin == Some(engine.root)
        ),
        "last event should be the emitted custom event",
    );
}
