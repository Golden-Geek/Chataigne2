use super::*;
use std::time::{Duration, Instant};

use crate::edit::Edit;
use crate::events::{CustomEvent, EventKind};
use crate::node::{Folder, EventPropagation, EventSubscription, Manager, Node, NodeData, NodeId};
use crate::parameter::{ParamValue, Parameter, ParameterChangeCheck, ParameterEventBehaviour};
use crate::process_ctx::{ExecutionPhase, ProcessCtx};

#[test]
fn absorb_edits_reports_node_type_mismatch() {
    let root = Folder::new("root".to_string());
    let mut engine = Engine::new(root);
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);

    ctx.add_child(NodeId(0), Manager::new("manager".to_string()), None);

    let result = engine.absorb_edits(&mut ctx);
    assert!(matches!(result, Err(EngineEditError::NodeTypeMismatch { operation: "AddNode", .. })));
}

#[test]
fn absorb_edits_accepts_matching_node_type() {
    let root = Folder::new("root".to_string());
    let mut engine = Engine::new(root);
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);

    ctx.add_child(NodeId(0), Folder::new("child".to_string()), None);

    let result = engine.absorb_edits(&mut ctx);
    assert!(result.is_ok());
    assert_eq!(engine.edits.pending.len(), 1);
}

#[test]
fn external_edit_sender_sets_param_from_another_thread() {
    let root = Parameter::new("root_param", ParamValue::Int(0), ParameterChangeCheck::None);
    let mut engine = Engine::new(root);
    let root_id = engine.root;
    let sender = engine.external_edit_sender();

    std::thread::spawn(move || {
        sender
            .send(Edit::SetParam {
                node: root_id,
                value: ParamValue::Int(33),
                behaviour: ParameterEventBehaviour::Coalesce,
            })
            .expect("external edit send should succeed");
    })
    .join()
    .expect("external producer thread should join");

    engine.apply_edits().expect("external edit should apply");
    assert_eq!(
        engine.nodes.get(root_id).expect("root parameter should exist").value,
        ParamValue::Int(33),
        "external set-param should be drained and applied",
    );
}

#[test]
fn external_edit_sender_adds_node_from_another_thread() {
    let mut engine = Engine::new(Folder::new("root".to_string()));
    let root_id = engine.root;
    let sender = engine.external_edit_sender();

    std::thread::spawn(move || {
        sender
            .send(Edit::AddNode {
                parent: root_id,
                prev_sibling: None,
                node: Box::new(Folder::new("external_child".to_string())),
            })
            .expect("external add-node send should succeed");
    })
    .join()
    .expect("external producer thread should join");

    engine.apply_edits().expect("external add-node edit should apply");

    let child = engine
        .nodes
        .get(root_id)
        .and_then(|root| root.node_data().first_child)
        .expect("root should contain one child");
    assert_eq!(
        engine.nodes.get(child).expect("child node should exist").node_data().meta.label,
        "external_child"
    );
}

#[test]
fn run_tick_drains_external_edits_without_manual_apply_call() {
    let root = Parameter::new("root_param", ParamValue::Int(0), ParameterChangeCheck::None);
    let mut engine = Engine::new(root);
    engine.resolve().expect("resolve should succeed");

    let root_id = engine.root;
    let sender = engine.external_edit_sender();
    sender
        .send(Edit::SetParam {
            node: root_id,
            value: ParamValue::Int(7),
            behaviour: ParameterEventBehaviour::Coalesce,
        })
        .expect("external set-param send should succeed");

    engine
        .run_tick(Duration::from_millis(1))
        .expect("tick should drain and apply external edits");

    assert_eq!(
        engine.nodes.get(root_id).expect("root parameter should exist").value,
        ParamValue::Int(7)
    );
}

#[test]
fn external_coalesced_set_param_edits_keep_latest_value() {
    let root = Parameter::new("root_param", ParamValue::Int(0), ParameterChangeCheck::None);
    let mut engine = Engine::new(root);
    let root_id = engine.root;
    let sender = engine.external_edit_sender();

    sender
        .send(Edit::SetParam {
            node: root_id,
            value: ParamValue::Int(1),
            behaviour: ParameterEventBehaviour::Coalesce,
        })
        .expect("first external set-param send should succeed");
    sender
        .send(Edit::SetParam {
            node: root_id,
            value: ParamValue::Int(2),
            behaviour: ParameterEventBehaviour::Coalesce,
        })
        .expect("second external set-param send should succeed");

    engine.absorb_external_edits().expect("external edits should be absorbed");
    assert_eq!(engine.edits.pending.len(), 1, "coalesced external edits should collapse before apply");

    engine.apply_edits().expect("external edits should apply");
    assert_eq!(
        engine.nodes.get(root_id).expect("root parameter should exist").value,
        ParamValue::Int(2),
        "latest coalesced value should win",
    );
}

#[crate::node("auto_declared")]
struct AutoDeclaredNode {
    #[param(default = 0.5, label = "Decay", description = "Envelope decay time")]
    decay: crate::node::ParameterHandle<f64>,

    #[potential_node(decl_id = "value")]
    value: crate::node::PotentialNodeHandle,
}

struct DslParamsNode {
    node_data: NodeData,
    feedback: crate::node::ParameterHandle<f64>,
    host: crate::node::ParameterHandle<String>,
    gamma: crate::node::ParameterHandle<f64>,
}

impl DslParamsNode {
    fn new(label: impl Into<String>) -> Self {
        Self {
            node_data: NodeData::new(label.into()),
            feedback: crate::node::ParameterHandle::new(NodeId(0), 0.5),
            host: crate::node::ParameterHandle::new(NodeId(0), "127.0.0.1".to_string()),
            gamma: crate::node::ParameterHandle::new(NodeId(0), 2.2),
        }
    }
}

#[crate::node("dsl_params_node")]
impl Node for DslParamsNode {
    crate::params! {
        feedback: f64 = 0.5 [0.0..1.0] (label = "Feedback", description = "Delay feedback amount");

        folder(output, label = "Output") {
            host: String = "127.0.0.1" (label = "Host", description = "OSC destination host");

            folder(color, label = "Color") {
                gamma: f64 = 2.2 (behavior = "Append");
            }
        }
    }
}

crate::define_node_enum!(
    enum MacroTestNode {
        AutoDeclaredNode,
        DslParamsNode
    }
);

