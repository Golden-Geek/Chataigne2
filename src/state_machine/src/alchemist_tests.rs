use std::time::Duration;

use golden_alchemist::{
    ANodeInstance, ANodeTypeId, AlchemistGraph, AlchemistRuntime, CompileCtx, CompileResult, DiagnosticOrigin,
    EvaluationCtx, InputSocketRef, OutputSocketRef, RuntimeInputSnapshot, RuntimeRegistries, RuntimeValue, StableRef,
    ValueTypeId, ValueTypeRegistry, compile_graph, primitive_node_registry,
};

use crate::alchemist::{
    CONDITIONS_MANAGER_TYPE, INPUTS_MANAGER_TYPE, MODULE_ENDPOINT_TYPE, MODULE_TYPE, OUTPUTS_MANAGER_TYPE,
    ROUTING_TYPE, STATE_TYPE, register_nodes, register_value_types,
};

fn node(id: &str) -> ANodeInstance {
    ANodeInstance::new(ANodeTypeId::new(id), id)
}

fn compile_single_node(type_id: &str) -> CompileResult {
    let mut graph = AlchemistGraph::new();
    graph.add_node(node(type_id)).unwrap();
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

#[test]
fn manager_reference_nodes_compile_as_explicit_unsupported_diagnostics() {
    for (node_type, label, required_behavior) in [
        (CONDITIONS_MANAGER_TYPE, "Conditions", "condition manager evaluation"),
        (INPUTS_MANAGER_TYPE, "Inputs", "ParamArray resolution"),
        (OUTPUTS_MANAGER_TYPE, "Output Commands", "lane-aware processor intents"),
    ] {
        let result = compile_single_node(node_type);
        assert!(result.compiled.is_none(), "{node_type} must not compile silently");
        assert!(result.has_errors(), "{node_type} must report a compile error");
        assert_eq!(
            result.diagnostics.len(),
            1,
            "{node_type} should emit one explicit diagnostic"
        );
        let diagnostic = &result.diagnostics[0];
        assert_eq!(diagnostic.code, "chataigne_manager_node_unsupported");
        assert!(
            diagnostic.message.contains(label),
            "{node_type} diagnostic should name the manager role: {:?}",
            diagnostic.message
        );
        assert!(
            diagnostic.message.contains(required_behavior),
            "{node_type} diagnostic should describe the missing real behavior: {:?}",
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
    }
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
