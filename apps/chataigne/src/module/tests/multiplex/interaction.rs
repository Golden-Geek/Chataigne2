use super::*;

#[test]
fn multiplex_sample_state_machine_edits_stay_interactive() {
    let _performance_guard = lock_performance_test();
    const SAMPLE: &str = "test_multiplex.noisette";
    const WARMUP: usize = 10;

    let path = targeted_performance_sample_path(SAMPLE);
    let mut engine = load_sparse_project_file::<AppNode, _>(&path).expect("multiplex sample should load");
    configure_loaded_engine(&mut engine).expect("multiplex sample should configure");
    prepare_engine_for_runtime(&mut engine).expect("multiplex sample should prepare");
    for _ in 0..WARMUP {
        engine
            .run_tick(Duration::from_millis(8))
            .expect("multiplex warmup tick should run");
    }

    let (processor, processor_source_nodes) = largest_duplicable_subtree_by_type(&engine, "state_processor")
        .expect("multiplex sample should contain a duplicable processor");
    let processor_nodes_before = engine.nodes.len();
    let (_, processor_duplicate_ms) =
        elapsed_ms(|| duplicate_node(&mut engine, processor).expect("processor duplicate should apply"));
    let processor_nodes_added = engine.nodes.len().saturating_sub(processor_nodes_before);
    let processor_rebuild_tick_ms = max_tick_elapsed_ms(&mut engine, 3);

    let (state, state_source_nodes) = largest_duplicable_subtree_by_type(&engine, "state")
        .expect("multiplex sample should contain a duplicable state");
    let state_nodes_before = engine.nodes.len();
    let (_, state_duplicate_ms) =
        elapsed_ms(|| duplicate_node(&mut engine, state).expect("state duplicate should apply"));
    let state_nodes_added = engine.nodes.len().saturating_sub(state_nodes_before);
    let state_rebuild_tick_ms = max_tick_elapsed_ms(&mut engine, 3);

    eprintln!(
        "multiplex edits: processor_nodes={processor_nodes_added} processor_duplicate={processor_duplicate_ms}ms processor_rebuild_tick={processor_rebuild_tick_ms}ms state_nodes={state_nodes_added} state_duplicate={state_duplicate_ms}ms state_rebuild_tick={state_rebuild_tick_ms}ms"
    );

    assert_eq!(
        processor_nodes_added, processor_source_nodes,
        "the processor benchmark must duplicate the complete selected subtree"
    );
    assert_eq!(
        state_nodes_added, state_source_nodes,
        "the state benchmark must duplicate the complete selected subtree"
    );
    if strict_serial_performance_assertions() {
        assert!(
            processor_duplicate_ms < 50,
            "processor duplicate took {processor_duplicate_ms}ms"
        );
        assert!(
            processor_rebuild_tick_ms < 50,
            "post-processor-duplicate tick took {processor_rebuild_tick_ms}ms"
        );
        assert!(state_duplicate_ms < 100, "state duplicate took {state_duplicate_ms}ms");
        assert!(
            state_rebuild_tick_ms < 100,
            "post-state-duplicate tick took {state_rebuild_tick_ms}ms"
        );
    }
}

