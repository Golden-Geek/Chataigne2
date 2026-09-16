use std::{sync::Arc, time::Duration};

use chataigne_alchemist::{
    ChannelDescriptor, ChannelLayout, ColorValue, CompileCtx, DebugCaptureMode, EvaluationCtx,
    MANAGED_AUTO_INPUT_COUNT_FIELD, ManagedFilterValueMode, ManagedItemInstance, PrimitiveNodeKind,
    RuntimeInputSnapshot, RuntimeRegistries, SocketId, StableRef, ValueComponent, ValueTypeId,
};
use golden_values::Value as RuntimeValue;

use super::managed_formula::managed_item_for_primitive;
use crate::{
    ChannelFrame, ChannelValidity, ManagedStageChain, ManagedStageRuntime, ManagedStageSpecializationCache,
    RuntimeInputBinding, ValueLaneKey,
};

fn typed_layout(channels: &[(&str, &str)]) -> ChannelLayout {
    ChannelLayout::new(
        channels
            .iter()
            .map(|(id, value_type)| {
                ChannelDescriptor::input(
                    ValueLaneKey::new(*id).unwrap(),
                    *id,
                    StableRef::new(ValueTypeId::new("source"), *id),
                    Some(ValueTypeId::new(*value_type)),
                )
            })
            .collect(),
    )
    .unwrap()
}

fn frame(layout: ChannelLayout, values: &[RuntimeValue], tick: u64) -> ChannelFrame {
    let mut frame = ChannelFrame::new(Arc::new(layout));
    frame.begin_tick(tick);
    for (index, value) in values.iter().enumerate() {
        frame
            .set(index, Some(value.clone()), ChannelValidity::Valid, true)
            .unwrap();
    }
    frame
}

fn ctx<'a>(inputs: &'a RuntimeInputSnapshot, registries: &'a RuntimeRegistries<'a>, tick: u64) -> EvaluationCtx<'a> {
    EvaluationCtx {
        logical_tick: tick,
        delta_time: Duration::from_millis(20),
        events: &[],
        inputs,
        registries,
    }
}

fn remap() -> ManagedItemInstance {
    let mut item = managed_item_for_primitive(PrimitiveNodeKind::Remap);
    for (socket, value) in [("in_min", 0.0), ("in_max", 10.0), ("out_min", 0.0), ("out_max", 1.0)] {
        item.anode
            .input_defaults
            .insert(SocketId::new(socket), RuntimeValue::Float(value));
    }
    item
}

#[test]
fn disabled_input_count_specializes_reduction_to_the_complete_tuple() {
    let value_types = crate::alchemist::value_type_registry();
    let nodes = crate::alchemist::node_registry();
    let compile_ctx = CompileCtx {
        value_types: &value_types,
        nodes: &nodes,
        properties: None,
    };
    let layout = typed_layout(&[("x", "float"), ("y", "float"), ("z", "float")]);
    let mut sum = managed_item_for_primitive(PrimitiveNodeKind::Sum);
    sum.anode
        .config
        .set(MANAGED_AUTO_INPUT_COUNT_FIELD, RuntimeValue::Bool(true));
    let mut stage = ManagedStageRuntime::compile(sum, &layout, &compile_ctx, ManagedFilterValueMode::Tuple)
        .unwrap()
        .unwrap();
    let input = frame(
        layout,
        &[
            RuntimeValue::Float(1.0),
            RuntimeValue::Float(2.0),
            RuntimeValue::Float(3.0),
        ],
        1,
    );
    let inputs = RuntimeInputSnapshot::default();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let (output, result) = stage.evaluate(&input, &ctx(&inputs, &registries, 1)).unwrap();
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(output.slots()[0].value, Some(RuntimeValue::Float(6.0)));
}

