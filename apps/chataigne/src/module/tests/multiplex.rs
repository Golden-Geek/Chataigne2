use std::{
    collections::HashSet,
    time::{Duration, Instant},
};

use golden_core::{
    app::{
        configure_loaded_engine, from_sparse_project_json, load_sparse_project_file, prepare_engine_for_runtime,
        to_sparse_project_json_pretty, ProjectFileSpec, ProjectNode,
    },
    application::ProductionRuntime,
    edit::Edit,
    node::{Node, NodeId},
    parameter::{ParamValue, ParameterControlMode, ParameterEventBehaviour},
    ui_read_model::UiReadModel,
    ui_sync::{UiEditIntent, UiProjectFileSpec, UiSubscriptionScope},
};

use chataigne_state_machine::ProcessorOverviewDemandDto;

use crate::app::{module::MODULE_ITEM_KIND, AppNode};

use super::lock_performance_test;

/// Measures ticks while deterministically dirtying the multiplex source.
///
/// The sample's Signals worker is wall-clock driven. A tight optimized test loop can otherwise
/// outrun it and accidentally benchmark mostly idle ticks.
struct MultiplexTickMeasurements {
    elapsed_us: Vec<u64>,
    published_elapsed_us: Vec<u64>,
    published_events: usize,
    callbacks_fired: usize,
    ticks_with_callbacks: usize,
    snapshot_builds: usize,
}

fn measure_multiplex_source_ticks(
    engine: &mut crate::app::AppEngine,
    source: NodeId,
    n: usize,
    read_model: Option<&UiReadModel>,
) -> MultiplexTickMeasurements {
    let dt = Duration::from_millis(8);
    let mut elapsed_us = Vec::with_capacity(n);
    let mut published_elapsed_us = Vec::with_capacity(n);
    let mut published_events = 0usize;
    let mut callbacks_fired = 0usize;
    let mut ticks_with_callbacks = 0usize;
    let mut snapshot_builds = 0usize;
    for index in 0..n {
        let previous_event_time = read_model.and_then(|_| engine.ui_event_log().last().map(|event| event.time));
        engine.edits.push(Edit::SetParam {
            node: source,
            value: ParamValue::Float(if index % 2 == 0 { -1.0 } else { 2.0 }),
            behaviour: ParameterEventBehaviour::Coalesce,
        });
        let published_started = Instant::now();
        let tick_started = Instant::now();
        engine.run_tick(dt).expect("multiplex source tick should run");
        elapsed_us.push(tick_started.elapsed().as_micros() as u64);
        if let Some(read_model) = read_model {
            let capture = read_model.collect_event_batch(engine, previous_event_time);
            published_events += read_model.apply_event_capture(capture).events.len();
            published_elapsed_us.push(published_started.elapsed().as_micros() as u64);
        }
        let stats = engine.tick_stats();
        callbacks_fired += stats.callbacks_fired;
        ticks_with_callbacks += usize::from(stats.callbacks_fired > 0);
        snapshot_builds += stats.snapshot_builds;
    }
    MultiplexTickMeasurements {
        elapsed_us,
        published_elapsed_us,
        published_events,
        callbacks_fired,
        ticks_with_callbacks,
        snapshot_builds,
    }
}

fn percentile_us(sorted_samples: &[u64], percentile: usize) -> u64 {
    assert!(!sorted_samples.is_empty());
    assert!((1..=100).contains(&percentile));
    let rank = sorted_samples.len().saturating_mul(percentile).div_ceil(100);
    sorted_samples[rank.saturating_sub(1).min(sorted_samples.len() - 1)]
}

fn strict_serial_performance_assertions() -> bool {
    let args = std::env::args().collect::<Vec<_>>();
    args.iter().any(|arg| arg == "--test-threads=1")
        || args
            .windows(2)
            .any(|pair| pair[0] == "--test-threads" && pair[1] == "1")
}

fn sample_project_path(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("samples")
        .join(name)
}

fn context_provider_rebuilds(engine: &crate::app::AppEngine) -> u64 {
    engine
        .nodes
        .iter()
        .find_map(|(_, node)| match node {
            AppNode::StateMachineManager(manager) => Some(manager.runtime_perf_stats().context_provider_rebuilds),
            _ => None,
        })
        .expect("app should contain a state-machine manager")
}

