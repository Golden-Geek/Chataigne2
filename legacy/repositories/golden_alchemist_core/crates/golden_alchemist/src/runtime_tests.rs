use std::time::Duration;

use indexmap::{IndexMap, IndexSet};

use crate::{
    ANodeId, ANodeInstance, ANodeTypeId, AlchemistGraph, AlchemistMemory, AlchemistRuntime, ColorValue, CompileCtx,
    ContextKey, DebugCaptureMode, DebugCaptureSink, EvaluationCtx, EvaluationFrame, ExtensionValue, FormulaId,
    FormulaPropertyDecl, FormulaPropertyId, FormulaPropertySchema, InputSocketRef, InputValueSource, LaneRuntimePool,
    OutputPreviewStatus, OutputSocketRef, PROCESS_ON_INPUT_CHANGE_ONLY_CONFIG, RuntimeContextFrame,
    RuntimeInputSnapshot, RuntimeOutput, RuntimePropertyFrame, RuntimeRegistries, RuntimeValue, SocketId, StableRef,
    TriggerValue, TypeBindingSource, TypeVar, ValueStorageKind, ValueTypeDescriptor, ValueTypeId, ValueTypeRegistry,
    compile_graph, evaluate_compiled_graph, formula_input_value_ref, primitive_node_registry,
};

fn node(type_id: &str) -> ANodeInstance {
    ANodeInstance::new(ANodeTypeId::new(type_id), type_id)
}

fn constant(value: RuntimeValue) -> ANodeInstance {
    let mut node = node("constant");
    node.config.set("value", value);
    node
}

fn runtime(graph: &AlchemistGraph) -> AlchemistRuntime {
    runtime_with_properties(graph, None)
}

fn runtime_with_properties(graph: &AlchemistGraph, properties: Option<&FormulaPropertySchema>) -> AlchemistRuntime {
    let result = compile_graph(
        graph,
        &CompileCtx {
            value_types: &ValueTypeRegistry::with_primitives(),
            nodes: &primitive_node_registry(),
            properties,
        },
    );
    assert!(!result.has_errors(), "{:?}", result.diagnostics);
    AlchemistRuntime::new(result.compiled.unwrap())
}

fn runtime_with_value_types(graph: &AlchemistGraph, value_types: &ValueTypeRegistry) -> AlchemistRuntime {
    let result = compile_graph(
        graph,
        &CompileCtx {
            value_types,
            nodes: &primitive_node_registry(),
            properties: None,
        },
    );
    assert!(!result.has_errors(), "{:?}", result.diagnostics);
    AlchemistRuntime::new(result.compiled.unwrap())
}

fn compile_with_properties(
    graph: &AlchemistGraph,
    properties: &FormulaPropertySchema,
) -> std::sync::Arc<crate::CompiledAlchemistGraph> {
    let value_types = ValueTypeRegistry::with_primitives();
    let nodes = primitive_node_registry();
    let result = compile_graph(
        graph,
        &CompileCtx {
            value_types: &value_types,
            nodes: &nodes,
            properties: Some(properties),
        },
    );
    assert!(!result.has_errors(), "{:?}", result.diagnostics);
    result.compiled.unwrap()
}

fn property_schema(id: &str, value_type: &str, default_value: RuntimeValue) -> FormulaPropertySchema {
    let mut schema = FormulaPropertySchema::default();
    schema.insert(FormulaPropertyDecl {
        id: FormulaPropertyId::new(id),
        label: id.into(),
        description: None,
        value_type: ValueTypeId::new(value_type),
        default_value,
        ui: crate::PropertyUiHints::default(),
    });
    schema
}

fn property_node(id: &str) -> ANodeInstance {
    let mut node = node("property");
    node.config.set(
        "property_id",
        RuntimeValue::Ref(StableRef::new(ValueTypeId::new("property"), id)),
    );
    node
}

fn evaluate(runtime: &mut AlchemistRuntime, logical_tick: u64) -> crate::RuntimeOutput {
    let value_types = ValueTypeRegistry::with_primitives();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    runtime.evaluate_with_capture_mode(
        &EvaluationCtx {
            logical_tick,
            delta_time: Duration::from_millis(16),
            events: &[],
            inputs: &RuntimeInputSnapshot::default(),
            registries: &registries,
        },
        DebugCaptureMode::All { history_len: 64 },
    )
}

fn evaluate_with_capture_mode(
    runtime: &mut AlchemistRuntime,
    logical_tick: u64,
    capture_mode: DebugCaptureMode,
) -> crate::RuntimeOutput {
    let value_types = ValueTypeRegistry::with_primitives();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    runtime.evaluate_with_capture_mode(
        &EvaluationCtx {
            logical_tick,
            delta_time: Duration::from_millis(16),
            events: &[],
            inputs: &RuntimeInputSnapshot::default(),
            registries: &registries,
        },
        capture_mode,
    )
}

fn evaluate_capturing_unchanged_outputs(runtime: &mut AlchemistRuntime, logical_tick: u64) -> crate::RuntimeOutput {
    let value_types = ValueTypeRegistry::with_primitives();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let ctx = EvaluationCtx {
        logical_tick,
        delta_time: Duration::from_millis(16),
        events: &[],
        inputs: &RuntimeInputSnapshot::default(),
        registries: &registries,
    };
    let context = RuntimeContextFrame::default_lane();
    let mut debug = DebugCaptureSink::new(DebugCaptureMode::All { history_len: 64 });
    evaluate_compiled_graph(
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
    )
}

