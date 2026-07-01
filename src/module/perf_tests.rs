use std::time::{Duration, Instant};

use golden_core::{
    app::{load_sparse_project_file, to_sparse_project_json_pretty, ProjectFileSpec, ProjectNode},
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

fn elapsed_ms<T>(operation: impl FnOnce() -> T) -> (T, u128) {
    let started = Instant::now();
    let result = operation();
    (result, started.elapsed().as_millis())
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
    let label = format!("{} Copy", source_node.node_data().meta.label);
    engine.duplicate_subtree_with(
        source,
        parent,
        Some(source),
        Some(label),
        |node| node.project_encode_data(),
        |node_type, data, meta| AppNode::project_decode_node(node_type, data, meta),
    )
}

#[test]
fn sample_project_structure_operations_stay_interactive() {
    const SAMPLE: &str = "test_command.noisette";

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

    let (saved_json, save_ms) = elapsed_ms(|| {
        to_sparse_project_json_pretty(&engine).expect("sample should serialize sparsely")
    });

    let (ui_snapshot, snapshot_ms) =
        elapsed_ms(|| engine.ui_snapshot(UiSubscriptionScope::WholeGraph));
    let (read_model, read_model_ms) = elapsed_ms(|| {
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
    const SAMPLE: &str = "test_command.noisette";
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
