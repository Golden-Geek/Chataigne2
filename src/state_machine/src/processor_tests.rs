use std::time::Duration;

use golden_alchemist::{
    ANodeInstance, ANodeTypeId, AlchemistGraph, CompileCtx, EvaluationCtx, RuntimeInputSnapshot, RuntimeRegistries,
    RuntimeValue, ValueTypeRegistry, primitive_node_registry,
};

use crate::{ProcessorLifecycleEvent, ProcessorNode, ProcessorRuntime};

fn processor() -> ProcessorNode {
    let mut graph = AlchemistGraph::new();
    let mut constant = ANodeInstance::new(ANodeTypeId::new("constant"), "Constant");
    constant.config.set("value", RuntimeValue::Float(1.0));
    graph.add_node(constant).unwrap();
    ProcessorNode::new("Processor", graph)
}

#[test]
fn processor_compiles_and_evaluates_only_while_active() {
    let processor = processor();
    let mut runtime = ProcessorRuntime::new(processor.id);
    let value_types = ValueTypeRegistry::with_primitives();
    let nodes = primitive_node_registry();
    assert!(runtime.compile(
        &processor,
        &CompileCtx {
            value_types: &value_types,
            nodes: &nodes,
        }
    ));
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let inputs = RuntimeInputSnapshot::default();
    let ctx = EvaluationCtx {
        logical_tick: 1,
        delta_time: Duration::ZERO,
        events: &[],
        inputs: &inputs,
        registries: &registries,
    };

    assert!(runtime.evaluate(&ctx).debug_samples.is_empty());
    runtime.apply_lifecycle(
        &processor,
        ProcessorLifecycleEvent::StateEnter(golden_statechart::StateId::new()),
    );
    assert_eq!(runtime.evaluate(&ctx).debug_samples.len(), 1);
}

#[test]
fn exposed_surface_is_present_in_ui_model() {
    let mut processor = processor();
    processor.exposed.actions.push(golden_alchemist::ExposedAction {
        decl_id: golden_alchemist::ExposedDeclId::new("run"),
        label: "Run".into(),
        target: golden_alchemist::ANodeFieldPath::new(*processor.graph.nodes.keys().next().unwrap(), "value"),
    });

    assert_eq!(processor.ui_model(Vec::new()).exposed.actions.len(), 1);
}