fn state_machine_manager_id(engine: &crate::app::AppEngine) -> NodeId {
    engine
        .nodes
        .iter()
        .find_map(|(node_id, node)| matches!(node, AppNode::StateMachineManager(_)).then_some(node_id))
        .expect("app should contain a state-machine manager")
}

fn state_machine_debug_samples_captured(engine: &crate::app::AppEngine) -> u64 {
    engine
        .nodes
        .iter()
        .find_map(|(_, node)| match node {
            AppNode::StateMachineManager(manager) => Some(manager.runtime_perf_stats().debug_samples_captured),
            _ => None,
        })
        .expect("app should contain a state-machine manager")
}

#[derive(Clone, Copy, Debug)]
struct ProcessorBatchStats {
    command_batches: u64,
    batched_executions: u64,
    rejected_executions: u64,
    budget_rejected_actions: u64,
    budget_rejected_intents: u64,
}

impl ProcessorBatchStats {
    fn since(self, previous: Self) -> Self {
        Self {
            command_batches: self.command_batches - previous.command_batches,
            batched_executions: self.batched_executions - previous.batched_executions,
            rejected_executions: self.rejected_executions - previous.rejected_executions,
            budget_rejected_actions: self.budget_rejected_actions - previous.budget_rejected_actions,
            budget_rejected_intents: self.budget_rejected_intents - previous.budget_rejected_intents,
        }
    }
}

fn state_machine_processor_batch_stats(engine: &crate::app::AppEngine) -> ProcessorBatchStats {
    engine
        .nodes
        .iter()
        .find_map(|(_, node)| match node {
            AppNode::StateMachineManager(manager) => {
                let stats = manager.runtime_perf_stats();
                Some(ProcessorBatchStats {
                    command_batches: stats.processor_command_batches,
                    batched_executions: stats.processor_batched_executions,
                    rejected_executions: stats.processor_rejected_command_executions,
                    budget_rejected_actions: stats.processor_budget_rejected_command_actions,
                    budget_rejected_intents: stats.processor_budget_rejected_command_intents,
                })
            }
            _ => None,
        })
        .expect("app should contain a state-machine manager")
}

fn targeted_performance_sample_path(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("test-samples")
        .join(name)
}

fn elapsed_ms<T>(operation: impl FnOnce() -> T) -> (T, u128) {
    let started = Instant::now();
    let result = operation();
    (result, started.elapsed().as_millis())
}

fn max_tick_elapsed_ms(engine: &mut crate::app::AppEngine, ticks: usize) -> u128 {
    (0..ticks)
        .map(|_| {
            elapsed_ms(|| {
                engine
                    .run_tick(Duration::from_millis(8))
                    .expect("post-edit tick should run")
            })
            .1
        })
        .max()
        .unwrap_or_default()
}

fn best_elapsed_ms<T>(attempts: usize, mut operation: impl FnMut() -> T) -> (T, u128) {
    assert!(attempts > 0);

    let (mut best_result, mut best_ms) = elapsed_ms(&mut operation);
    for _ in 1..attempts {
        let (result, elapsed_ms) = elapsed_ms(&mut operation);
        if elapsed_ms < best_ms {
            best_result = result;
            best_ms = elapsed_ms;
        }
    }

    (best_result, best_ms)
}

fn first_node_by_item_kind(engine: &crate::app::AppEngine, item_kind: &str) -> Option<NodeId> {
    engine
        .nodes
        .iter()
        .find(|(_, node)| {
            node.user_item_kind() == item_kind
                && node.node_data().meta.user_permissions.can_remove_and_duplicate
                && node.node_data().parent.is_some()
        })
        .map(|(node_id, _)| node_id)
}

fn largest_duplicable_subtree_by_type(engine: &crate::app::AppEngine, node_type: &str) -> Option<(NodeId, usize)> {
    let snapshot = engine.process_tree_snapshot();
    engine
        .nodes
        .iter()
        .filter(|(_, node)| {
            node.get_type() == node_type
                && node.node_data().meta.user_permissions.can_remove_and_duplicate
                && node.node_data().parent.is_some()
        })
        .map(|(node_id, _)| {
            let mut pending = vec![node_id];
            let mut node_count = 0usize;
            while let Some(candidate) = pending.pop() {
                node_count += 1;
                pending.extend_from_slice(snapshot.child_ids_slice(candidate));
            }
            (node_id, node_count)
        })
        .max_by(|(left_id, left_count), (right_id, right_count)| {
            left_count.cmp(right_count).then_with(|| right_id.0.cmp(&left_id.0))
        })
}

