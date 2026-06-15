use std::time::Duration;

use golden_alchemist::{
    ANodeInstance, ANodeTypeId, AlchemistFormula, AlchemistGraph, CompileCtx, EvaluationCtx, FormulaContextContract,
    FormulaId, FormulaPropertySchema, FormulaSurface, RuntimeInputSnapshot, RuntimeRegistries, RuntimeValue,
    SurfaceItem, SurfaceItemId, SurfaceItemKind, SurfaceSection, SurfaceSectionId, SurfaceSource, ValueTypeRegistry,
    primitive_node_registry,
};

use crate::{Processor, ProcessorLifecycleEvent, ProcessorRuntime};

fn formula() -> AlchemistFormula {
    let mut graph = AlchemistGraph::new();
    let mut constant = ANodeInstance::new(ANodeTypeId::new("constant"), "Constant");
    constant.config.set("value", RuntimeValue::Float(1.0));
    graph.add_node(constant).unwrap();
    AlchemistFormula {
        id: FormulaId::new("test"),
        version: 1,
        label: "Test".into(),
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
fn processor_compiles_and_evaluates_only_while_active() {
    let formula = formula();
    let processor = Processor::from_formula("Processor", &formula);
    let mut runtime = ProcessorRuntime::new(processor.id);
    let value_types = ValueTypeRegistry::with_primitives();
    let nodes = primitive_node_registry();
    assert!(runtime.compile(
        &processor,
        &formula,
        &CompileCtx {
            value_types: &value_types,
            nodes: &nodes,
            properties: Some(&formula.properties),
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
fn formula_surface_is_present_in_ui_model() {
    let mut formula = formula();
    formula.surface.sections.push(SurfaceSection {
        id: SurfaceSectionId::new("actions"),
        label: "Actions".into(),
        items: vec![SurfaceItem {
            id: SurfaceItemId::new("run"),
            label: "Run".into(),
            description: None,
            path: Vec::new(),
            kind: SurfaceItemKind::Action,
            value_type: None,
            ui: golden_alchemist::ParamUiHints::default(),
            bindings: Vec::new(),
        }],
        source: SurfaceSource::Formula,
    });

    let processor = Processor::from_formula("Processor", &formula);
    let ui = processor.ui_model(&formula, Vec::new());
    assert_eq!(ui.formula_id, "test");
    assert_eq!(ui.formula_label, "Test");
    assert_eq!(ui.surface.sections[0].items.len(), 1);
}