#[test]
fn closed_gate_suppresses_each_tuple_element_and_reopens_on_control_change() {
    let value_types = crate::alchemist::value_type_registry();
    let nodes = crate::alchemist::node_registry();
    let compile_ctx = CompileCtx {
        value_types: &value_types,
        nodes: &nodes,
        properties: None,
    };
    let layout = typed_layout(&[("a", "float"), ("b", "float")]);
    let mut gate = managed_item_for_primitive(PrimitiveNodeKind::ConditionGate);
    gate.anode
        .input_defaults
        .insert(SocketId::new("condition"), RuntimeValue::Bool(false));
    let mut stage = ManagedStageRuntime::compile(gate, &layout, &compile_ctx, ManagedFilterValueMode::Tuple)
        .unwrap()
        .unwrap();
    let sources = RuntimeInputSnapshot::default();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let first = frame(layout.clone(), &[RuntimeValue::Float(0.5), RuntimeValue::Float(1.5)], 1);
    let (blocked, result) = stage.evaluate(&first, &ctx(&sources, &registries, 1)).unwrap();
    assert!(result.diagnostics.is_empty());
    assert!(
        blocked
            .slots()
            .iter()
            .all(|slot| slot.validity == ChannelValidity::Suppressed && slot.value.is_none() && !slot.deliver)
    );

    stage
        .update_runtime_input(
            &SocketId::new("condition"),
            RuntimeInputBinding::Constant(RuntimeValue::Bool(true)),
        )
        .unwrap();
    let second = frame(layout, &[RuntimeValue::Float(0.5), RuntimeValue::Float(1.5)], 2);
    let (opened, result) = stage.evaluate(&second, &ctx(&sources, &registries, 2)).unwrap();
    assert!(result.diagnostics.is_empty());
    assert_eq!(
        opened.slots().iter().map(|slot| slot.value.clone()).collect::<Vec<_>>(),
        vec![Some(RuntimeValue::Float(0.5)), Some(RuntimeValue::Float(1.5))]
    );
    assert!(
        opened
            .slots()
            .iter()
            .all(|slot| slot.validity == ChannelValidity::Valid && slot.deliver)
    );
}

#[test]
fn smoothing_continues_after_input_stops_changing() {
    let value_types = crate::alchemist::value_type_registry();
    let nodes = crate::alchemist::node_registry();
    let compile_ctx = CompileCtx {
        value_types: &value_types,
        nodes: &nodes,
        properties: None,
    };
    let layout = typed_layout(&[("x", "float")]);
    let item = managed_item_for_primitive(PrimitiveNodeKind::SmoothFilter);
    let mut stage = ManagedStageRuntime::compile(item, &layout, &compile_ctx, ManagedFilterValueMode::Tuple)
        .unwrap()
        .unwrap();
    let sources = RuntimeInputSnapshot::default();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    stage
        .evaluate(
            &frame(layout.clone(), &[RuntimeValue::Float(0.0)], 1),
            &ctx(&sources, &registries, 1),
        )
        .unwrap();
    let (moving, _) = stage
        .evaluate(
            &frame(layout.clone(), &[RuntimeValue::Float(10.0)], 2),
            &ctx(&sources, &registries, 2),
        )
        .unwrap();
    let Some(RuntimeValue::Float(moving)) = moving.slots()[0].value else {
        panic!("expected smooth value")
    };
    let (settling, _) = stage
        .evaluate(
            &frame(layout, &[RuntimeValue::Float(10.0)], 3),
            &ctx(&sources, &registries, 3),
        )
        .unwrap();
    let Some(RuntimeValue::Float(settling)) = settling.slots()[0].value else {
        panic!("expected smooth value")
    };
    assert!(settling > moving && settling < 10.0);
}