#[test]
fn node_struct_macro_declares_param_and_binds_handle_after_child_event() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);
    engine.add_node(AutoDeclaredNode::new("declared").into(), None);

    // First pass adds the node and runs generated init, which queues param creation.
    engine.apply_edits().expect("first apply should succeed");
    // Second pass materializes generated child param nodes.
    engine.apply_edits().expect("second apply should succeed");

    let declared_id = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("declared child should exist");

    let child_added_decl = engine.inbox.events.iter().any(|event| {
        matches!(
            &event.kind,
            EventKind::ChildAdded { parent, decl_id, .. }
                if *parent == declared_id && decl_id.0 == "decay"
        )
    });
    assert!(child_added_decl, "generated param child should emit ChildAdded with decl_id=decay");

    let decay_param = find_child_by_decl(&engine, declared_id, "decay").expect("decay child should exist");
    let decay_meta = engine
        .nodes
        .get(decay_param)
        .expect("decay node should exist")
        .node_data()
        .meta
        .clone();
    assert_eq!(decay_meta.label, "Decay");
    assert_eq!(decay_meta.description.as_deref(), Some("Envelope decay time"));

    engine
        .dispatch_inbox(ExecutionPhase::EndOfTickStabilization)
        .expect("dispatch should succeed");

    let MacroTestNode::AutoDeclaredNode(node) = engine
        .nodes
        .get(declared_id)
        .expect("declared node should exist")
    else {
        panic!("expected AutoDeclaredNode variant");
    };

    assert!(node.decay.is_bound(), "generated param handle should be bound after ChildAdded dispatch");
    assert!(!node.value.is_pending_create(), "potential slot should not be pending by default");
}

fn find_child_by_decl(engine: &Engine<MacroTestNode>, parent: NodeId, decl_id: &str) -> Option<NodeId> {
    let mut child = engine.nodes.get(parent)?.node_data().first_child;
    while let Some(id) = child {
        let node = engine.nodes.get(id)?;
        if node.node_data().meta.decl_id.0 == decl_id {
            return Some(id);
        }
        child = node.node_data().next_sibling;
    }
    None
}

#[test]
fn params_macro_materializes_nested_folders_and_binds_handles() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);
    engine.add_node(DslParamsNode::new("dsl").into(), None);

    for _ in 0..6 {
        engine.apply_edits().expect("apply should succeed");
        engine
            .dispatch_inbox(ExecutionPhase::EndOfTickStabilization)
            .expect("dispatch should succeed");
    }

    let owner = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("dsl node should be attached under root");

    let output = find_child_by_decl(&engine, owner, "output").expect("output folder should exist");
    let color = find_child_by_decl(&engine, output, "output/color").expect("output/color folder should exist");
    let feedback = find_child_by_decl(&engine, owner, "feedback").expect("feedback parameter should exist");
    let host = find_child_by_decl(&engine, output, "output/host").expect("output/host parameter should exist");
    let gamma = find_child_by_decl(&engine, color, "output/color/gamma").expect("output/color/gamma parameter should exist");

    let MacroTestNode::DslParamsNode(node) = engine.nodes.get(owner).expect("dsl node should exist") else {
        panic!("expected DslParamsNode variant");
    };

    assert!(node.feedback.is_bound(), "feedback handle should be bound");
    assert!(node.host.is_bound(), "host handle should be bound");
    assert!(node.gamma.is_bound(), "gamma handle should be bound");
    assert_eq!(node.gamma.event_behaviour(), ParameterEventBehaviour::Append);
    assert_eq!(node.feedback.id(), feedback);
    assert_eq!(node.host.id(), host);
    assert_eq!(node.gamma.id(), gamma);

    let feedback_meta = engine
        .nodes
        .get(feedback)
        .expect("feedback node should exist")
        .node_data()
        .meta
        .clone();
    assert_eq!(feedback_meta.label, "Feedback");
    assert_eq!(feedback_meta.description.as_deref(), Some("Delay feedback amount"));

    let host_meta = engine
        .nodes
        .get(host)
        .expect("host node should exist")
        .node_data()
        .meta
        .clone();
    assert_eq!(host_meta.label, "Host");
    assert_eq!(host_meta.description.as_deref(), Some("OSC destination host"));
}

#[test]
fn apply_edits_adds_children_in_call_order() {
    let mut engine = Engine::new(Folder::new("root".to_string()));
    engine.add_node(Folder::new("child_a".to_string()), None);
    engine.add_node(Folder::new("child_b".to_string()), None);

    engine.apply_edits().expect("apply_edits should succeed");

    let root_data = engine.nodes.get(engine.root).expect("root should exist").node_data();
    let first = root_data.first_child.expect("first child should exist");
    let second = engine.nodes.get(first).and_then(|node| node.node_data().next_sibling).expect("second child should exist");

    assert_eq!(engine.nodes.get(first).expect("first node should exist").node_data().meta.label, "child_a");
    assert_eq!(engine.nodes.get(second).expect("second node should exist").node_data().meta.label, "child_b");
    assert_eq!(engine.nodes.get(second).and_then(|node| node.node_data().next_sibling), None);
}

#[test]
fn apply_edits_move_reorders_children() {
    let mut engine = Engine::new(Folder::new("root".to_string()));
    engine.add_node(Folder::new("child_a".to_string()), None);
    engine.add_node(Folder::new("child_b".to_string()), None);
    engine.apply_edits().expect("initial apply_edits should succeed");

    let child_a = engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child).expect("child_a should exist");
    let child_b = engine.nodes.get(child_a).and_then(|child| child.node_data().next_sibling).expect("child_b should exist");

    engine.edits.push(Edit::MoveNode {
        node: child_a,
        new_parent: engine.root,
        new_prev_sibling: Some(child_b),
    });
    engine.apply_edits().expect("move should succeed");

    let root_data = engine.nodes.get(engine.root).expect("root should exist").node_data();
    assert_eq!(root_data.first_child, Some(child_b));
    assert_eq!(engine.nodes.get(child_b).and_then(|node| node.node_data().next_sibling), Some(child_a));
    assert_eq!(engine.nodes.get(child_a).and_then(|node| node.node_data().next_sibling), None);
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
    let mut engine = Engine::new(Folder::new("root".to_string()));
    engine.add_node(Folder::new("parent".to_string()), None);
    engine.apply_edits().expect("initial apply should succeed");

    let parent = engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child).expect("parent should exist");

    engine.add_node(Folder::new("child".to_string()), Some(parent));
    engine.apply_edits().expect("second apply should succeed");

    let child = engine.nodes.get(parent).and_then(|node| node.node_data().first_child).expect("child should exist");

    engine.edits.push(Edit::MoveNode {
        node: parent,
        new_parent: child,
        new_prev_sibling: None,
    });

    let result = engine.apply_edits();
    assert!(matches!(result, Err(EngineEditError::CycleDetected { operation: "MoveNode", .. })));
}

