use std::{collections::HashMap, path::Path, sync::Arc};

use golden_core::{
    app::load_sparse_project_file,
    engine::EngineTime,
    events::{Event, EventKind},
    node::{Node, NodeId, NodeUuid},
    parameter::ParamValue,
    process_ctx::{ExecutionPhase, ProcessCtx, ProcessTreeSnapshot},
};

use crate::app::{AppNode, StateMachineState};

use super::super::{
    is_condition_valid_result, runtime_param_change_requires_snapshot, set_condition_valid_param, StateMachineManager,
};
use super::context_scope_test_node;

#[test]
fn authored_formula_value_invalidates_runtime_but_layout_and_status_do_not() {
    let (root, manager_id, library, formula, anode, position, config, value, valid) = (
        NodeId(1), NodeId(2), NodeId(3), NodeId(4), NodeId(5), NodeId(6), NodeId(7), NodeId(8), NodeId(9),
    );
    let formula_uuid = NodeUuid(uuid::Uuid::from_u128(4));
    let mut nodes = HashMap::from([
        (root, context_scope_test_node(root, None, Some(manager_id), None, "root")),
        (manager_id, context_scope_test_node(manager_id, Some(root), None, Some(library), "state_machine_manager")),
        (library, context_scope_test_node(library, Some(root), Some(formula), None, "alchemist_formula_library")),
        (formula, context_scope_test_node(formula, Some(library), Some(anode), None, "alchemist_formula")),
        (anode, context_scope_test_node(anode, Some(formula), Some(position), Some(valid), "alchemist_anode")),
        (position, context_scope_test_node(position, Some(anode), None, Some(config), "vec2")),
        (config, context_scope_test_node(config, Some(anode), Some(value), None, "folder")),
        (value, context_scope_test_node(value, Some(config), None, None, "float")),
        (valid, context_scope_test_node(valid, Some(formula), None, None, "bool")),
    ]);
    nodes.get_mut(&formula).unwrap().uuid = formula_uuid;
    nodes.get_mut(&anode).unwrap().tags.push("alchemist.anode.type:constant".into());
    nodes.get_mut(&position).unwrap().decl_id = "position".into();
    nodes.get_mut(&config).unwrap().decl_id = "config".into();
    nodes.get_mut(&value).unwrap().decl_id = "config/value".into();
    nodes.get_mut(&valid).unwrap().decl_id = "is_valid".into();
    let snapshot = Arc::new(ProcessTreeSnapshot::new(root, nodes));
    let mut manager = StateMachineManager::new();
    manager.node_data_mut().id = manager_id;
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, EngineTime { tick: 1, micro: 0, seq: 0 });
    ctx.set_tree_snapshot(Arc::clone(&snapshot));

    manager.on_param_change(&mut ctx, position, ParamValue::Vec2(0.0, 0.0));
    manager.on_param_change(&mut ctx, valid, ParamValue::Bool(false));
    assert!(manager.runtime_cache.structure_dirty.is_empty());

    manager.on_param_change(&mut ctx, value, ParamValue::Float(0.0));
    assert!(manager.runtime_cache.structure_dirty.contains(&formula_uuid));

    manager.runtime_cache.structure_dirty.clear();
    manager.runtime_cache.runtime_snapshot = Some(Arc::clone(&snapshot));
    manager.runtime_cache.topology_dirty = false;
    manager.runtime_cache.formula_catalog_dirty = false;
    manager.runtime_cache.formula_catalog_initialized = true;
    manager.runtime_cache.context_provider_dirty = false;
    manager.runtime_cache.command_dispatch_snapshot_dirty = false;
    let mut no_snapshot = ProcessCtx::new(ExecutionPhase::EngineTick, EngineTime { tick: 2, micro: 0, seq: 0 });
    no_snapshot.events.push_shared(Arc::new(Event {
        time: no_snapshot.time,
        kind: EventKind::ParamChanged {
            param: value,
            old_value: ParamValue::Float(0.0),
            new_value: ParamValue::Float(2.0),
        },
    }));
    assert!(!manager.inbox_requires_tree_snapshot(&no_snapshot.events));
    manager.runtime_cache.structure_dirty.insert(formula_uuid);
    assert!(manager.inbox_requires_tree_snapshot(&no_snapshot.events));
    manager.runtime_cache.structure_dirty.clear();
    let mut type_change = ProcessCtx::new(ExecutionPhase::EngineTick, no_snapshot.time);
    type_change.events.push_shared(Arc::new(Event {
        time: no_snapshot.time,
        kind: EventKind::ParamChanged {
            param: value,
            old_value: ParamValue::Float(0.0),
            new_value: ParamValue::Vec3(1.0, 2.0, 3.0),
        },
    }));
    assert!(manager.inbox_requires_tree_snapshot(&type_change.events));
    manager.on_param_change(&mut no_snapshot, manager_id, ParamValue::Float(0.0));
    assert!(manager.runtime_cache.structure_dirty.is_empty());
    manager.on_param_change(&mut no_snapshot, value, ParamValue::Float(0.0));
    assert!(manager.runtime_cache.structure_dirty.contains(&formula_uuid));
}