#[test]
fn elementwise_history_follows_source_identity_across_rename_and_reorder() {
    let value_types = crate::alchemist::value_type_registry();
    let nodes = crate::alchemist::node_registry();
    let compile_ctx = CompileCtx {
        value_types: &value_types,
        nodes: &nodes,
        properties: None,
    };
    let original = typed_layout(&[("a", "float"), ("b", "float")]);
    let mut item = managed_item_for_primitive(PrimitiveNodeKind::SmoothFilter);
    item.anode.config.set("method", RuntimeValue::String("sma".into()));
    item.anode.config.set("window", RuntimeValue::Int(2));
    let mut previous =
        ManagedStageRuntime::compile(item.clone(), &original, &compile_ctx, ManagedFilterValueMode::Tuple)
            .unwrap()
            .unwrap();
    let sources = RuntimeInputSnapshot::default();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    previous
        .evaluate(
            &frame(original, &[RuntimeValue::Float(2.0), RuntimeValue::Float(10.0)], 1),
            &ctx(&sources, &registries, 1),
        )
        .unwrap();
    item.anode.label = "Renamed smooth".into();
    let reordered = typed_layout(&[("b", "float"), ("a", "float")]);
    let mut rebuilt = ManagedStageRuntime::compile(item, &reordered, &compile_ctx, ManagedFilterValueMode::Tuple)
        .unwrap()
        .unwrap();
    assert!(rebuilt.migrate_memory_from(previous));
    let (output, effects) = rebuilt
        .evaluate(
            &frame(reordered, &[RuntimeValue::Float(30.0), RuntimeValue::Float(6.0)], 2),
            &ctx(&sources, &registries, 2),
        )
        .unwrap();
    assert!(effects.diagnostics.is_empty());
    assert_eq!(output.slots()[0].value, Some(RuntimeValue::Float(20.0)));
    assert_eq!(output.slots()[1].value, Some(RuntimeValue::Float(4.0)));
}

#[test]
fn removed_element_history_is_released_before_the_source_is_added_again() {
    let value_types = crate::alchemist::value_type_registry();
    let nodes = crate::alchemist::node_registry();
    let compile_ctx = CompileCtx {
        value_types: &value_types,
        nodes: &nodes,
        properties: None,
    };
    let mut item = managed_item_for_primitive(PrimitiveNodeKind::SmoothFilter);
    item.anode.config.set("method", RuntimeValue::String("sma".into()));
    item.anode.config.set("window", RuntimeValue::Int(2));
    let both = typed_layout(&[("a", "float"), ("b", "float")]);
    let mut original = ManagedStageRuntime::compile(item.clone(), &both, &compile_ctx, ManagedFilterValueMode::Tuple)
        .unwrap()
        .unwrap();
    let sources = RuntimeInputSnapshot::default();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    original
        .evaluate(
            &frame(both.clone(), &[RuntimeValue::Float(2.0), RuntimeValue::Float(10.0)], 1),
            &ctx(&sources, &registries, 1),
        )
        .unwrap();
    assert_eq!(original.retained_state_lane_count(), 2);

    let only_a = typed_layout(&[("a", "float")]);
    let mut removed = ManagedStageRuntime::compile(item.clone(), &only_a, &compile_ctx, ManagedFilterValueMode::Tuple)
        .unwrap()
        .unwrap();
    assert!(removed.migrate_memory_from(original));
    assert_eq!(removed.retained_state_lane_count(), 1);
    let (output, _) = removed
        .evaluate(
            &frame(only_a, &[RuntimeValue::Float(6.0)], 2),
            &ctx(&sources, &registries, 2),
        )
        .unwrap();
    assert_eq!(output.slots()[0].value, Some(RuntimeValue::Float(4.0)));

    let mut added = ManagedStageRuntime::compile(item, &both, &compile_ctx, ManagedFilterValueMode::Tuple)
        .unwrap()
        .unwrap();
    assert!(added.migrate_memory_from(removed));
    assert_eq!(added.retained_state_lane_count(), 1);
    let (output, _) = added
        .evaluate(
            &frame(both, &[RuntimeValue::Float(8.0), RuntimeValue::Float(30.0)], 3),
            &ctx(&sources, &registries, 3),
        )
        .unwrap();
    assert_eq!(output.slots()[0].value, Some(RuntimeValue::Float(7.0)));
    assert_eq!(output.slots()[1].value, Some(RuntimeValue::Float(30.0)));
    assert_eq!(added.retained_state_lane_count(), 2);
}