#[test]
fn apply_edits_set_param_rejects_non_parameter_node() {
    let mut engine = Engine::new(Folder::new("root".to_string()));
    engine.edits.push(Edit::SetParam {
        node: engine.root,
        value: ParamValue::Int(12),
        behaviour: ParameterEventBehaviour::Coalesce,
    });

    let result = engine.apply_edits();
    assert!(matches!(result, Err(EngineEditError::ParamEditTargetMismatch { .. })));
}

#[test]
fn apply_edits_set_param_updates_parameter_node() {
    let root = Parameter::new("root_param", ParamValue::Int(10), ParameterChangeCheck::None);
    let mut engine = Engine::new(root);

    engine.edits.push(Edit::SetParam {
        node: engine.root,
        value: ParamValue::Int(42),
        behaviour: ParameterEventBehaviour::Coalesce,
    });

    engine.apply_edits().expect("set param should succeed");

    let node = engine.nodes.get(engine.root).expect("root parameter should exist");
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
fn parameter_set_coalesces_pending_set_param_edits_by_default() {
    let mut parameter = Parameter::new("param", ParamValue::Int(0), ParameterChangeCheck::None);
    let mut ctx = ProcessCtx::new(
        ExecutionPhase::EngineTick,
        EngineTime {
            tick: 0,
            micro: 0,
            seq: 0,
        },
    );

    parameter.set(&mut ctx, ParamValue::Int(1));
    parameter.set(&mut ctx, ParamValue::Int(2));

    assert_eq!(ctx.edits.pending.len(), 1, "coalesce mode should keep only one queued SetParam");
    assert!(
        matches!(
            ctx.edits.pending.first().map(|request| &request.edit),
            Some(Edit::SetParam {
                node,
                value: ParamValue::Int(2),
                behaviour: ParameterEventBehaviour::Coalesce,
            }) if *node == parameter.id()
        ),
        "queued SetParam should keep the latest value",
    );
}

#[test]
fn parameter_set_append_behaviour_keeps_all_pending_set_param_edits() {
    let mut parameter = Parameter::new("param", ParamValue::Int(0), ParameterChangeCheck::None);
    parameter.event_behaviour = ParameterEventBehaviour::Append;

    let mut ctx = ProcessCtx::new(
        ExecutionPhase::EngineTick,
        EngineTime {
            tick: 0,
            micro: 0,
            seq: 0,
        },
    );

    parameter.set(&mut ctx, ParamValue::Int(1));
    parameter.set(&mut ctx, ParamValue::Int(2));

    assert_eq!(ctx.edits.pending.len(), 2, "append mode should keep every queued SetParam");
    assert!(
        matches!(
            ctx.edits.pending.first().map(|request| &request.edit),
            Some(Edit::SetParam {
                behaviour: ParameterEventBehaviour::Append,
                ..
            })
        ),
        "queued edits should retain append behaviour metadata",
    );
}

#[test]
fn emit_custom_event_uses_edit_pipeline() {
    let mut engine = Engine::new(Folder::new("root".to_string()));
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);

    ctx.emit_custom_event(CustomEvent::new("transport.play", Some(engine.root), serde_json::Value::Null));

    assert!(ctx.events.is_empty(), "custom event should not be injected directly into ctx events");
    assert_eq!(ctx.edits.pending.len(), 1, "custom event should enqueue one edit request");

    engine.absorb_edits(&mut ctx).expect("absorb_edits should accept custom event edits");
    engine.apply_edits().expect("apply_edits should convert custom event edit into engine event");

    assert!(
        matches!(
            engine.inbox.events.last().map(|event| &event.kind),
            Some(EventKind::Custom(event))
                if event.topic == "transport.play" && event.origin == Some(engine.root)
        ),
        "last event should be the emitted custom event",
    );
}

