use std::{collections::HashSet, path::PathBuf, time::{Duration, Instant}};

use golden_core::{
    app::{
        configure_loaded_engine, from_sparse_project_json, load_sparse_project_file,
        prepare_engine_for_runtime, to_sparse_project_json_pretty,
    },
    node::Node,
};
use sysinfo::{ProcessesToUpdate, System, get_current_pid};

use crate::app::{AppEngine, AppNode};

use super::lock_performance_test;

fn resident_bytes(system: &mut System) -> u64 {
    let pid = get_current_pid().expect("current PID should exist");
    system.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
    system.process(pid).expect("current process should exist").memory()
}

fn graph_root_uuids(engine: &AppEngine) -> HashSet<String> {
    engine
        .nodes
        .iter()
        .filter_map(|(_, node)| {
            let meta = &node.node_data().meta;
            meta.decl_id
                .0
                .starts_with("scale_constant_")
                .then(|| meta.uuid.0.to_string())
        })
        .collect()
}

#[test]
#[ignore = "manual T19 authored-node product load and reload qualification"]
fn authored_graph_project_loads_ticks_and_round_trips() {
    let _performance_guard = lock_performance_test();
    let fixture = PathBuf::from(
        std::env::var_os("CHATAIGNE_AUTHORED_SCALE_FIXTURE")
            .expect("set CHATAIGNE_AUTHORED_SCALE_FIXTURE to a generated project path"),
    );
    let minimum_live_nodes = std::env::var("CHATAIGNE_AUTHORED_SCALE_MIN_NODES")
        .expect("set CHATAIGNE_AUTHORED_SCALE_MIN_NODES to the qualification target")
        .parse::<usize>()
        .expect("minimum live node count must be an integer");
    let expected_graph_roots = std::env::var("CHATAIGNE_AUTHORED_SCALE_GRAPH_ROOTS")
        .expect("set CHATAIGNE_AUTHORED_SCALE_GRAPH_ROOTS from fixture metadata")
        .parse::<usize>()
        .expect("graph root count must be an integer");
    let mut system = System::new();

    let started = Instant::now();
    let mut engine = load_sparse_project_file::<AppNode, _>(&fixture).expect("authored project should load");
    let load_ms = started.elapsed().as_millis();
    let authored_nodes = engine.nodes.iter().count();
    assert!(
        authored_nodes >= minimum_live_nodes,
        "loaded {authored_nodes} live nodes, below the {minimum_live_nodes}-node target"
    );
    let authored_graph_roots = graph_root_uuids(&engine);
    assert_eq!(authored_graph_roots.len(), expected_graph_roots);
    let load_rss_mb = resident_bytes(&mut system) / 1_000_000;

    let started = Instant::now();
    configure_loaded_engine(&mut engine).expect("authored project should configure");
    prepare_engine_for_runtime(&mut engine).expect("authored project should prepare");
    let prepare_ms = started.elapsed().as_millis();
    let prepared_nodes = engine.nodes.iter().count();
    let prepare_rss_mb = resident_bytes(&mut system) / 1_000_000;

    let started = Instant::now();
    engine.run_tick(Duration::from_millis(8)).expect("authored project tick should run");
    let tick_us = started.elapsed().as_micros();
    let tick_callbacks = engine.tick_stats().callbacks_fired;

    let started = Instant::now();
    let saved = to_sparse_project_json_pretty(&engine).expect("authored project should save");
    let save_ms = started.elapsed().as_millis();
    let saved_bytes = saved.len();
    drop(engine);

    let started = Instant::now();
    let reloaded = from_sparse_project_json::<AppNode>(&saved).expect("saved authored project should reload");
    let reload_ms = started.elapsed().as_millis();
    let reloaded_nodes = reloaded.nodes.iter().count();
    let reloaded_graph_roots = graph_root_uuids(&reloaded);
    assert_eq!(
        reloaded_graph_roots, authored_graph_roots,
        "save/reload must preserve every authored graph root"
    );
    assert!(reloaded_nodes >= minimum_live_nodes);
    let reload_rss_mb = resident_bytes(&mut system) / 1_000_000;

    println!(
        "AUTHORED_SCALE_RESULT={}",
        serde_json::json!({
            "authored_nodes": authored_nodes,
            "graph_roots": authored_graph_roots.len(),
            "minimum_live_nodes": minimum_live_nodes,
            "prepared_nodes": prepared_nodes,
            "reloaded_nodes": reloaded_nodes,
            "load_ms": load_ms,
            "prepare_ms": prepare_ms,
            "tick_us": tick_us,
            "tick_callbacks": tick_callbacks,
            "save_ms": save_ms,
            "saved_bytes": saved_bytes,
            "reload_ms": reload_ms,
            "load_rss_mb": load_rss_mb,
            "prepare_rss_mb": prepare_rss_mb,
            "reload_rss_mb": reload_rss_mb,
        })
    );
}
