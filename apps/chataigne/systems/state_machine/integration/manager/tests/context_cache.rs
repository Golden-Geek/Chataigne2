use super::*;

#[test]
fn multiplex_lane_count_uses_axis_cardinality_without_materializing_keys() {
    let processor_id = ProcessorId::new();
    let primary_axis = ContextAxisId::new("primary");
    let secondary_axis = ContextAxisId::new("secondary");
    let runtime = context_runtime(
        vec![
            context_axis(
                primary_axis.clone(),
                "Primary",
                (0..1000).map(|index| ContextItemId::new(format!("p{index}"))).collect(),
            ),
            context_axis(
                secondary_axis.clone(),
                "Secondary",
                (0..1000).map(|index| ContextItemId::new(format!("s{index}"))).collect(),
            ),
        ],
        Vec::new(),
    );
    let provider = context_provider(processor_id, Arc::clone(&runtime));
    let axes = AxisSet::from_iter([primary_axis.clone(), secondary_axis.clone()]);
    let valid = ContextKey::new([
        chataigne_alchemist::ContextKeyPart::new(primary_axis, ContextItemId::new("p999")),
        chataigne_alchemist::ContextKeyPart::new(secondary_axis, ContextItemId::new("s999")),
    ]);

    assert_eq!(provider.lane_count_for_axes(processor_id, &axes), 1_000_000);
    assert_eq!(
        provider.context_key_at_preview_index(processor_id, &axes, 1_000_000),
        Some(valid.clone())
    );
    assert_eq!(
        provider.preview_index_for_context_key(processor_id, &axes, &valid),
        Some(1_000_000)
    );
    assert!(provider.context_key_matches_axes(processor_id, &axes, &valid));
    assert!(!provider.context_key_matches_axes(processor_id, &axes, &ContextKey::single("primary", "missing")));
    assert!(
        runtime
            .context_key_cache
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .by_axes
            .is_empty(),
        "cardinality and indexed preview lookup must not materialize the Cartesian product"
    );
}

#[test]
fn multiplex_context_keys_are_cached_across_processors_with_the_same_scope() {
    let primary_processor = ProcessorId::new();
    let secondary_processor = ProcessorId::new();
    let axis = ContextAxisId::new("lane");
    let runtime = context_runtime(
        vec![context_axis(
            axis.clone(),
            "Lane",
            (0..125)
                .map(|index| ContextItemId::new(format!("lane-{index}")))
                .collect(),
        )],
        Vec::new(),
    );
    let mut provider = SnapshotProcessorContextProvider::default();
    provider.insert_processor_runtime(primary_processor, Arc::clone(&runtime));
    provider.insert_processor_runtime(secondary_processor, Arc::clone(&runtime));
    let axes = AxisSet::from_iter([axis]);

    let primary_keys = provider.iter_context_keys(primary_processor, &axes).collect::<Vec<_>>();
    let cached_after_primary = runtime.context_keys_for_axes(&axes);
    let secondary_keys = provider
        .iter_context_keys(secondary_processor, &axes)
        .collect::<Vec<_>>();
    let cached_after_secondary = runtime.context_keys_for_axes(&axes);

    assert_eq!(primary_keys.len(), 125);
    assert_eq!(secondary_keys, primary_keys);
    assert!(Arc::ptr_eq(&cached_after_primary, &cached_after_secondary));
    assert_eq!(
        runtime
            .context_key_cache
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .by_axes
            .len(),
        1
    );
}

#[test]
fn rebuilding_context_indexes_invalidates_materialized_lane_keys() {
    let axis = ContextAxisId::new("lane");
    let mut runtime = ProcessorContextRuntime {
        axes: vec![context_axis(
            axis.clone(),
            "Lane",
            vec![ContextItemId::new("left"), ContextItemId::new("right")],
        )],
        ..ProcessorContextRuntime::default()
    };
    runtime.rebuild_indexes();
    let axes = AxisSet::from_iter([axis]);
    let original = runtime.context_keys_for_axes(&axes);

    runtime.axes[0].items.push(ContextItemId::new("center"));
    runtime.rebuild_indexes();
    let rebuilt = runtime.context_keys_for_axes(&axes);

    assert_eq!(original.len(), 2);
    assert_eq!(rebuilt.len(), 3);
    assert!(!Arc::ptr_eq(&original, &rebuilt));
}

