use std::time::Duration;

use golden_alchemist::{
    ANodeInstance, ANodeTypeId, AlchemistGraph, AlchemistRuntime, CompileCtx, CompileResult, DiagnosticOrigin,
    EvaluationCtx, InputSocketRef, OutputSocketRef, RuntimeInputSnapshot, RuntimeOutput, RuntimeRegistries,
    RuntimeValue, SignatureCtx, StableRef, TriggerValue, TypeBindings, TypeConstraint, ValueStorageKind, ValueTypeId,
    ValueTypeRegistry, compile_graph, primitive_node_registry,
};

use crate::alchemist::{
    CONDITIONS_MANAGER_TYPE, INPUTS_MANAGER_TYPE, MODULE_ENDPOINT_TYPE, MODULE_TYPE, OUTPUTS_MANAGER_TYPE,
    ROUTING_TYPE, STATE_TYPE, VALUE_SET_TYPE, register_nodes, register_value_types,
};
use crate::{COMMAND_INTENT_KIND, INPUT_SOURCE_FIELD, OUTPUT_TARGET_FIELD, ValueLaneKey, ValueSet, ValueSetEntry};

fn node(id: &str) -> ANodeInstance {
    ANodeInstance::new(ANodeTypeId::new(id), id)
}

fn compile_single_node(type_id: &str) -> CompileResult {
    compile_node(node(type_id))
}

fn compile_node(instance: ANodeInstance) -> CompileResult {
    let mut graph = AlchemistGraph::new();
    graph.add_node(instance).unwrap();
    let mut value_types = ValueTypeRegistry::with_primitives();
    register_value_types(&mut value_types).unwrap();
    let mut nodes = primitive_node_registry();
    register_nodes(&mut nodes).unwrap();
    compile_graph(
        &graph,
        &CompileCtx {
            value_types: &value_types,
            nodes: &nodes,
            properties: None,
        },
    )
}

fn manager_ref(value_type: &str, stable_id: &str) -> StableRef {
    StableRef::new(ValueTypeId::new(value_type), stable_id)
}

fn evaluate_source_bridge(node_type: &str, field: &str, source: StableRef, values: ValueSet) -> RuntimeOutput {
    let mut instance = node(node_type);
    instance.config.set(field, RuntimeValue::Ref(source.clone()));
    let compiled = compile_node(instance);
    assert!(!compiled.has_errors(), "{:?}", compiled.diagnostics);
    let mut runtime = AlchemistRuntime::new(compiled.compiled.unwrap());
    let mut inputs = RuntimeInputSnapshot::default();
    inputs.insert(source, values.to_runtime_value().unwrap());
    let value_types = crate::alchemist::value_type_registry();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };

    runtime.evaluate(&EvaluationCtx {
        logical_tick: 22,
        delta_time: Duration::ZERO,
        events: &[],
        inputs: &inputs,
        registries: &registries,
    })
}

fn compile_output_bridge_graph(target: StableRef, value: RuntimeValue, trigger: Option<RuntimeValue>) -> CompileResult {
    let mut value_node = node("constant");
    value_node.config.set("value", value);
    let mut output_node = node(OUTPUTS_MANAGER_TYPE);
    output_node.config.set(OUTPUT_TARGET_FIELD, RuntimeValue::Ref(target));

    let mut graph = AlchemistGraph::new();
    let value_node = graph.add_node(value_node).unwrap();
    let output_node = graph.add_node(output_node).unwrap();
    graph
        .connect(
            OutputSocketRef::new(value_node, "value"),
            InputSocketRef::new(output_node, "values"),
        )
        .unwrap();

    if let Some(trigger) = trigger {
        let mut trigger_node = node("constant");
        trigger_node.config.set("value", trigger);
        let trigger_node = graph.add_node(trigger_node).unwrap();
        graph
            .connect(
                OutputSocketRef::new(trigger_node, "value"),
                InputSocketRef::new(output_node, "trigger"),
            )
            .unwrap();
    }

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
    compiled
}

fn sample_value(result: &RuntimeOutput, socket: &str) -> RuntimeValue {
    result
        .debug_samples
        .iter()
        .find(|sample| sample.output_socket.as_str() == socket)
        .unwrap_or_else(|| panic!("missing `{socket}` sample"))
        .value
        .clone()
}