#[test]
fn debug_capture_default_is_off() {
    assert_eq!(DebugCaptureMode::default(), DebugCaptureMode::Off);
}

#[test]
fn runtime_input_snapshot_overrides_unconnected_socket_default() {
    let mut graph = AlchemistGraph::new();
    let mut debug = node("debug_value");
    let socket = SocketId::new("value");
    debug.input_defaults.insert(socket.clone(), RuntimeValue::Float(1.0));
    let debug_id = graph.add_node(debug).unwrap();
    let reference = formula_input_value_ref(graph.id, debug_id, &socket);
    let mut runtime = runtime(&graph);
    let value_types = ValueTypeRegistry::with_primitives();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };

    let fallback_ctx = EvaluationCtx {
        logical_tick: 1,
        delta_time: Duration::from_millis(16),
        events: &[],
        inputs: &RuntimeInputSnapshot::default(),
        registries: &registries,
    };
    let fallback = runtime.evaluate_with_capture_mode(&fallback_ctx, DebugCaptureMode::All { history_len: 16 });
    assert_eq!(sample_value(&fallback, debug_id, "value"), RuntimeValue::Float(1.0));

    let mut inputs = RuntimeInputSnapshot::default();
    inputs.insert(reference, RuntimeValue::Float(2.5));
    let override_ctx = EvaluationCtx {
        logical_tick: 2,
        delta_time: Duration::from_millis(16),
        events: &[],
        inputs: &inputs,
        registries: &registries,
    };
    let overridden = runtime.evaluate_with_capture_mode(&override_ctx, DebugCaptureMode::All { history_len: 16 });
    assert_eq!(sample_value(&overridden, debug_id, "value"), RuntimeValue::Float(2.5));
}

fn sample_value(output: &RuntimeOutput, node: ANodeId, socket: &str) -> RuntimeValue {
    output
        .debug_samples
        .iter()
        .find(|sample| sample.author_node_id == node && sample.output_socket == SocketId::new(socket))
        .unwrap_or_else(|| panic!("missing sample for {node:?}.{socket}"))
        .value
        .clone()
}

#[test]
fn condition_gate_true_condition_passes_value() {
    let mut graph = AlchemistGraph::new();
    let value = graph.add_node(constant(RuntimeValue::Float(5.0))).unwrap();
    let condition = graph.add_node(constant(RuntimeValue::Bool(true))).unwrap();
    let gate = graph.add_node(node("condition_gate")).unwrap();
    graph
        .connect(OutputSocketRef::new(value, "value"), InputSocketRef::new(gate, "value"))
        .unwrap();
    graph
        .connect(
            OutputSocketRef::new(condition, "value"),
            InputSocketRef::new(gate, "condition"),
        )
        .unwrap();
    let mut runtime = runtime(&graph);

    let output = evaluate(&mut runtime, 1);

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    assert_eq!(sample_value(&output, gate, "value"), RuntimeValue::Float(5.0));
    assert_eq!(sample_value(&output, gate, "passed"), RuntimeValue::Bool(true));
    assert_eq!(sample_value(&output, gate, "blocked"), RuntimeValue::Bool(false));
}

#[test]
fn condition_gate_false_condition_blocks_value() {
    let mut graph = AlchemistGraph::new();
    let value = graph.add_node(constant(RuntimeValue::Float(5.0))).unwrap();
    let condition = graph.add_node(constant(RuntimeValue::Bool(false))).unwrap();
    let gate = graph.add_node(node("condition_gate")).unwrap();
    graph
        .connect(OutputSocketRef::new(value, "value"), InputSocketRef::new(gate, "value"))
        .unwrap();
    graph
        .connect(
            OutputSocketRef::new(condition, "value"),
            InputSocketRef::new(gate, "condition"),
        )
        .unwrap();
    let mut runtime = runtime(&graph);

    let output = evaluate(&mut runtime, 1);

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    assert_eq!(sample_value(&output, gate, "value"), RuntimeValue::Float(0.0));
    assert_eq!(sample_value(&output, gate, "passed"), RuntimeValue::Bool(false));
    assert_eq!(sample_value(&output, gate, "blocked"), RuntimeValue::Bool(true));
}

#[test]
fn condition_gate_hold_last_outputs_previous_passed_value() {
    let mut graph = AlchemistGraph::new();
    let value = graph.add_node(constant(RuntimeValue::Float(5.0))).unwrap();
    let condition = graph.add_node(property_node("condition")).unwrap();
    let mut gate_node = node("condition_gate");
    gate_node.config.set("mode", RuntimeValue::String("hold_last".into()));
    let gate = graph.add_node(gate_node).unwrap();
    graph
        .connect(OutputSocketRef::new(value, "value"), InputSocketRef::new(gate, "value"))
        .unwrap();
    graph
        .connect(
            OutputSocketRef::new(condition, "value"),
            InputSocketRef::new(gate, "condition"),
        )
        .unwrap();
    let schema = property_schema("condition", "bool", RuntimeValue::Bool(true));
    let compiled = compile_with_properties(&graph, &schema);
    let mut overrides = IndexMap::new();
    overrides.insert(FormulaPropertyId::new("condition"), RuntimeValue::Bool(true));
    let frame = RuntimePropertyFrame::with_overrides(&compiled.properties, &overrides).unwrap();
    let mut runtime = AlchemistRuntime::with_property_frame(compiled.clone(), frame);

    let first = evaluate(&mut runtime, 1);
    assert!(first.diagnostics.is_empty(), "{:?}", first.diagnostics);
    assert_eq!(sample_value(&first, gate, "value"), RuntimeValue::Float(5.0));

    overrides.insert(FormulaPropertyId::new("condition"), RuntimeValue::Bool(false));
    runtime.set_property_frame(RuntimePropertyFrame::with_overrides(&compiled.properties, &overrides).unwrap());
    let second = evaluate_capturing_unchanged_outputs(&mut runtime, 2);

    assert!(second.diagnostics.is_empty(), "{:?}", second.diagnostics);
    assert_eq!(sample_value(&second, gate, "value"), RuntimeValue::Float(5.0));
    assert_eq!(sample_value(&second, gate, "passed"), RuntimeValue::Bool(false));
    assert_eq!(sample_value(&second, gate, "blocked"), RuntimeValue::Bool(true));
}

