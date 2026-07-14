use std::time::Duration;

use golden_alchemist::{
    ANodeInstance, ANodeTypeId, AlchemistGraph, AlchemistRuntime, CompileCtx, DebugCaptureMode, EvaluationCtx,
    InputSocketRef, OutputSocketRef, RuntimeInputSnapshot, RuntimeRegistries, SignatureCtx, SocketId, StableRef,
    TriggerValue, TypeBindings, TypeConstraint, ValueStorageKind, ValueTypeId, ValueTypeRegistry, compile_graph,
    primitive_node_registry,
};
use golden_values::Value as RuntimeValue;

use crate::alchemist::{
    CONDITIONS_MANAGER_TYPE, INPUTS_MANAGER_TYPE, MANAGER_PROPERTY_FIELD, MODULE_ENDPOINT_TYPE, MODULE_TYPE,
    OUTPUTS_MANAGER_TYPE, ROUTING_TYPE, STATE_TYPE, TRIGGER_ON_VALUES_SIGNAL_FIELD, VALUE_SET_TYPE, register_nodes,
    register_value_types,
};
use crate::{ValueLaneKey, ValueSet, ValueSetEntry};

fn node(id: &str) -> ANodeInstance {
    ANodeInstance::new(ANodeTypeId::new(id), id)
}

#[test]
fn valueset_type_is_registered_as_extension_without_legacy_alias() {
    let mut registry = ValueTypeRegistry::with_primitives();
    register_value_types(&mut registry).unwrap();

    let descriptor = registry.get(&ValueTypeId::new(VALUE_SET_TYPE)).unwrap();
    assert_eq!(descriptor.label, "Value Set");
    assert_eq!(descriptor.storage, ValueStorageKind::Extension);
    assert!(!registry.contains(&ValueTypeId::new("chataigne.param_array")));
    let default_value = registry.default_value(&ValueTypeId::new(VALUE_SET_TYPE)).unwrap();
    assert_eq!(default_value.value_type(), ValueTypeId::new(VALUE_SET_TYPE));
    assert_eq!(ValueSet::from_runtime_value(&default_value).unwrap(), ValueSet::new(0));
}

#[test]
fn manager_anode_sockets_use_explicit_manager_value_shapes() {
    let mut value_types = ValueTypeRegistry::with_primitives();
    register_value_types(&mut value_types).unwrap();
    let mut nodes = primitive_node_registry();
    register_nodes(&mut nodes).unwrap();
    let ctx = SignatureCtx {
        value_types: &value_types,
        properties: None,
    };
    let bindings = TypeBindings::default();

    let conditions = nodes
        .get(&ANodeTypeId::new(CONDITIONS_MANAGER_TYPE))
        .unwrap()
        .signature(&ctx, &node(CONDITIONS_MANAGER_TYPE), &bindings);
    assert_eq!(conditions.outputs.len(), 3);
    assert_eq!(conditions.outputs[0].id.as_str(), "valid");
    assert_eq!(
        conditions.outputs[0].constraint,
        TypeConstraint::Exact(ValueTypeId::new("bool"))
    );
    assert_eq!(conditions.outputs[1].id.as_str(), "on_true");
    assert_eq!(
        conditions.outputs[1].constraint,
        TypeConstraint::Exact(ValueTypeId::new("trigger"))
    );
    assert_eq!(conditions.outputs[2].id.as_str(), "on_false");
    assert_eq!(
        conditions.outputs[2].constraint,
        TypeConstraint::Exact(ValueTypeId::new("trigger"))
    );

    let outputs = nodes.get(&ANodeTypeId::new(OUTPUTS_MANAGER_TYPE)).unwrap().signature(
        &ctx,
        &node(OUTPUTS_MANAGER_TYPE),
        &bindings,
    );
    assert_eq!(outputs.inputs[0].id.as_str(), "values");
    assert_eq!(
        outputs.inputs[0].constraint,
        TypeConstraint::Exact(ValueTypeId::new(VALUE_SET_TYPE))
    );

    let fields = nodes
        .get(&ANodeTypeId::new(OUTPUTS_MANAGER_TYPE))
        .unwrap()
        .config_fields();
    let trigger_on_signal = fields
        .iter()
        .find(|field| field.id.as_str() == TRIGGER_ON_VALUES_SIGNAL_FIELD)
        .unwrap();
    assert_eq!(trigger_on_signal.default_value, RuntimeValue::Bool(true));
}