#[test]
fn upstream_temporal_edit_resets_downstream_history() {
    let value_types = crate::alchemist::value_type_registry();
    let nodes = crate::alchemist::node_registry();
    let compile_ctx = CompileCtx {
        value_types: &value_types,
        nodes: &nodes,
        properties: None,
    };
    let layout = typed_layout(&[("a", "float")]);
    let smooth = || {
        let mut item = managed_item_for_primitive(PrimitiveNodeKind::SmoothFilter);
        item.anode.config.set("method", RuntimeValue::String("sma".into()));
        item.anode.config.set("window", RuntimeValue::Int(2));
        item
    };
    let first = smooth();
    let second = smooth();
    let mut original = ManagedStageChain::compile(
        &[first.clone(), second.clone()],
        Arc::new(layout.clone()),
        &compile_ctx,
        ManagedFilterValueMode::Tuple,
    )
    .unwrap();
    let sources = RuntimeInputSnapshot::default();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    original
        .evaluate(
            &frame(layout.clone(), &[RuntimeValue::Float(2.0)], 1),
            &ctx(&sources, &registries, 1),
        )
        .unwrap();
    original
        .evaluate(
            &frame(layout.clone(), &[RuntimeValue::Float(4.0)], 2),
            &ctx(&sources, &registries, 2),
        )
        .unwrap();

    let mut changed_first = first;
    changed_first.anode.config.set("window", RuntimeValue::Int(3));
    let mut rebuilt = ManagedStageChain::compile(
        &[changed_first, second],
        Arc::new(layout.clone()),
        &compile_ctx,
        ManagedFilterValueMode::Tuple,
    )
    .unwrap();
    rebuilt.migrate_memory_from(original);
    let third = frame(layout, &[RuntimeValue::Float(6.0)], 3);
    let (output, _) = rebuilt.evaluate(&third, &ctx(&sources, &registries, 3)).unwrap();
    assert_eq!(output.slots()[0].value, Some(RuntimeValue::Float(6.0)));
}

#[test]
fn three_source_tuple_merges_to_sum_or_average() {
    let value_types = crate::alchemist::value_type_registry();
    let nodes = crate::alchemist::node_registry();
    let compile_ctx = CompileCtx {
        value_types: &value_types,
        nodes: &nodes,
        properties: None,
    };
    let layout = typed_layout(&[("x", "float"), ("y", "float"), ("z", "float")]);
    let input = frame(
        layout.clone(),
        &[
            RuntimeValue::Float(2.0),
            RuntimeValue::Float(4.0),
            RuntimeValue::Float(6.0),
        ],
        1,
    );
    let sources = RuntimeInputSnapshot::default();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    for (kind, expected) in [(PrimitiveNodeKind::Sum, 12.0), (PrimitiveNodeKind::Average, 4.0)] {
        let mut item = managed_item_for_primitive(kind);
        item.anode.config.set("num_inputs", RuntimeValue::Int(3));
        let mut stage = ManagedStageRuntime::compile(item, &layout, &compile_ctx, ManagedFilterValueMode::Routed)
            .unwrap()
            .unwrap();
        let (output, effects) = stage.evaluate(&input, &ctx(&sources, &registries, 1)).unwrap();
        assert!(effects.diagnostics.is_empty(), "{kind:?}: {:?}", effects.diagnostics);
        assert_eq!(output.layout().channels().len(), 1);
        assert_eq!(output.slots()[0].value, Some(RuntimeValue::Float(expected)));
    }
}