#[test]
fn undo_redo_set_param_restores_value() {
    let root = Parameter::new("root_param", ParamValue::Int(10), ParameterChangeCheck::None);
    let mut engine = Engine::new(root);

    engine.edits.push(Edit::SetParam {
        node: engine.root,
        value: ParamValue::Int(42),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().expect("set param should succeed");

    assert_eq!(engine.nodes.get(engine.root).expect("root parameter should exist").value, ParamValue::Int(42));
    assert_eq!(engine.undo_len(), 1);

    assert!(engine.undo().expect("undo should succeed"));
    assert_eq!(engine.nodes.get(engine.root).expect("root parameter should exist").value, ParamValue::Int(10));
    assert_eq!(engine.redo_len(), 1);

    assert!(engine.redo().expect("redo should succeed"));
    assert_eq!(engine.nodes.get(engine.root).expect("root parameter should exist").value, ParamValue::Int(42));
}

#[test]
fn same_tick_coalesced_set_param_keeps_first_old_value_for_undo() {
    let root = Parameter::new("root_param", ParamValue::Float(0.3), ParameterChangeCheck::None);
    let mut engine = Engine::new(root);

    engine.edits.push(Edit::SetParam {
        node: engine.root,
        value: ParamValue::Float(0.5),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().expect("first set should succeed");

    engine.edits.push(Edit::SetParam {
        node: engine.root,
        value: ParamValue::Float(0.7),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().expect("second set should succeed");

    assert_eq!(
        engine.nodes.get(engine.root).expect("root parameter should exist").value,
        ParamValue::Float(0.7)
    );
    assert_eq!(engine.undo_len(), 1, "same-tick coalesced updates should keep one undo step");

    assert!(engine.undo().expect("undo should succeed"));
    assert_eq!(
        engine.nodes.get(engine.root).expect("root parameter should exist").value,
        ParamValue::Float(0.3),
        "undo should restore the original value before the first coalesced update",
    );
}

#[test]
fn same_tick_append_set_param_keeps_distinct_undo_steps() {
    let root = Parameter::new("root_param", ParamValue::Float(0.3), ParameterChangeCheck::None);
    let mut engine = Engine::new(root);

    engine.edits.push(Edit::SetParam {
        node: engine.root,
        value: ParamValue::Float(0.5),
        behaviour: ParameterEventBehaviour::Append,
    });
    engine.apply_edits().expect("first append set should succeed");

    engine.edits.push(Edit::SetParam {
        node: engine.root,
        value: ParamValue::Float(0.7),
        behaviour: ParameterEventBehaviour::Append,
    });
    engine.apply_edits().expect("second append set should succeed");

    assert_eq!(engine.undo_len(), 2, "append mode should keep both updates in undo history");

    assert!(engine.undo().expect("undo should succeed"));
    assert_eq!(
        engine.nodes.get(engine.root).expect("root parameter should exist").value,
        ParamValue::Float(0.5),
        "append undo should step back to the immediately previous value",
    );
}

#[test]
fn undo_redo_add_node_restores_same_node_id() {
    let mut engine = Engine::new(Folder::new("root".to_string()));
    engine.add_node(Folder::new("child".to_string()), None);
    engine.apply_edits().expect("add should succeed");

    let child = engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child).expect("child should exist");

    assert!(engine.undo().expect("undo should succeed"));
    assert!(engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child).is_none(), "child should be detached after undo",);
    assert!(engine.nodes.get(child).is_none(), "detached child should not be accessible while undone",);

    assert!(engine.redo().expect("redo should succeed"));
    assert_eq!(engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child), Some(child));
}

#[test]
fn undo_redo_remove_node_restores_same_node_id() {
    let mut engine = Engine::new(Folder::new("root".to_string()));
    engine.add_node(Folder::new("child".to_string()), None);
    engine.apply_edits().expect("initial add should succeed");

    let child = engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child).expect("child should exist");

    engine.edits.push(Edit::RemoveNode { node: child });
    engine.apply_edits().expect("remove should succeed");
    assert!(engine.nodes.get(child).is_none(), "removed child should be detached");

    assert!(engine.undo().expect("undo should succeed"));
    assert_eq!(engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child), Some(child), "undo should restore removed child id",);

    assert!(engine.redo().expect("redo should succeed"));
    assert!(engine.nodes.get(child).is_none(), "redo should detach the child again",);
}

#[test]
fn undo_redo_move_restores_child_order() {
    let mut engine = Engine::new(Folder::new("root".to_string()));
    engine.add_node(Folder::new("child_a".to_string()), None);
    engine.add_node(Folder::new("child_b".to_string()), None);
    engine.apply_edits().expect("initial add should succeed");

    let child_a = engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child).expect("child_a should exist");
    let child_b = engine.nodes.get(child_a).and_then(|node| node.node_data().next_sibling).expect("child_b should exist");

    engine.edits.push(Edit::MoveNode {
        node: child_a,
        new_parent: engine.root,
        new_prev_sibling: Some(child_b),
    });
    engine.apply_edits().expect("move should succeed");

    assert_eq!(engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child), Some(child_b), "move should reorder children",);

    assert!(engine.undo().expect("undo should succeed"));
    assert_eq!(engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child), Some(child_a), "undo should restore original order",);

    assert!(engine.redo().expect("redo should succeed"));
    assert_eq!(engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child), Some(child_b), "redo should reapply reordered state",);
}

#[test]
fn undo_redo_replace_restores_original_node_id() {
    let mut engine = Engine::new(Folder::new("root".to_string()));
    engine.add_node(Folder::new("original".to_string()), None);
    engine.apply_edits().expect("initial add should succeed");

    let original_id = engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child).expect("original child should exist");

    engine.replace_node(original_id, Folder::new("replacement".to_string()));
    engine.apply_edits().expect("replace should succeed");

    let replacement_id = engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child).expect("replacement child should exist");
    assert_eq!(replacement_id, original_id, "replace should preserve the same node id",);
    assert_eq!(engine.nodes.get(replacement_id).expect("replacement node should exist").node_data().meta.label, "replacement");

    assert!(engine.undo().expect("undo should succeed"));
    let restored = engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child).expect("restored child should exist");
    assert_eq!(restored, original_id);
    assert_eq!(engine.nodes.get(restored).expect("restored node should exist").node_data().meta.label, "original");

    assert!(engine.redo().expect("redo should succeed"));
    let replaced_again = engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child).expect("replaced node should exist");
    assert_eq!(replaced_again, replacement_id);
    assert_eq!(engine.nodes.get(replaced_again).expect("replacement node should exist").node_data().meta.label, "replacement");
}

#[test]
fn applying_new_edits_after_undo_clears_redo_stack() {
    let mut engine = Engine::new(Folder::new("root".to_string()));
    engine.add_node(Folder::new("first".to_string()), None);
    engine.apply_edits().expect("initial add should succeed");

    assert!(engine.undo().expect("undo should succeed"));
    assert_eq!(engine.redo_len(), 1);

    engine.add_node(Folder::new("second".to_string()), None);
    engine.apply_edits().expect("second add should succeed");

    assert_eq!(engine.redo_len(), 0, "new edits should invalidate redo history");
    assert!(!engine.redo().expect("redo query should succeed"));
}

#[derive(Clone, Debug, PartialEq)]
struct RoutingNode {
    node_data: NodeData,
    interest_depth: u32,
    bubble_depth: u32,
    propagation: EventPropagation,
    observed_node_created: usize,
    observed_child_added: usize,
    observed_custom_events: usize,
    last_inbox_size: usize,
}

impl RoutingNode {
    fn with_policy(label: &str, interest_depth: u32, bubble_depth: u32, propagation: EventPropagation) -> Self {
        Self {
            node_data: NodeData::new(label.to_string()),
            interest_depth,
            bubble_depth,
            propagation,
            observed_node_created: 0,
            observed_child_added: 0,
            observed_custom_events: 0,
            last_inbox_size: 0,
        }
    }
}

impl Node for RoutingNode {
    fn node_data(&self) -> &NodeData {
        &self.node_data
    }

    fn node_data_mut(&mut self) -> &mut NodeData {
        &mut self.node_data
    }

    fn get_type(&self) -> &str {
        "routing_node"
    }

    fn child_event_interest_depth(&self, _event: &crate::events::Event) -> u32 {
        self.interest_depth
    }

    fn bubble_event_depth(&self, _event: &crate::events::Event) -> u32 {
        self.bubble_depth
    }

    fn event_propagation(&self, _event: &crate::events::Event, _depth: u32) -> EventPropagation {
        self.propagation
    }

    fn on_inbox(&mut self, ctx: &mut ProcessCtx) {
        self.last_inbox_size = ctx.events.len();
        <Self as Node>::dispatch_inbox(self, ctx);
    }