#[test]
fn outputs_manager_emits_on_values_signal_without_trigger_wire() {
    let input_ref = StableRef::new(ValueTypeId::new("property"), "inputs");
    let mut input = node(INPUTS_MANAGER_TYPE);
    input.config.set(MANAGER_PROPERTY_FIELD, RuntimeValue::Ref(input_ref));
    let mut output = node(OUTPUTS_MANAGER_TYPE);
    output.config.set(
        MANAGER_PROPERTY_FIELD,
        RuntimeValue::Ref(StableRef::new(ValueTypeId::new("property"), "outputs")),
    );
    let mut graph = AlchemistGraph::new();
    let input = graph.add_node(input).unwrap();
    let output = graph.add_node(output).unwrap();
    graph
        .connect(
            OutputSocketRef::new(input, "values"),
            InputSocketRef::new(output, "values"),
        )
        .unwrap();

    let mut value_types = ValueTypeRegistry::with_primitives();
    register_value_types(&mut value_types).unwrap();
    let mut nodes = primitive_node_registry();
    register_nodes(&mut nodes).unwrap();
    let compiled = compile_graph(
        &graph,
        &CompileCtx {
            value_types: &value_types,
            nodes: &nodes,
            properties: None,
        },
    );
    assert!(!compiled.has_errors(), "{:?}", compiled.diagnostics);

    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let mut runtime = AlchemistRuntime::new(compiled.compiled.unwrap());
    let source_ref = StableRef::new(ValueTypeId::new(INPUTS_MANAGER_TYPE), "inputs");
    let entry = ValueSetEntry::new(ValueLaneKey::new("source").unwrap(), "Source", RuntimeValue::Float(0.5));
    let mut inputs = RuntimeInputSnapshot::default();
    inputs.insert(
        source_ref.clone(),
        ValueSet::with_entries(1, vec![entry.clone()])
            .to_runtime_value()
            .unwrap(),
    );

    let first = runtime.evaluate_with_capture_mode(
        &EvaluationCtx {
            logical_tick: 1,
            delta_time: Duration::ZERO,
            events: &[],
            inputs: &inputs,
            registries: &registries,
        },
        DebugCaptureMode::All {
            history_len: usize::MAX,
        },
    );
    assert_eq!(first.intents.len(), 1);
    assert_eq!(first.intents[0].kind.as_ref(), crate::COMMAND_INTENT_KIND);
    assert_eq!(
        first.intents[0].target.as_ref(),
        Some(&StableRef::new(ValueTypeId::new(OUTPUTS_MANAGER_TYPE), "outputs"))
    );
    assert_eq!(
        ValueSet::from_runtime_value(&first.intents[0].payload).unwrap(),
        ValueSet::with_entries(1, vec![entry.clone()])
    );
    let first_output_sample = first
        .debug_samples
        .iter()
        .find(|sample| sample.author_node_id == output && sample.output_socket.as_str() == "values")
        .unwrap();
    assert_eq!(
        ValueSet::from_runtime_value(&first_output_sample.value).unwrap(),
        ValueSet::with_entries(1, vec![entry.clone()])
    );

    inputs.insert(
        source_ref,
        ValueSet::with_entries(2, vec![entry.clone()])
            .to_runtime_value()
            .unwrap(),
    );
    let second = runtime.evaluate_with_capture_mode(
        &EvaluationCtx {
            logical_tick: 2,
            delta_time: Duration::ZERO,
            events: &[],
            inputs: &inputs,
            registries: &registries,
        },
        DebugCaptureMode::All {
            history_len: usize::MAX,
        },
    );
    assert_eq!(second.intents.len(), 1);
    assert_eq!(
        ValueSet::from_runtime_value(&second.intents[0].payload).unwrap(),
        ValueSet::with_entries(2, vec![entry.clone()])
    );
    let second_output_sample = second
        .debug_samples
        .iter()
        .find(|sample| sample.author_node_id == output && sample.output_socket.as_str() == "values")
        .unwrap();
    assert_eq!(
        ValueSet::from_runtime_value(&second_output_sample.value).unwrap(),
        ValueSet::with_entries(2, vec![entry])
    );
}