#[test]
fn condition_gate_output_default_uses_default_input() {
    let mut graph = AlchemistGraph::new();
    let value = graph.add_node(constant(RuntimeValue::Float(5.0))).unwrap();
    let condition = graph.add_node(constant(RuntimeValue::Bool(false))).unwrap();
    let default = graph.add_node(constant(RuntimeValue::Float(9.0))).unwrap();
    let mut gate_node = node("condition_gate");
    gate_node
        .config
        .set("mode", RuntimeValue::String("output_default".into()));
    let gate = graph.add_node(gate_node).unwrap();
    graph
        .connect(OutputSocketRef::new(value, "value"), InputSocketRef::new(gate, "value"))
        .unwrap();
    graph
        .connect(
            OutputSocketRef::new(condition, "value"),
            InputSocketRef::new(gate, "condition"),
        )
        .unwrap();
    graph
        .connect(
            OutputSocketRef::new(default, "value"),
            InputSocketRef::new(gate, "default_value"),
        )
        .unwrap();
    let mut runtime = runtime(&graph);

    let output = evaluate(&mut runtime, 1);

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    assert_eq!(sample_value(&output, gate, "value"), RuntimeValue::Float(9.0));
}

#[test]
fn condition_gate_block_trigger_suppresses_fired_edge() {
    let mut graph = AlchemistGraph::new();
    let value = graph
        .add_node(constant(RuntimeValue::Trigger(TriggerValue::fired(7, 1))))
        .unwrap();
    let condition = graph.add_node(constant(RuntimeValue::Bool(false))).unwrap();
    let mut gate_node = node("condition_gate");
    gate_node
        .config
        .set("mode", RuntimeValue::String("block_trigger".into()));
    let gate = graph.add_node(gate_node).unwrap();
    graph
        .connect(OutputSocketRef::new(value, "value"), InputSocketRef::new(gate, "value"))
        .unwrap();
    graph
        .connect(
            OutputSocketRef::new(condition, "value"),
            InputSocketRef::new(gate, "condition"),
        )
        .unwrap();
    let mut runtime = runtime(&graph);

    let output = evaluate(&mut runtime, 1);

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let RuntimeValue::Trigger(trigger) = sample_value(&output, gate, "value") else {
        panic!("ConditionGate should output a trigger");
    };
    assert!(!trigger.fired);
    assert_eq!(trigger.edge_id, 7);
}

#[test]
fn condition_gate_whole_valueset_gate_uses_default_whole_value() {
    let value_type = ValueTypeId::new("example.value_set");
    let mut value_types = ValueTypeRegistry::with_primitives();
    value_types
        .register(ValueTypeDescriptor::new(
            value_type.clone(),
            "Value Set",
            ValueStorageKind::Extension,
            {
                let value_type = value_type.clone();
                move || RuntimeValue::Extension(ExtensionValue::new(value_type.clone(), Vec::<u8>::new()))
            },
        ))
        .unwrap();
    let input_value = RuntimeValue::Extension(ExtensionValue::new(value_type.clone(), vec![1_u8, 2, 3]));
    let default_value = RuntimeValue::Extension(ExtensionValue::new(value_type, vec![9_u8]));
    let mut graph = AlchemistGraph::new();
    let value = graph.add_node(constant(input_value)).unwrap();
    let condition = graph.add_node(constant(RuntimeValue::Bool(false))).unwrap();
    let default = graph.add_node(constant(default_value.clone())).unwrap();
    let gate = graph.add_node(node("condition_gate")).unwrap();
    graph
        .connect(OutputSocketRef::new(value, "value"), InputSocketRef::new(gate, "value"))
        .unwrap();
    graph
        .connect(
            OutputSocketRef::new(condition, "value"),
            InputSocketRef::new(gate, "condition"),
        )
        .unwrap();
    graph
        .connect(
            OutputSocketRef::new(default, "value"),
            InputSocketRef::new(gate, "default_value"),
        )
        .unwrap();
    let mut runtime = runtime_with_value_types(&graph, &value_types);

    let output = evaluate(&mut runtime, 1);

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    assert_eq!(sample_value(&output, gate, "value"), default_value);
}

#[test]
fn condition_gate_per_lane_application_reports_incompatible_mode() {
    let mut graph = AlchemistGraph::new();
    let value = graph.add_node(constant(RuntimeValue::Float(5.0))).unwrap();
    let condition = graph.add_node(constant(RuntimeValue::Bool(true))).unwrap();
    let mut gate_node = node("condition_gate");
    gate_node
        .config
        .set("gate_application", RuntimeValue::String("per_lane".into()));
    let gate = graph.add_node(gate_node).unwrap();
    graph
        .connect(OutputSocketRef::new(value, "value"), InputSocketRef::new(gate, "value"))
        .unwrap();
    graph
        .connect(
            OutputSocketRef::new(condition, "value"),
            InputSocketRef::new(gate, "condition"),
        )
        .unwrap();
    let mut runtime = runtime(&graph);

    let output = evaluate(&mut runtime, 1);

    assert_eq!(output.diagnostics.len(), 1);
    assert!(
        output.diagnostics[0]
            .message
            .contains("ConditionGate per-lane application requires lane-aware ValueSet lowering"),
        "{:?}",
        output.diagnostics
    );
}

