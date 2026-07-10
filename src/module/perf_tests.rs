use std::time::{Duration, Instant};

use golden_core::{
    app::{
        from_sparse_project_json, load_sparse_project_file, to_sparse_project_json_pretty,
        ProjectFileSpec, ProjectNode,
    },
    node::{Folder, Node, NodeId},
    ui_read_model::UiReadModel,
    ui_sync::UiSubscriptionScope,
};

use crate::app::{AppNode, GamepadModule, MidiModule};

fn create_engine(n_gamepad: usize, n_midi: usize) -> crate::app::AppEngine {
    let root: AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);

    for _ in 0..n_gamepad {
        let mut m = GamepadModule::create();
        m.disable_runtime_for_test();
        engine.add_node(m.into(), None);
    }
    for _ in 0..n_midi {
        engine.add_node(MidiModule::create().into(), None);
    }

    // Apply edits until stable (modules declare child nodes during init)
    for _ in 0..20 {
        engine.apply_edits().expect("module init should not fail");
    }
    engine.resolve().expect("schedule should resolve");
    engine
}

/// Run N ticks and return (min_us, max_us, total_us).
fn measure_ticks(engine: &mut crate::app::AppEngine, n: usize) -> (u64, u64, u64) {
    let dt = Duration::from_millis(8); // ~120 Hz
    let mut min_us = u64::MAX;
    let mut max_us = 0u64;
    let mut total_us = 0u64;
    for _ in 0..n {
        let t = Instant::now();
        engine.run_tick(dt).expect("tick should not fail");
        let elapsed = t.elapsed().as_micros() as u64;
        if elapsed < min_us { min_us = elapsed; }
        if elapsed > max_us { max_us = elapsed; }
        total_us += elapsed;
    }
    (min_us, max_us, total_us)
}

fn sample_project_path(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("test-samples")
        .join(name)
}

fn direct_child_by_decl(
    engine: &crate::app::AppEngine,
    parent: NodeId,
    decl_id: &str,
) -> Option<NodeId> {
    engine
        .nodes
        .iter()
        .find(|(_, node)| {
            node.node_data().parent == Some(parent)
                && node.node_data().meta.decl_id.0 == decl_id
        })
        .map(|(node_id, _)| node_id)
}

fn first_node_by_label(engine: &crate::app::AppEngine, label: &str) -> Option<NodeId> {
    engine
        .nodes
        .iter()
        .find(|(_, node)| node.node_data().meta.label == label)
        .map(|(node_id, _)| node_id)
}

fn ancestor_by_type(
    engine: &crate::app::AppEngine,
    node: NodeId,
    node_type: &str,
) -> Option<NodeId> {
    let mut current = engine
        .nodes
        .get(node)
        .and_then(|candidate| candidate.node_data().parent);
    while let Some(node_id) = current {
        let candidate = engine.nodes.get(node_id)?;
        if candidate.get_type() == node_type {
            return Some(node_id);
        }
        current = candidate.node_data().parent;
    }
    None
}

fn run_full_engine_tick(engine: &mut crate::app::AppEngine, delta: Duration) {
    engine
        .dispatch_inbox(golden_core::process_ctx::ExecutionPhase::EngineTick)
        .expect("engine inbox should dispatch");
    engine.run_tick(delta).expect("engine tick should run");
    engine.apply_edits().expect("engine tick edits should apply");
}

fn elapsed_ms<T>(operation: impl FnOnce() -> T) -> (T, u128) {
    let started = Instant::now();
    let result = operation();
    (result, started.elapsed().as_millis())
}