#[test]
fn outputs_manager_emits_and_samples_on_trigger_without_values_signal() {
    let mut output = node(OUTPUTS_MANAGER_TYPE);
    output.config.set(
        MANAGER_PROPERTY_FIELD,
        RuntimeValue::Ref(StableRef::new(ValueTypeId::new("property"), "outputs")),
    );
    output
        .config
        .set(TRIGGER_ON_VALUES_SIGNAL_FIELD, RuntimeValue::Bool(false));
    output.input_defaults.insert(
        SocketId::new("trigger"),
        RuntimeValue::Trigger(TriggerValue::fired(7, 1)),
    );
    let mut graph = AlchemistGraph::new();
    let output = graph.add_node(output).unwrap();

    let mut value_types = ValueTypeRegistry::with_primitives();
    register_value_types(&mut value_types).unwrap();
    let mut nodes = primitive_node_registry();
    register_nodes(&mut nodes).unwrap();
    let compiled = compile_graph(
        &graph,
        &CompileCtx {
            value_types: &value_types,
            nodes: &nodes,
            properties: None,
        },
    );
    assert!(!compiled.has_errors(), "{:?}", compiled.diagnostics);

    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let mut runtime = AlchemistRuntime::new(compiled.compiled.unwrap());
    let result = runtime.evaluate_with_capture_mode(
        &EvaluationCtx {
            logical_tick: 1,
            delta_time: Duration::ZERO,
            events: &[],
            inputs: &RuntimeInputSnapshot::default(),
            registries: &registries,
        },
        DebugCaptureMode::All {
            history_len: usize::MAX,
        },
    );

    assert_eq!(result.intents.len(), 1);
    assert_eq!(result.intents[0].kind.as_ref(), crate::COMMAND_INTENT_KIND);
    assert_eq!(
        result.intents[0].target.as_ref(),
        Some(&StableRef::new(ValueTypeId::new(OUTPUTS_MANAGER_TYPE), "outputs"))
    );
    assert_eq!(
        ValueSet::from_runtime_value(&result.intents[0].payload).unwrap(),
        ValueSet::new(0)
    );
    assert_eq!(result.debug_samples.len(), 1);
    assert_eq!(result.debug_samples[0].author_node_id, output);
    assert_eq!(result.debug_samples[0].output_socket.as_str(), "values");
    assert_eq!(
        ValueSet::from_runtime_value(&result.debug_samples[0].value).unwrap(),
        ValueSet::new(0)
    );
}

#[test]
#[ignore = "stale pre-manager-ref behavior; Phase 13 will replace this with real manager-node semantics"]
fn module_input_can_emit_state_transition_intent() {
    let source_ref = StableRef::new(ValueTypeId::new(MODULE_ENDPOINT_TYPE), "module/value");
    let state_ref = StableRef::new(ValueTypeId::new(STATE_TYPE), "state-b");
    let mut input = node("chataigne.module_value_input");
    input.config.set("source", RuntimeValue::Ref(source_ref.clone()));
    input.config.set("value_type", RuntimeValue::String("bool".into()));
    let mut state = node("constant");
    state.config.set("value", RuntimeValue::Ref(state_ref.clone()));
    let mut graph = AlchemistGraph::new();
    let input = graph.add_node(input).unwrap();
    let edge = graph.add_node(node("edge")).unwrap();
    let state = graph.add_node(state).unwrap();
    let output = graph
        .add_node(node("chataigne.state_transition_intent_output"))
        .unwrap();
    graph
        .connect(OutputSocketRef::new(input, "value"), InputSocketRef::new(edge, "value"))
        .unwrap();
    graph
        .connect(
            OutputSocketRef::new(edge, "trigger"),
            InputSocketRef::new(output, "trigger"),
        )
        .unwrap();
    graph
        .connect(
            OutputSocketRef::new(state, "value"),
            InputSocketRef::new(output, "target"),
        )
        .unwrap();
    let mut value_types = ValueTypeRegistry::with_primitives();
    register_value_types(&mut value_types).unwrap();
    let mut nodes = primitive_node_registry();
    register_nodes(&mut nodes).unwrap();
    let compiled = compile_graph(
        &graph,
        &CompileCtx {
            value_types: &value_types,
            nodes: &nodes,
            properties: None,
        },
    );
    assert!(!compiled.has_errors(), "{:?}", compiled.diagnostics);
    let mut runtime = AlchemistRuntime::new(compiled.compiled.unwrap());
    let mut inputs = RuntimeInputSnapshot::default();
    inputs.insert(source_ref, RuntimeValue::Bool(true));
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };

    let result = runtime.evaluate(&EvaluationCtx {
        logical_tick: 1,
        delta_time: Duration::ZERO,
        events: &[],
        inputs: &inputs,
        registries: &registries,
    });

    assert_eq!(result.intents.len(), 1);
    assert_eq!(result.intents[0].kind.as_ref(), "chataigne.state_transition");
    assert_eq!(result.intents[0].target.as_ref(), Some(&state_ref));
}

