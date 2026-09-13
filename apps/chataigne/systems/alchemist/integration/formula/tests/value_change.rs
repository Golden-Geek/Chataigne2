use std::sync::Arc;

use golden_core::{
    engine::EngineTime,
    events::{Event, EventFrame, EventKind},
    process_ctx::ProcessCtx,
};

use super::super::constant_numeric_value_change_keeps_signature;
use super::*;

fn value_change_ctx(
    snapshot: Arc<golden_core::process_ctx::ProcessTreeSnapshot>,
    param: NodeId,
    old_value: ParamValue,
    new_value: ParamValue,
) -> ProcessCtx {
    let time = EngineTime { tick: 1, micro: 0, seq: 0 };
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, time);
    ctx.set_tree_snapshot(snapshot);
    ctx.events = EventFrame::from_shared(vec![Arc::new(Event {
        time,
        kind: EventKind::ParamChanged {
            param,
            old_value,
            new_value,
        },
    })]);
    ctx
}

#[test]
fn numeric_constant_value_changes_skip_shape_reconciliation() {
    let (mut engine, formula) = engine_with_formula();
    let anode = create_anode(&mut engine, formula, "constant", 1.0, 2.0);
    let config = find_child_by_decl(&engine, anode, "config").unwrap();
    let value = find_child_by_decl(&engine, config, "config/value").unwrap();
    let snapshot = engine.process_tree_snapshot();

    let mut anode_ctx = value_change_ctx(
        Arc::clone(&snapshot),
        value,
        ParamValue::Float(0.0),
        ParamValue::Float(2.0),
    );
    assert!(constant_numeric_value_change_keeps_signature(&anode_ctx, value));
    assert!(!engine.nodes.get(anode).unwrap().inbox_requires_tree_snapshot(&anode_ctx.events));
    anode_ctx.clear_tree_snapshot();
    engine.nodes.get_mut(anode).unwrap().on_inbox(&mut anode_ctx);
    assert!(anode_ctx.edits.pending.is_empty());

    let mut formula_ctx = value_change_ctx(
        snapshot,
        value,
        ParamValue::Float(0.0),
        ParamValue::Float(2.0),
    );
    assert!(!engine.nodes.get(formula).unwrap().inbox_requires_tree_snapshot(&formula_ctx.events));
    formula_ctx.clear_tree_snapshot();
    engine.nodes.get_mut(formula).unwrap().on_inbox(&mut formula_ctx);
    assert!(formula_ctx.edits.pending.is_empty());

    engine
        .nodes
        .get_mut(formula)
        .unwrap()
        .node_data_mut()
        .meta
        .tags
        .push(super::FORMULA_EXTERNAL_FILE_TAG.into());
    assert!(engine.nodes.get(formula).unwrap().inbox_requires_tree_snapshot(&formula_ctx.events));
}

#[test]
fn constant_value_type_changes_still_require_reconciliation() {
    let (mut engine, formula) = engine_with_formula();
    let anode = create_anode(&mut engine, formula, "constant", 1.0, 2.0);
    let config = find_child_by_decl(&engine, anode, "config").unwrap();
    let value = find_child_by_decl(&engine, config, "config/value").unwrap();
    let value_type = find_child_by_decl(&engine, config, "config/value__type").unwrap();
    let ctx = value_change_ctx(
        engine.process_tree_snapshot(),
        value,
        ParamValue::Float(0.0),
        ParamValue::Vec3(1.0, 2.0, 3.0),
    );
    assert!(!constant_numeric_value_change_keeps_signature(&ctx, value));

    let ctx = value_change_ctx(
        engine.process_tree_snapshot(),
        value_type,
        ParamValue::Enum("float".into()),
        ParamValue::Enum("vec3".into()),
    );
    assert!(!constant_numeric_value_change_keeps_signature(&ctx, value_type));
    assert!(engine.nodes.get(anode).unwrap().inbox_requires_tree_snapshot(&ctx.events));
    assert!(engine.nodes.get(formula).unwrap().inbox_requires_tree_snapshot(&ctx.events));
}
