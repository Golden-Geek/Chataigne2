use std::time::Duration;

use golden_alchemist::{
    ANodeInstance, ANodeTypeId, AlchemistFormula, AlchemistGraph, CompileCtx, EvaluationCtx, FormulaContextContract,
    FormulaId, FormulaPropertySchema, FormulaSurface, InputSocketRef, OutputSocketRef, RuntimeInputSnapshot,
    RuntimeRegistries, RuntimeValue, ValueTypeRegistry, primitive_node_registry,
};
use golden_statechart::Statechart;

use crate::{
    ChataigneStateMachine, ChataigneStateMachineRuntime, Processor,
    alchemist::{register_nodes, register_value_types},
};

fn constant_formula() -> AlchemistFormula {
    let mut graph = AlchemistGraph::new();
    let mut node = ANodeInstance::new(ANodeTypeId::new("constant"), "Constant");
    node.config.set("value", RuntimeValue::Float(1.0));
    graph.add_node(node).unwrap();
    AlchemistFormula {
        id: FormulaId::new("constant"),
        version: 1,
        label: "Constant".into(),
        description: None,
        tags: Vec::new(),
        graph,
        properties: FormulaPropertySchema::default(),
        surface: FormulaSurface::default(),
        context_contract: FormulaContextContract::default(),
        migrations: Vec::new(),
    }
}

#[test]
fn state_transition_updates_active_processor_matrix() {
    let mut chart = Statechart::new();
    let first = chart.add_leaf(chart.root_region, "First").unwrap();
    let second = chart.add_leaf(chart.root_region, "Second").unwrap();
    chart.set_initial(chart.root_region, first).unwrap();
    let transition = chart.add_transition(first, second, 0).unwrap();
    let mut machine = ChataigneStateMachine::new(chart);
    let formula = constant_formula();
    let first_processor = Processor::from_formula("First Processor", &formula);
    let first_id = first_processor.id;
    let second_processor = Processor::from_formula("Second Processor", &formula);
    let second_id = second_processor.id;
    machine.add_formula(formula);
    machine.add_processor(first, first_processor).unwrap();
    machine.add_processor(second, second_processor).unwrap();

    let mut guard = AlchemistGraph::new();
    let mut source = ANodeInstance::new(ANodeTypeId::new("constant"), "True");
    source.config.set("value", RuntimeValue::Bool(true));
    let source = guard.add_node(source).unwrap();
    let edge = guard
        .add_node(ANodeInstance::new(ANodeTypeId::new("edge"), "Edge"))
        .unwrap();
    guard
        .connect(
            OutputSocketRef::new(source, "value"),
            InputSocketRef::new(edge, "value"),
        )
        .unwrap();
    machine.set_transition_graphs(transition, Some(guard), None);

    let mut value_types = ValueTypeRegistry::with_primitives();
    register_value_types(&mut value_types).unwrap();
    let mut nodes = primitive_node_registry();
    register_nodes(&mut nodes).unwrap();
    let mut runtime = ChataigneStateMachineRuntime::compile(
        &machine,
        &CompileCtx {
            value_types: &value_types,
            nodes: &nodes,
            properties: None,
        },
    )
    .unwrap();
    runtime.initialize(&mut machine).unwrap();
    assert_eq!(runtime.execution.active_processors, vec![first_id]);
    let inputs = RuntimeInputSnapshot::default();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };

    let output = runtime
        .tick(
            &mut machine,
            &EvaluationCtx {
                logical_tick: 1,
                delta_time: Duration::ZERO,
                events: &[],
                inputs: &inputs,
                registries: &registries,
            },
        )
        .unwrap();

    assert!(output.transition.is_some());
    assert_eq!(runtime.execution.active_processors, vec![second_id]);
    assert!(output.processor_outputs.contains_key(&second_id));
    assert!(!output.processor_outputs.contains_key(&first_id));
}
