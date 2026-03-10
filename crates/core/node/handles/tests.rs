use super::*;
use crate::engine::EngineTime;
use crate::process_ctx::ExecutionPhase;

#[test]
fn parameter_handle_set_queues_coalesced_set_param_and_updates_cache() {
    let mut handle = ParameterHandle::new(0.5f64);
    handle.set_node_id(NodeId(10));
    let mut ctx = ProcessCtx::new(
        ExecutionPhase::EngineTick,
        EngineTime {
            tick: 0,
            micro: 0,
            seq: 0,
        },
    );

    handle.set(&mut ctx, 0.75);

    assert_eq!(handle.get(), 0.75);
    assert_eq!(ctx.edits.pending.len(), 1);
    match &ctx.edits.pending[0].edit {
        Edit::SetParam { node, value, behaviour } => {
            assert_eq!(*node, NodeId(10));
            assert_eq!(value, &ParamValue::Float(0.75));
            assert_eq!(*behaviour, ParameterEventBehaviour::Coalesce);
        }
        other => panic!("expected SetParam edit, got {:?}", std::mem::discriminant(other)),
    }
}

#[test]
fn parameter_handle_value_change_policy_skips_unchanged_values() {
    let mut handle = ParameterHandle::with_policies(
        1.0f64,
        ParameterChangeCheck::ValueChange,
        ParameterEventBehaviour::Coalesce,
    );
    handle.set_node_id(NodeId(11));
    let mut ctx = ProcessCtx::new(
        ExecutionPhase::EngineTick,
        EngineTime {
            tick: 0,
            micro: 0,
            seq: 0,
        },
    );

    handle.set(&mut ctx, 1.0);
    assert!(ctx.edits.pending.is_empty());

    handle.set_change_check(ParameterChangeCheck::None);
    handle.set(&mut ctx, 1.0);
    assert_eq!(ctx.edits.pending.len(), 1);
}

#[test]
fn parameter_handle_can_update_cache_from_runtime_value() {
    let mut handle = ParameterHandle::new(0.0f64);
    handle.set_node_id(NodeId(12));
    let ok = handle.apply_runtime_value(&ParamValue::Int(3));
    assert!(ok);
    assert_eq!(handle.get(), 3.0);
}

#[test]
fn node_handle_helpers_queue_structural_edits() {
    let handle = NodeHandle::new(NodeId(20));
    let mut ctx = ProcessCtx::new(
        ExecutionPhase::EngineTick,
        EngineTime {
            tick: 0,
            micro: 0,
            seq: 0,
        },
    );

    handle.move_to(&mut ctx, NodeId(30), Some(NodeId(31)));
    handle.remove(&mut ctx);

    assert_eq!(ctx.edits.pending.len(), 2);
    match &ctx.edits.pending[0].edit {
        Edit::MoveNode {
            node,
            new_parent,
            new_prev_sibling,
        } => {
            assert_eq!(*node, NodeId(20));
            assert_eq!(*new_parent, NodeId(30));
            assert_eq!(*new_prev_sibling, Some(NodeId(31)));
        }
        _ => panic!("expected MoveNode edit"),
    }

    match &ctx.edits.pending[1].edit {
        Edit::RemoveNode { node } => assert_eq!(*node, NodeId(20)),
        _ => panic!("expected RemoveNode edit"),
    }
}

#[test]
fn node_handle_patch_meta_queues_patch_meta_edit() {
    let handle = NodeHandle::new(NodeId(21));
    let mut ctx = ProcessCtx::new(
        ExecutionPhase::EngineTick,
        EngineTime {
            tick: 0,
            micro: 0,
            seq: 0,
        },
    );

    handle.patch_meta(
        &mut ctx,
        NodeMetaPatch {
            label: Some("renamed".to_string()),
            ..Default::default()
        },
    );

    assert_eq!(ctx.edits.pending.len(), 1);
    match &ctx.edits.pending[0].edit {
        Edit::PatchMeta { node, patch } => {
            assert_eq!(*node, NodeId(21));
            assert_eq!(patch.label.as_deref(), Some("renamed"));
        }
        _ => panic!("expected PatchMeta edit"),
    }
}

#[test]
fn parameter_handle_warning_helpers_queue_meta_edits() {
    let mut handle = ParameterHandle::new(0.5f64);
    handle.set_node_id(NodeId(50));
    let mut ctx = ProcessCtx::new(
        ExecutionPhase::EngineTick,
        EngineTime {
            tick: 0,
            micro: 0,
            seq: 0,
        },
    );

    handle.set_warning(&mut ctx, "default warning");
    handle.set_warning_with(
        &mut ctx,
        Some("port"),
        "invalid port",
        Some("port must be in [1..65535]"),
    );
    handle.clear_warning(&mut ctx, Some("port"));
    handle.clear_warnings(&mut ctx);
    handle.set_child_warning_depth(&mut ctx, 2);

    assert_eq!(ctx.edits.pending.len(), 5);
    match &ctx.edits.pending[0].edit {
        Edit::SetNodeWarning { node, warning } => {
            assert_eq!(*node, NodeId(50));
            assert_eq!(warning.id, "");
            assert_eq!(warning.message, "default warning");
            assert_eq!(warning.detail, None);
        }
        _ => panic!("expected SetNodeWarning edit"),
    }
    match &ctx.edits.pending[1].edit {
        Edit::SetNodeWarning { warning, .. } => {
            assert_eq!(warning.id, "port");
            assert_eq!(warning.message, "invalid port");
            assert_eq!(warning.detail.as_deref(), Some("port must be in [1..65535]"));
        }
        _ => panic!("expected SetNodeWarning edit"),
    }
    match &ctx.edits.pending[2].edit {
        Edit::ClearNodeWarning { warning_id, .. } => {
            assert_eq!(warning_id.as_deref(), Some("port"));
        }
        _ => panic!("expected ClearNodeWarning edit"),
    }
    match &ctx.edits.pending[3].edit {
        Edit::ClearNodeWarning { warning_id, .. } => assert_eq!(*warning_id, None),
        _ => panic!("expected ClearNodeWarning edit"),
    }
    match &ctx.edits.pending[4].edit {
        Edit::SetNodeChildWarningDepth { max_depth, .. } => assert_eq!(*max_depth, 2),
        _ => panic!("expected SetNodeChildWarningDepth edit"),
    }
}

