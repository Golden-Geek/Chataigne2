use chataigne_alchemist::{
    ANodeId, ANodeInstance, ANodeTypeId, AlchemistFormula, AlchemistFormulaInstance, AlchemistGraphDomain,
    AlchemistGraphTransaction, CompileCtx, InputSocketRef, ManagedFilterValueMode, ManagedRegionId, ManagedSocketRef,
    OutputSocketRef, RuntimeInputSnapshot, RuntimeRegistries, StableRef,
};
use chataigne_state_machine_model::StateId;
use golden_values::Value as RuntimeValue;

use super::managed_formula::{
    command_target, endpoint_ref, eval_ctx, formula_and_instance, input_item, output_item, region, registries,
    remap_item,
};
use crate::{
    ChannelSourceSchema, DefaultProcessorContextProvider, ManagedFormulaRuntime, Processor, ProcessorDebugCapture,
    ProcessorLifecycleEvent, ProcessorRuntime,
    alchemist::{FILTERS_MANAGER_TYPE, INPUTS_MANAGER_TYPE, OUTPUTS_MANAGER_TYPE, ROUTING_TYPE},
};

fn graph_fixture() -> (
    AlchemistFormula,
    AlchemistFormulaInstance,
    StableRef,
    StableRef,
    ANodeId,
    ANodeId,
) {
    let (mut formula, mut instance) = formula_and_instance();
    let source = endpoint_ref("module/value");
    let target = command_target("target/command");
    let input = ANodeInstance::new(ANodeTypeId::new(INPUTS_MANAGER_TYPE), "Inputs");
    let mut before = ANodeInstance::new(ANodeTypeId::new(ROUTING_TYPE), "Before Filters");
    before.config.set("log", RuntimeValue::Bool(true));
    let filter = ANodeInstance::new(ANodeTypeId::new(FILTERS_MANAGER_TYPE), "Filters");
    let mut after = ANodeInstance::new(ANodeTypeId::new(ROUTING_TYPE), "After Filters");
    after.config.set("log", RuntimeValue::Bool(true));
    let output = ANodeInstance::new(ANodeTypeId::new(OUTPUTS_MANAGER_TYPE), "Outputs");
    let (input_id, before_id, filter_id, after_id, output_id) = (input.id, before.id, filter.id, after.id, output.id);
    let (value_types, nodes) = registries();
    let domain = AlchemistGraphDomain::new(nodes.clone(), value_types.clone(), Some(formula.properties.clone()));
    let mut transaction = AlchemistGraphTransaction::for_document(&formula.graph);
    for node in [input, before, filter, after, output] {
        AlchemistGraphDomain::insert_node(&mut transaction, node);
    }
    for (from_node, from_socket, to_node, to_socket) in [
        (input_id, "values", before_id, "in"),
        (before_id, "out", filter_id, "values"),
        (filter_id, "values", after_id, "in"),
        (after_id, "out", output_id, "values"),
    ] {
        AlchemistGraphDomain::connect(
            &mut transaction,
            &formula.graph,
            OutputSocketRef::new(from_node, from_socket),
            InputSocketRef::new(to_node, to_socket),
        );
    }
    transaction.commit(&mut formula.graph, &domain).unwrap();

    for definition in &mut formula.surface.managed_regions {
        match definition.id.as_str() {
            "inputs" => definition.output_socket = Some(ManagedSocketRef::new(input_id, "values")),
            "filters" => {
                definition.input_socket = Some(ManagedSocketRef::new(filter_id, "values"));
                definition.output_socket = Some(ManagedSocketRef::new(filter_id, "values"));
                definition.filter_value_mode = ManagedFilterValueMode::Tuple;
            }
            "outputs" => definition.input_socket = Some(ManagedSocketRef::new(output_id, "values")),
            _ => unreachable!(),
        }
    }
    instance.managed_regions.regions.insert(
        ManagedRegionId::new("inputs"),
        region("inputs", vec![input_item("Value", source.clone())]),
    );
    instance.managed_regions.regions.insert(
        ManagedRegionId::new("filters"),
        region("filters", vec![remap_item(0.0, 10.0, 0.0, 1.0)]),
    );
    instance.managed_regions.regions.insert(
        ManagedRegionId::new("outputs"),
        region("outputs", vec![output_item("Output", target.clone())]),
    );
    (formula, instance, source, target, before_id, after_id)
}

#[test]
fn managed_regions_execute_inside_custom_formula_graph() {
    let (formula, instance, source, target, _, _) = graph_fixture();
    let (value_types, nodes) = registries();
    let compile_ctx = CompileCtx {
        value_types: &value_types,
        nodes: &nodes,
        properties: Some(&formula.properties),
    };
    let mut runtime = ManagedFormulaRuntime::compile(&formula, &instance, &compile_ctx)
        .unwrap()
        .unwrap();
    assert!(runtime.uses_authored_graph());
    runtime
        .reconcile_input_source_schema(|_| {
            Some(ChannelSourceSchema {
                value_type: "float".into(),
                metadata: Default::default(),
            })
        })
        .unwrap();
    let mut inputs = RuntimeInputSnapshot::default();
    inputs.insert(source, RuntimeValue::Float(5.0));
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let output = runtime.evaluate(&eval_ctx(1, &inputs, &registries));
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    assert!(output.debug_samples.is_empty());
    assert_eq!(
        output
            .intents
            .iter()
            .filter(|intent| intent.kind.as_ref() == "debug.log")
            .count(),
        2
    );
    let command = output
        .intents
        .iter()
        .find(|intent| intent.kind.as_ref() != "debug.log")
        .unwrap();
    assert_eq!(command.target.as_ref(), Some(&target));
    assert_eq!(command.payload, RuntimeValue::Float(0.5));
}