#[test]
fn evaluate_compiled_graph_uses_supplied_memory() {
    let mut graph = AlchemistGraph::new();
    let source = graph.add_node(constant(RuntimeValue::Bool(true))).unwrap();
    let edge = graph.add_node(node("trigger_on_off")).unwrap();
    graph
        .connect(
            OutputSocketRef::new(source, "value"),
            InputSocketRef::new(edge, "value"),
        )
        .unwrap();
    let compiled = compile_with_properties(&graph, &FormulaPropertySchema::default());
    let mut memory = AlchemistMemory::for_graph(&compiled);
    let properties = RuntimePropertyFrame::from_defaults(&compiled.properties);
    let value_types = ValueTypeRegistry::with_primitives();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let inputs = RuntimeInputSnapshot::default();
    let context = RuntimeContextFrame::default_lane();

    let first_ctx = EvaluationCtx {
        logical_tick: 1,
        delta_time: Duration::from_millis(16),
        events: &[],
        inputs: &inputs,
        registries: &registries,
    };
    let mut first_debug = DebugCaptureSink::new(DebugCaptureMode::All { history_len: 64 });
    let first = evaluate_compiled_graph(
        &compiled,
        &mut memory,
        EvaluationFrame {
            ctx: &first_ctx,
            properties: &properties,
            context: &context,
            debug: &mut first_debug,
            force_process_unchanged_inputs: false,
            capture_unchanged_outputs: false,
        },
    );

    let second_ctx = EvaluationCtx {
        logical_tick: 2,
        delta_time: Duration::from_millis(16),
        events: &[],
        inputs: &inputs,
        registries: &registries,
    };
    let mut second_debug = DebugCaptureSink::default();
    let second = evaluate_compiled_graph(
        &compiled,
        &mut memory,
        EvaluationFrame {
            ctx: &second_ctx,
            properties: &properties,
            context: &context,
            debug: &mut second_debug,
            force_process_unchanged_inputs: false,
            capture_unchanged_outputs: false,
        },
    );

    let first_trigger = first
        .debug_samples
        .iter()
        .find_map(|sample| match sample.value {
            RuntimeValue::Trigger(value) => Some(value),
            _ => None,
        })
        .unwrap();
    assert!(first_trigger.fired);
    assert!(second.debug_samples.is_empty());
    assert_eq!(first_debug.samples(), first.debug_samples.as_slice());
}

#[test]
fn stateless_graph_has_no_persistent_state_memory() {
    let mut graph = AlchemistGraph::new();
    graph.add_node(constant(RuntimeValue::Float(1.0))).unwrap();
    let compiled = compile_with_properties(&graph, &FormulaPropertySchema::default());
    let memory = AlchemistMemory::for_graph(&compiled);

    assert!(memory.is_stateless());
    assert_eq!(memory.state_len(), 0);
    assert_eq!(memory.value_len(), 1);
}

#[test]
fn input_gated_stateless_lane_pool_keeps_process_cache() {
    let mut graph = AlchemistGraph::new();
    graph.add_node(constant(RuntimeValue::Float(1.0))).unwrap();
    let compiled = compile_with_properties(&graph, &FormulaPropertySchema::default());
    let mut lanes = LaneRuntimePool::for_graph(&compiled);

    assert!(!lanes.is_stateless());
    assert!(
        lanes
            .memory_for_key(ContextKey::single("device", "a"), &compiled)
            .is_some()
    );
    assert_eq!(lanes.memory_count(), 1);
}

#[test]
fn stateful_lane_pool_is_sparse_and_keyed_by_stable_context() {
    let mut graph = AlchemistGraph::new();
    let source = graph.add_node(constant(RuntimeValue::Bool(true))).unwrap();
    let edge = graph.add_node(node("trigger_on_off")).unwrap();
    graph
        .connect(
            OutputSocketRef::new(source, "value"),
            InputSocketRef::new(edge, "value"),
        )
        .unwrap();
    let compiled = compile_with_properties(&graph, &FormulaPropertySchema::default());
    let mut lanes = LaneRuntimePool::for_graph(&compiled);
    let a = ContextKey::single("device", "a");
    let b = ContextKey::single("device", "b");

    assert!(!lanes.is_stateless());
    assert_eq!(lanes.memory_count(), 0);
    assert!(lanes.memory_for_key(a.clone(), &compiled).is_some());
    assert_eq!(lanes.memory_count(), 1);
    assert!(lanes.memory_for_key(a, &compiled).is_some());
    assert_eq!(lanes.memory_count(), 1);
    assert!(lanes.memory_for_key(b.clone(), &compiled).is_some());
    assert_eq!(lanes.memory_count(), 2);

    let mut live_keys = IndexSet::new();
    live_keys.insert(b);
    lanes.retain_keys(&live_keys);
    assert_eq!(lanes.memory_count(), 1);
}

