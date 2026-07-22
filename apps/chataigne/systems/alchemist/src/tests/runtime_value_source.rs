use std::time::Duration;

use crate::test_support::TestGraph;
use crate::{
    ANodeId, ANodeInstance, ANodeTypeId, AlchemistRuntime, ColorValue, CompileCtx, DebugCaptureMode, EvaluationCtx,
    FormulaPropertyDecl, FormulaPropertyId, FormulaPropertySchema, InputSocketRef, OutputSocketRef, PropertyUiHints,
    RuntimeOutput, RuntimeRegistries, RuntimeValue, StableRef, TriggerValue, ValueTypeId, ValueTypeRegistry,
    compile_graph, primitive_node_registry,
};

fn primitive_values() -> Vec<RuntimeValue> {
    vec![
        RuntimeValue::Unit,
        RuntimeValue::Bool(true),
        RuntimeValue::Trigger(TriggerValue::default()),
        RuntimeValue::Int(42),
        RuntimeValue::Float(3.5),
        RuntimeValue::String("source".into()),
        RuntimeValue::Vec2([1.0, 2.0]),
        RuntimeValue::Vec3([1.0, 2.0, 3.0]),
        RuntimeValue::Color(ColorValue {
            red: 0.1,
            green: 0.2,
            blue: 0.3,
            alpha: 0.4,
        }),
        RuntimeValue::Duration(Duration::from_millis(750)),
    ]
}

fn node(type_id: &str) -> ANodeInstance {
    ANodeInstance::new(ANodeTypeId::new(type_id), type_id)
}

fn compile(graph: &TestGraph, properties: Option<&FormulaPropertySchema>) -> AlchemistRuntime {
    let value_types = ValueTypeRegistry::with_primitives();
    let nodes = primitive_node_registry();
    let result = compile_graph(
        &graph.to_document(),
        &CompileCtx {
            value_types: &value_types,
            nodes: &nodes,
            properties,
        },
    );
    assert!(!result.has_errors(), "{:?}", result.diagnostics);
    AlchemistRuntime::new(result.compiled.unwrap())
}

fn evaluate(runtime: &mut AlchemistRuntime) -> RuntimeOutput {
    let value_types = ValueTypeRegistry::with_primitives();
    runtime.evaluate_with_capture_mode(
        &EvaluationCtx {
            logical_tick: 1,
            delta_time: Duration::from_millis(16),
            events: &[],
            inputs: &Default::default(),
            registries: &RuntimeRegistries {
                value_types: &value_types,
            },
        },
        DebugCaptureMode::All { history_len: 64 },
    )
}

fn assert_source_output(output: &RuntimeOutput, node: ANodeId, expected: &RuntimeValue) {
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    assert!(
        output.debug_samples.iter().any(|sample| {
            sample.author_node_id == node
                && sample.output_socket.as_str() == "value"
                && &sample.value == expected
                && sample.value_type == expected.value_type()
        }),
        "expected {expected:?}, got {:?}",
        output.debug_samples
    );
}

#[test]
fn constant_source_matrix_preserves_every_primitive_value() {
    for expected in primitive_values() {
        let mut graph = TestGraph::new();
        let mut constant = node("constant");
        constant.config.set("value", expected.clone());
        let constant = graph.add_node(constant).unwrap();
        let mut runtime = compile(&graph, None);

        let output = evaluate(&mut runtime);

        assert_source_output(&output, constant, &expected);
    }

    let mut graph = TestGraph::new();
    let constant = graph.add_node(node("constant")).unwrap();
    let mut runtime = compile(&graph, None);
    let output = evaluate(&mut runtime);
    assert_source_output(&output, constant, &RuntimeValue::Float(0.0));
}

#[test]
fn property_source_matrix_reads_every_primitive_schema_default() {
    for (index, expected) in primitive_values().into_iter().enumerate() {
        let property_id = FormulaPropertyId::new(format!("property_{index}"));
        let mut schema = FormulaPropertySchema::default();
        schema.insert(FormulaPropertyDecl {
            id: property_id.clone(),
            label: format!("Property {index}"),
            description: None,
            value_type: expected.value_type(),
            default_value: expected.clone(),
            ui: PropertyUiHints::default(),
        });
        let mut property = node("property");
        property.config.set(
            "property_id",
            RuntimeValue::Ref(StableRef::new(ValueTypeId::new("property"), property_id.as_str())),
        );
        let mut graph = TestGraph::new();
        let property = graph.add_node(property).unwrap();
        let mut runtime = compile(&graph, Some(&schema));

        let output = evaluate(&mut runtime);

        assert_source_output(&output, property, &expected);
    }
}

#[test]
fn debug_value_passes_every_primitive_value_without_effects() {
    for expected in primitive_values() {
        let mut graph = TestGraph::new();
        let mut constant = node("constant");
        constant.config.set("value", expected.clone());
        let constant = graph.add_node(constant).unwrap();
        let debug = graph.add_node(node("debug_value")).unwrap();
        graph
            .connect(
                OutputSocketRef::new(constant, "value"),
                InputSocketRef::new(debug, "value"),
            )
            .unwrap();
        let mut runtime = compile(&graph, None);

        let output = evaluate(&mut runtime);

        assert_source_output(&output, debug, &expected);
        assert!(output.intents.is_empty(), "{:?}", output.intents);
    }
}

#[test]
fn debug_log_emits_every_primitive_value_as_an_intent_only() {
    for expected in primitive_values() {
        let mut graph = TestGraph::new();
        let mut constant = node("constant");
        constant.config.set("value", expected.clone());
        let constant = graph.add_node(constant).unwrap();
        let debug_log = graph.add_node(node("debug_log")).unwrap();
        graph
            .connect(
                OutputSocketRef::new(constant, "value"),
                InputSocketRef::new(debug_log, "value"),
            )
            .unwrap();
        let mut runtime = compile(&graph, None);

        let output = evaluate(&mut runtime);

        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        assert_eq!(output.intents.len(), 1, "{:?}", output.intents);
        let intent = &output.intents[0];
        assert_eq!(intent.kind.as_ref(), "debug.log");
        assert_eq!(intent.source_node, Some(debug_log));
        assert_eq!(intent.source_socket, None);
        assert_eq!(intent.target, None);
        assert_eq!(intent.payload, expected);
        assert_eq!(intent.logical_tick, 1);
        assert!(
            !output
                .debug_samples
                .iter()
                .any(|sample| sample.author_node_id == debug_log),
            "effect-only Debug Log must not fabricate an output sample"
        );
    }
}