#[test]
fn processor_runs_graph_backed_managed_formula_once_and_captures_authored_nodes() {
    let (formula, instance, source, target, before, after) = graph_fixture();
    let filter_node = instance.managed_regions.regions[&ManagedRegionId::new("filters")].items[0]
        .anode
        .id;
    let processor = Processor::new("Graph-backed Mapping", instance);
    let (value_types, nodes) = registries();
    let compile_ctx = CompileCtx {
        value_types: &value_types,
        nodes: &nodes,
        properties: Some(&formula.properties),
    };
    let mut runtime = ProcessorRuntime::new(processor.id);
    assert!(
        runtime.compile(&processor, &formula, &compile_ctx),
        "{:?}",
        runtime.diagnostics
    );
    runtime
        .managed_formula
        .as_mut()
        .unwrap()
        .reconcile_input_source_schema(|_| {
            Some(ChannelSourceSchema {
                value_type: "float".into(),
                metadata: Default::default(),
            })
        })
        .unwrap();
    runtime.apply_lifecycle(&processor, ProcessorLifecycleEvent::StateEnter(StateId::new()));
    let mut inputs = RuntimeInputSnapshot::default();
    inputs.insert(source, RuntimeValue::Float(5.0));
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let mut lanes = runtime.evaluate_processor_with_context_provider_and_runtime_capture(
        &processor,
        &eval_ctx(1, &inputs, &registries),
        &DefaultProcessorContextProvider,
        &ProcessorDebugCapture::All { history_len: 64 },
    );
    assert_eq!(lanes.len(), 1);
    let output = lanes.remove(0).output;
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    assert_eq!(
        output
            .intents
            .iter()
            .filter(|intent| intent.kind.as_ref() == "debug.log")
            .count(),
        2
    );
    assert!(
        output
            .intents
            .iter()
            .any(|intent| intent.target.as_ref() == Some(&target) && intent.payload == RuntimeValue::Float(0.5))
    );
    assert!(
        output
            .debug_samples
            .iter()
            .any(|sample| sample.author_node_id == before)
    );
    assert!(output.debug_samples.iter().any(|sample| sample.author_node_id == after));
    assert!(
        output
            .debug_samples
            .iter()
            .any(|sample| sample.author_node_id == filter_node)
    );
}

#[test]
fn graph_stage_error_suppresses_prior_graph_intents_and_commands() {
    let (formula, mut instance, source, _, _, _) = graph_fixture();
    let filter = instance
        .managed_regions
        .regions
        .get_mut(&ManagedRegionId::new("filters"))
        .unwrap();
    filter.items[0].anode.input_defaults.insert(
        "in_max".into(),
        RuntimeValue::Ref(StableRef::new("float".into(), "missing/auxiliary")),
    );
    let (value_types, nodes) = registries();
    let compile_ctx = CompileCtx {
        value_types: &value_types,
        nodes: &nodes,
        properties: Some(&formula.properties),
    };
    let mut runtime = ManagedFormulaRuntime::compile(&formula, &instance, &compile_ctx)
        .unwrap()
        .unwrap();
    runtime
        .reconcile_input_source_schema(|_| {
            Some(ChannelSourceSchema {
                value_type: "float".into(),
                metadata: Default::default(),
            })
        })
        .unwrap();
    let mut inputs = RuntimeInputSnapshot::default();
    inputs.insert(source, RuntimeValue::Float(5.0));
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let output = runtime.evaluate(&eval_ctx(1, &inputs, &registries));
    assert!(!output.diagnostics.is_empty());
    assert!(output.intents.is_empty());
    assert!(output.debug_samples.is_empty());
}

#[test]
fn managed_formula_rejects_unbound_authored_graph() {
    let (mut formula, instance) = formula_and_instance();
    let (value_types, nodes) = registries();
    let domain = AlchemistGraphDomain::new(nodes.clone(), value_types.clone(), Some(formula.properties.clone()));
    let mut transaction = AlchemistGraphTransaction::for_document(&formula.graph);
    AlchemistGraphDomain::insert_node(
        &mut transaction,
        ANodeInstance::new(ANodeTypeId::new(INPUTS_MANAGER_TYPE), "Unbound Input"),
    );
    transaction.commit(&mut formula.graph, &domain).unwrap();
    let compile_ctx = CompileCtx {
        value_types: &value_types,
        nodes: &nodes,
        properties: Some(&formula.properties),
    };
    let result = ManagedFormulaRuntime::compile(&formula, &instance, &compile_ctx);
    assert!(matches!(result, Err(crate::ManagedFormulaError::GraphBoundary(_))));
}