    fn on_node_created(&mut self, _ctx: &mut ProcessCtx, _node: NodeId) {
        self.observed_node_created += 1;
    }

    fn on_child_added(&mut self, _ctx: &mut ProcessCtx, _parent: NodeId, _child: NodeId) {
        self.observed_child_added += 1;
    }

    fn on_custom_event(&mut self, _ctx: &mut ProcessCtx, _event: CustomEvent) {
        self.observed_custom_events += 1;
    }
}

#[test]
fn precompute_inbox_dispatch_builds_per_node_event_batches() {
    let root = RoutingNode::with_policy("root", 1, 0, EventPropagation::Notify);
    let mut engine = Engine::new(root);

    engine.add_node(RoutingNode::with_policy("child", 0, 0, EventPropagation::Notify), None);
    engine.apply_edits().expect("add should succeed");

    let per_node_events = engine.precompute_inbox_dispatch();
    let root_events = per_node_events.iter().find(|(node, _)| *node == engine.root).map(|(_, events)| events.len()).expect("root should receive precomputed inbox events");
    assert_eq!(root_events, 2, "root should get node-created and child-added");

    engine.dispatch_precomputed_inbox(ExecutionPhase::EngineTick, per_node_events).expect("dispatching precomputed events should succeed");

    let root = engine.nodes.get(engine.root).expect("root should still exist after dispatch");
    assert_eq!(root.last_inbox_size, 2, "ctx.events should be prefilled per node");
    assert_eq!(root.observed_node_created, 1);
    assert_eq!(root.observed_child_added, 1);
    assert_eq!(engine.inbox.events.len(), 2, "dispatch_precomputed_inbox should not clear engine inbox",);
}

#[test]
fn bubbling_interest_and_bubble_are_additive() {
    let root = RoutingNode::with_policy("root", 2, 0, EventPropagation::Notify);
    let mut engine = Engine::new(root);

    engine.add_node(RoutingNode::with_policy("parent", 1, 0, EventPropagation::Notify), None);
    engine.apply_edits().expect("parent add should succeed");
    let parent = engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child).expect("parent should exist");
    engine.inbox.clear();

    engine.add_node(RoutingNode::with_policy("leaf", 0, 0, EventPropagation::Notify), Some(parent));
    engine.apply_edits().expect("leaf add should succeed");
    engine.dispatch_inbox(ExecutionPhase::EngineTick).expect("inbox dispatch should succeed");

    let parent_node = engine.nodes.get(parent).expect("parent should exist");
    let root_node = engine.nodes.get(engine.root).expect("root should exist");
    assert_eq!(parent_node.observed_child_added, 1, "parent should receive leaf add");
    assert_eq!(root_node.observed_child_added, 1, "root should receive leaf add via additive interest+bubbling",);
}

#[test]
fn bubbling_pass_on_skips_notification_but_keeps_propagating() {
    let root = RoutingNode::with_policy("root", 2, 0, EventPropagation::Notify);
    let mut engine = Engine::new(root);

    engine.add_node(RoutingNode::with_policy("parent", 1, 1, EventPropagation::PassOn), None);
    engine.apply_edits().expect("parent add should succeed");
    let parent = engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child).expect("parent should exist");
    engine.inbox.clear();

    engine.add_node(RoutingNode::with_policy("leaf", 0, 0, EventPropagation::Notify), Some(parent));
    engine.apply_edits().expect("leaf add should succeed");
    engine.dispatch_inbox(ExecutionPhase::EngineTick).expect("inbox dispatch should succeed");

    let parent_node = engine.nodes.get(parent).expect("parent should exist");
    let root_node = engine.nodes.get(engine.root).expect("root should exist");
    assert_eq!(parent_node.observed_child_added, 0, "pass-on should suppress parent notification",);
    assert_eq!(root_node.observed_child_added, 1, "pass-on should still let bubbling reach ancestors",);
}

#[test]
fn bubbling_stop_prevents_further_propagation() {
    let root = RoutingNode::with_policy("root", 2, 0, EventPropagation::Notify);
    let mut engine = Engine::new(root);

    engine.add_node(RoutingNode::with_policy("parent", 1, 1, EventPropagation::Stop), None);
    engine.apply_edits().expect("parent add should succeed");
    let parent = engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child).expect("parent should exist");
    engine.inbox.clear();

    engine.add_node(RoutingNode::with_policy("leaf", 0, 0, EventPropagation::Notify), Some(parent));
    engine.apply_edits().expect("leaf add should succeed");
    engine.dispatch_inbox(ExecutionPhase::EngineTick).expect("inbox dispatch should succeed");

    let parent_node = engine.nodes.get(parent).expect("parent should exist");
    let root_node = engine.nodes.get(engine.root).expect("root should exist");
    assert_eq!(parent_node.observed_child_added, 1, "stop should still notify the node that stops propagation",);
    assert_eq!(root_node.observed_child_added, 0, "stop should prevent bubbling to ancestors",);
}

#[test]
fn subscription_to_specific_subtree_respects_depth() {
    let root = RoutingNode::with_policy("root", 0, 0, EventPropagation::Notify);
    let mut engine = Engine::new(root);

    engine.add_node(RoutingNode::with_policy("parent", 0, 0, EventPropagation::Notify), None);
    engine.apply_edits().expect("parent add should succeed");
    let parent = engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child).expect("parent should exist");

    engine.add_node(RoutingNode::with_policy("watch_depth0", 0, 0, EventPropagation::Notify), None);
    engine.add_node(RoutingNode::with_policy("watch_depth1", 0, 0, EventPropagation::Notify), None);
    engine.apply_edits().expect("watcher add should succeed");

    let watch_depth0 = engine.nodes.get(parent).and_then(|node| node.node_data().next_sibling).expect("watch_depth0 should exist");
    let watch_depth1 = engine
        .nodes
        .get(watch_depth0)
        .and_then(|node| node.node_data().next_sibling)
        .expect("watch_depth1 should exist");

    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    ctx.add_event_listener_subtree(watch_depth0, parent, 0);
    ctx.add_event_listener_subtree(watch_depth1, parent, 1);
    engine.absorb_edits(&mut ctx).expect("listener add edits should be accepted");
    engine.apply_edits().expect("listener add should succeed");

    engine.inbox.clear();

    engine.add_node(RoutingNode::with_policy("leaf", 0, 0, EventPropagation::Notify), Some(parent));
    engine.apply_edits().expect("leaf add should succeed");
    engine.dispatch_inbox(ExecutionPhase::EngineTick).expect("inbox dispatch should succeed");

    let depth0 = engine.nodes.get(watch_depth0).expect("watch_depth0 should exist");
    let depth1 = engine.nodes.get(watch_depth1).expect("watch_depth1 should exist");
    assert_eq!(depth0.observed_child_added, 0, "depth-0 subscription should not match child events");
    assert_eq!(depth1.observed_child_added, 1, "depth-1 subscription should match direct child events");
}