#[test]
fn multiplex_sample_production_duplicate_transactions_stay_interactive() {
    let _performance_guard = lock_performance_test();
    const SAMPLE: &str = "test_multiplex.noisette";
    const WARMUP: usize = 10;

    let path = targeted_performance_sample_path(SAMPLE);
    let mut engine = load_sparse_project_file::<AppNode, _>(&path).expect("multiplex sample should load");
    configure_loaded_engine(&mut engine).expect("multiplex sample should configure");
    prepare_engine_for_runtime(&mut engine).expect("multiplex sample should prepare");
    for _ in 0..WARMUP {
        engine
            .run_tick(Duration::from_millis(8))
            .expect("multiplex warmup tick should run");
    }

    let (processor, processor_source_nodes) = largest_duplicable_subtree_by_type(&engine, "state_processor")
        .expect("multiplex sample should contain a duplicable processor");
    let (state, _) = largest_duplicable_subtree_by_type(&engine, "state")
        .expect("multiplex sample should contain a duplicable state");
    let duplicate_intent = |source| {
        let source_node = engine.nodes.get(source).expect("duplicate source should exist");
        UiEditIntent::DuplicateNode {
            source,
            new_parent: source_node
                .node_data()
                .parent
                .expect("duplicate source should have a parent"),
            new_prev_sibling: Some(source),
            initial_params: Vec::new(),
        }
    };
    let processor_intent = duplicate_intent(processor);
    let state_intent = duplicate_intent(state);
    let initial_node_count = engine.nodes.len();
    let runtime = ProductionRuntime::new(
        engine,
        UiProjectFileSpec::from_project_file_spec(ProjectFileSpec::new("Noisette", "noisette"), None),
    );

    let (processor_result, processor_transaction_ms) =
        elapsed_ms(|| runtime.apply_ui_transaction(processor_intent, Some("multiplex-edit-performance")));
    assert!(
        processor_result.acknowledgement.success,
        "processor duplication should apply: {:?}",
        processor_result.acknowledgement.error_message
    );
    let processor_rebuild_tick_ms = (0..3)
        .map(|_| {
            elapsed_ms(|| {
                runtime
                    .run_tick(Duration::from_millis(8))
                    .expect("post-processor-duplicate tick should run")
            })
            .1
        })
        .max()
        .unwrap_or_default();
    let processor_node_count = runtime
        .read_model()
        .snapshot_for_scope(UiSubscriptionScope::WholeGraph)
        .nodes
        .len();
    let state_source_nodes_at_transaction = runtime
        .read_model()
        .snapshot_for_scope(UiSubscriptionScope::Subtree {
            root: state,
            max_depth: u32::MAX,
        })
        .nodes
        .len();

    let (state_result, state_transaction_ms) =
        elapsed_ms(|| runtime.apply_ui_transaction(state_intent, Some("multiplex-edit-performance")));
    assert!(
        state_result.acknowledgement.success,
        "state duplication should apply: {:?}",
        state_result.acknowledgement.error_message
    );
    let state_rebuild_tick_ms = (0..3)
        .map(|_| {
            elapsed_ms(|| {
                runtime
                    .run_tick(Duration::from_millis(8))
                    .expect("post-state-duplicate tick should run")
            })
            .1
        })
        .max()
        .unwrap_or_default();
    let final_node_count = runtime
        .read_model()
        .snapshot_for_scope(UiSubscriptionScope::WholeGraph)
        .nodes
        .len();

    eprintln!(
        "multiplex production edits: processor_nodes={} processor_transaction={}ms processor_apply={}us processor_publish={}us processor_rebuild_tick={}ms state_nodes={} state_transaction={}ms state_apply={}us state_publish={}us state_rebuild_tick={}ms",
        processor_node_count.saturating_sub(initial_node_count),
        processor_transaction_ms,
        processor_result.timing.apply.as_micros(),
        processor_result.timing.event_collect.as_micros(),
        processor_rebuild_tick_ms,
        final_node_count.saturating_sub(processor_node_count),
        state_transaction_ms,
        state_result.timing.apply.as_micros(),
        state_result.timing.event_collect.as_micros(),
        state_rebuild_tick_ms,
    );

    assert_eq!(
        processor_node_count.saturating_sub(initial_node_count),
        processor_source_nodes,
        "the production transaction must duplicate the complete processor subtree"
    );
    assert_eq!(
        final_node_count.saturating_sub(processor_node_count),
        state_source_nodes_at_transaction,
        "the production transaction must duplicate the complete state subtree"
    );
    if strict_serial_performance_assertions() {
        assert!(
            processor_transaction_ms < 100,
            "production processor duplicate took {processor_transaction_ms}ms"
        );
        assert!(
            processor_rebuild_tick_ms < 50,
            "post-production-processor-duplicate tick took {processor_rebuild_tick_ms}ms"
        );
        assert!(
            state_transaction_ms < 150,
            "production state duplicate took {state_transaction_ms}ms"
        );
        assert!(
            state_rebuild_tick_ms < 100,
            "post-production-state-duplicate tick took {state_rebuild_tick_ms}ms"
        );
    }
}