#[test]
fn tuple_mode_composes_elementwise_remap_sum_and_smooth() {
    let value_types = crate::alchemist::value_type_registry();
    let nodes = crate::alchemist::node_registry();
    let compile_ctx = CompileCtx {
        value_types: &value_types,
        nodes: &nodes,
        properties: None,
    };
    let layout = typed_layout(&[("x", "float"), ("y", "float"), ("z", "float")]);
    let mut sum = managed_item_for_primitive(PrimitiveNodeKind::Sum);
    sum.anode.config.set("num_inputs", RuntimeValue::Int(3));
    let mut smooth = managed_item_for_primitive(PrimitiveNodeKind::SmoothFilter);
    smooth.anode.config.set("method", RuntimeValue::String("sma".into()));
    smooth.anode.config.set("window", RuntimeValue::Int(2));
    let mut chain = ManagedStageChain::compile(
        &[remap(), sum, smooth],
        Arc::new(layout.clone()),
        &compile_ctx,
        ManagedFilterValueMode::Tuple,
    )
    .unwrap();
    assert_eq!(chain.output_layout().channels().len(), 1);
    let sources = RuntimeInputSnapshot::default();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let first = frame(
        layout.clone(),
        &[
            RuntimeValue::Float(2.0),
            RuntimeValue::Float(4.0),
            RuntimeValue::Float(6.0),
        ],
        1,
    );
    let (output, effects) = chain.evaluate(&first, &ctx(&sources, &registries, 1)).unwrap();
    assert!(effects.diagnostics.is_empty());
    assert!(matches!(output.slots()[0].value, Some(RuntimeValue::Float(value)) if (value - 1.2).abs() < 1e-12));
    let second = frame(
        layout,
        &[
            RuntimeValue::Float(4.0),
            RuntimeValue::Float(4.0),
            RuntimeValue::Float(6.0),
        ],
        2,
    );
    let (output, effects) = chain.evaluate(&second, &ctx(&sources, &registries, 2)).unwrap();
    assert!(effects.diagnostics.is_empty());
    assert!(matches!(output.slots()[0].value, Some(RuntimeValue::Float(value)) if (value - 1.3).abs() < 1e-12));
}

#[test]
fn tuple_mode_packs_three_elementwise_results_as_one_vec3() {
    let value_types = crate::alchemist::value_type_registry();
    let nodes = crate::alchemist::node_registry();
    let compile_ctx = CompileCtx {
        value_types: &value_types,
        nodes: &nodes,
        properties: None,
    };
    let layout = typed_layout(&[("x", "float"), ("y", "float"), ("z", "float")]);
    let mut math = managed_item_for_primitive(PrimitiveNodeKind::Math);
    math.anode
        .config
        .set("application", RuntimeValue::String("each".into()));
    math.anode
        .input_defaults
        .insert(SocketId::new("value2"), RuntimeValue::Float(1.0));
    let mut chain = ManagedStageChain::compile(
        &[math, managed_item_for_primitive(PrimitiveNodeKind::PackVec3)],
        Arc::new(layout.clone()),
        &compile_ctx,
        ManagedFilterValueMode::Tuple,
    )
    .unwrap();
    assert_eq!(chain.output_layout().channels().len(), 1);
    let input = frame(
        layout,
        &[
            RuntimeValue::Float(1.0),
            RuntimeValue::Float(2.0),
            RuntimeValue::Float(3.0),
        ],
        1,
    );
    let sources = RuntimeInputSnapshot::default();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let (output, effects) = chain.evaluate(&input, &ctx(&sources, &registries, 1)).unwrap();
    assert!(effects.diagnostics.is_empty());
    assert_eq!(output.slots()[0].value, Some(RuntimeValue::Vec3([2.0, 3.0, 4.0])));
}