#[test]
fn context_key_cache_bounds_distinct_axis_combinations() {
    let axes = (0..6)
        .map(|index| ContextAxisId::new(format!("axis-{index}")))
        .collect::<Vec<_>>();
    let runtime = context_runtime(
        axes.iter()
            .map(|axis| context_axis(axis.clone(), axis.as_str(), vec![ContextItemId::new("only-item")]))
            .collect(),
        Vec::new(),
    );

    for mask in 1usize..(1usize << axes.len()) {
        let requested = axes
            .iter()
            .enumerate()
            .filter(|(index, _)| mask & (1usize << index) != 0)
            .map(|(_, axis)| axis.clone())
            .collect::<AxisSet>();
        assert_eq!(runtime.context_keys_for_axes(&requested).len(), 1);
    }

    let cache = runtime
        .context_key_cache
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    assert!(cache.by_axes.len() <= MAX_CACHED_CONTEXT_KEY_SETS);
    assert_eq!(cache.retained_key_count, cache.by_axes.len());
    let latest = axes.iter().cloned().collect::<smallvec::SmallVec<_>>();
    assert!(
        cache.by_axes.contains_key(&latest),
        "the active request must survive bounded cache eviction"
    );
}

#[test]
fn context_key_cache_retains_active_maximum_lane_set_after_budget_eviction() {
    let small_axis = ContextAxisId::new("small");
    let large_axis = ContextAxisId::new("large");
    let runtime = context_runtime(
        vec![
            context_axis(small_axis.clone(), "Small", vec![ContextItemId::new("only-item")]),
            context_axis(
                large_axis.clone(),
                "Large",
                (0..MAX_RETAINED_CONTEXT_KEYS)
                    .map(|index| ContextItemId::new(format!("item-{index}")))
                    .collect(),
            ),
        ],
        Vec::new(),
    );
    runtime.context_keys_for_axes(&AxisSet::from_iter([small_axis]));
    let requested = AxisSet::from_iter([large_axis]);

    let first = runtime.context_keys_for_axes(&requested);
    let repeated = runtime.context_keys_for_axes(&requested);

    assert_eq!(first.len(), MAX_RETAINED_CONTEXT_KEYS);
    assert!(
        Arc::ptr_eq(&first, &repeated),
        "the active maximum-sized lane set must not rematerialize every tick"
    );
    let cache = runtime
        .context_key_cache
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    assert_eq!(cache.by_axes.len(), 1);
    assert_eq!(cache.retained_key_count, MAX_RETAINED_CONTEXT_KEYS);
}

#[test]
fn context_key_cache_rejects_expansions_above_the_retained_lane_budget() {
    let axis = ContextAxisId::new("large-axis");
    let runtime = context_runtime(
        vec![context_axis(
            axis.clone(),
            "Large",
            (0..=MAX_RETAINED_CONTEXT_KEYS)
                .map(|index| ContextItemId::new(format!("item-{index}")))
                .collect(),
        )],
        Vec::new(),
    );
    let requested = AxisSet::from_iter([axis]);

    assert!(runtime.context_keys_for_axes(&requested).is_empty());
    let cache = runtime
        .context_key_cache
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    assert_eq!(cache.by_axes.len(), 1);
    assert_eq!(cache.retained_key_count, 0);
}