#[test]
#[ignore = "stale pre-manager-ref behavior; Phase 13 will replace this with real manager-node semantics"]
fn routing_node_passes_value_to_downstream_consumers() {
    let source_ref = StableRef::new(ValueTypeId::new(MODULE_ENDPOINT_TYPE), "module/value");
    let target_ref = StableRef::new(ValueTypeId::new(MODULE_TYPE), "module");
    let mut input = node("chataigne.module_value_input");
    input.config.set("source", RuntimeValue::Ref(source_ref.clone()));
    input.config.set("value_type", RuntimeValue::String("bool".into()));
    let mut target = node("constant");
    target.config.set("value", RuntimeValue::Ref(target_ref.clone()));
    let mut graph = AlchemistGraph::new();
    let input = graph.add_node(input).unwrap();
    let route = graph.add_node(node(ROUTING_TYPE)).unwrap();
    let edge = graph.add_node(node("edge")).unwrap();
    let target = graph.add_node(target).unwrap();
    let builder = graph.add_node(node("chataigne.command_builder")).unwrap();
    let output = graph.add_node(node("chataigne.command_intent_output")).unwrap();
    graph
        .connect(OutputSocketRef::new(input, "value"), InputSocketRef::new(route, "in"))
        .unwrap();
    graph
        .connect(
            OutputSocketRef::new(route, "out"),
            InputSocketRef::new(builder, "payload"),
        )
        .unwrap();
    graph
        .connect(OutputSocketRef::new(input, "value"), InputSocketRef::new(edge, "value"))
        .unwrap();
    graph
        .connect(
            OutputSocketRef::new(edge, "trigger"),
            InputSocketRef::new(output, "trigger"),
        )
        .unwrap();
    graph
        .connect(
            OutputSocketRef::new(target, "value"),
            InputSocketRef::new(builder, "target"),
        )
        .unwrap();
    graph
        .connect(
            OutputSocketRef::new(builder, "command"),
            InputSocketRef::new(output, "command"),
        )
        .unwrap();
    let mut value_types = ValueTypeRegistry::with_primitives();
    register_value_types(&mut value_types).unwrap();
    let mut nodes = primitive_node_registry();
    register_nodes(&mut nodes).unwrap();
    let compiled = compile_graph(
        &graph,
        &CompileCtx {
            value_types: &value_types,
            nodes: &nodes,
            properties: None,
        },
    );
    assert!(!compiled.has_errors(), "{:?}", compiled.diagnostics);
    let mut runtime = AlchemistRuntime::new(compiled.compiled.unwrap());
    let mut inputs = RuntimeInputSnapshot::default();
    inputs.insert(source_ref, RuntimeValue::Bool(true));
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };

    let result = runtime.evaluate(&EvaluationCtx {
        logical_tick: 1,
        delta_time: Duration::ZERO,
        events: &[],
        inputs: &inputs,
        registries: &registries,
    });

    assert_eq!(result.intents.len(), 1);
    assert_eq!(result.intents[0].kind.as_ref(), "chataigne.command");
    assert_eq!(result.intents[0].target.as_ref(), Some(&target_ref));
    assert_eq!(result.intents[0].payload, RuntimeValue::Bool(true));
}

#[test]
fn chataigne_values_are_registered_through_facets() {
    let mut registry = ValueTypeRegistry::with_primitives();
    register_value_types(&mut registry).unwrap();

    assert!(registry.supports_facet(
        &ValueTypeId::new(MODULE_TYPE),
        &golden_alchemist::FacetId::new("command_target")
    ));
}