#[test]
fn pure_math_graph_evaluates_in_compiled_order() {
    let mut graph = AlchemistGraph::new();
    let left = graph.add_node(constant(RuntimeValue::Float(2.0))).unwrap();
    let right = graph.add_node(constant(RuntimeValue::Float(3.0))).unwrap();
    let add = graph.add_node(node("math")).unwrap();
    graph
        .connect(OutputSocketRef::new(left, "value"), InputSocketRef::new(add, "value1"))
        .unwrap();
    graph
        .connect(OutputSocketRef::new(right, "value"), InputSocketRef::new(add, "value2"))
        .unwrap();
    let mut runtime = runtime(&graph);
    let add_exec = runtime
        .compiled
        .debug_map
        .exec_to_authored
        .iter()
        .position(|node| *node == add)
        .map(|index| crate::ExecNodeId::new(index as u32))
        .unwrap();

    let output = evaluate(&mut runtime, 1);

    assert!(output.diagnostics.is_empty());
    assert!(
        output
            .debug_samples
            .iter()
            .any(|sample| { sample.exec_node == add_exec && sample.value == RuntimeValue::Float(5.0) })
    );
    assert_eq!(runtime.execution_count(add_exec), 1);
}

#[test]
fn idle_pure_graph_skips_all_exec_nodes_after_initial_evaluation() {
    let mut graph = AlchemistGraph::new();
    let left = graph.add_node(constant(RuntimeValue::Float(2.0))).unwrap();
    let right = graph.add_node(constant(RuntimeValue::Float(3.0))).unwrap();
    let add = graph.add_node(node("math")).unwrap();
    graph
        .connect(OutputSocketRef::new(left, "value"), InputSocketRef::new(add, "value1"))
        .unwrap();
    graph
        .connect(OutputSocketRef::new(right, "value"), InputSocketRef::new(add, "value2"))
        .unwrap();
    let mut runtime = runtime(&graph);
    let exec_ids = runtime
        .compiled
        .debug_map
        .exec_to_authored
        .iter()
        .enumerate()
        .filter_map(|(index, node)| ([left, right, add].contains(node)).then_some(crate::ExecNodeId::new(index as u32)))
        .collect::<Vec<_>>();

    let first = evaluate(&mut runtime, 1);
    assert!(first.diagnostics.is_empty(), "{:?}", first.diagnostics);
    assert_eq!(runtime.memory.last_executed_nodes().len(), exec_ids.len());

    let second = evaluate(&mut runtime, 2);

    assert!(second.diagnostics.is_empty(), "{:?}", second.diagnostics);
    assert!(
        runtime.memory.last_executed_nodes().is_empty(),
        "unchanged pure graph should not execute nodes on an idle tick"
    );
    for exec_id in exec_ids {
        assert_eq!(runtime.execution_count(exec_id), 1);
    }
}

#[test]
fn disabled_single_input_output_node_bypasses_matching_value_type() {
    let mut graph = AlchemistGraph::new();
    let source = graph.add_node(constant(RuntimeValue::Float(4.0))).unwrap();
    let mut negate_node = node("negate");
    negate_node.enabled = false;
    let negate = graph.add_node(negate_node).unwrap();
    graph
        .connect(
            OutputSocketRef::new(source, "value"),
            InputSocketRef::new(negate, "value"),
        )
        .unwrap();
    let mut runtime = runtime(&graph);
    let negate_exec = runtime
        .compiled
        .debug_map
        .exec_to_authored
        .iter()
        .position(|node| *node == negate)
        .map(|index| crate::ExecNodeId::new(index as u32))
        .unwrap();

    let output = evaluate(&mut runtime, 1);

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    assert!(
        output
            .debug_samples
            .iter()
            .any(|sample| { sample.exec_node == negate_exec && sample.value == RuntimeValue::Float(4.0) })
    );
}

#[test]
fn disabled_effect_node_is_noop() {
    let mut graph = AlchemistGraph::new();
    let source = graph.add_node(constant(RuntimeValue::String("hello".into()))).unwrap();
    let mut log_node = node("debug_log");
    log_node.enabled = false;
    let log = graph.add_node(log_node).unwrap();
    graph
        .connect(OutputSocketRef::new(source, "value"), InputSocketRef::new(log, "value"))
        .unwrap();
    let mut runtime = runtime(&graph);

    let output = evaluate(&mut runtime, 1);

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    assert!(output.intents.is_empty(), "{:?}", output.intents);
}

#[test]
fn forced_float_math_coerces_vec3_input_at_runtime() {
    let mut graph = AlchemistGraph::new();
    let source = graph.add_node(constant(RuntimeValue::Vec3([2.0, 4.0, 8.0]))).unwrap();
    let mut add_node = node("math");
    add_node.forced_type_bindings.insert(
        TypeVar::new("TNumeric"),
        ValueTypeId::new("float"),
        TypeBindingSource::ForcedByUser,
    );
    let add = graph.add_node(add_node).unwrap();
    graph
        .connect(
            OutputSocketRef::new(source, "value"),
            InputSocketRef::new(add, "value1"),
        )
        .unwrap();
    let mut runtime = runtime(&graph);
    let add_exec = runtime
        .compiled
        .debug_map
        .exec_to_authored
        .iter()
        .position(|node| *node == add)
        .map(|index| crate::ExecNodeId::new(index as u32))
        .unwrap();
    let add_inputs = &runtime.compiled.exec_nodes[add_exec.index()].inputs;
    assert!(
        matches!(
            &add_inputs[0],
            InputValueSource::Converted { target_type, .. } if *target_type == ValueTypeId::new("float")
        ),
        "{add_inputs:?}"
    );

    let output = evaluate(&mut runtime, 1);

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    assert!(
        output
            .debug_samples
            .iter()
            .any(|sample| sample.exec_node == add_exec && sample.value == RuntimeValue::Float(2.0))
    );
}

