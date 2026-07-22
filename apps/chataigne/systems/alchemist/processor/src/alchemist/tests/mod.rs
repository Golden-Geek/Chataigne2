use crate::testkit::TestGraph;

use std::time::Duration;

use chataigne_alchemist::{
    ANodeInstance, ANodeTypeId, AlchemistRuntime, CompileCtx, ContextKey, DebugCaptureMode, DebugCaptureSink,
    EvaluationCtx, EvaluationFrame, InputSocketRef, OutputSocketRef, RuntimeContextFrame, RuntimeInputSnapshot,
    RuntimeRegistries, SignatureCtx, SocketId, StableRef, TriggerValue, TypeBindings, TypeConstraint, ValueStorageKind,
    ValueTypeId, ValueTypeRegistry, compile_graph, evaluate_compiled_graph, primitive_node_registry,
};
use golden_values::Value as RuntimeValue;

use crate::alchemist::{
    CONDITIONS_MANAGER_TYPE, ChataigneNodeKind, FILTERS_MANAGER_TYPE, INPUTS_MANAGER_TYPE, MANAGER_PROPERTY_FIELD,
    MODULE_TYPE, OUTPUTS_MANAGER_TYPE, ROUTING_TYPE, TRIGGER_ON_VALUES_SIGNAL_FIELD, VALUE_SET_TYPE, node_registry,
    register_nodes, register_value_types,
};
use crate::{ValueLaneKey, ValueSet, ValueSetEntry};

fn node(id: &str) -> ANodeInstance {
    ANodeInstance::new(ANodeTypeId::new(id), id)
}

