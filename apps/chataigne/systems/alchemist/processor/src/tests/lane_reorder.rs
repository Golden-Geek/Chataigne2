use crate::testkit::TestGraph;

use std::time::Duration;

use chataigne_alchemist::{
    ANodeInstance, ANodeTypeId, AlchemistFormula, AxisSet, CompileCtx, ContextAxisId, ContextKey, ContextValuePath,
    EvaluationCtx, FormulaContextContract, FormulaId, FormulaPropertyDecl, FormulaPropertyId, FormulaPropertySchema,
    FormulaSurface, InputSocketRef, LaneRuntimePool, OutputSocketRef, RuntimeInputSnapshot, RuntimeOutput,
    RuntimeRegistries, StableRef, SurfaceItemId, ValueTypeId, ValueTypeRegistry, primitive_node_registry,
};
use golden_values::Value as RuntimeValue;

use crate::{
    Processor, ProcessorBindingAnalysis, ProcessorContextPropertyBinding, ProcessorContextProvider,
    ProcessorDebugCapture, ProcessorId, ProcessorLifecycleEvent, ProcessorRuntime,
};

fn stateful_context_formula() -> AlchemistFormula {
    let mut graph = TestGraph::new();
    let mut property = ANodeInstance::new(ANodeTypeId::new("property"), "Active");
    property.config.set(
        "property_id",
        RuntimeValue::Ref(StableRef::new(ValueTypeId::new("property"), "active")),
    );
    let source = graph.add_node(property).unwrap();
    let edge = graph
        .add_node(ANodeInstance::new(ANodeTypeId::new("trigger_on_off"), "Edge"))
        .unwrap();
    graph
        .connect(
            OutputSocketRef::new(source, "value"),
            InputSocketRef::new(edge, "value"),
        )
        .unwrap();
    let mut properties = FormulaPropertySchema::default();
    properties.insert(FormulaPropertyDecl {
        id: FormulaPropertyId::new("active"),
        label: "Active".into(),
        description: None,
        value_type: ValueTypeId::new("bool"),
        default_value: RuntimeValue::Bool(false),
        ui: chataigne_alchemist::PropertyUiHints::default(),
    });
    AlchemistFormula {
        id: FormulaId::new("stateful-context-reorder"),
        version: 1,
        label: "Stateful context reorder".into(),
        description: None,
        tags: Vec::new(),
        graph: graph.to_document().unwrap(),
        properties,
        surface: FormulaSurface::default(),
        context_contract: FormulaContextContract::default(),
        migrations: Vec::new(),
    }
}

struct BoolLaneProvider {
    keys: Vec<ContextKey>,
}

impl BoolLaneProvider {
    fn new(keys: Vec<ContextKey>) -> Self {
        Self { keys }
    }
}

impl ProcessorContextProvider for BoolLaneProvider {
    fn available_axes(&self, _processor_id: ProcessorId) -> AxisSet {
        [ContextAxisId::new("device")].into_iter().collect()
    }

    fn iter_context_keys<'a>(
        &'a self,
        _processor_id: ProcessorId,
        _axes: &'a AxisSet,
    ) -> Box<dyn Iterator<Item = ContextKey> + 'a> {
        Box::new(self.keys.iter().cloned())
    }

    fn resolve_context_value(
        &self,
        key: &ContextKey,
        axis: &ContextAxisId,
        path: &ContextValuePath,
    ) -> Option<RuntimeValue> {
        if axis.as_str() != "device" || path.segments.first()?.as_str() != "active" {
            return None;
        }
        match key.iter().find(|part| &part.axis == axis)?.item.as_str() {
            "a" => Some(RuntimeValue::Bool(true)),
            "b" => Some(RuntimeValue::Bool(false)),
            _ => None,
        }
    }
}

fn trigger_fired(output: &RuntimeOutput) -> bool {
    output
        .debug_samples
        .iter()
        .any(|sample| matches!(&sample.value, RuntimeValue::Trigger(trigger) if trigger.fired))
}

#[test]
fn distinct_stateful_lane_values_follow_keys_across_reorder() {
    let formula = stateful_context_formula();
    let mut processor = Processor::from_formula("Processor", &formula);
    processor.context_property_bindings.insert(
        SurfaceItemId::new("active"),
        ProcessorContextPropertyBinding {
            axis: ContextAxisId::new("device"),
            path: ContextValuePath::new(["active"]),
        },
    );
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
        },
    ));
    runtime.apply_lifecycle(
        &processor,
        ProcessorLifecycleEvent::StateEnter(chataigne_state_machine_model::StateId::new()),
    );
    let a = ContextKey::single("device", "a");
    let b = ContextKey::single("device", "b");
    let provider = BoolLaneProvider::new(vec![a.clone(), b.clone()]);
    runtime.rebuild_execution_plan(
        &provider,
        &ProcessorBindingAnalysis {
            property_axes: provider.available_axes(processor.id),
            ..ProcessorBindingAnalysis::default()
        },
    );
    let inputs = RuntimeInputSnapshot::default();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let capture = ProcessorDebugCapture::All { history_len: 4 };
    let first = runtime.evaluate_processor_with_context_provider_and_capture(
        &processor,
        &EvaluationCtx {
            logical_tick: 1,
            delta_time: Duration::ZERO,
            events: &[],
            inputs: &inputs,
            registries: &registries,
        },
        &provider,
        &capture,
    );
    assert_eq!(
        first.iter().map(|lane| lane.context_key.as_ref()).collect::<Vec<_>>(),
        vec![Some(&a), Some(&b)]
    );
    assert!(trigger_fired(&first[0].output));
    assert!(!trigger_fired(&first[1].output));
    let LaneRuntimePool::Stateful(lanes) = &runtime.lanes else {
        panic!("stateful formula must retain keyed lane memory");
    };
    assert_ne!(
        lanes[&a], lanes[&b],
        "lane inputs must produce distinct retained states"
    );

    let reordered_provider = BoolLaneProvider::new(vec![b.clone(), a.clone()]);
    let second = runtime.evaluate_processor_with_context_provider_and_capture(
        &processor,
        &EvaluationCtx {
            logical_tick: 2,
            delta_time: Duration::ZERO,
            events: &[],
            inputs: &inputs,
            registries: &registries,
        },
        &reordered_provider,
        &capture,
    );
    assert_eq!(
        second.iter().map(|lane| lane.context_key.as_ref()).collect::<Vec<_>>(),
        vec![Some(&b), Some(&a)]
    );
    assert!(
        second.iter().all(|lane| !trigger_fired(&lane.output)),
        "reordering must not replay either lane's edge"
    );
    assert_eq!(runtime.lanes.memory_count(), 2);
}