#[test]
fn sample_project_structure_operations_stay_interactive() {
    let _performance_guard = lock_performance_test();
    const SAMPLE: &str = "test_perf.noisette";

    let path = sample_project_path(SAMPLE);
    let (loaded, load_ms) = elapsed_ms(|| load_sparse_project_file::<AppNode, _>(&path).expect("sample should load"));
    let mut engine = loaded;
    let node_count = engine.nodes.len();
    let module_count = engine
        .nodes
        .iter()
        .filter(|(_, node)| node.user_item_kind() == MODULE_ITEM_KIND)
        .count();

    let (saved_json, save_ms) = best_elapsed_ms(3, || {
        to_sparse_project_json_pretty(&engine).expect("sample should serialize sparsely")
    });

    let (ui_snapshot, snapshot_ms) = best_elapsed_ms(3, || engine.ui_snapshot(UiSubscriptionScope::WholeGraph));
    let (read_model, read_model_ms) = best_elapsed_ms(3, || {
        UiReadModel::from_engine(&engine, ProjectFileSpec::new("Noisette", "noisette"))
    });

    let duplicate_source = first_node_by_item_kind(&engine, MODULE_ITEM_KIND)
        .expect("sample should contain at least one duplicable module");
    let previous_event_time = read_model.current_event_time();
    let (duplicated, duplicate_ms) =
        elapsed_ms(|| duplicate_node(&mut engine, duplicate_source).expect("duplicate should apply"));
    let duplicate_node_count = engine.nodes.len().saturating_sub(node_count);

    let (capture, collect_ms) = elapsed_ms(|| read_model.collect_event_batch(&engine, previous_event_time));
    let event_count = capture.batch().events.len();
    let (_batch, apply_capture_ms) = elapsed_ms(|| read_model.apply_event_capture(capture));

    eprintln!(
        "sample {SAMPLE}: nodes={node_count} modules={module_count} saved_bytes={} ui_nodes={} duplicated={:?} duplicated_nodes={duplicate_node_count}",
        saved_json.len(),
        ui_snapshot.nodes.len(),
        duplicated,
    );
    eprintln!(
        "sample {SAMPLE}: load={load_ms}ms save={save_ms}ms ui_snapshot={snapshot_ms}ms read_model={read_model_ms}ms duplicate={duplicate_ms}ms collect_events={collect_ms}ms apply_capture={apply_capture_ms}ms events={event_count}",
    );

    assert!(load_ms < 1_500, "sample load took {load_ms}ms for {node_count} nodes");
    assert!(save_ms < 250, "sample save took {save_ms}ms for {node_count} nodes");
    assert!(
        snapshot_ms < 250,
        "whole-graph UI snapshot took {snapshot_ms}ms for {node_count} nodes"
    );
    assert!(
        read_model_ms < 250,
        "read model build took {read_model_ms}ms for {node_count} nodes"
    );
    assert!(
        duplicate_ms < 400,
        "module duplicate took {duplicate_ms}ms for {node_count} existing nodes"
    );
    assert!(collect_ms < 50, "event capture took {collect_ms}ms after duplicate");
    assert!(
        apply_capture_ms < 50,
        "read model event apply took {apply_capture_ms}ms after duplicate"
    );
}

#[test]
fn sample_project_active_runtime_stays_responsive() {
    let _performance_guard = lock_performance_test();
    const SAMPLE: &str = "test_perf.noisette";
    const WARMUP: usize = 20;
    const MEASURED: usize = 80;
    let path = sample_project_path(SAMPLE);
    let mut engine = load_sparse_project_file::<AppNode, _>(&path).expect("sample should load");
    let node_count = engine.nodes.len();
    let (_, process_snapshot_ms) = elapsed_ms(|| engine.process_tree_snapshot());

    for _ in 0..WARMUP {
        std::thread::sleep(Duration::from_millis(16));
        engine
            .run_tick(Duration::from_millis(16))
            .expect("active sample tick should not fail");
    }

    let mut min_us = u64::MAX;
    let mut max_us = 0u64;
    let mut total_us = 0u64;
    for _ in 0..MEASURED {
        std::thread::sleep(Duration::from_millis(16));
        let started = Instant::now();
        engine
            .run_tick(Duration::from_millis(16))
            .expect("active sample tick should not fail");
        let elapsed = started.elapsed().as_micros() as u64;
        min_us = min_us.min(elapsed);
        max_us = max_us.max(elapsed);
        total_us += elapsed;
    }

    let duplicate_source = first_node_by_item_kind(&engine, MODULE_ITEM_KIND)
        .expect("sample should contain at least one duplicable module");
    let (_, duplicate_ms) =
        elapsed_ms(|| duplicate_node(&mut engine, duplicate_source).expect("duplicate should apply"));
    let avg_us = total_us / MEASURED as u64;

    eprintln!(
        "active sample {SAMPLE}: nodes={node_count} process_snapshot={process_snapshot_ms}ms tick_avg={avg_us}us tick_min={min_us}us tick_max={max_us}us duplicate={duplicate_ms}ms"
    );
}