#[test]
fn equivalent_stage_instances_share_plan_but_keep_separate_history_and_preview_ids() {
    let value_types = crate::alchemist::value_type_registry();
    let nodes = crate::alchemist::node_registry();
    let compile_ctx = CompileCtx {
        value_types: &value_types,
        nodes: &nodes,
        properties: None,
    };
    let layout = typed_layout(&[("x", "float")]);
    let mut cache = ManagedStageSpecializationCache::default();
    let make_item = || {
        let mut item = managed_item_for_primitive(PrimitiveNodeKind::SmoothFilter);
        item.anode.config.set("method", RuntimeValue::String("sma".into()));
        item.anode.config.set("window", RuntimeValue::Int(2));
        item
    };
    let first_item = make_item();
    let mut second_item = make_item();
    second_item.anode.label = "Other presentation label".to_owned();
    let mut first = ManagedStageRuntime::compile_with_cache(
        first_item.clone(),
        &layout,
        &compile_ctx,
        ManagedFilterValueMode::Tuple,
        Some(&mut cache),
    )
    .unwrap()
    .unwrap();
    let mut second = ManagedStageRuntime::compile_with_cache(
        second_item.clone(),
        &layout,
        &compile_ctx,
        ManagedFilterValueMode::Tuple,
        Some(&mut cache),
    )
    .unwrap()
    .unwrap();
    assert_eq!(cache.len(), 1);
    assert!(first.shares_compiled_plan_with(&second));
    let mut changed_item = make_item();
    changed_item.anode.config.set("window", RuntimeValue::Int(3));
    let changed = ManagedStageRuntime::compile_with_cache(
        changed_item,
        &layout,
        &compile_ctx,
        ManagedFilterValueMode::Tuple,
        Some(&mut cache),
    )
    .unwrap()
    .unwrap();
    assert_eq!(cache.len(), 2);
    assert!(!first.shares_compiled_plan_with(&changed));

    let inputs = RuntimeInputSnapshot::default();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let capture = DebugCaptureMode::All { history_len: 32 };
    first
        .evaluate_with_capture(
            &frame(layout.clone(), &[RuntimeValue::Float(2.0)], 1),
            &ctx(&inputs, &registries, 1),
            capture.clone(),
        )
        .unwrap();
    let (first_output, first_effects) = first
        .evaluate_with_capture(
            &frame(layout.clone(), &[RuntimeValue::Float(4.0)], 2),
            &ctx(&inputs, &registries, 2),
            capture.clone(),
        )
        .unwrap();
    assert_eq!(first_output.slots()[0].value, Some(RuntimeValue::Float(3.0)));
    assert!(
        first_effects
            .debug_samples
            .iter()
            .any(|sample| sample.author_node_id == first_item.anode.id)
    );
    let (second_output, second_effects) = second
        .evaluate_with_capture(
            &frame(layout, &[RuntimeValue::Float(10.0)], 1),
            &ctx(&inputs, &registries, 1),
            capture,
        )
        .unwrap();
    assert_eq!(second_output.slots()[0].value, Some(RuntimeValue::Float(10.0)));
    assert!(
        second_effects
            .debug_samples
            .iter()
            .any(|sample| sample.author_node_id == second_item.anode.id)
    );
}

#[test]
fn tuple_mode_rejects_implicit_subset_routing() {
    let value_types = crate::alchemist::value_type_registry();
    let nodes = crate::alchemist::node_registry();
    let compile_ctx = CompileCtx {
        value_types: &value_types,
        nodes: &nodes,
        properties: None,
    };
    let layout = typed_layout(&[("value", "float"), ("flag", "bool"), ("label", "string")]);
    assert!(matches!(
        ManagedStageChain::compile(
            &[remap()],
            Arc::new(layout),
            &compile_ctx,
            ManagedFilterValueMode::Tuple,
        ),
        Err(crate::ManagedStageError::Application(
            chataigne_alchemist::ManagedApplicationError::IncompatibleTuple
        ))
    ));
}