#[test]
fn edge_trigger_fires_once_and_preserves_state() {
    let mut graph = AlchemistGraph::new();
    let source = graph.add_node(constant(RuntimeValue::Bool(true))).unwrap();
    let edge = graph.add_node(node("trigger_on_off")).unwrap();
    graph
        .connect(
            OutputSocketRef::new(source, "value"),
            InputSocketRef::new(edge, "value"),
        )
        .unwrap();
    let mut runtime = runtime(&graph);
    let edge_exec = runtime
        .compiled
        .debug_map
        .exec_to_authored
        .iter()
        .position(|node| *node == edge)
        .map(|index| crate::ExecNodeId::new(index as u32))
        .unwrap();
    let on_slot = runtime.compiled.exec_nodes[edge_exec.index()].outputs[0];

    let first = evaluate(&mut runtime, 10);
    let second = evaluate(&mut runtime, 11);
    let first_trigger = first
        .debug_samples
        .iter()
        .find_map(|sample| match sample.value {
            RuntimeValue::Trigger(value) if sample.output_slot == on_slot => Some(value),
            _ => None,
        })
        .unwrap();
    let second_trigger = second.debug_samples.iter().find_map(|sample| match sample.value {
        RuntimeValue::Trigger(value) if sample.output_slot == on_slot => Some(value),
        _ => None,
    });

    assert_eq!(
        first_trigger,
        TriggerValue {
            fired: true,
            edge_id: 1,
            logical_tick: 10,
        }
    );
    assert!(
        second_trigger.is_none_or(|trigger| !trigger.fired),
        "{second_trigger:?}"
    );
}

#[test]
fn metronome_on_output_toggles_without_tap_input() {
    let mut graph = AlchemistGraph::new();
    let mut metronome = node("metronome");
    metronome.config.set("mode", RuntimeValue::String("time".into()));
    metronome.config.set("value", RuntimeValue::Float(0.064));
    metronome.config.set("on_ratio", RuntimeValue::Float(0.5));
    let metronome = graph.add_node(metronome).unwrap();
    let mut runtime = runtime(&graph);
    let on_socket = SocketId::new("on");
    let mut seen_on_values = Vec::new();

    for logical_tick in 1..=6 {
        let output = evaluate(&mut runtime, logical_tick);
        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        seen_on_values.extend(output.debug_samples.iter().filter_map(|sample| {
            if sample.author_node_id != metronome || sample.output_socket != on_socket {
                return None;
            }
            match &sample.value {
                RuntimeValue::Bool(value) => Some(*value),
                _ => None,
            }
        }));
    }

    assert!(seen_on_values.contains(&true), "{seen_on_values:?}");
    assert!(seen_on_values.contains(&false), "{seen_on_values:?}");
}

#[test]
fn counter_add_trigger_accumulates_default_amount() {
    let mut graph = AlchemistGraph::new();
    let source = graph
        .add_node(constant(RuntimeValue::Trigger(TriggerValue::fired(7, 1))))
        .unwrap();
    let mut counter_node = node("counter");
    counter_node
        .config
        .set(PROCESS_ON_INPUT_CHANGE_ONLY_CONFIG, RuntimeValue::Bool(false));
    let counter = graph.add_node(counter_node).unwrap();
    graph
        .connect(
            OutputSocketRef::new(source, "value"),
            InputSocketRef::new(counter, "add"),
        )
        .unwrap();
    let mut runtime = runtime(&graph);
    let counter_exec = runtime
        .compiled
        .debug_map
        .exec_to_authored
        .iter()
        .position(|node| *node == counter)
        .map(|index| crate::ExecNodeId::new(index as u32))
        .unwrap();
    assert!(
        runtime.compiled.exec_nodes[counter_exec.index()].process_on_input_change_only,
        "Counter must stay input-change driven even with stale always-process config"
    );

    let first = evaluate_with_capture_mode(&mut runtime, 1, DebugCaptureMode::All { history_len: 64 });
    let second = evaluate_with_capture_mode(&mut runtime, 2, DebugCaptureMode::All { history_len: 64 });

    assert!(
        first
            .debug_samples
            .iter()
            .any(|sample| sample.exec_node == counter_exec && sample.value == RuntimeValue::Float(1.0))
    );
    assert!(
        second
            .debug_samples
            .iter()
            .all(|sample| sample.exec_node != counter_exec),
        "Counter should not process again until an input changes"
    );
}