#[test]
fn subscription_to_specific_node_receives_events() {
    let root = RoutingNode::with_policy("root", 0, 0, EventPropagation::Notify);
    let mut engine = Engine::new(root);

    engine.add_node(RoutingNode::with_policy("parent", 0, 0, EventPropagation::Notify), None);
    engine.add_node(RoutingNode::with_policy("watcher", 0, 0, EventPropagation::Notify), None);
    engine.apply_edits().expect("initial setup should succeed");
    let parent = engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child).expect("parent should exist");
    let watcher = engine.nodes.get(parent).and_then(|node| node.node_data().next_sibling).expect("watcher should exist");

    engine.add_node(RoutingNode::with_policy("leaf", 0, 0, EventPropagation::Notify), Some(parent));
    engine.apply_edits().expect("leaf add should succeed");
    let leaf = engine.nodes.get(parent).and_then(|node| node.node_data().first_child).expect("leaf should exist");

    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    ctx.add_event_listener(watcher, leaf);
    engine.absorb_edits(&mut ctx).expect("listener add edits should be accepted");
    engine.apply_edits().expect("listener add should succeed");
    engine.inbox.clear();

    engine.edits.push(Edit::EmitCustomEvent {
        event: CustomEvent::new("leaf.changed", Some(leaf), serde_json::Value::Null),
    });
    engine.apply_edits().expect("custom event emit should succeed");
    engine.dispatch_inbox(ExecutionPhase::EngineTick).expect("inbox dispatch should succeed");

    let watcher_node = engine.nodes.get(watcher).expect("watcher should exist");
    assert_eq!(watcher_node.observed_custom_events, 1, "watcher subscribed to leaf should receive leaf-originated events");
}

#[test]
fn runtime_listener_can_be_added_and_removed_via_ctx() {
    let root = RoutingNode::with_policy("root", 0, 0, EventPropagation::Notify);
    let mut engine = Engine::new(root);

    engine.add_node(RoutingNode::with_policy("source", 0, 0, EventPropagation::Notify), None);
    engine.add_node(RoutingNode::with_policy("watcher", 0, 0, EventPropagation::Notify), None);
    engine.apply_edits().expect("initial setup should succeed");

    let source = engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child).expect("source should exist");
    let watcher = engine.nodes.get(source).and_then(|node| node.node_data().next_sibling).expect("watcher should exist");

    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    ctx.add_event_listener(watcher, source);
    engine.absorb_edits(&mut ctx).expect("listener add edits should be accepted");
    engine.apply_edits().expect("listener add should succeed");

    assert!(
        engine
            .event_listeners
            .get(&watcher)
            .is_some_and(|subscriptions| subscriptions.contains(&EventSubscription::node(source))),
        "watcher should have runtime listener to source",
    );

    engine.edits.push(Edit::EmitCustomEvent {
        event: CustomEvent::new("source.changed", Some(source), serde_json::Value::Null),
    });
    engine.apply_edits().expect("custom event emit should succeed");
    engine.dispatch_inbox(ExecutionPhase::EngineTick).expect("inbox dispatch should succeed");
    assert_eq!(engine.nodes.get(watcher).expect("watcher should exist").observed_custom_events, 1);

    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    ctx.remove_event_listener(watcher, source);
    engine.absorb_edits(&mut ctx).expect("listener remove edits should be accepted");
    engine.apply_edits().expect("listener remove should succeed");

    engine.edits.push(Edit::EmitCustomEvent {
        event: CustomEvent::new("source.changed", Some(source), serde_json::Value::Null),
    });
    engine.apply_edits().expect("custom event emit should succeed");
    engine.dispatch_inbox(ExecutionPhase::EngineTick).expect("inbox dispatch should succeed");
    assert_eq!(
        engine.nodes.get(watcher).expect("watcher should exist").observed_custom_events,
        1,
        "watcher should not receive events after removing listener",
    );
}

#[test]
fn runtime_listener_is_removed_automatically_when_target_is_deleted() {
    let root = RoutingNode::with_policy("root", 0, 0, EventPropagation::Notify);
    let mut engine = Engine::new(root);

    engine.add_node(RoutingNode::with_policy("source", 0, 0, EventPropagation::Notify), None);
    engine.add_node(RoutingNode::with_policy("watcher", 0, 0, EventPropagation::Notify), None);
    engine.apply_edits().expect("initial setup should succeed");

    let source = engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child).expect("source should exist");
    let watcher = engine.nodes.get(source).and_then(|node| node.node_data().next_sibling).expect("watcher should exist");

    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    ctx.add_event_listener(watcher, source);
    engine.absorb_edits(&mut ctx).expect("listener add edits should be accepted");
    engine.apply_edits().expect("listener add should succeed");

    engine.edits.push(Edit::RemoveNode { node: source });
    engine.apply_edits().expect("source removal should succeed");

    assert!(
        !engine
            .event_listeners
            .values()
            .any(|subscriptions| subscriptions.iter().any(|subscription| subscription.node == source)),
        "listeners targeting deleted node should be purged automatically",
    );
}

#[derive(Clone, Debug, PartialEq)]
struct RuntimeNode {
    node_data: NodeData,
    rule: NodeExecutionRule,
    updates: usize,
    delta_times: Vec<Duration>,
    bounce_custom_events: bool,
}

impl RuntimeNode {
    fn new(label: &str, rule: NodeExecutionRule) -> Self {
        Self {
            node_data: NodeData::new(label.to_string()),
            rule,
            updates: 0,
            delta_times: Vec::new(),
            bounce_custom_events: false,
        }
    }

    fn bouncing(label: &str) -> Self {
        Self {
            node_data: NodeData::new(label.to_string()),
            rule: NodeExecutionRule::passive(),
            updates: 0,
            delta_times: Vec::new(),
            bounce_custom_events: true,
        }
    }
}

impl Node for RuntimeNode {
    fn node_data(&self) -> &NodeData {
        &self.node_data
    }

    fn node_data_mut(&mut self) -> &mut NodeData {
        &mut self.node_data
    }

