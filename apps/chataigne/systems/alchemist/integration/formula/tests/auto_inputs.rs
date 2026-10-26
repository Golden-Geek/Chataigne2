use super::super::{sync_auto_input_count, sync_auto_input_counts};
use super::*;
use golden_core::{
    engine::EngineTime,
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};
use std::collections::HashMap;

fn context() -> ProcessCtx {
    ProcessCtx::new(
        ExecutionPhase::EngineTick,
        EngineTime {
            tick: 1,
            micro: 0,
            seq: 0,
        },
    )
}

fn writes(ctx: &ProcessCtx) -> HashMap<NodeId, ParamValue> {
    ctx.edits
        .pending
        .iter()
        .filter_map(|request| match &request.edit {
            Edit::SetParam { node, value, .. } => Some((*node, value.clone())),
            _ => None,
        })
        .collect()
}

#[test]
fn bulk_auto_inputs_match_local_reconciliation_and_respect_skip_explicit_counts_and_deduplication() {
    let (mut engine, formula) = engine_with_formula();
    let first = create_anode(&mut engine, formula, "constant", 0.0, 0.0);
    let second = create_anode(&mut engine, formula, "constant", 1.0, 0.0);
    let mut targets = Vec::new();
    for type_id in ["math", "concatenate", "math"] {
        let anode = create_anode(&mut engine, formula, type_id, 2.0, 0.0);
        let prefix = if type_id == "math" { "value" } else { "part" };
        create_connection(&mut engine, formula, first, "value", anode, &format!("{prefix}1"));
        create_connection(&mut engine, formula, second, "value", anode, &format!("{prefix}2"));
        let config = find_child_by_decl(&engine, anode, "config").unwrap();
        let parameter = find_child_by_decl(&engine, config, "config/num_inputs").unwrap();
        targets.push((anode, config, parameter, type_id));
    }
    let base = engine.process_tree_snapshot();
    let mut nodes = engine
        .nodes
        .iter()
        .map(|(id, _)| (id, base.node(id).unwrap().clone()))
        .collect::<HashMap<_, _>>();
    for (index, (_, _, parameter, _)) in targets.iter().enumerate() {
        let count = nodes.get_mut(parameter).unwrap();
        count.param_value = Some(ParamValue::Int(2));
        count.enabled = index == 2;
    }
    let snapshot = ProcessTreeSnapshot::new(base.root(), nodes);
    let mut local = context();
    for (anode, config, _, type_id) in &targets {
        sync_auto_input_count(&mut local, &snapshot, *anode, *config, type_id);
    }
    let expected = HashMap::from([(targets[0].2, ParamValue::Int(3)), (targets[1].2, ParamValue::Int(3))]);
    assert_eq!(writes(&local), expected);
    let mut bulk = context();
    sync_auto_input_counts(&mut bulk, &snapshot, formula, None);
    assert_eq!(writes(&bulk), expected);
    sync_auto_input_counts(&mut bulk, &snapshot, formula, None);
    assert_eq!(bulk.edits.pending.len(), expected.len());
    let mut skipped = context();
    sync_auto_input_counts(&mut skipped, &snapshot, formula, Some(targets[0].0));
    assert_eq!(writes(&skipped), HashMap::from([(targets[1].2, ParamValue::Int(3))]));
}

#[test]
fn formula_waits_for_automatic_input_counts_before_compiling_and_validating() {
    use super::super::reconcile::FormulaReconciliation;

    let (mut engine, formula) = engine_with_formula();
    let first = create_anode(&mut engine, formula, "constant", 0.0, 0.0);
    let second = create_anode(&mut engine, formula, "constant", 1.0, 0.0);
    let math = create_anode(&mut engine, formula, "math", 2.0, 0.0);
    create_connection(&mut engine, formula, first, "value", math, "value1");
    create_connection(&mut engine, formula, second, "value", math, "value2");
    let config = find_child_by_decl(&engine, math, "config").unwrap();
    let count = find_child_by_decl(&engine, config, "config/num_inputs").unwrap();
    // Recreate the intermediate signature seen before automatic count settlement.
    engine
        .nodes
        .get_mut(count)
        .unwrap()
        .engine_set_param_value(ParamValue::Int(2));
    let mut pending = context();
    pending.set_tree_snapshot(engine.process_tree_snapshot());
    let node = engine
        .nodes
        .get_mut(formula)
        .unwrap()
        .as_any_mut()
        .downcast_mut::<AlchemistFormulaDefinition>()
        .unwrap();
    let outcome = node.sync_anode_sockets(&mut pending, None);
    assert!(matches!(outcome, Some(FormulaReconciliation::AwaitingInputCounts)));
    node.validate(&mut pending, outcome);
    assert_eq!(
        pending.edits.pending.len(),
        1,
        "the intermediate graph must only queue its count repair"
    );
    assert_eq!(writes(&pending), HashMap::from([(count, ParamValue::Int(3))]));
    engine.edits.pending.extend(pending.edits.drain());
    engine.apply_edits().unwrap();

    let mut settled = context();
    settled.set_tree_snapshot(engine.process_tree_snapshot());
    let node = engine
        .nodes
        .get_mut(formula)
        .unwrap()
        .as_any_mut()
        .downcast_mut::<AlchemistFormulaDefinition>()
        .unwrap();
    let Some(FormulaReconciliation::Compiled(compilation)) = node.sync_anode_sockets(&mut settled, None) else {
        panic!("settled counts must resume compilation");
    };
    assert!(!compilation.has_errors());
    assert_eq!(compilation.resolved.nodes.len(), 3);
}
