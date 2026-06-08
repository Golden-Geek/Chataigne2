use std::time::Duration;

use golden_alchemist::{
    ANodeInstance, ANodeTypeId, AlchemistGraph, AlchemistRuntime, CompileCtx, EvaluationCtx, InputSocketRef,
    OutputSocketRef, RuntimeInputSnapshot, RuntimeRegistries, RuntimeValue, StableRef, ValueTypeId, ValueTypeRegistry,
    compile_graph, primitive_node_registry,
};

use crate::alchemist::{MODULE_ENDPOINT_TYPE, MODULE_TYPE, STATE_TYPE, register_nodes, register_value_types};

fn node(id: &str) -> ANodeInstance {
    ANodeInstance::new(ANodeTypeId::new(id), id)
}

#[test]
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
fn chataigne_values_are_registered_through_facets() {
    let mut registry = ValueTypeRegistry::with_primitives();
    register_value_types(&mut registry).unwrap();

    assert!(registry.supports_facet(
        &ValueTypeId::new(MODULE_TYPE),
        &golden_alchemist::FacetId::new("command_target")
    ));
}
