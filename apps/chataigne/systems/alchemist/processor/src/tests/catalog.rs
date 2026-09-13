use std::{sync::Arc, time::Duration};

use chataigne_alchemist::{
    ChannelDescriptor, ChannelLayout, CompileCtx, EvaluationCtx, ManagedFilterValueMode, PrimitiveNodeKind,
    RuntimeInputSnapshot, RuntimeRegistries, StableRef, ValueTypeId,
};
use golden_values::Value as RuntimeValue;

use super::managed_formula::managed_item_for_primitive;
use crate::{ChannelFrame, ChannelValidity, ManagedStageChain, ValueLaneKey};

fn layout(types: &[(&str, &str)]) -> ChannelLayout {
    ChannelLayout::new(
        types
            .iter()
            .map(|(name, value_type)| {
                ChannelDescriptor::input(
                    ValueLaneKey::new(*name).unwrap(),
                    *name,
                    StableRef::new(ValueTypeId::new("source"), *name),
                    Some(ValueTypeId::new(*value_type)),
                )
            })
            .collect(),
    )
    .unwrap()
}

#[test]
fn mixed_tuple_explicitly_converts_before_numeric_reduction() {
    let value_types = crate::alchemist::value_type_registry();
    let nodes = crate::alchemist::node_registry();
    let compile = CompileCtx {
        value_types: &value_types,
        nodes: &nodes,
        properties: None,
    };
    let shape = layout(&[("x", "float"), ("enabled", "bool"), ("amount", "string")]);
    let mut convert = managed_item_for_primitive(PrimitiveNodeKind::ConvertTuple);
    convert.anode.config.set("target", RuntimeValue::String("float".into()));
    convert.anode.config.set("num_inputs", RuntimeValue::Int(3));
    let mut sum = managed_item_for_primitive(PrimitiveNodeKind::Sum);
    sum.anode.config.set("num_inputs", RuntimeValue::Int(3));
    let mut chain = ManagedStageChain::compile(
        &[convert, sum],
        Arc::new(shape.clone()),
        &compile,
        ManagedFilterValueMode::Tuple,
    )
    .unwrap();
    assert_eq!(chain.output_layout().channels().len(), 1);
    let mut input = ChannelFrame::new(Arc::new(shape));
    input.begin_tick(1);
    for (index, value) in [
        RuntimeValue::Float(1.0),
        RuntimeValue::Bool(true),
        RuntimeValue::String("2.5".into()),
    ]
    .into_iter()
    .enumerate()
    {
        input.set(index, Some(value), ChannelValidity::Valid, true).unwrap();
    }
    let sources = RuntimeInputSnapshot::default();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let context = EvaluationCtx {
        logical_tick: 1,
        delta_time: Duration::ZERO,
        events: &[],
        inputs: &sources,
        registries: &registries,
    };
    let (output, effects) = chain.evaluate(&input, &context).unwrap();
    assert!(effects.diagnostics.is_empty(), "{:?}", effects.diagnostics);
    assert_eq!(output.slots()[0].value, Some(RuntimeValue::Float(4.5)));

    input.begin_tick(2);
    input
        .set(
            2,
            Some(RuntimeValue::String("invalid".into())),
            ChannelValidity::Valid,
            true,
        )
        .unwrap();
    let invalid_context = EvaluationCtx {
        logical_tick: 2,
        ..context
    };
    let (output, effects) = chain.evaluate(&input, &invalid_context).unwrap();
    assert!(
        effects
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("valid float"))
    );
    assert!(
        output.slots()[0].value.is_none(),
        "a failed conversion must not reuse the old sum"
    );
}

#[test]
fn named_reductions_execute_identically_in_mapping_stages() {
    let value_types = crate::alchemist::value_type_registry();
    let nodes = crate::alchemist::node_registry();
    let compile = CompileCtx {
        value_types: &value_types,
        nodes: &nodes,
        properties: None,
    };
    let shape = layout(&[("left", "float"), ("right", "float")]);
    let sources = RuntimeInputSnapshot::default();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    for (kind, expected) in [
        (PrimitiveNodeKind::Sum, 8.0),
        (PrimitiveNodeKind::Product, 12.0),
        (PrimitiveNodeKind::Minimum, 2.0),
        (PrimitiveNodeKind::Maximum, 6.0),
        (PrimitiveNodeKind::Average, 4.0),
        (PrimitiveNodeKind::Difference, 4.0),
        (PrimitiveNodeKind::Distance, 4.0),
    ] {
        let mut chain = ManagedStageChain::compile(
            &[managed_item_for_primitive(kind)],
            Arc::new(shape.clone()),
            &compile,
            ManagedFilterValueMode::Tuple,
        )
        .unwrap();
        let mut input = ChannelFrame::new(Arc::new(shape.clone()));
        input.begin_tick(1);
        input
            .set(0, Some(RuntimeValue::Float(6.0)), ChannelValidity::Valid, true)
            .unwrap();
        input
            .set(1, Some(RuntimeValue::Float(2.0)), ChannelValidity::Valid, true)
            .unwrap();
        let context = EvaluationCtx {
            logical_tick: 1,
            delta_time: Duration::ZERO,
            events: &[],
            inputs: &sources,
            registries: &registries,
        };
        let (output, effects) = chain.evaluate(&input, &context).unwrap();
        assert!(effects.diagnostics.is_empty(), "{kind:?}: {:?}", effects.diagnostics);
        assert_eq!(output.slots()[0].value, Some(RuntimeValue::Float(expected)), "{kind:?}");
    }
}

#[test]
fn mapping_timed_delay_suppresses_until_due_and_preserves_first_value() {
    let value_types = crate::alchemist::value_type_registry();
    let nodes = crate::alchemist::node_registry();
    let compile = CompileCtx {
        value_types: &value_types,
        nodes: &nodes,
        properties: None,
    };
    let shape = layout(&[("value", "float")]);
    let mut item = managed_item_for_primitive(PrimitiveNodeKind::TimedDelay);
    item.anode.config.set("seconds", RuntimeValue::Float(0.05));
    let mut chain = ManagedStageChain::compile(
        &[item],
        Arc::new(shape.clone()),
        &compile,
        ManagedFilterValueMode::Tuple,
    )
    .unwrap();
    let mut input = ChannelFrame::new(Arc::new(shape));
    let sources = RuntimeInputSnapshot::default();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    for (tick, value, expected) in [
        (1, 10.0, None),
        (2, 20.0, None),
        (3, 30.0, Some(RuntimeValue::Float(10.0))),
        (4, 40.0, Some(RuntimeValue::Float(20.0))),
    ] {
        input.begin_tick(tick);
        input
            .set(0, Some(RuntimeValue::Float(value)), ChannelValidity::Valid, true)
            .unwrap();
        let context = EvaluationCtx {
            logical_tick: tick,
            delta_time: Duration::from_millis(25),
            events: &[],
            inputs: &sources,
            registries: &registries,
        };
        let (output, effects) = chain.evaluate(&input, &context).unwrap();
        assert!(effects.diagnostics.is_empty(), "{:?}", effects.diagnostics);
        assert_eq!(output.slots()[0].value, expected, "tick {tick}");
    }
}