#[test]
fn color_conversion_nodes_pack_and_extract_channels() {
    let mut graph = AlchemistGraph::new();
    let mut convert = node("convert_to_color");
    convert.config.set("mode", RuntimeValue::String("hsva".into()));
    convert
        .input_defaults
        .insert(SocketId::new("h"), RuntimeValue::Float(120.0));
    convert
        .input_defaults
        .insert(SocketId::new("s"), RuntimeValue::Float(1.0));
    convert
        .input_defaults
        .insert(SocketId::new("v"), RuntimeValue::Float(1.0));
    convert
        .input_defaults
        .insert(SocketId::new("a"), RuntimeValue::Float(0.5));
    let convert = graph.add_node(convert).unwrap();
    let mut extract = node("extract_color");
    extract.config.set("mode", RuntimeValue::String("cmyka".into()));
    let extract = graph.add_node(extract).unwrap();
    graph
        .connect(
            OutputSocketRef::new(convert, "color"),
            InputSocketRef::new(extract, "color"),
        )
        .unwrap();
    let mut runtime = runtime(&graph);
    let convert_exec = runtime
        .compiled
        .debug_map
        .exec_to_authored
        .iter()
        .position(|node| *node == convert)
        .map(|index| crate::ExecNodeId::new(index as u32))
        .unwrap();
    let extract_exec = runtime
        .compiled
        .debug_map
        .exec_to_authored
        .iter()
        .position(|node| *node == extract)
        .map(|index| crate::ExecNodeId::new(index as u32))
        .unwrap();

    let output = evaluate(&mut runtime, 1);

    assert!(output.debug_samples.iter().any(|sample| {
        sample.exec_node == convert_exec
            && sample.value
                == RuntimeValue::Color(ColorValue {
                    red: 0.0,
                    green: 1.0,
                    blue: 0.0,
                    alpha: 0.5,
                })
    }));
    assert!(
        output
            .debug_samples
            .iter()
            .any(|sample| sample.exec_node == extract_exec && sample.value == RuntimeValue::Float(1.0))
    );
}

#[test]
fn log_config_emits_debug_intent_for_processed_node() {
    let mut graph = AlchemistGraph::new();
    let mut source = constant(RuntimeValue::Float(4.0));
    source.config.set("log", RuntimeValue::Bool(true));
    let source = graph.add_node(source).unwrap();
    let mut runtime = runtime(&graph);

    let output = evaluate(&mut runtime, 7);

    assert_eq!(output.intents.len(), 1);
    assert_eq!(output.intents[0].kind.as_ref(), "debug.log");
    assert_eq!(output.intents[0].source_node, Some(source));
    assert_eq!(output.intents[0].source_socket, Some(SocketId::new("value")));
    assert_eq!(output.intents[0].payload, RuntimeValue::Float(4.0));
    assert_eq!(output.intents[0].logical_tick, 7);
}

#[test]
fn runtime_diagnostics_propagate_node_failures() {
    let mut graph = AlchemistGraph::new();
    graph.add_node(node("remap")).unwrap();
    let mut runtime = runtime(&graph);

    let output = evaluate(&mut runtime, 1);

    assert_eq!(output.diagnostics.len(), 1);
    assert!(output.diagnostics[0].message.contains("range cannot be zero"));
}

#[test]
fn effect_node_emits_intent_without_dispatching_side_effect() {
    let mut graph = AlchemistGraph::new();
    let source = graph.add_node(constant(RuntimeValue::String("hello".into()))).unwrap();
    let log = graph.add_node(node("debug_log")).unwrap();
    graph
        .connect(OutputSocketRef::new(source, "value"), InputSocketRef::new(log, "value"))
        .unwrap();
    let mut runtime = runtime(&graph);

    let output = evaluate(&mut runtime, 7);

    assert_eq!(output.intents.len(), 1);
    assert_eq!(output.intents[0].kind.as_ref(), "debug.log");
    assert_eq!(output.intents[0].logical_tick, 7);
}

#[test]
fn property_node_reads_default_from_runtime_frame() {
    let mut graph = AlchemistGraph::new();
    let mut property = property_node("amount");
    property.config.set("value", RuntimeValue::Float(99.0));
    graph.add_node(property).unwrap();
    let schema = property_schema("amount", "float", RuntimeValue::Float(1.5));
    let mut runtime = runtime_with_properties(&graph, Some(&schema));

    let output = evaluate(&mut runtime, 1);

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    assert!(
        output
            .debug_samples
            .iter()
            .any(|sample| sample.value == RuntimeValue::Float(1.5))
    );
}

#[test]
fn formula_default_preview_shows_node_output_values() {
    let mut graph = AlchemistGraph::new();
    let source = graph.add_node(constant(RuntimeValue::Float(4.25))).unwrap();
    let mut runtime = runtime(&graph);
    let formula_id = FormulaId::new("preview_formula");

    let output = evaluate_with_capture_mode(
        &mut runtime,
        42,
        DebugCaptureMode::FormulaDefaults {
            formula_id: formula_id.clone(),
            history_len: 8,
        },
    );

    assert_eq!(output.debug_samples.len(), 1);
    let sample = &output.debug_samples[0];
    assert_eq!(sample.formula_id.as_ref(), Some(&formula_id));
    assert_eq!(sample.context_key, None);
    assert_eq!(sample.author_node_id, source);
    assert_eq!(sample.output_socket, SocketId::new("value"));
    assert_eq!(sample.value_type, ValueTypeId::new("float"));
    assert_eq!(sample.value, RuntimeValue::Float(4.25));
    assert_eq!(sample.logical_tick, 42);
    assert_eq!(sample.status, OutputPreviewStatus::DefaultPreview);
}

#[test]
fn preview_capture_off_when_editor_not_visible() {
    let mut graph = AlchemistGraph::new();
    graph.add_node(constant(RuntimeValue::Float(1.0))).unwrap();
    let mut runtime = runtime(&graph);

    let output = evaluate_with_capture_mode(&mut runtime, 1, DebugCaptureMode::Off);

    assert!(output.debug_samples.is_empty());
}

#[test]
fn output_preview_history_is_bounded() {
    let mut graph = AlchemistGraph::new();
    graph.add_node(constant(RuntimeValue::Float(1.0))).unwrap();
    graph.add_node(constant(RuntimeValue::Float(2.0))).unwrap();
    let mut runtime = runtime(&graph);

    let output = evaluate_with_capture_mode(&mut runtime, 1, DebugCaptureMode::All { history_len: 1 });

    assert_eq!(output.debug_samples.len(), 1);
    assert_eq!(output.debug_samples[0].value, RuntimeValue::Float(2.0));
}

