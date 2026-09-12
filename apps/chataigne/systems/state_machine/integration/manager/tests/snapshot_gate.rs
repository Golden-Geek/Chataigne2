use std::{collections::HashMap, path::Path, sync::Arc};

use golden_core::{
    app::load_sparse_project_file,
    engine::EngineTime,
    events::{Event, EventKind},
    node::{Node, NodeId},
    parameter::ParamValue,
    process_ctx::{ExecutionPhase, ProcessCtx},
};

use crate::app::{AppNode, StateMachineState};

use super::super::{
    is_condition_valid_result, runtime_param_change_requires_snapshot, set_condition_valid_param, StateMachineManager,
};

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