fn multiplex_condition_source(engine: &crate::app::AppEngine, processor_count: usize) -> NodeId {
    let snapshot = engine.process_tree_snapshot();
    let conditions = engine
        .nodes
        .iter()
        .filter(|(_, node)| node.get_type() == "sm_input_value_condition")
        .map(|(node_id, _)| node_id)
        .collect::<Vec<_>>();
    assert_eq!(
        conditions.len(),
        processor_count,
        "each sample processor must have one multiplex input-value condition"
    );

    let mut sources = HashSet::new();
    for condition in conditions {
        let source_param = snapshot
            .find_child_by_decl_id(condition, "source")
            .expect("multiplex condition should expose its source reference");
        let source_value = snapshot
            .node(source_param)
            .and_then(|node| node.param_value.as_ref())
            .expect("multiplex condition source should have a value");
        let ParamValue::Reference(reference) = source_value else {
            panic!("multiplex condition source must be a node reference");
        };
        let source = snapshot
            .node_id_by_uuid(reference.uuid())
            .expect("multiplex condition source reference should resolve");
        assert!(
            snapshot
                .node(source)
                .and_then(|node| node.param_value.as_ref())
                .is_some_and(|value| matches!(value, ParamValue::Float(_))),
            "multiplex condition source must resolve to a floating-point parameter"
        );

        let threshold = snapshot
            .find_child_by_decl_id(condition, "reference")
            .expect("multiplex condition should expose its threshold parameter");
        assert!(
            snapshot
                .node(threshold)
                .and_then(|node| node.param_control.as_ref())
                .is_some_and(|control| control.mode == ParameterControlMode::ContextLink),
            "multiplex condition threshold must be context-linked"
        );
        sources.insert(source);
    }

    assert_eq!(
        sources.len(),
        1,
        "all multiplex conditions must observe the same signal source"
    );
    let source = sources.into_iter().next().expect("one source was asserted");
    let mut ancestor = Some(source);
    let mut belongs_to_signals_module = false;
    while let Some(node_id) = ancestor {
        let node = snapshot.node(node_id).expect("source ancestry should remain valid");
        if node.node_type == "signals_module" {
            belongs_to_signals_module = true;
            break;
        }
        ancestor = node.parent;
    }
    assert!(
        belongs_to_signals_module,
        "multiplex condition source must belong to the sample Signals module"
    );
    source
}

fn duplicate_node(
    engine: &mut crate::app::AppEngine,
    source: NodeId,
) -> Result<NodeId, golden_core::engine::ProjectPersistenceError> {
    let source_node = engine.nodes.get(source).expect("duplicate source should exist");
    let parent = source_node
        .node_data()
        .parent
        .expect("duplicate source should have a parent");
    engine.duplicate_subtree_with(
        source,
        parent,
        Some(source),
        None,
        |node| node.project_encode_data(),
        AppNode::project_decode_node,
    )
}

#[test]
fn simple_sample_project_loads_and_round_trips() {
    const SAMPLE: &str = "test_simple_load.noisette";

    let path = sample_project_path(SAMPLE);
    let engine = load_sparse_project_file::<AppNode, _>(&path).expect("simple sample should load");
    assert_eq!(
        engine
            .nodes
            .iter()
            .filter(|(_, node)| node.get_type() == "signals_module")
            .count(),
        1,
        "simple sample should contain one signals module"
    );
    assert_eq!(
        engine
            .nodes
            .iter()
            .filter(|(_, node)| node.get_type() == "state_processor")
            .count(),
        1,
        "simple sample should contain one state action"
    );

    let saved_json = to_sparse_project_json_pretty(&engine).expect("simple sample should save");
    let reloaded = from_sparse_project_json::<AppNode>(&saved_json).expect("saved simple sample should reload");
    assert_eq!(
        reloaded
            .nodes
            .iter()
            .filter(|(_, node)| node.get_type() == "signals_module")
            .count(),
        1
    );
}

