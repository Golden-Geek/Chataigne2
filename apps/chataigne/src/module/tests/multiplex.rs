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
struct ProcessorRuntimeStats {
    input_preparation_ns: u64,
    evaluation_ns: u64,
    evaluation_calls: u64,
    lanes_evaluated: u64,
    command_batches: u64,
    batched_executions: u64,
    rejected_executions: u64,
    budget_rejected_actions: u64,
    budget_rejected_intents: u64,
}

impl ProcessorRuntimeStats {
    fn since(self, previous: Self) -> Self {
        Self {
            input_preparation_ns: self.input_preparation_ns - previous.input_preparation_ns,
            evaluation_ns: self.evaluation_ns - previous.evaluation_ns,
            evaluation_calls: self.evaluation_calls - previous.evaluation_calls,
            lanes_evaluated: self.lanes_evaluated - previous.lanes_evaluated,
            command_batches: self.command_batches - previous.command_batches,
            batched_executions: self.batched_executions - previous.batched_executions,
            rejected_executions: self.rejected_executions - previous.rejected_executions,
            budget_rejected_actions: self.budget_rejected_actions - previous.budget_rejected_actions,
            budget_rejected_intents: self.budget_rejected_intents - previous.budget_rejected_intents,
        }
    }
}

fn state_machine_processor_runtime_stats(engine: &crate::app::AppEngine) -> ProcessorRuntimeStats {
    engine
        .nodes
        .iter()
        .find_map(|(_, node)| match node {
            AppNode::StateMachineManager(manager) => {
                let stats = manager.runtime_perf_stats();
                Some(ProcessorRuntimeStats {
                    input_preparation_ns: stats.processor_input_preparation_ns,
                    evaluation_ns: stats.processor_evaluation_ns,
                    evaluation_calls: stats.processor_evaluation_calls,
                    lanes_evaluated: stats.processor_lanes_evaluated,
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

mod runtime;

mod interaction;