#[test]
fn formula_children_do_not_reconcile_state_network_topology() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("samples")
        .join("test_simple_load.noisette");
    let engine = load_sparse_project_file::<AppNode, _>(fixture).expect("product fixture should load");
    let snapshot = engine.process_tree_snapshot();
    let formula = engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == "alchemist_formula")
        .map(|(id, _)| id)
        .expect("fixture should contain a formula");
    let formula_child = snapshot
        .child_ids(formula)
        .into_iter()
        .next()
        .expect("formula should have a child");
    let state = engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == StateMachineState::NODE_TYPE)
        .map(|(id, _)| id)
        .expect("fixture should contain a state");
    let state_parent = snapshot.node(state).and_then(|node| node.parent).expect("state should have a parent");
    let manager_id = engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == StateMachineManager::NODE_TYPE)
        .map(|(id, _)| id)
        .expect("fixture should contain a manager");
    let mut manager = StateMachineManager::new();
    manager.node_data_mut().id = manager_id;
    let mut ctx = ProcessCtx::new(
        ExecutionPhase::EngineTick,
        EngineTime {
            tick: 1,
            micro: 0,
            seq: 0,
        },
    );
    ctx.set_tree_snapshot(snapshot);

    assert!(!manager.child_change_affects_state_topology(&ctx, formula, formula_child));
    assert!(manager.child_change_affects_state_topology(&ctx, state_parent, state));
}

#[test]
fn generated_condition_validity_does_not_dirty_processor_overrides() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("samples")
        .join("test_simple_load.noisette");
    let engine = load_sparse_project_file::<AppNode, _>(fixture).expect("product fixture should load");
    let snapshot = engine.process_tree_snapshot();
    let condition = engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == "sm_condition_manager")
        .map(|(id, _)| id)
        .expect("product fixture should contain a condition manager");
    let valid = snapshot
        .find_child_by_decl_id(condition, "valid")
        .expect("condition manager should expose validity");
    assert!(is_condition_valid_result(snapshot.as_ref(), valid));
    assert!(!is_condition_valid_result(snapshot.as_ref(), condition));

    let mut manager = StateMachineManager::new();
    manager.runtime_cache.runtime_snapshot = Some(Arc::clone(&snapshot));
    manager.runtime_cache.topology_dirty = false;
    let mut ctx = ProcessCtx::new(
        ExecutionPhase::EngineTick,
        EngineTime {
            tick: 1,
            micro: 0,
            seq: 0,
        },
    );
    ctx.events.push_shared(Arc::new(Event {
        time: ctx.time,
        kind: EventKind::ParamChanged {
            param: valid,
            old_value: ParamValue::Bool(false),
            new_value: ParamValue::Bool(true),
        },
    }));
    assert!(!manager.inbox_requires_tree_snapshot(&ctx.events));
    manager.on_inbox(&mut ctx);
    assert!(manager.runtime_cache.dirty_processor_overrides.is_empty());
    assert_eq!(manager.runtime_cache.condition_valid_param_values.get(&valid), Some(&true));
    assert!(!manager.update_requires_tree_snapshot());

    manager.runtime_cache.context_provider_params.insert(valid);
    assert!(manager.inbox_requires_tree_snapshot(&ctx.events));
    assert!(runtime_param_change_requires_snapshot(false, false, false, false));
    assert!(!runtime_param_change_requires_snapshot(false, false, false, true));
    assert!(manager.inbox_requires_tree_snapshot(&event_for_unknown_param(ctx.time)));
}

#[test]
fn generated_condition_validity_uses_live_value_when_snapshot_is_stale() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("samples")
        .join("test_simple_load.noisette");
    let engine = load_sparse_project_file::<AppNode, _>(fixture).expect("product fixture should load");
    let snapshot = engine.process_tree_snapshot();
    let condition = engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == "sm_condition_manager")
        .map(|(id, _)| id)
        .expect("product fixture should contain a condition manager");
    let valid = snapshot
        .find_child_by_decl_id(condition, "valid")
        .expect("condition manager should expose validity");
    assert_eq!(snapshot.node(valid).and_then(|node| node.param_value.as_ref()).and_then(ParamValue::as_bool), Some(false));
    let time = EngineTime {
        tick: 1,
        micro: 0,
        seq: 0,
    };
    let mut values = HashMap::new();
    let mut true_tick = ProcessCtx::new(ExecutionPhase::EngineTick, time);
    set_condition_valid_param(&mut true_tick, snapshot.as_ref(), condition, true, &mut values);
    assert_eq!(true_tick.edits.pending.len(), 1);
    assert_eq!(values.get(&valid), Some(&true));

    let mut false_tick = ProcessCtx::new(ExecutionPhase::EngineTick, time);
    set_condition_valid_param(&mut false_tick, snapshot.as_ref(), condition, false, &mut values);
    assert_eq!(false_tick.edits.pending.len(), 1, "the stale false snapshot must not hide a true-to-false edit");
    assert_eq!(values.get(&valid), Some(&false));

    let mut settled_tick = ProcessCtx::new(ExecutionPhase::EngineTick, time);
    set_condition_valid_param(&mut settled_tick, snapshot.as_ref(), condition, false, &mut values);
    assert!(settled_tick.edits.pending.is_empty());
}

fn event_for_unknown_param(time: EngineTime) -> golden_core::events::EventFrame {
    let mut events = golden_core::events::EventFrame::default();
    events.push_shared(Arc::new(Event {
        time,
        kind: EventKind::ParamChanged {
            param: NodeId(u64::MAX),
            old_value: ParamValue::Bool(false),
            new_value: ParamValue::Bool(true),
        },
    }));
    events
}