#[test]
fn multiplex_sample_project_loads_and_round_trips() {
    const SAMPLE: &str = "test_multiplex.noisette";

    let path = sample_project_path(SAMPLE);
    let engine = load_sparse_project_file::<AppNode, _>(&path).expect("multiplex sample should load");
    assert_eq!(
        engine
            .nodes
            .iter()
            .filter(|(_, node)| node.get_type() == "signals_module")
            .count(),
        1,
        "multiplex sample should contain one signals module"
    );

    let saved_json = to_sparse_project_json_pretty(&engine).expect("multiplex sample should save");
    from_sparse_project_json::<AppNode>(&saved_json).expect("saved multiplex sample should reload");
}

#[test]
fn multiplex_sample_active_runtime_stays_realtime() {
    let _performance_guard = lock_performance_test();
    const SAMPLE: &str = "test_multiplex.noisette";
    const WARMUP: usize = 10;
    const DIRTY_WARMUP: usize = 8;
    // Cover initial bounded log-stream draining and the 200-tick keepalive window.
    const MEASURED: usize = 240;

    let path = targeted_performance_sample_path(SAMPLE);
    let mut engine = load_sparse_project_file::<AppNode, _>(&path).expect("multiplex sample should load");
    let processor_ids = engine
        .nodes
        .iter()
        .filter(|(_, node)| node.get_type() == "state_processor")
        .map(|(_, node)| node.node_data().meta.uuid.0.to_string())
        .collect::<Vec<_>>();
    let processor_count = processor_ids.len();
    assert!(
        processor_count >= 5,
        "the performance regression must exercise at least five sample processors"
    );
    configure_loaded_engine(&mut engine).expect("multiplex sample should configure");
    prepare_engine_for_runtime(&mut engine).expect("multiplex sample should prepare");
    let overview_demand = ProcessorOverviewDemandDto {
        subscription_id: "multiplex-performance-overview".to_owned(),
        processor_ids,
    };
    let manager_id = state_machine_manager_id(&engine);
    let overview_ack = engine.apply_ui_intent(UiEditIntent::SendNodeEvent {
        node: manager_id,
        topic: "chataigne.state_machine.processor_overview_demand".to_owned(),
        payload: serde_json::to_value(overview_demand).expect("overview demand should serialize"),
    });
    assert!(overview_ack.success, "processor overview demand should apply");

    for _ in 0..WARMUP {
        engine
            .run_tick(Duration::from_millis(8))
            .expect("multiplex warmup tick should run");
    }

    let source = multiplex_condition_source(&engine, processor_count);
    let dirty_warmup = measure_multiplex_source_ticks(&mut engine, source, DIRTY_WARMUP, None);
    assert_eq!(
        dirty_warmup.ticks_with_callbacks, DIRTY_WARMUP,
        "dirty-path warmup must exercise scheduled runtime work"
    );
    let read_model = UiReadModel::from_engine(&engine, ProjectFileSpec::new("Noisette", "noisette"));
    let provider_rebuilds_before = context_provider_rebuilds(&engine);
    let debug_samples_before = state_machine_debug_samples_captured(&engine);
    let batch_stats_before = state_machine_processor_batch_stats(&engine);
    let measurements = measure_multiplex_source_ticks(&mut engine, source, MEASURED, Some(&read_model));
    let provider_rebuilds_after = context_provider_rebuilds(&engine);
    let debug_samples_after = state_machine_debug_samples_captured(&engine);
    let batch_stats = state_machine_processor_batch_stats(&engine).since(batch_stats_before);
    let negative_source_avg_us = measurements.elapsed_us.iter().step_by(2).sum::<u64>() / (MEASURED / 2) as u64;
    let positive_source_avg_us = measurements.elapsed_us.iter().skip(1).step_by(2).sum::<u64>() / (MEASURED / 2) as u64;
    let mut elapsed_us = measurements.elapsed_us;
    elapsed_us.sort_unstable();
    let min_us = elapsed_us[0];
    let max_us = elapsed_us[MEASURED - 1];
    let total_us = elapsed_us.iter().sum::<u64>();
    let avg_us = total_us / MEASURED as u64;
    let p95_us = percentile_us(&elapsed_us, 95);
    let p99_us = percentile_us(&elapsed_us, 99);
    let deadline_misses = elapsed_us.iter().filter(|elapsed| **elapsed >= 10_000).count();
    let mut published_elapsed_us = measurements.published_elapsed_us;
    published_elapsed_us.sort_unstable();
    let published_avg_us = published_elapsed_us.iter().sum::<u64>() / published_elapsed_us.len() as u64;
    let published_p95_us = percentile_us(&published_elapsed_us, 95);
    let published_p99_us = percentile_us(&published_elapsed_us, 99);
    let published_deadline_misses = published_elapsed_us
        .iter()
        .filter(|elapsed| **elapsed >= 10_000)
        .count();
    eprintln!(
        "multiplex runtime: avg={avg_us}us negative_avg={negative_source_avg_us}us positive_avg={positive_source_avg_us}us p95={p95_us}us p99={p99_us}us min={min_us}us max={max_us}us deadline_misses={deadline_misses} published_avg={published_avg_us}us published_p95={published_p95_us}us published_p99={published_p99_us}us published_deadline_misses={published_deadline_misses} published_events={} callbacks={} callback_ticks={} snapshot_builds={} provider_rebuilds={} budget_rejected_actions={} budget_rejected_intents={}",
        measurements.published_events,
        measurements.callbacks_fired,
        measurements.ticks_with_callbacks,
        measurements.snapshot_builds,
        provider_rebuilds_after - provider_rebuilds_before,
        batch_stats.budget_rejected_actions,
        batch_stats.budget_rejected_intents,
    );
    assert_eq!(
        measurements.ticks_with_callbacks, MEASURED,
        "every dirty measured tick must execute scheduled runtime work"
    );
    assert_eq!(
        measurements.snapshot_builds, 0,
        "all steady multiplex ticks must reuse the state runtime snapshot"
    );
    assert_eq!(
        debug_samples_after, debug_samples_before,
        "the all-processor overview must not enable Alchemist debug capture"
    );
    assert!(
        batch_stats.batched_executions > 0,
        "the sample's output containers should use ordered command batches"
    );
    assert!(
        batch_stats.command_batches.saturating_mul(8) < batch_stats.batched_executions,
        "command batching must collapse lane fan-out: {} batches for {} executions",
        batch_stats.command_batches,
        batch_stats.batched_executions,
    );
    assert_eq!(
        batch_stats.rejected_executions, 0,
        "the checked multiplex sample must not emit non-finite command overrides"
    );
    assert_eq!(
        (batch_stats.budget_rejected_actions, batch_stats.budget_rejected_intents,),
        (0, 0),
        "the checked multiplex sample must fit within the explicit per-tick command budget"
    );
    assert!(
        measurements.published_events >= MEASURED,
        "every dirty tick must publish at least its source-value event"
    );
    if strict_serial_performance_assertions() {
        assert!(
            avg_us < 5_000,
            "serial multiplex runtime averaged {avg_us}us per dirty tick; the 200 Hz development budget is 5000us"
        );
        assert!(
            p95_us < 10_000,
            "serial multiplex runtime p95 reached {p95_us}us"
        );
        assert!(
            p99_us < 10_000,
            "serial multiplex runtime p99 reached {p99_us}us; dirty compute must stay inside the 100 Hz deadline"
        );
        assert!(
            deadline_misses <= 2,
            "serial multiplex runtime missed the 100 Hz deadline {deadline_misses} times; at most two host-deschedule outliers are allowed"
        );
        assert!(
            published_avg_us < 6_000,
            "serial multiplex runtime plus incremental UI publication averaged {published_avg_us}us; the full dev-host path must stay responsive"
        );
        assert!(
            published_p95_us < 10_000,
            "serial multiplex runtime plus incremental UI publication p95 reached {published_p95_us}us"
        );
        assert!(
            published_p99_us < 10_000,
            "serial multiplex runtime plus incremental UI publication p99 reached {published_p99_us}us"
        );
        assert!(
            published_deadline_misses <= 2,
            "serial multiplex runtime plus UI publication missed the 100 Hz deadline {published_deadline_misses} times"
        );
    }
}