#[test]
fn property_node_reads_processor_override_from_runtime_frame() {
    let mut graph = AlchemistGraph::new();
    graph.add_node(property_node("amount")).unwrap();
    let schema = property_schema("amount", "float", RuntimeValue::Float(1.5));
    let compiled = compile_with_properties(&graph, &schema);
    let mut overrides = IndexMap::new();
    overrides.insert(FormulaPropertyId::new("amount"), RuntimeValue::Float(2.5));
    let frame = RuntimePropertyFrame::with_overrides(&compiled.properties, &overrides).unwrap();
    let mut runtime = AlchemistRuntime::with_property_frame(compiled, frame);

    let output = evaluate(&mut runtime, 1);

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    assert!(
        output
            .debug_samples
            .iter()
            .any(|sample| sample.value == RuntimeValue::Float(2.5))
    );
}

#[test]
fn property_frame_change_reevaluates_property_dependents_only() {
    let mut graph = AlchemistGraph::new();
    let property = graph.add_node(property_node("amount")).unwrap();
    let base = graph.add_node(constant(RuntimeValue::Float(10.0))).unwrap();
    let unrelated = graph.add_node(constant(RuntimeValue::Float(99.0))).unwrap();
    let add = graph.add_node(node("math")).unwrap();
    graph
        .connect(
            OutputSocketRef::new(property, "value"),
            InputSocketRef::new(add, "value1"),
        )
        .unwrap();
    graph
        .connect(OutputSocketRef::new(base, "value"), InputSocketRef::new(add, "value2"))
        .unwrap();
    let schema = property_schema("amount", "float", RuntimeValue::Float(1.0));
    let compiled = compile_with_properties(&graph, &schema);
    let mut overrides = IndexMap::new();
    overrides.insert(FormulaPropertyId::new("amount"), RuntimeValue::Float(1.0));
    let frame = RuntimePropertyFrame::with_overrides(&compiled.properties, &overrides).unwrap();
    let mut runtime = AlchemistRuntime::with_property_frame(compiled.clone(), frame);
    let first = evaluate(&mut runtime, 1);
    assert!(first.diagnostics.is_empty(), "{:?}", first.diagnostics);

    overrides.insert(FormulaPropertyId::new("amount"), RuntimeValue::Float(2.0));
    runtime.set_property_frame(RuntimePropertyFrame::with_overrides(&compiled.properties, &overrides).unwrap());
    let second = evaluate(&mut runtime, 2);

    assert!(second.diagnostics.is_empty(), "{:?}", second.diagnostics);
    assert!(
        second
            .debug_samples
            .iter()
            .any(|sample| { sample.author_node_id == property && sample.value == RuntimeValue::Float(2.0) })
    );
    assert!(
        second
            .debug_samples
            .iter()
            .any(|sample| { sample.author_node_id == add && sample.value == RuntimeValue::Float(12.0) })
    );
    assert!(!second.debug_samples.iter().any(|sample| sample.author_node_id == base));
    assert!(
        !second
            .debug_samples
            .iter()
            .any(|sample| sample.author_node_id == unrelated)
    );
}

#[test]
fn processor_override_rejects_invalid_property_type() {
    let mut graph = AlchemistGraph::new();
    graph.add_node(property_node("amount")).unwrap();
    let schema = property_schema("amount", "float", RuntimeValue::Float(1.5));
    let compiled = compile_with_properties(&graph, &schema);
    let mut overrides = IndexMap::new();
    overrides.insert(FormulaPropertyId::new("amount"), RuntimeValue::Bool(true));

    let result = RuntimePropertyFrame::with_overrides(&compiled.properties, &overrides);

    assert!(matches!(
        result,
        Err(crate::RuntimePropertyFrameError::InvalidOverrideType { .. })
    ));
}

#[test]
fn changing_processor_override_does_not_recompile_formula() {
    let mut graph = AlchemistGraph::new();
    graph.add_node(property_node("amount")).unwrap();
    let schema = property_schema("amount", "float", RuntimeValue::Float(1.5));
    let compiled = compile_with_properties(&graph, &schema);

    let mut first_overrides = IndexMap::new();
    first_overrides.insert(FormulaPropertyId::new("amount"), RuntimeValue::Float(2.0));
    let first_frame = RuntimePropertyFrame::with_overrides(&compiled.properties, &first_overrides).unwrap();
    let mut first_runtime = AlchemistRuntime::with_property_frame(compiled.clone(), first_frame);

    let mut second_overrides = IndexMap::new();
    second_overrides.insert(FormulaPropertyId::new("amount"), RuntimeValue::Float(3.0));
    let second_frame = RuntimePropertyFrame::with_overrides(&compiled.properties, &second_overrides).unwrap();
    let mut second_runtime = AlchemistRuntime::with_property_frame(compiled.clone(), second_frame);

    let first_output = evaluate(&mut first_runtime, 1);
    let second_output = evaluate(&mut second_runtime, 1);

    assert!(std::sync::Arc::ptr_eq(
        &first_runtime.compiled,
        &second_runtime.compiled
    ));
    assert!(
        first_output
            .debug_samples
            .iter()
            .any(|sample| sample.value == RuntimeValue::Float(2.0))
    );
    assert!(
        second_output
            .debug_samples
            .iter()
            .any(|sample| sample.value == RuntimeValue::Float(3.0))
    );
}