#[test]
fn manager_reference_nodes_require_configured_bridge_refs() {
    for (node_type, label, field, value_type) in [
        (
            CONDITIONS_MANAGER_TYPE,
            "Conditions",
            INPUT_SOURCE_FIELD,
            CONDITIONS_MANAGER_TYPE,
        ),
        (INPUTS_MANAGER_TYPE, "Inputs", INPUT_SOURCE_FIELD, INPUTS_MANAGER_TYPE),
        (
            OUTPUTS_MANAGER_TYPE,
            "Output Commands",
            OUTPUT_TARGET_FIELD,
            OUTPUTS_MANAGER_TYPE,
        ),
    ] {
        let result = compile_single_node(node_type);
        assert!(
            result.compiled.is_none(),
            "{node_type} must not compile without a bridge ref"
        );
        assert!(result.has_errors(), "{node_type} must report a compile error");
        assert_eq!(
            result.diagnostics.len(),
            1,
            "{node_type} should emit one explicit diagnostic"
        );
        let diagnostic = &result.diagnostics[0];
        assert_eq!(diagnostic.code, "chataigne_manager_bridge_missing_ref");
        assert!(
            diagnostic.message.contains(label),
            "{node_type} diagnostic should name the manager role: {:?}",
            diagnostic.message
        );
        assert!(
            diagnostic.message.contains(field) && diagnostic.message.contains(value_type),
            "{node_type} diagnostic should describe the missing bridge ref: {:?}",
            diagnostic.message
        );
        assert!(
            diagnostic.message.contains("does not return fallback values"),
            "{node_type} diagnostic should reject fake fallback behavior: {:?}",
            diagnostic.message
        );
        assert!(
            matches!(&diagnostic.origin, DiagnosticOrigin::Node(_)),
            "{node_type} diagnostic should point at the authored ANode"
        );

        let mut unbound = node(node_type);
        unbound.config.set(
            field,
            RuntimeValue::Ref(StableRef::new(ValueTypeId::new(value_type), "")),
        );
        let result = compile_node(unbound);
        assert!(result.has_errors(), "{node_type} must reject unbound bridge refs");
        assert_eq!(result.diagnostics[0].code, "chataigne_manager_bridge_unbound_ref");

        let mut invalid = node(node_type);
        invalid.config.set(field, RuntimeValue::Float(1.0));
        let result = compile_node(invalid);
        assert!(result.has_errors(), "{node_type} must reject non-ref bridge config");
        assert_eq!(result.diagnostics[0].code, "chataigne_manager_bridge_invalid_ref");
    }
}

#[test]
fn input_manager_bridge_exposes_valueset_from_runtime_source() {
    let source = manager_ref(INPUTS_MANAGER_TYPE, "inputs/main");
    let values = ValueSet::with_entries(
        3,
        vec![ValueSetEntry::new(
            ValueLaneKey::new("x").unwrap(),
            "X",
            RuntimeValue::Float(0.5),
        )],
    );
    let result = evaluate_source_bridge(INPUTS_MANAGER_TYPE, INPUT_SOURCE_FIELD, source.clone(), values.clone());

    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let sample = result
        .debug_samples
        .iter()
        .find(|sample| sample.output_socket.as_str() == "values")
        .expect("inputs bridge should publish values output");
    assert_eq!(ValueSet::from_runtime_value(&sample.value).unwrap(), values);
}

#[test]
fn input_manager_bridge_missing_runtime_source_emits_no_fallback_sample() {
    let source = manager_ref(INPUTS_MANAGER_TYPE, "inputs/missing");
    let mut instance = node(INPUTS_MANAGER_TYPE);
    let authored = instance.id;
    instance.config.set(INPUT_SOURCE_FIELD, RuntimeValue::Ref(source));
    let compiled = compile_node(instance);
    assert!(!compiled.has_errors(), "{:?}", compiled.diagnostics);
    let mut runtime = AlchemistRuntime::new(compiled.compiled.unwrap());
    let value_types = crate::alchemist::value_type_registry();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let inputs = RuntimeInputSnapshot::default();

    let result = runtime.evaluate(&EvaluationCtx {
        logical_tick: 1,
        delta_time: Duration::ZERO,
        events: &[],
        inputs: &inputs,
        registries: &registries,
    });

    assert_eq!(result.diagnostics.len(), 1);
    assert!(
        result
            .debug_samples
            .iter()
            .all(|sample| sample.author_node_id != authored)
    );
}

#[test]
fn condition_manager_bridge_exposes_bool_and_trigger_lanes() {
    let source = manager_ref(CONDITIONS_MANAGER_TYPE, "conditions/main");
    let on_true = TriggerValue::fired(7, 22);
    let on_false = TriggerValue::default();
    let values = ValueSet::with_entries(
        22,
        vec![
            ValueSetEntry::new(ValueLaneKey::new("valid").unwrap(), "Valid", RuntimeValue::Bool(true)),
            ValueSetEntry::new(
                ValueLaneKey::new("on_true").unwrap(),
                "On True",
                RuntimeValue::Trigger(on_true),
            ),
            ValueSetEntry::new(
                ValueLaneKey::new("on_false").unwrap(),
                "On False",
                RuntimeValue::Trigger(on_false),
            ),
        ],
    );
    let result = evaluate_source_bridge(CONDITIONS_MANAGER_TYPE, INPUT_SOURCE_FIELD, source.clone(), values);

    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(sample_value(&result, "valid"), RuntimeValue::Bool(true));
    assert_eq!(sample_value(&result, "on_true"), RuntimeValue::Trigger(on_true));
    assert_eq!(sample_value(&result, "on_false"), RuntimeValue::Trigger(on_false));
}