#[test]
fn multiplex_sample_production_runtime_stays_realtime() {
    let _performance_guard = lock_performance_test();
    const SAMPLE: &str = "test_multiplex.noisette";
    const WARMUP: usize = 10;
    const MEASURED: usize = 240;

    let path = targeted_performance_sample_path(SAMPLE);
    let mut engine = load_sparse_project_file::<AppNode, _>(&path).expect("multiplex sample should load");
    let processor_ids = engine
        .nodes
        .iter()
        .filter(|(_, node)| node.get_type() == "state_processor")
        .map(|(_, node)| node.node_data().meta.uuid.0.to_string())
        .collect::<Vec<_>>();
    let processor_count = processor_ids.len();
    assert!(
        processor_count >= 5,
        "the production regression must exercise at least five sample processors"
    );
    configure_loaded_engine(&mut engine).expect("multiplex sample should configure");
    prepare_engine_for_runtime(&mut engine).expect("multiplex sample should prepare");
    let manager_id = state_machine_manager_id(&engine);
    let overview_ack = engine.apply_ui_intent(UiEditIntent::SendNodeEvent {
        node: manager_id,
        topic: "chataigne.state_machine.processor_overview_demand".to_owned(),
        payload: serde_json::to_value(ProcessorOverviewDemandDto {
            subscription_id: "multiplex-production-performance-overview".to_owned(),
            processor_ids,
        })
        .expect("overview demand should serialize"),
    });
    assert!(overview_ack.success, "processor overview demand should apply");
    let source = multiplex_condition_source(&engine, processor_count);
    let runtime = ProductionRuntime::new(
        engine,
        UiProjectFileSpec::from_project_file_spec(ProjectFileSpec::new("Noisette", "noisette"), None),
    );
    let input = runtime.input_port();

    for index in 0..WARMUP {
        input
            .publish(
                source,
                ParamValue::Float(if index % 2 == 0 { -1.0 } else { 2.0 }),
                index as u64 + 1,
            )
            .expect("multiplex warmup input should publish");
        runtime
            .run_tick(Duration::from_millis(8))
            .expect("multiplex warmup tick should run");
    }

    let mut elapsed_us = Vec::with_capacity(MEASURED);
    let mut published_events = 0usize;
    for index in 0..MEASURED {
        input
            .publish(
                source,
                ParamValue::Float(if index % 2 == 0 { -1.0 } else { 2.0 }),
                (WARMUP + index) as u64 + 1,
            )
            .expect("multiplex measured input should publish");
        let started = Instant::now();
        let result = runtime
            .run_tick(Duration::from_millis(8))
            .expect("multiplex measured tick should run");
        elapsed_us.push(started.elapsed().as_micros() as u64);
        published_events += result.events.events.len();
    }

    elapsed_us.sort_unstable();
    let avg_us = elapsed_us.iter().sum::<u64>() / MEASURED as u64;
    let p95_us = percentile_us(&elapsed_us, 95);
    let p99_us = percentile_us(&elapsed_us, 99);
    let min_us = elapsed_us[0];
    let max_us = elapsed_us[MEASURED - 1];
    let deadline_misses = elapsed_us.iter().filter(|elapsed| **elapsed >= 10_000).count();
    eprintln!(
        "multiplex production runtime: avg={avg_us}us p95={p95_us}us p99={p99_us}us min={min_us}us max={max_us}us deadline_misses={deadline_misses} published_events={published_events}"
    );

    assert!(
        published_events >= MEASURED,
        "every production tick must publish at least its source-value event"
    );
    if strict_serial_performance_assertions() {
        assert!(
            avg_us < 6_000,
            "serial production runtime averaged {avg_us}us; the full control/read-model path must stay responsive"
        );
        assert!(
            p95_us < 10_000,
            "serial production runtime p95 reached {p95_us}us"
        );
        assert!(
            p99_us < 10_000,
            "serial production runtime p99 reached {p99_us}us; the full path must stay inside the 100 Hz deadline"
        );
        assert!(
            deadline_misses <= 2,
            "serial production runtime missed the 100 Hz deadline {deadline_misses} times"
        );
    }
}

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