fn best_elapsed_ms<T>(attempts: usize, mut operation: impl FnMut() -> T) -> (T, u128) {
    assert!(attempts > 0);

    let (mut best_result, mut best_ms) = elapsed_ms(|| operation());
    for _ in 1..attempts {
        let (result, elapsed_ms) = elapsed_ms(|| operation());
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

fn duplicate_node(
    engine: &mut crate::app::AppEngine,
    source: NodeId,
) -> Result<NodeId, golden_core::engine::ProjectPersistenceError> {
    let source_node = engine
        .nodes
        .get(source)
        .expect("duplicate source should exist");
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
        |node_type, data, meta| AppNode::project_decode_node(node_type, data, meta),
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
    let reloaded =
        from_sparse_project_json::<AppNode>(&saved_json).expect("saved simple sample should reload");
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
#[ignore = "AAA performance benchmark: run explicitly with --ignored --nocapture"]
fn benchmark_multiplex_sample_active_actions_with_runtime_preview() {
    const SAMPLE: &str = "test_multiplex.noisette";
    const WARMUP: usize = 5;
    const MEASURED: usize = 30;
    const DELTA: Duration = Duration::from_micros(16_667);

    let path = sample_project_path(SAMPLE);
    let mut engine = load_sparse_project_file::<AppNode, _>(&path).expect("multiplex sample should load");
    golden_core::app::configure_loaded_engine(&mut engine)
        .expect("multiplex sample runtime should configure");
    golden_core::app::prepare_engine_for_runtime(&mut engine)
        .expect("multiplex sample runtime should prepare");
    let formula = first_node_by_label(&engine, "ActionTest").expect("ActionTest formula should exist");
    let formula_uuid = engine
        .nodes
        .get(formula)
        .expect("ActionTest formula should exist")
        .node_data()
        .meta
        .uuid;
    let actions = ["MAction", "MAction 2"].map(|label| {
        let action = first_node_by_label(&engine, label)
            .unwrap_or_else(|| panic!("multiplex sample should contain {label}"));
        assert_eq!(
            engine.nodes.get(action).expect("action should exist").get_type(),
            "state_processor",
            "{label} should identify the processor itself",
        );
        action
    });
    let manager_id = ancestor_by_type(&engine, actions[0], "state_machine_manager")
        .expect("MAction should belong to a state machine manager");
    for action in actions {
        let formula_param = direct_child_by_decl(&engine, action, "formula")
            .expect("processor Formula parameter should exist");
        engine.edits.push(golden_core::edit::Edit::PatchMeta {
            node: action,
            patch: golden_core::node::NodeMetaPatch {
                enabled: Some(true),
                ..Default::default()
            },
        });
        engine.edits.push(golden_core::edit::Edit::SetParam {
            node: formula_param,
            value: golden_core::parameter::ParamValue::Reference(
                golden_core::node::NodeReference::new(formula_uuid),
            ),
            behaviour: golden_core::parameter::ParameterEventBehaviour::Coalesce,
        });
    }
    engine.apply_edits().expect("benchmark setup should apply");
    let preview_processors = engine
        .nodes
        .iter()
        .filter(|(_, node)| node.get_type() == "state_processor")
        .map(|(_, node)| node.node_data().meta.uuid.0.to_string())
        .collect::<Vec<_>>();
    for processor_id in preview_processors {
        let ack = engine.apply_ui_intent_from_client(
            golden_core::ui_sync::UiEditIntent::SetRuntimeViewInterest {
                view_id: format!("benchmark-processor-{processor_id}"),
                topic: "chataigne.state_machine.runtime_preview_interest".to_string(),
                payload: Some(serde_json::json!({
                    "kind": "processor_default_lane",
                    "processor_id": processor_id,
                })),
            },
            Some("multiplex-benchmark"),
        );
        assert!(ack.success, "benchmark preview interest should apply");
    }
    for _ in 0..4 {
        run_full_engine_tick(&mut engine, DELTA);
    }
    let preview_lanes = engine
        .nodes
        .get(manager_id)
        .and_then(|node| match node {
            crate::app::AppNode::StateMachineManager(manager) => {
                Some(manager.runtime_preview_lanes())
            }
            _ => None,
        })
        .expect("state machine manager should exist");
    let bootstrap_view_ids = engine
        .nodes
        .iter()
        .filter(|(_, node)| node.get_type() == "state_processor")
        .map(|(_, node)| {
            format!(
                "benchmark-processor-{}",
                node.node_data().meta.uuid.0
            )
        })
        .collect::<Vec<_>>();
    for view_id in bootstrap_view_ids {
        let ack = engine.apply_ui_intent_from_client(
            golden_core::ui_sync::UiEditIntent::SetRuntimeViewInterest {
                view_id,
                topic: "chataigne.state_machine.runtime_preview_interest".to_string(),
                payload: None,
            },
            Some("multiplex-benchmark"),
        );
        assert!(ack.success, "benchmark bootstrap interest should clear");
    }
    let mut previewed_processors = std::collections::HashSet::new();
    for (view_index, lane) in preview_lanes.into_iter().enumerate() {
        if !previewed_processors.insert(lane.processor_id.clone()) {
            continue;
        }
        if previewed_processors.len() > 2 {
            break;
        }
        let ack = engine.apply_ui_intent_from_client(
            golden_core::ui_sync::UiEditIntent::SetRuntimeViewInterest {
                view_id: format!("benchmark-observer-{view_index}"),
                topic: "chataigne.state_machine.runtime_preview_interest".to_string(),
                payload: Some(serde_json::json!({
                    "kind": "processor_lane",
                    "processor_id": lane.processor_id,
                    "context_key": lane.context_key,
                })),
            },
            Some("multiplex-benchmark"),
        );
        assert!(ack.success, "benchmark lane preview interest should apply");
    }

    let signal = engine
        .nodes
        .iter()
        .find(|(_, node)| {
            node.node_data().meta.uuid.0.to_string() == "15673b51-9ef7-4ff3-8415-af13d32de7c3"
        })
        .map(|(node_id, _)| node_id)
        .expect("Signal source should resolve");
    for tick in 0..WARMUP {
        engine.edits.push(golden_core::edit::Edit::SetParam {
            node: signal,
            value: golden_core::parameter::ParamValue::Float(tick as f64 / WARMUP as f64),
            behaviour: golden_core::parameter::ParameterEventBehaviour::Coalesce,
        });
        engine.apply_edits().expect("warmup Signal edit should apply");
        run_full_engine_tick(&mut engine, DELTA);
    }

    let manager_stats_before = engine
        .nodes
        .get(manager_id)
        .and_then(|node| match node {
            crate::app::AppNode::StateMachineManager(manager) => Some(manager.runtime_perf_stats()),
            _ => None,
        })
        .expect("state machine manager should exist");
    let mut samples = Vec::with_capacity(MEASURED);
    for tick in 0..MEASURED {
        let phase = tick as f64 / MEASURED as f64 * std::f64::consts::TAU;
        engine.edits.push(golden_core::edit::Edit::SetParam {
            node: signal,
            value: golden_core::parameter::ParamValue::Float(phase.sin()),
            behaviour: golden_core::parameter::ParameterEventBehaviour::Coalesce,
        });
        engine.apply_edits().expect("measured Signal edit should apply");
        let started = Instant::now();
        run_full_engine_tick(&mut engine, DELTA);
        samples.push(started.elapsed());
    }
    let manager_stats_after = engine
        .nodes
        .get(manager_id)
        .and_then(|node| match node {
            crate::app::AppNode::StateMachineManager(manager) => Some(manager.runtime_perf_stats()),
            _ => None,
        })
        .expect("state machine manager should exist");

    samples.sort_unstable();
    let percentile = |percent: usize| {
        samples[(samples.len() - 1) * percent / 100].as_secs_f64() * 1_000.0
    };
    let evaluations = manager_stats_after.processor_evaluations
        - manager_stats_before.processor_evaluations;
    let lanes = manager_stats_after.lanes_evaluated - manager_stats_before.lanes_evaluated;
    let command_intents = manager_stats_after.command_intents_dispatched
        - manager_stats_before.command_intents_dispatched;
    let elapsed_ms = |after: u64, before: u64| (after - before) as f64 / 1_000_000.0;
    eprintln!(
        "multiplex_sample ticks={MEASURED} p50_ms={:.3} p95_ms={:.3} p99_ms={:.3} evaluations={evaluations} lanes={lanes} command_intents={command_intents} provider_ms={:.3} inputs_ms={:.3} semantic_ms={:.3} projection_ms={:.3} inspection_ms={:.3} effects_ms={:.3} aggregation_ms={:.3} publish_ms={:.3} manager_total_ms={:.3}",
        percentile(50),
        percentile(95),
        percentile(99),
        elapsed_ms(manager_stats_after.context_provider_ns, manager_stats_before.context_provider_ns),
        elapsed_ms(manager_stats_after.runtime_inputs_ns, manager_stats_before.runtime_inputs_ns),
        elapsed_ms(manager_stats_after.semantic_evaluation_ns, manager_stats_before.semantic_evaluation_ns),
        elapsed_ms(manager_stats_after.preview_projection_ns, manager_stats_before.preview_projection_ns),
        elapsed_ms(manager_stats_after.lane_preview_ns, manager_stats_before.lane_preview_ns),
        elapsed_ms(manager_stats_after.lane_effects_ns, manager_stats_before.lane_effects_ns),
        elapsed_ms(manager_stats_after.preview_aggregation_ns, manager_stats_before.preview_aggregation_ns),
        elapsed_ms(manager_stats_after.preview_publish_ns, manager_stats_before.preview_publish_ns),
        elapsed_ms(manager_stats_after.run_processors_ns, manager_stats_before.run_processors_ns),
    );
    assert!(evaluations >= MEASURED as u64);
    assert_eq!(lanes, evaluations * 127);
}

#[test]
fn sample_project_structure_operations_stay_interactive() {
    const SAMPLE: &str = "test_perf.noisette";

    let path = sample_project_path(SAMPLE);
    let (loaded, load_ms) =
        elapsed_ms(|| load_sparse_project_file::<AppNode, _>(&path).expect("sample should load"));
    let mut engine = loaded;
    let node_count = engine.nodes.len();
    let module_count = engine
        .nodes
        .iter()
        .filter(|(_, node)| node.user_item_kind() == super::MODULE_ITEM_KIND)
        .count();

    let (saved_json, save_ms) = best_elapsed_ms(3, || {
        to_sparse_project_json_pretty(&engine).expect("sample should serialize sparsely")
    });

    let (ui_snapshot, snapshot_ms) =
        best_elapsed_ms(3, || engine.ui_snapshot(UiSubscriptionScope::WholeGraph));
    let (read_model, read_model_ms) = best_elapsed_ms(3, || {
        UiReadModel::from_engine(&engine, ProjectFileSpec::new("Noisette", "noisette"))
    });

    let duplicate_source = first_node_by_item_kind(&engine, super::MODULE_ITEM_KIND)
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

    assert!(
        load_ms < 1_500,
        "sample load took {load_ms}ms for {node_count} nodes"
    );
    assert!(
        save_ms < 250,
        "sample save took {save_ms}ms for {node_count} nodes"
    );
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
    assert!(
        collect_ms < 50,
        "event capture took {collect_ms}ms after duplicate"
    );
    assert!(
        apply_capture_ms < 50,
        "read model event apply took {apply_capture_ms}ms after duplicate"
    );
}

#[test]
fn sample_project_active_runtime_stays_responsive() {
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

    let duplicate_source = first_node_by_item_kind(&engine, super::MODULE_ITEM_KIND)
        .expect("sample should contain at least one duplicable module");
    let (_, duplicate_ms) =
        elapsed_ms(|| duplicate_node(&mut engine, duplicate_source).expect("duplicate should apply"));
    let avg_us = total_us / MEASURED as u64;

    eprintln!(
        "active sample {SAMPLE}: nodes={node_count} process_snapshot={process_snapshot_ms}ms tick_avg={avg_us}us tick_min={min_us}us tick_max={max_us}us duplicate={duplicate_ms}ms"
    );
}

#[test]
fn idle_gamepad_modules_tick_time_does_not_scale_with_count() {
    const WARMUP: usize = 20;
    const MEASURED: usize = 100;

    let mut e1 = create_engine(1, 0);
    let mut e20 = create_engine(20, 0);

    // Warmup — let each engine settle fully before measuring
    for _ in 0..WARMUP {
        e1.run_tick(Duration::from_millis(8)).ok();
        e20.run_tick(Duration::from_millis(8)).ok();
    }

    let (min1, max1, total1) = measure_ticks(&mut e1, MEASURED);
    let (min20, max20, total20) = measure_ticks(&mut e20, MEASURED);

    let avg1 = total1 / MEASURED as u64;
    let avg20 = total20 / MEASURED as u64;

    eprintln!("  1 gamepad: avg={avg1}us  min={min1}us  max={max1}us  (total {total1}us)");
    eprintln!(" 20 gamepad: avg={avg20}us  min={min20}us  max={max20}us  (total {total20}us)");

    // 20 idle modules should tick in less than 4× the time of 1 idle module.
    // In a correct implementation both should be ~0.
    assert!(
        avg20 < avg1.max(500) * 4,
        "20 gamepad modules tick avg {avg20}us vs 1 module avg {avg1}us — scaling too high"
    );
}

#[test]
fn idle_midi_modules_tick_time_does_not_scale_with_count() {
    const WARMUP: usize = 20;
    const MEASURED: usize = 100;

    let mut e1 = create_engine(0, 1);
    let mut e30 = create_engine(0, 30);

    for _ in 0..WARMUP {
        e1.run_tick(Duration::from_millis(8)).ok();
        e30.run_tick(Duration::from_millis(8)).ok();
    }

    let (min1, max1, total1) = measure_ticks(&mut e1, MEASURED);
    let (min30, max30, total30) = measure_ticks(&mut e30, MEASURED);

    let avg1 = total1 / MEASURED as u64;
    let avg30 = total30 / MEASURED as u64;

    eprintln!("  1 midi: avg={avg1}us  min={min1}us  max={max1}us  (total {total1}us)");
    eprintln!(" 30 midi: avg={avg30}us  min={min30}us  max={max30}us  (total {total30}us)");

    assert!(
        avg30 < avg1.max(500) * 4,
        "30 midi modules tick avg {avg30}us vs 1 module avg {avg1}us — scaling too high"
    );
}

#[test]
fn steady_state_tick_budget_1000_nodes() {
    const WARMUP: usize = 20;
    const MEASURED: usize = 100;
    // 10 gamepad modules ≈ 10 × ~100 nodes = ~1000 total nodes
    let mut engine = create_engine(10, 0);

    for _ in 0..WARMUP {
        engine.run_tick(Duration::from_millis(8)).ok();
    }

    let (_min, max, total) = measure_ticks(&mut engine, MEASURED);
    let avg = total / MEASURED as u64;

    eprintln!(" 10 gamepad (~1000 nodes): avg={avg}us  max={max}us  (total {total}us)");

    // In steady state, 1000 nodes idle should tick in under 1ms on average
    assert!(
        avg < 1_000,
        "1000-node engine ticks at {avg}us avg — expected under 1000us"
    );
}