#[test]
fn typed_stages_compose_remap_sum_smooth_while_preserving_unselected_bool() {
    let value_types = crate::alchemist::value_type_registry();
    let nodes = crate::alchemist::node_registry();
    let compile_ctx = CompileCtx {
        value_types: &value_types,
        nodes: &nodes,
        properties: None,
    };
    let input_layout = typed_layout(&[("left", "float"), ("flag", "bool"), ("right", "float")]);
    let mut remap = ManagedStageRuntime::compile(remap(), &input_layout, &compile_ctx, ManagedFilterValueMode::Routed)
        .unwrap()
        .unwrap();
    let mut sum = managed_item_for_primitive(PrimitiveNodeKind::Math);
    sum.anode
        .config
        .set("application", RuntimeValue::String("combine".into()));
    sum.anode.config.set("operator", RuntimeValue::String("add".into()));
    let mut sum =
        ManagedStageRuntime::compile(sum, remap.output_layout(), &compile_ctx, ManagedFilterValueMode::Routed)
            .unwrap()
            .unwrap();
    let mut smooth = managed_item_for_primitive(PrimitiveNodeKind::SmoothFilter);
    smooth.anode.config.set("method", RuntimeValue::String("sma".into()));
    smooth.anode.config.set("window", RuntimeValue::Int(2));
    let mut smooth = ManagedStageRuntime::compile(
        smooth,
        sum.output_layout(),
        &compile_ctx,
        ManagedFilterValueMode::Routed,
    )
    .unwrap()
    .unwrap();
    let input = frame(
        input_layout.clone(),
        &[
            RuntimeValue::Float(5.0),
            RuntimeValue::Bool(true),
            RuntimeValue::Float(10.0),
        ],
        1,
    );
    let inputs = RuntimeInputSnapshot::default();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let (first, remap_output) = remap.evaluate(&input, &ctx(&inputs, &registries, 1)).unwrap();
    assert!(remap_output.diagnostics.is_empty(), "{:?}", remap_output.diagnostics);
    let (first, sum_output) = sum.evaluate(first, &ctx(&inputs, &registries, 1)).unwrap();
    assert!(sum_output.diagnostics.is_empty(), "{:?}", sum_output.diagnostics);
    let (first, smooth_output) = smooth.evaluate(first, &ctx(&inputs, &registries, 1)).unwrap();
    assert!(smooth_output.diagnostics.is_empty(), "{:?}", smooth_output.diagnostics);
    assert_eq!(first.layout().channels().len(), 2);
    assert_eq!(first.slots()[0].value, Some(RuntimeValue::Float(1.5)));
    assert_eq!(first.slots()[1].value, Some(RuntimeValue::Bool(true)));
    assert_eq!(first.layout().channels()[1].id, ValueLaneKey::new("flag").unwrap());

    let input = frame(
        input_layout,
        &[
            RuntimeValue::Float(6.0),
            RuntimeValue::Bool(true),
            RuntimeValue::Float(10.0),
        ],
        2,
    );
    let (second, _) = remap.evaluate(&input, &ctx(&inputs, &registries, 2)).unwrap();
    let (second, _) = sum.evaluate(second, &ctx(&inputs, &registries, 2)).unwrap();
    let (second, _) = smooth.evaluate(second, &ctx(&inputs, &registries, 2)).unwrap();
    assert_eq!(second.slots()[0].value, Some(RuntimeValue::Float(1.55)));
    assert_eq!(second.slots()[1].value, Some(RuntimeValue::Bool(true)));
}

#[test]
fn typed_extract_color_produces_stable_component_channels() {
    let value_types = crate::alchemist::value_type_registry();
    let nodes = crate::alchemist::node_registry();
    let compile_ctx = CompileCtx {
        value_types: &value_types,
        nodes: &nodes,
        properties: None,
    };
    let input_layout = typed_layout(&[("color", "color")]);
    let item = managed_item_for_primitive(PrimitiveNodeKind::ExtractColor);
    let mut stage = ManagedStageRuntime::compile(item, &input_layout, &compile_ctx, ManagedFilterValueMode::Routed)
        .unwrap()
        .unwrap();
    let input = frame(
        input_layout,
        &[RuntimeValue::Color(ColorValue {
            red: 0.1,
            green: 0.2,
            blue: 0.3,
            alpha: 0.4,
        })],
        1,
    );
    let inputs = RuntimeInputSnapshot::default();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let (output, result) = stage.evaluate(&input, &ctx(&inputs, &registries, 1)).unwrap();
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(
        output.slots().iter().map(|slot| slot.value.clone()).collect::<Vec<_>>(),
        vec![
            Some(RuntimeValue::Float(0.1)),
            Some(RuntimeValue::Float(0.2)),
            Some(RuntimeValue::Float(0.3)),
            Some(RuntimeValue::Float(0.4)),
        ]
    );
    assert_eq!(
        output.layout().channels()[0].id,
        ValueLaneKey::new("color").unwrap().extracted(ValueComponent::R)
    );
}