#[test]
fn app_anode_catalog_extends_every_primitive_with_every_app_declaration() {
    let mut expected = chataigne_alchemist::primitive_node_registry()
        .iter()
        .map(|declaration| declaration.type_id().to_string())
        .collect::<Vec<_>>();
    expected.extend(ChataigneNodeKind::all().iter().map(|kind| kind.type_id().to_owned()));
    let actual = node_registry()
        .iter()
        .map(|declaration| declaration.type_id().to_string())
        .collect::<Vec<_>>();

    assert_eq!(actual, expected);
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

    let inputs = nodes.get(&ANodeTypeId::new(INPUTS_MANAGER_TYPE)).unwrap().signature(
        &ctx,
        &node(INPUTS_MANAGER_TYPE),
        &bindings,
    );
    assert!(inputs.inputs.is_empty());
    assert_eq!(inputs.outputs.len(), 1);
    assert_eq!(inputs.outputs[0].id.as_str(), "values");
    assert_eq!(
        inputs.outputs[0].constraint,
        TypeConstraint::Exact(ValueTypeId::new(VALUE_SET_TYPE))
    );

    let filters = nodes.get(&ANodeTypeId::new(FILTERS_MANAGER_TYPE)).unwrap().signature(
        &ctx,
        &node(FILTERS_MANAGER_TYPE),
        &bindings,
    );
    assert_eq!(filters.inputs.len(), 1);
    assert_eq!(filters.inputs[0].id.as_str(), "values");
    assert_eq!(
        filters.inputs[0].constraint,
        TypeConstraint::Exact(ValueTypeId::new(VALUE_SET_TYPE))
    );
    assert_eq!(filters.outputs.len(), 1);
    assert_eq!(filters.outputs[0].id.as_str(), "values");
    assert_eq!(
        filters.outputs[0].constraint,
        TypeConstraint::Exact(ValueTypeId::new(VALUE_SET_TYPE))
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
fn conditions_manager_selects_lane_values_and_falls_back_to_global_values() {
    let manager_id = "conditions";
    let mut manager = node(CONDITIONS_MANAGER_TYPE);
    manager.config.set(
        MANAGER_PROPERTY_FIELD,
        RuntimeValue::Ref(StableRef::new(ValueTypeId::new("property"), manager_id)),
    );
    let mut graph = TestGraph::new();
    let manager = graph.add_node(manager).unwrap();

    let mut value_types = ValueTypeRegistry::with_primitives();
    register_value_types(&mut value_types).unwrap();
    let mut nodes = primitive_node_registry();
    register_nodes(&mut nodes).unwrap();
    let compiled = compile_graph(
        &graph.to_document().unwrap(),
        &CompileCtx {
            value_types: &value_types,
            nodes: &nodes,
            properties: None,
        },
    );
    assert!(!compiled.has_errors(), "{:?}", compiled.diagnostics);
    let compiled = compiled.compiled.unwrap();
    let source = StableRef::new(ValueTypeId::new(CONDITIONS_MANAGER_TYPE), manager_id);
    let lane = ContextKey::single("device", "left");
    let global_values = ValueSet::with_entries(
        4,
        vec![
            ValueSetEntry::new(ValueLaneKey::new("valid").unwrap(), "Valid", RuntimeValue::Bool(false)),
            ValueSetEntry::new(
                ValueLaneKey::new("on_false").unwrap(),
                "On False",
                RuntimeValue::Trigger(TriggerValue::fired(4, 1)),
            ),
        ],
    );
    let lane_values = ValueSet::with_entries(
        5,
        vec![
            ValueSetEntry::new(ValueLaneKey::new("valid").unwrap(), "Valid", RuntimeValue::Bool(true)),
            ValueSetEntry::new(
                ValueLaneKey::new("on_true").unwrap(),
                "On True",
                RuntimeValue::Trigger(TriggerValue::fired(5, 2)),
            ),
            ValueSetEntry::new(
                ValueLaneKey::new("ignored").unwrap(),
                "Ignored",
                RuntimeValue::String("ignored".into()),
            ),
        ],
    );
    let mut inputs = RuntimeInputSnapshot::default();
    inputs.insert(source.clone(), global_values.to_runtime_value().unwrap());
    inputs.insert(
        crate::lane_scoped_stable_ref(&source, &lane),
        lane_values.to_runtime_value().unwrap(),
    );
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let ctx = EvaluationCtx {
        logical_tick: 5,
        delta_time: Duration::ZERO,
        events: &[],
        inputs: &inputs,
        registries: &registries,
    };
    let context = RuntimeContextFrame::new(lane.clone());
    let mut runtime = AlchemistRuntime::new(compiled.clone());
    let mut debug = DebugCaptureSink::new(DebugCaptureMode::All { history_len: 16 });
    let lane_result = evaluate_compiled_graph(
        &runtime.compiled,
        &mut runtime.memory,
        EvaluationFrame {
            ctx: &ctx,
            properties: &runtime.properties,
            context: &context,
            debug: &mut debug,
            force_process_unchanged_inputs: false,
            capture_unchanged_outputs: true,
        },
    );
    assert!(lane_result.diagnostics.is_empty(), "{:?}", lane_result.diagnostics);
    let lane_output = |socket: &str| {
        lane_result
            .debug_samples
            .iter()
            .find(|sample| sample.author_node_id == manager && sample.output_socket.as_str() == socket)
            .unwrap()
            .value
            .clone()
    };
    assert_eq!(lane_output("valid"), RuntimeValue::Bool(true));
    assert_eq!(lane_output("on_true"), RuntimeValue::Trigger(TriggerValue::fired(5, 2)));
    assert_eq!(lane_output("on_false"), RuntimeValue::Trigger(TriggerValue::default()));
    assert!(
        lane_result
            .debug_samples
            .iter()
            .all(|sample| sample.context_key.as_ref() == Some(&lane))
    );

    let mut global_only_inputs = RuntimeInputSnapshot::default();
    global_only_inputs.insert(source, global_values.to_runtime_value().unwrap());
    let global_ctx = EvaluationCtx {
        logical_tick: 6,
        delta_time: Duration::ZERO,
        events: &[],
        inputs: &global_only_inputs,
        registries: &registries,
    };
    let mut fallback_runtime = AlchemistRuntime::new(compiled);
    let mut fallback_debug = DebugCaptureSink::new(DebugCaptureMode::All { history_len: 16 });
    let fallback_result = evaluate_compiled_graph(
        &fallback_runtime.compiled,
        &mut fallback_runtime.memory,
        EvaluationFrame {
            ctx: &global_ctx,
            properties: &fallback_runtime.properties,
            context: &context,
            debug: &mut fallback_debug,
            force_process_unchanged_inputs: false,
            capture_unchanged_outputs: true,
        },
    );
    let fallback_output = |socket: &str| {
        fallback_result
            .debug_samples
            .iter()
            .find(|sample| sample.author_node_id == manager && sample.output_socket.as_str() == socket)
            .unwrap()
            .value
            .clone()
    };
    assert_eq!(fallback_output("valid"), RuntimeValue::Bool(false));
    assert_eq!(
        fallback_output("on_true"),
        RuntimeValue::Trigger(TriggerValue::default())
    );
    assert_eq!(
        fallback_output("on_false"),
        RuntimeValue::Trigger(TriggerValue::fired(4, 1))
    );
}

#[test]
fn filters_manager_passes_value_sets_through_and_defaults_to_an_empty_set() {
    let entry = ValueSetEntry::new(
        ValueLaneKey::new("source").unwrap(),
        "Source",
        RuntimeValue::Float(0.75),
    );
    let expected = ValueSet::with_entries(9, vec![entry]);
    let mut filter = node(FILTERS_MANAGER_TYPE);
    filter
        .input_defaults
        .insert(SocketId::new("values"), expected.to_runtime_value().unwrap());
    let mut graph = TestGraph::new();
    let filter = graph.add_node(filter).unwrap();

    let mut value_types = ValueTypeRegistry::with_primitives();
    register_value_types(&mut value_types).unwrap();
    let mut nodes = primitive_node_registry();
    register_nodes(&mut nodes).unwrap();
    let compiled = compile_graph(
        &graph.to_document().unwrap(),
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
            logical_tick: 9,
            delta_time: Duration::ZERO,
            events: &[],
            inputs: &RuntimeInputSnapshot::default(),
            registries: &registries,
        },
        DebugCaptureMode::All { history_len: 16 },
    );
    let output = result
        .debug_samples
        .iter()
        .find(|sample| sample.author_node_id == filter && sample.output_socket.as_str() == "values")
        .unwrap();
    assert_eq!(ValueSet::from_runtime_value(&output.value).unwrap(), expected);

    let mut empty_graph = TestGraph::new();
    let empty_filter = empty_graph.add_node(node(FILTERS_MANAGER_TYPE)).unwrap();
    let empty_compiled = compile_graph(
        &empty_graph.to_document().unwrap(),
        &CompileCtx {
            value_types: &value_types,
            nodes: &nodes,
            properties: None,
        },
    );
    assert!(!empty_compiled.has_errors(), "{:?}", empty_compiled.diagnostics);
    let mut empty_runtime = AlchemistRuntime::new(empty_compiled.compiled.unwrap());
    let empty_result = empty_runtime.evaluate_with_capture_mode(
        &EvaluationCtx {
            logical_tick: 10,
            delta_time: Duration::ZERO,
            events: &[],
            inputs: &RuntimeInputSnapshot::default(),
            registries: &registries,
        },
        DebugCaptureMode::All { history_len: 16 },
    );
    let empty_output = empty_result
        .debug_samples
        .iter()
        .find(|sample| sample.author_node_id == empty_filter && sample.output_socket.as_str() == "values")
        .unwrap();
    assert_eq!(
        ValueSet::from_runtime_value(&empty_output.value).unwrap(),
        ValueSet::new(0)
    );
}

#[test]
fn inputs_and_outputs_managers_propagate_value_set_signals_without_trigger_wire() {
    let input_ref = StableRef::new(ValueTypeId::new("property"), "inputs");
    let mut input = node(INPUTS_MANAGER_TYPE);
    input.config.set(MANAGER_PROPERTY_FIELD, RuntimeValue::Ref(input_ref));
    let mut output = node(OUTPUTS_MANAGER_TYPE);
    output.config.set(
        MANAGER_PROPERTY_FIELD,
        RuntimeValue::Ref(StableRef::new(ValueTypeId::new("property"), "outputs")),
    );
    let mut graph = TestGraph::new();
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
        &graph.to_document().unwrap(),
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
    let mut graph = TestGraph::new();
    let output = graph.add_node(output).unwrap();

    let mut value_types = ValueTypeRegistry::with_primitives();
    register_value_types(&mut value_types).unwrap();
    let mut nodes = primitive_node_registry();
    register_nodes(&mut nodes).unwrap();
    let compiled = compile_graph(
        &graph.to_document().unwrap(),
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
fn routing_node_passes_its_current_typed_value_without_effects() {
    let mut route = node(ROUTING_TYPE);
    route
        .input_defaults
        .insert(SocketId::new("in"), RuntimeValue::Float(0.75));
    let mut graph = TestGraph::new();
    let route = graph.add_node(route).unwrap();
    let mut value_types = ValueTypeRegistry::with_primitives();
    register_value_types(&mut value_types).unwrap();
    let mut nodes = primitive_node_registry();
    register_nodes(&mut nodes).unwrap();
    let compiled = compile_graph(
        &graph.to_document().unwrap(),
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

    assert!(result.intents.is_empty());
    let output = result
        .debug_samples
        .iter()
        .find(|sample| sample.author_node_id == route && sample.output_socket.as_str() == "out")
        .expect("routing output should be captured");
    assert_eq!(output.value, RuntimeValue::Float(0.75));
}

#[test]
fn chataigne_values_are_registered_through_facets() {
    let mut registry = ValueTypeRegistry::with_primitives();
    register_value_types(&mut registry).unwrap();

    assert!(registry.supports_facet(
        &ValueTypeId::new(MODULE_TYPE),
        &chataigne_alchemist::FacetId::new("command_target")
    ));
}
