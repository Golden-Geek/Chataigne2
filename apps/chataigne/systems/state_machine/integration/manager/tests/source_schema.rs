use std::collections::HashMap;

use chataigne_alchemist::{CompileCtx, EvaluationCtx, ManagedRegionId, RuntimeInputSnapshot, RuntimeRegistries, StableRef, ValueTypeId};
use chataigne_state_machine::{ProcessorRuntime, INPUT_SOURCE_FIELD};
use golden_core::{
    node::{NodeId, NodeUuid},
    parameter::{ParamValue, ParameterConstraints, RangeConstraint},
    process_ctx::ProcessTreeSnapshot,
};

use super::super::source_schema::{insert_managed_source_values, managed_source_bindings, managed_source_schema};
use super::context_scope_test_node;
use super::snapshot_gate::managed_remap_processor;

#[test]
fn managed_source_schema_uses_target_declaration_and_range() {
    let (root, number, flag, folder) = (NodeId(1), NodeId(2), NodeId(3), NodeId(4));
    let number_uuid = NodeUuid(uuid::Uuid::from_u128(2));
    let flag_uuid = NodeUuid(uuid::Uuid::from_u128(3));
    let folder_uuid = NodeUuid(uuid::Uuid::from_u128(4));
    let mut nodes = HashMap::from([
        (root, context_scope_test_node(root, None, Some(number), None, "root")),
        (number, context_scope_test_node(number, Some(root), None, Some(flag), "float")),
        (flag, context_scope_test_node(flag, Some(root), None, Some(folder), "bool")),
        (folder, context_scope_test_node(folder, Some(root), None, None, "folder")),
    ]);
    nodes.get_mut(&number).unwrap().uuid = number_uuid;
    nodes.get_mut(&number).unwrap().param_value = Some(ParamValue::Float(2.0));
    nodes.get_mut(&number).unwrap().param_constraints = Some(ParameterConstraints {
        range: Some(RangeConstraint::Uniform { min: Some(0.0), max: Some(10.0) }),
        ..ParameterConstraints::default()
    });
    nodes.get_mut(&flag).unwrap().uuid = flag_uuid;
    nodes.get_mut(&flag).unwrap().param_value = Some(ParamValue::Bool(true));
    nodes.get_mut(&folder).unwrap().uuid = folder_uuid;
    let snapshot = ProcessTreeSnapshot::new(root, nodes);
    let reference = |uuid: NodeUuid| StableRef::new(ValueTypeId::new("chataigne.module_endpoint"), uuid.0.to_string());
    let number = managed_source_schema(&snapshot, &reference(number_uuid)).unwrap();
    assert_eq!(number.value_type, ValueTypeId::new("float"));
    assert_eq!(number.metadata.minimum, Some(0.0));
    assert_eq!(number.metadata.maximum, Some(10.0));
    assert_eq!(managed_source_schema(&snapshot, &reference(flag_uuid)).unwrap().value_type, ValueTypeId::new("bool"));
    assert!(managed_source_schema(&snapshot, &reference(folder_uuid)).is_none());
}

#[test]
fn managed_source_snapshot_uses_latest_param_event_value_for_each_reference() {
    let (root, source, missing) = (NodeId(1), NodeId(2), NodeId(3));
    let source_uuid = NodeUuid(uuid::Uuid::from_u128(2));
    let mut nodes = HashMap::from([
        (root, context_scope_test_node(root, None, Some(source), None, "root")),
        (source, context_scope_test_node(source, Some(root), None, None, "float")),
    ]);
    nodes.get_mut(&source).unwrap().uuid = source_uuid;
    nodes.get_mut(&source).unwrap().param_value = Some(ParamValue::Float(1.0));
    let snapshot = ProcessTreeSnapshot::new(root, nodes);
    let references = [
        StableRef::new(ValueTypeId::new("chataigne.module_endpoint"), source_uuid.0.to_string()),
        StableRef::new(ValueTypeId::new("float"), source_uuid.0.to_string()),
        StableRef::new(ValueTypeId::new("chataigne.module_endpoint"), uuid::Uuid::from_u128(3).to_string()),
    ];
    let bindings = [
        (references[0].clone(), source),
        (references[1].clone(), source),
        (references[2].clone(), missing),
    ];
    let live = HashMap::from([(source, ParamValue::Float(2.0))]);
    let mut inputs = RuntimeInputSnapshot::default();
    insert_managed_source_values(&snapshot, &live, &bindings, &mut inputs);
    assert_eq!(inputs.get(&references[0]), Some(&golden_values::Value::Float(2.0)));
    assert_eq!(inputs.get(&references[1]), Some(&golden_values::Value::Float(2.0)));
    assert_eq!(inputs.get(&references[2]), None);
}

#[test]
fn managed_formula_reads_declared_parameter_through_host_snapshot() {
    let (root, source) = (NodeId(1), NodeId(2));
    let source_uuid = NodeUuid(uuid::Uuid::from_u128(2));
    let mut nodes = HashMap::from([
        (root, context_scope_test_node(root, None, Some(source), None, "root")),
        (source, context_scope_test_node(source, Some(root), None, None, "float")),
    ]);
    nodes.get_mut(&source).unwrap().uuid = source_uuid;
    nodes.get_mut(&source).unwrap().param_value = Some(ParamValue::Float(5.0));
    let snapshot = ProcessTreeSnapshot::new(root, nodes);
    let reference = StableRef::new(ValueTypeId::new("chataigne.module_endpoint"), source_uuid.0.to_string());
    let (formula, mut processor, _) = managed_remap_processor(NodeUuid(uuid::Uuid::from_u128(4)));
    processor.formula_instance.managed_regions.regions.get_mut(&ManagedRegionId::new("inputs"))
        .unwrap().items[0].anode.config.set(INPUT_SOURCE_FIELD, golden_values::Value::Ref(reference.clone()));

    let value_types = chataigne_state_machine::alchemist::value_type_registry();
    let nodes = chataigne_alchemist::primitive_node_registry();
    let ctx = CompileCtx { value_types: &value_types, nodes: &nodes, properties: Some(&formula.properties) };
    let mut runtime = ProcessorRuntime::new(processor.id);
    assert!(runtime.compile(&processor, &formula, &ctx));
    runtime.managed_formula.as_mut().unwrap()
        .reconcile_input_source_schema(|source| managed_source_schema(&snapshot, source)).unwrap();
    let bindings = managed_source_bindings(&snapshot, &processor, &runtime);
    assert_eq!(bindings, vec![(reference.clone(), source)]);
    let mut inputs = RuntimeInputSnapshot::default();
    insert_managed_source_values(&snapshot, &HashMap::new(), &bindings, &mut inputs);
    let registries = RuntimeRegistries { value_types: &value_types };
    let eval = EvaluationCtx { logical_tick: 1, delta_time: std::time::Duration::ZERO, events: &[], inputs: &inputs, registries: &registries };
    let output = runtime.managed_formula.as_mut().unwrap().evaluate(&eval);
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    assert_eq!(output.intents.len(), 1);
    assert_eq!(output.intents[0].payload, golden_values::Value::Float(0.5));
}