#[test]
fn compiled_chain_retains_stage_results_and_state_across_ticks() {
    let value_types = crate::alchemist::value_type_registry();
    let nodes = crate::alchemist::node_registry();
    let compile_ctx = CompileCtx {
        value_types: &value_types,
        nodes: &nodes,
        properties: None,
    };
    let layout = typed_layout(&[("left", "float"), ("right", "float")]);
    let mut sum = managed_item_for_primitive(PrimitiveNodeKind::Math);
    sum.anode
        .config
        .set("application", RuntimeValue::String("combine".into()));
    let mut smooth = managed_item_for_primitive(PrimitiveNodeKind::SmoothFilter);
    smooth.anode.config.set("method", RuntimeValue::String("sma".into()));
    smooth.anode.config.set("window", RuntimeValue::Int(2));
    let mut chain = ManagedStageChain::compile(
        &[remap(), sum, smooth],
        Arc::new(layout.clone()),
        &compile_ctx,
        ManagedFilterValueMode::Routed,
    )
    .unwrap();
    assert_eq!(chain.output_layout().channels().len(), 1);
    let inputs = RuntimeInputSnapshot::default();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let first = frame(
        layout.clone(),
        &[RuntimeValue::Float(5.0), RuntimeValue::Float(10.0)],
        1,
    );
    let (output, runtime_output) = chain.evaluate(&first, &ctx(&inputs, &registries, 1)).unwrap();
    assert!(
        runtime_output.diagnostics.is_empty(),
        "{:?}",
        runtime_output.diagnostics
    );
    assert_eq!(output.slots()[0].value, Some(RuntimeValue::Float(1.5)));
    let second = frame(layout, &[RuntimeValue::Float(6.0), RuntimeValue::Float(10.0)], 2);
    let (output, runtime_output) = chain.evaluate(&second, &ctx(&inputs, &registries, 2)).unwrap();
    assert!(
        runtime_output.diagnostics.is_empty(),
        "{:?}",
        runtime_output.diagnostics
    );
    assert_eq!(output.slots()[0].value, Some(RuntimeValue::Float(1.55)));
}

#[test]
fn pack_extract_math_pack_chain_executes_every_declared_stage() {
    let value_types = crate::alchemist::value_type_registry();
    let nodes = crate::alchemist::node_registry();
    let compile_ctx = CompileCtx {
        value_types: &value_types,
        nodes: &nodes,
        properties: None,
    };
    let layout = typed_layout(&[("x", "float"), ("y", "float"), ("z", "float")]);
    let first_pack = managed_item_for_primitive(PrimitiveNodeKind::PackVec3);
    let extract = managed_item_for_primitive(PrimitiveNodeKind::ExtractVec3);
    let mut math = managed_item_for_primitive(PrimitiveNodeKind::Math);
    math.anode
        .config
        .set("application", RuntimeValue::String("each".into()));
    math.anode
        .input_defaults
        .insert(SocketId::new("value2"), RuntimeValue::Float(1.0));
    let second_pack = managed_item_for_primitive(PrimitiveNodeKind::PackVec3);
    let mut chain = ManagedStageChain::compile(
        &[first_pack, extract, math, second_pack],
        Arc::new(layout.clone()),
        &compile_ctx,
        ManagedFilterValueMode::Tuple,
    )
    .unwrap();
    let input = frame(
        layout,
        &[
            RuntimeValue::Float(1.0),
            RuntimeValue::Float(2.0),
            RuntimeValue::Float(3.0),
        ],
        1,
    );
    let inputs = RuntimeInputSnapshot::default();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let (output, result) = chain.evaluate(&input, &ctx(&inputs, &registries, 1)).unwrap();
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(output.layout().channels().len(), 1);
    assert_eq!(output.slots()[0].value, Some(RuntimeValue::Vec3([2.0, 3.0, 4.0])));
}