    fn get_type(&self) -> &str {
        "runtime_node"
    }

    fn execution_rule(&self) -> NodeExecutionRule {
        self.rule.clone()
    }

    fn update(&mut self, ctx: &mut ProcessCtx) {
        self.updates += 1;
        self.delta_times.push(ctx.delta_time);
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, _event: CustomEvent) {
        if self.bounce_custom_events {
            ctx.emit_custom_event(CustomEvent::new("runtime.loop", Some(self.id()), serde_json::Value::Null));
        }
    }
}

#[test]
fn resolve_builds_topological_rate_buckets() {
    let root = RuntimeNode::new("root", NodeExecutionRule::passive());
    let mut engine = Engine::new(root);

    engine.add_node(RuntimeNode::new("slow", NodeExecutionRule::passive()), None);
    engine.add_node(RuntimeNode::new("fast_a", NodeExecutionRule::passive()), None);
    engine.add_node(RuntimeNode::new("fast_b", NodeExecutionRule::passive()), None);
    engine.apply_edits().expect("setup edits should succeed");

    let slow = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("slow node should exist");
    let fast_a = engine
        .nodes
        .get(slow)
        .and_then(|node| node.node_data().next_sibling)
        .expect("fast_a should exist");
    let fast_b = engine
        .nodes
        .get(fast_a)
        .and_then(|node| node.node_data().next_sibling)
        .expect("fast_b should exist");

    engine.nodes.get_mut(slow).expect("slow node should exist").rule = NodeExecutionRule::periodic(3);
    engine.nodes.get_mut(fast_a).expect("fast_a should exist").rule = NodeExecutionRule::periodic(200).with_dependencies([slow]);
    engine.nodes.get_mut(fast_b).expect("fast_b should exist").rule = NodeExecutionRule::periodic(200).with_dependencies([fast_a]);

    engine.resolve().expect("resolve should succeed");

    let topo = engine.schedule_topology();
    let slow_pos = topo.iter().position(|node| *node == slow).expect("slow should be in topo order");
    let fast_a_pos = topo.iter().position(|node| *node == fast_a).expect("fast_a should be in topo order");
    let fast_b_pos = topo.iter().position(|node| *node == fast_b).expect("fast_b should be in topo order");
    assert!(slow_pos < fast_a_pos && fast_a_pos < fast_b_pos, "topology should honor dependency chain");

    assert_eq!(engine.schedule_bucket_nodes(3), Some([slow].as_slice()));
    assert_eq!(engine.schedule_bucket_nodes(200), Some([fast_a, fast_b].as_slice()));
}

#[test]
fn resolve_detects_dependency_cycles() {
    let root = RuntimeNode::new("root", NodeExecutionRule::passive());
    let mut engine = Engine::new(root);

    engine.add_node(RuntimeNode::new("a", NodeExecutionRule::passive()), None);
    engine.add_node(RuntimeNode::new("b", NodeExecutionRule::passive()), None);
    engine.apply_edits().expect("setup edits should succeed");

    let node_a = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("node_a should exist");
    let node_b = engine
        .nodes
        .get(node_a)
        .and_then(|node| node.node_data().next_sibling)
        .expect("node_b should exist");

    engine.nodes.get_mut(node_a).expect("node_a should exist").rule = NodeExecutionRule::periodic(10).with_dependencies([node_b]);
    engine.nodes.get_mut(node_b).expect("node_b should exist").rule = NodeExecutionRule::periodic(10).with_dependencies([node_a]);

    let result = engine.resolve();
    assert!(
        matches!(result, Err(EngineRuntimeError::DependencyCycle { .. })),
        "mutual dependencies should fail topological sorting",
    );
}

#[test]
fn reevaluate_graph_edit_marks_and_rebuilds_schedule() {
    let root = RuntimeNode::new("root", NodeExecutionRule::passive());
    let mut engine = Engine::new(root);

    engine.add_node(RuntimeNode::new("runner", NodeExecutionRule::periodic(2)), None);
    engine.apply_edits().expect("setup edits should succeed");
    engine.resolve().expect("initial resolve should succeed");

    let runner = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("runner should exist");
    assert_eq!(engine.schedule_bucket_nodes(2), Some([runner].as_slice()));

    engine.nodes.get_mut(runner).expect("runner should exist").rule = NodeExecutionRule::periodic(120);
    engine.request_graph_reevaluation();
    engine.apply_edits().expect("reevaluate edit should succeed");

    assert!(engine.is_resolve_pending(), "reevaluate edit should mark schedule dirty");
    assert!(engine.resolve_if_needed().expect("resolve_if_needed should succeed"));
    assert_eq!(engine.schedule_bucket_nodes(120), Some([runner].as_slice()));
    assert!(engine.schedule_bucket_nodes(2).is_none(), "old rate bucket should be dropped");
}

#[test]
fn run_tick_respects_update_rate_buckets() {
    let root = RuntimeNode::new("root", NodeExecutionRule::passive());
    let mut engine = Engine::new(root);

    engine.add_node(RuntimeNode::new("runner", NodeExecutionRule::periodic(2)), None);
    engine.apply_edits().expect("setup edits should succeed");
    engine.resolve().expect("resolve should succeed");

    let runner = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("runner should exist");

    engine.run_tick(Duration::from_millis(200)).expect("tick should succeed");
    assert_eq!(engine.nodes.get(runner).expect("runner should exist").updates, 0);

    engine.run_tick(Duration::from_millis(300)).expect("tick should succeed");
    assert_eq!(engine.nodes.get(runner).expect("runner should exist").updates, 1);

    engine.run_tick(Duration::from_millis(1000)).expect("tick should succeed");
    let runner = engine.nodes.get(runner).expect("runner should exist");
    assert_eq!(runner.updates, 3);
    assert_eq!(
        runner.delta_times,
        vec![
            Duration::from_millis(500),
            Duration::from_millis(500),
            Duration::from_millis(500),
        ],
        "runner should receive real elapsed deltas between update callbacks",
    );
}