#[test]
fn unbound_parameter_handle_warning_helpers_skip_edits() {
    let handle = ParameterHandle::new(0.5f64);
    let mut ctx = ProcessCtx::new(
        ExecutionPhase::EngineTick,
        EngineTime {
            tick: 0,
            micro: 0,
            seq: 0,
        },
    );

    handle.set_warning(&mut ctx, "default warning");
    handle.set_warning_with(&mut ctx, Some("port"), "invalid port", Some("detail"));
    handle.clear_warning(&mut ctx, Some("port"));
    handle.clear_warnings(&mut ctx);
    handle.set_child_warning_depth(&mut ctx, 2);

    assert!(ctx.edits.pending.is_empty());
}

#[test]
fn potential_handle_clear_removes_bound_node() {
    let mut handle = PotentialNodeHandle::with_current(NodeId(1), "value", NodeId(2));
    let mut ctx = ProcessCtx::new(
        ExecutionPhase::EngineTick,
        EngineTime {
            tick: 0,
            micro: 0,
            seq: 0,
        },
    );

    let removed = handle.clear(&mut ctx);
    assert_eq!(removed, Some(NodeId(2)));
    assert!(!handle.is_present());
    assert_eq!(ctx.edits.pending.len(), 1);
    match &ctx.edits.pending[0].edit {
        Edit::RemoveNode { node } => assert_eq!(*node, NodeId(2)),
        _ => panic!("expected RemoveNode edit"),
    }
}

#[test]
fn potential_handle_replace_without_current_adds_node_under_parent() {
    let mut handle = PotentialNodeHandle::new(NodeId(5), "value");
    let mut ctx = ProcessCtx::new(
        ExecutionPhase::EngineTick,
        EngineTime {
            tick: 0,
            micro: 0,
            seq: 0,
        },
    );

    handle.replace_with(&mut ctx, crate::node::Folder::new("slot-value"));
    assert!(!handle.is_present());
    assert!(handle.is_pending_create());
    assert_eq!(ctx.edits.pending.len(), 1);
    match &ctx.edits.pending[0].edit {
        Edit::AddNode { parent, node, .. } => {
            assert_eq!(*parent, NodeId(5));
            assert_eq!(node.node_data().meta.decl_id, DeclId("value".to_string()));
        }
        _ => panic!("expected AddNode edit"),
    }
}

#[test]
fn potential_handle_replace_with_current_queues_replace() {
    let mut handle = PotentialNodeHandle::with_current(NodeId(5), "value", NodeId(6));
    let mut ctx = ProcessCtx::new(
        ExecutionPhase::EngineTick,
        EngineTime {
            tick: 0,
            micro: 0,
            seq: 0,
        },
    );

    handle.replace_with(&mut ctx, crate::node::Folder::new("slot-value"));
    assert!(handle.is_present());
    assert_eq!(handle.current_id(), Some(NodeId(6)));
    assert_eq!(ctx.edits.pending.len(), 1);
    match &ctx.edits.pending[0].edit {
        Edit::ReplaceNode { node, .. } => assert_eq!(*node, NodeId(6)),
        _ => panic!("expected ReplaceNode edit"),
    }
}

#[test]
fn potential_handle_attach_existing_moves_and_binds() {
    let mut handle = PotentialNodeHandle::new(NodeId(8), "target");
    let mut ctx = ProcessCtx::new(
        ExecutionPhase::EngineTick,
        EngineTime {
            tick: 0,
            micro: 0,
            seq: 0,
        },
    );

    handle.attach_existing(&mut ctx, NodeId(42), None);
    assert_eq!(handle.current_id(), Some(NodeId(42)));
    assert_eq!(ctx.edits.pending.len(), 1);
    match &ctx.edits.pending[0].edit {
        Edit::MoveNode {
            node,
            new_parent,
            new_prev_sibling,
        } => {
            assert_eq!(*node, NodeId(42));
            assert_eq!(*new_parent, NodeId(8));
            assert_eq!(*new_prev_sibling, None);
        }
        _ => panic!("expected MoveNode edit"),
    }
}

#[test]
fn potential_handle_reconcile_child_added_binds_pending_slot() {
    let mut handle = PotentialNodeHandle::new(NodeId(9), "value");
    let mut ctx = ProcessCtx::new(
        ExecutionPhase::EngineTick,
        EngineTime {
            tick: 0,
            micro: 0,
            seq: 0,
        },
    );

    handle.replace_with(&mut ctx, crate::node::Folder::new("slot-value"));
    assert!(handle.is_pending_create());
    assert_eq!(handle.current_id(), None);

    let changed = handle.reconcile_event(&EventKind::ChildAdded {
        parent: NodeId(9),
        child: NodeId(99),
        decl_id: DeclId("value".to_string()),
    });
    assert!(changed);
    assert!(!handle.is_pending_create());
    assert_eq!(handle.current_id(), Some(NodeId(99)));
}