#[test]
fn context_key_materialization_rejects_oversized_cartesian_products_before_allocation() {
    let axis_width = (1usize..)
        .find(|width| {
            width
                .checked_mul(*width)
                .is_some_and(|cardinality| cardinality > MAX_MATERIALIZED_CONTEXT_KEYS)
        })
        .expect("the materialization limit must have a finite square root");
    let first_axis = ContextAxisId::new("first");
    let second_axis = ContextAxisId::new("second");
    let runtime = context_runtime(
        vec![
            context_axis(
                first_axis.clone(),
                "First",
                (0..axis_width)
                    .map(|index| ContextItemId::new(format!("first-{index}")))
                    .collect(),
            ),
            context_axis(
                second_axis.clone(),
                "Second",
                (0..axis_width)
                    .map(|index| ContextItemId::new(format!("second-{index}")))
                    .collect(),
            ),
        ],
        Vec::new(),
    );
    let requested = AxisSet::from_iter([first_axis, second_axis]);
    let processor_id = ProcessorId::new();
    let provider = context_provider(processor_id, Arc::clone(&runtime));

    let keys = runtime.context_keys_for_axes(&requested);

    assert!(keys.is_empty());
    assert_eq!(
        runtime.bounded_context_key_cardinality(&requested),
        Err(ContextKeyCardinalityError::MaterializationLimitExceeded)
    );
    assert_eq!(
        provider.lane_count_for_axes(processor_id, &requested),
        axis_width * axis_width
    );
    assert!(provider.context_key_limit_exceeded(processor_id, &requested));
    assert_eq!(provider.iter_context_keys(processor_id, &requested).count(), 0);
    assert!(CONTEXT_LANE_LIMIT_WARNING.contains("65,536-lane safety limit"));
    assert_eq!(
        runtime
            .context_key_cache
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .by_axes
            .len(),
        1,
        "the fail-closed empty result should be cached without retaining lane keys"
    );
}

#[test]
fn context_provider_rebuild_refreshes_and_clears_the_lane_limit_warning() {
    let axis_width = (1usize..)
        .find(|width| {
            width
                .checked_mul(*width)
                .is_some_and(|cardinality| cardinality > MAX_MATERIALIZED_CONTEXT_KEYS)
        })
        .expect("the materialization limit must have a finite square root");
    let first_axis = ContextAxisId::new("first");
    let second_axis = ContextAxisId::new("second");
    let formula = continuous_formula();
    let processor = Processor::from_formula("Oversized processor", &formula);
    let processor_id = processor.id;
    let required_axes = AxisSet::from_iter([first_axis.clone(), second_axis.clone()]);
    let mut runtime = ProcessorRuntime::new(processor_id);
    runtime.plan = Some(ProcessorExecutionPlan {
        processor_id,
        available_axes: required_axes.clone(),
        required_eval_axes: required_axes.clone(),
        required_memory_axes: AxisSet::new(),
        strategy: ProcessorExecutionStrategy::MultiStateless,
    });
    let runtime_processor = RuntimeProcessor {
        processor,
        runtime,
        compile_warning: None,
        formula,
        formula_node: None,
        formula_ui: chataigne_state_machine::ProcessorFormulaUiState::project(),
        formula_source_key: "test".to_owned(),
        command_dispatch_plans: Default::default(),
    };
    let oversized_provider = context_provider(
        processor_id,
        context_runtime(
            vec![
                context_axis(
                    first_axis.clone(),
                    "First",
                    (0..axis_width)
                        .map(|index| ContextItemId::new(format!("first-{index}")))
                        .collect(),
                ),
                context_axis(
                    second_axis.clone(),
                    "Second",
                    (0..axis_width)
                        .map(|index| ContextItemId::new(format!("second-{index}")))
                        .collect(),
                ),
            ],
            Vec::new(),
        ),
    );
    let bounded_provider = context_provider(
        processor_id,
        context_runtime(
            vec![
                context_axis(first_axis, "First", vec![ContextItemId::new("first-only")]),
                context_axis(second_axis, "Second", vec![ContextItemId::new("second-only")]),
            ],
            Vec::new(),
        ),
    );
    let processor_node = NodeId(42);
    let mut ctx = ProcessCtx::new(
        ExecutionPhase::EngineTick,
        EngineTime {
            tick: 1,
            micro: 0,
            seq: 0,
        },
    );

    sync_runtime_processor_warning(&mut ctx, processor_node, &runtime_processor, &oversized_provider);
    assert!(matches!(
        ctx.edits.pending.last().map(|request| &request.edit),
        Some(Edit::SetNodeWarning { node, warning })
            if *node == processor_node
                && warning.detail.as_deref() == Some(CONTEXT_LANE_LIMIT_WARNING)
    ));

    ctx.edits.pending.clear();
    sync_runtime_processor_warning(&mut ctx, processor_node, &runtime_processor, &bounded_provider);
    assert!(matches!(
        ctx.edits.pending.last().map(|request| &request.edit),
        Some(Edit::ClearNodeWarning { node, warning_id })
            if *node == processor_node
                && warning_id.as_deref() == Some(super::super::STATE_MACHINE_RUNTIME_WARNING_ID)
    ));
}