#[test]
fn delta_time_starts_from_node_creation_time() {
    let root = RuntimeNode::new("root", NodeExecutionRule::passive());
    let mut engine = Engine::new(root);
    engine.resolve().expect("resolve should succeed");

    engine.run_tick(Duration::from_millis(1000)).expect("initial tick should succeed");

    engine.add_node(RuntimeNode::new("runner", NodeExecutionRule::periodic(2)), None);
    engine.apply_edits().expect("node creation should succeed");

    let runner = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("runner should exist");

    engine.run_tick(Duration::from_millis(250)).expect("tick should succeed");
    engine.run_tick(Duration::from_millis(300)).expect("tick should succeed");

    let runner = engine.nodes.get(runner).expect("runner should exist");
    assert_eq!(runner.updates, 1, "runner should have updated once");
    assert_eq!(
        runner.delta_times,
        vec![Duration::from_millis(550)],
        "first delta should measure time since node creation",
    );
}

#[test]
fn run_tick_detects_event_edit_cycles() {
    let root = RuntimeNode::new("root", NodeExecutionRule::passive());
    let mut engine = Engine::new(root);

    engine.add_node(RuntimeNode::bouncing("looper"), None);
    engine.apply_edits().expect("setup edits should succeed");
    engine.resolve().expect("resolve should succeed");

    let looper = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("looper should exist");

    engine.set_runtime_limits(RuntimeLimits {
        max_stabilization_passes_per_tick: 8,
        ..RuntimeLimits::default()
    });

    engine.edits.push(Edit::EmitCustomEvent {
        event: CustomEvent::new("runtime.loop", Some(looper), serde_json::Value::Null),
    });

    let result = engine.run_tick(Duration::from_millis(1));
    assert!(
        matches!(result, Err(EngineRuntimeError::InfiniteEventEditCycle { .. })),
        "run_tick should abort when event/edit stabilization never converges",
    );
}

#[derive(Clone, Debug, PartialEq)]
struct StressNode {
    node_data: NodeData,
    rule: NodeExecutionRule,
    updates: usize,
    value: ParamValue,
    emit_set_param_in_update: bool,
}

impl StressNode {
    fn new(label: &str, rule: NodeExecutionRule, emit_set_param_in_update: bool) -> Self {
        Self {
            node_data: NodeData::new(label.to_string()),
            rule,
            updates: 0,
            value: ParamValue::Int(0),
            emit_set_param_in_update,
        }
    }
}

impl Node for StressNode {
    fn node_data(&self) -> &NodeData {
        &self.node_data
    }

    fn node_data_mut(&mut self) -> &mut NodeData {
        &mut self.node_data
    }

    fn get_type(&self) -> &str {
        "stress_node"
    }

    fn execution_rule(&self) -> NodeExecutionRule {
        self.rule.clone()
    }

    fn engine_set_param_value(&mut self, value: ParamValue) -> Option<ParamValue> {
        let old = std::mem::replace(&mut self.value, value);
        Some(old)
    }

    fn update(&mut self, ctx: &mut ProcessCtx) {
        self.updates += 1;
        if !self.emit_set_param_in_update {
            return;
        }

        let next_value = match self.value {
            ParamValue::Int(current) => current.wrapping_add(1),
            _ => 1,
        };

        ctx.set_param_with_behaviour(self.id(), ParamValue::Int(next_value), ParameterEventBehaviour::Coalesce);
    }
}

fn bench_env_usize(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|raw| raw.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(default)
}

#[test]
#[ignore = "stress benchmark: run manually with --ignored --nocapture"]
fn bench_stress_20k_nodes_fast_updates_and_edits() {
    let node_count = bench_env_usize("GC_BENCH_NODES", 20_000);
    let rate_hz = bench_env_usize("GC_BENCH_RATE_HZ", 240) as u32;
    let warmup_ticks = bench_env_usize("GC_BENCH_WARMUP_TICKS", 1);
    let bench_ticks = bench_env_usize("GC_BENCH_TICKS", 1);
    let elapsed_per_tick_ms = bench_env_usize("GC_BENCH_ELAPSED_MS", 16) as u64;
    let elapsed_per_tick = Duration::from_millis(elapsed_per_tick_ms);

    eprintln!(
        "[bench] starting: nodes={node_count}, rate_hz={rate_hz}, warmup_ticks={warmup_ticks}, bench_ticks={bench_ticks}, elapsed_per_tick={elapsed_per_tick_ms}ms"
    );

    let mut engine = Engine::new(StressNode::new("root", NodeExecutionRule::passive(), false));

    let setup_start = Instant::now();
    eprintln!("[bench] setup: queueing node additions");
    for _ in 0..node_count {
        engine.add_node(
            StressNode::new("stress", NodeExecutionRule::periodic(rate_hz), true),
            None,
        );
    }
    eprintln!("[bench] setup: applying edits + resolving schedule");
    engine.apply_edits().expect("setup edits should succeed");
    engine.resolve().expect("resolve should succeed");
    let setup_elapsed = setup_start.elapsed();
    eprintln!("[bench] setup complete in {:?}", setup_elapsed);

    eprintln!("[bench] warmup: {} tick(s)", warmup_ticks);
    for _ in 0..warmup_ticks {
        engine
            .run_tick(elapsed_per_tick)
            .expect("warmup tick should succeed");
    }
    eprintln!("[bench] warmup complete");

    let updates_before: usize = engine.nodes.values().map(|node| node.updates).sum();
    let benchmark_start = Instant::now();
    eprintln!("[bench] benchmark: {} tick(s)", bench_ticks);
    for tick in 0..bench_ticks {
        engine
            .run_tick(elapsed_per_tick)
            .expect("benchmark tick should succeed");
        eprintln!("[bench] benchmark tick {}/{}", tick + 1, bench_ticks);
    }
    let benchmark_elapsed = benchmark_start.elapsed();
    let updates_after: usize = engine.nodes.values().map(|node| node.updates).sum();

    let benchmark_updates = updates_after.saturating_sub(updates_before);
    let benchmark_edits = benchmark_updates.saturating_sub(bench_ticks);
    let secs = benchmark_elapsed.as_secs_f64().max(f64::EPSILON);
    let updates_per_sec = benchmark_updates as f64 / secs;
    let edits_per_sec = benchmark_edits as f64 / secs;

    println!(
        "stress bench: nodes={node_count}, rate_hz={rate_hz}, warmup_ticks={warmup_ticks}, bench_ticks={bench_ticks}, elapsed_per_tick={elapsed_per_tick_ms}ms"
    );
    println!("setup: {:?}, benchmark: {:?}", setup_elapsed, benchmark_elapsed);
    println!(
        "workload: updates={}, edits~= {} | throughput: updates/s={:.0}, edits/s={:.0}",
        benchmark_updates, benchmark_edits, updates_per_sec, edits_per_sec
    );

    assert!(benchmark_updates > 0, "benchmark should execute update callbacks");
}