#[test]
fn output_manager_bridge_emits_command_intent_with_optional_trigger() {
    let target = manager_ref(OUTPUTS_MANAGER_TYPE, "outputs/main");
    let compiled = compile_output_bridge_graph(target.clone(), RuntimeValue::Float(0.75), None);
    let mut runtime = AlchemistRuntime::new(compiled.compiled.unwrap());
    let value_types = crate::alchemist::value_type_registry();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let inputs = RuntimeInputSnapshot::default();

    let result = runtime.evaluate(&EvaluationCtx {
        logical_tick: 12,
        delta_time: Duration::ZERO,
        events: &[],
        inputs: &inputs,
        registries: &registries,
    });

    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(result.intents.len(), 1);
    assert_eq!(result.intents[0].kind.as_ref(), COMMAND_INTENT_KIND);
    assert_eq!(result.intents[0].target.as_ref(), Some(&target));
    assert_eq!(result.intents[0].payload, RuntimeValue::Float(0.75));
}

#[test]
fn output_manager_bridge_emits_valueset_payload() {
    let target = manager_ref(OUTPUTS_MANAGER_TYPE, "outputs/main");
    let values = ValueSet::with_entries(
        12,
        vec![ValueSetEntry::new(
            ValueLaneKey::new("level").unwrap(),
            "Level",
            RuntimeValue::Float(0.75),
        )],
    );
    let payload = values.to_runtime_value().unwrap();
    let compiled = compile_output_bridge_graph(target.clone(), payload.clone(), None);
    let mut runtime = AlchemistRuntime::new(compiled.compiled.unwrap());
    let value_types = crate::alchemist::value_type_registry();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let inputs = RuntimeInputSnapshot::default();

    let result = runtime.evaluate(&EvaluationCtx {
        logical_tick: 12,
        delta_time: Duration::ZERO,
        events: &[],
        inputs: &inputs,
        registries: &registries,
    });

    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(result.intents.len(), 1);
    assert_eq!(result.intents[0].target.as_ref(), Some(&target));
    assert_eq!(result.intents[0].payload, payload);
}

#[test]
fn output_manager_bridge_suppresses_idle_trigger() {
    let target = manager_ref(OUTPUTS_MANAGER_TYPE, "outputs/main");
    let compiled = compile_output_bridge_graph(
        target,
        RuntimeValue::Float(0.75),
        Some(RuntimeValue::Trigger(TriggerValue::default())),
    );
    let mut runtime = AlchemistRuntime::new(compiled.compiled.unwrap());
    let value_types = crate::alchemist::value_type_registry();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let inputs = RuntimeInputSnapshot::default();

    let result = runtime.evaluate(&EvaluationCtx {
        logical_tick: 12,
        delta_time: Duration::ZERO,
        events: &[],
        inputs: &inputs,
        registries: &registries,
    });

    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert!(result.intents.is_empty());
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
fn manager_reference_sockets_expose_valueset() {
    let mut value_types = ValueTypeRegistry::with_primitives();
    register_value_types(&mut value_types).unwrap();
    let mut nodes = primitive_node_registry();
    register_nodes(&mut nodes).unwrap();
    let ctx = SignatureCtx {
        value_types: &value_types,
        properties: None,
    };
    let bindings = TypeBindings::default();

    let inputs_decl = nodes.get(&ANodeTypeId::new(INPUTS_MANAGER_TYPE)).unwrap();
    let inputs = inputs_decl.signature(&ctx, &node(INPUTS_MANAGER_TYPE), &bindings);
    assert_eq!(inputs.outputs[0].id.as_str(), "values");
    assert_eq!(inputs.outputs[0].label, "Values");
    assert_eq!(
        inputs.outputs[0].constraint,
        TypeConstraint::Exact(ValueTypeId::new(VALUE_SET_TYPE))
    );

    let outputs_decl = nodes.get(&ANodeTypeId::new(OUTPUTS_MANAGER_TYPE)).unwrap();
    let outputs = outputs_decl.signature(&ctx, &node(OUTPUTS_MANAGER_TYPE), &bindings);
    assert_eq!(outputs.inputs[0].id.as_str(), "values");
    assert_eq!(outputs.inputs[0].label, "Values");
    assert!(
        outputs.inputs[0]
            .constraint
            .accepts_value_type(&ValueTypeId::new(VALUE_SET_TYPE), &value_types)
    );
    assert!(
        outputs.inputs[0]
            .constraint
            .accepts_value_type(&ValueTypeId::new("float"), &value_types)
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
