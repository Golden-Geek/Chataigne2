use std::{collections::HashSet, path::PathBuf, time::{Duration, Instant}};

use golden_core::{
    app::{
        configure_loaded_engine, from_sparse_project_json, load_sparse_project_file, ProjectNode,
        prepare_engine_for_runtime, to_sparse_project_json_pretty,
    },
    edit::{Edit, EditOrigin},
    node::{Folder, Node, NodeId, NodeUuid},
    parameter::{ParamValue, ParameterEventBehaviour},
    ui_sync::{UiDuplicateNodeSpec, UiEditIntent},
};
use golden_values::Value as RuntimeValue;
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

#[derive(Debug, PartialEq)]
struct AuthoredNodeState {
    depth: usize,
    node_type: String,
    decl_id: String,
    label: String,
    param_value: Option<ParamValue>,
}

fn authored_tree_states(engine: &AppEngine) -> Vec<AuthoredNodeState> {
    let mut states = Vec::new();
    let mut pending = vec![(engine.root, 0)];
    while let Some((id, depth)) = pending.pop() {
        let node = engine.nodes.get(id).expect("tree node should exist");
        let data = node.node_data();
        let mut children = Vec::new();
        let mut child = data.first_child;
        while let Some(child_id) = child {
            let child_node = engine.nodes.get(child_id).expect("child should exist");
            children.push(child_id);
            child = child_node.node_data().next_sibling;
        }
        pending.extend(children.into_iter().rev().map(|child_id| (child_id, depth + 1)));
        states.push(AuthoredNodeState {
            depth,
            node_type: node.get_type().to_owned(),
            decl_id: data.meta.decl_id.0.clone(),
            label: data.meta.label.clone(),
            param_value: node.engine_param_snapshot().map(|param| param.value),
        });
    }
    states
}

fn direct_child_uuids(engine: &AppEngine, parent: NodeId) -> Vec<NodeUuid> {
    let mut uuids = Vec::new();
    let mut child = engine.nodes.get(parent).expect("parent should exist").node_data().first_child;
    while let Some(id) = child {
        let node = engine.nodes.get(id).expect("child should exist");
        uuids.push(node.node_data().meta.uuid);
        child = node.node_data().next_sibling;
    }
    uuids
}

fn manager_formula_materializations(engine: &AppEngine) -> u64 {
    engine
        .nodes
        .iter()
        .find_map(|(_, node)| match node {
            AppNode::StateMachineManager(manager) => Some(manager.runtime_perf_stats().formula_materializations),
            _ => None,
        })
        .expect("project should contain a state-machine manager")
}

fn manager_live_edit_phase_ns(engine: &AppEngine) -> [u64; 3] {
    let stats = engine
        .nodes
        .iter()
        .find_map(|(_, node)| match node {
            AppNode::StateMachineManager(manager) => Some(manager.runtime_perf_stats()),
            _ => None,
        })
        .expect("project should contain a state-machine manager");
    [
        stats.formula_cache_refresh_ns,
        stats.formula_catalog_build_ns,
        stats.runtime_cache_rebuild_ns,
    ]
}

fn assert_parameter_values(engine: &AppEngine, params: &[(NodeId, ParamValue, ParamValue)], edited: bool) {
    for (id, before, after) in params {
        let value = engine
            .nodes
            .get(*id)
            .and_then(Node::engine_param_snapshot)
            .expect("selected Constant value parameter should exist")
            .value;
        assert_eq!(&value, if edited { after } else { before });
    }
}

fn assert_runtime_constant_values(
    engine: &AppEngine,
    anodes: &[NodeUuid],
    params: &[(NodeId, ParamValue, ParamValue)],
    edited: bool,
) {
    let values = engine
        .nodes
        .iter()
        .find_map(|(_, node)| match node {
            AppNode::StateMachineManager(manager) => Some(manager.runtime_constant_values()),
            _ => None,
        })
        .expect("project should contain a state-machine manager");
    for (anode, (_, before, after)) in anodes.iter().zip(params) {
        let expected = if edited { after } else { before };
        let expected = match expected {
            ParamValue::Float(value) => RuntimeValue::Float(*value),
            ParamValue::Int(value) => RuntimeValue::Int(i64::from(*value)),
            other => panic!("Constant value should be numeric, got {other:?}"),
        };
        assert_eq!(values.get(anode), Some(&expected), "runtime Formula should reflect Constant {anode:?}");
    }
}

#[test]
fn initial_formula_cache_is_ready_before_the_first_runtime_tick() {
    let _performance_guard = lock_performance_test();
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("samples")
        .join("test_simple_load.noisette");
    let mut engine = load_sparse_project_file::<AppNode, _>(&fixture).expect("sample project should load");
    configure_loaded_engine(&mut engine).expect("sample project should configure");
    prepare_engine_for_runtime(&mut engine).expect("sample project should prepare");

    let prepared = manager_formula_materializations(&engine);
    assert!(prepared > 0, "activation should materialize the project formulas");

    engine.run_tick(Duration::from_millis(8)).expect("first runtime tick should run");
    assert_eq!(manager_formula_materializations(&engine), prepared);
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
    let before_reload = authored_tree_states(&engine);
    assert_eq!(before_reload.len(), authored_nodes, "all loaded nodes should be reachable from the root");
    let load_rss_mb = resident_bytes(&mut system) / 1_000_000;

    let started = Instant::now();
    configure_loaded_engine(&mut engine).expect("authored project should configure");
    prepare_engine_for_runtime(&mut engine).expect("authored project should prepare");
    let prepare_ms = started.elapsed().as_millis();
    let prepared_nodes = engine.nodes.iter().count();
    let prepare_rss_mb = resident_bytes(&mut system) / 1_000_000;

    let mut tick_us = Vec::with_capacity(5);
    let mut tick_callbacks = Vec::with_capacity(5);
    let mut tick_snapshot_builds = Vec::with_capacity(5);
    let mut tick_snapshot_nodes_cloned = Vec::with_capacity(5);
    let mut tick_edits_applied = Vec::with_capacity(5);
    for _ in 0..5 {
        let started = Instant::now();
        engine.run_tick(Duration::from_millis(8)).expect("authored project tick should run");
        tick_us.push(started.elapsed().as_micros());
        let stats = engine.tick_stats();
        tick_callbacks.push(stats.callbacks_fired);
        tick_snapshot_builds.push(stats.snapshot_builds);
        tick_snapshot_nodes_cloned.push(stats.snapshot_nodes_cloned);
        tick_edits_applied.push(stats.edits_applied);
    }
    let manager_stats = engine.nodes.iter().find_map(|(_, node)| match node {
        AppNode::StateMachineManager(manager) => Some(manager.runtime_perf_stats()),
        _ => None,
    }).expect("authored project should contain a state-machine manager");
    println!(
        "AUTHORED_SCALE_MANAGER_PHASES_NS={} {} {}",
        manager_stats.formula_cache_refresh_ns,
        manager_stats.formula_catalog_build_ns,
        manager_stats.runtime_cache_rebuild_ns,
    );

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
    assert_eq!(reloaded_nodes, authored_nodes, "save/reload must preserve the live-node count");
    let after_reload = authored_tree_states(&reloaded);
    assert_eq!(after_reload.len(), before_reload.len());
    for (index, (before, after)) in before_reload.iter().zip(&after_reload).enumerate() {
        assert_eq!(
            after,
            before,
            "save/reload changed ordered authored tree entry {index}"
        );
    }
    let reload_rss_mb = resident_bytes(&mut system) / 1_000_000;

    println!(
        "AUTHORED_SCALE_RESULT={}",
        serde_json::json!({
            "authored_nodes": authored_nodes,
            "graph_roots": authored_graph_roots.len(),
            "minimum_live_nodes": minimum_live_nodes,
            "prepared_nodes": prepared_nodes,
            "reloaded_nodes": reloaded_nodes,
            "verified_tree_nodes": after_reload.len(),
            "load_ms": load_ms,
            "prepare_ms": prepare_ms,
            "tick_us": tick_us,
            "tick_callbacks": tick_callbacks,
            "tick_snapshot_builds": tick_snapshot_builds,
            "tick_snapshot_nodes_cloned": tick_snapshot_nodes_cloned,
            "tick_edits_applied": tick_edits_applied,
            "save_ms": save_ms,
            "saved_bytes": saved_bytes,
            "reload_ms": reload_ms,
            "load_rss_mb": load_rss_mb,
            "prepare_rss_mb": prepare_rss_mb,
            "reload_rss_mb": reload_rss_mb,
        })
    );
}

#[test]
#[ignore = "manual T19 sparse/dense authored Constant parameter qualification"]
fn authored_graph_changes_constant_values_and_replays_one_batch() {
    let _performance_guard = lock_performance_test();
    let fixture = PathBuf::from(
        std::env::var_os("CHATAIGNE_AUTHORED_SCALE_FIXTURE")
            .expect("set CHATAIGNE_AUTHORED_SCALE_FIXTURE to a generated project path"),
    );
    let edit_count = std::env::var("CHATAIGNE_AUTHORED_SCALE_PARAMETER_EDITS")
        .expect("set CHATAIGNE_AUTHORED_SCALE_PARAMETER_EDITS")
        .parse::<usize>()
        .expect("parameter edit count must be an integer");
    assert!(edit_count > 0, "parameter edit count must be positive");
    let mut engine = load_sparse_project_file::<AppNode, _>(&fixture).expect("authored project should load");
    configure_loaded_engine(&mut engine).expect("authored project should configure");
    prepare_engine_for_runtime(&mut engine).expect("authored project should prepare");
    engine.run_tick(Duration::from_millis(8)).expect("authored project should warm");

    let base_nodes = engine.nodes.iter().count();
    let mut sources = engine
        .nodes
        .iter()
        .filter(|(_, node)| node.node_data().meta.decl_id.0.starts_with("scale_constant_"))
        .map(|(id, node)| (node.node_data().meta.decl_id.0.clone(), id))
        .collect::<Vec<_>>();
    sources.sort_by(|left, right| left.0.cmp(&right.0));
    assert!(sources.len() >= edit_count, "fixture needs {edit_count} authored Constants");
    let snapshot = engine.process_tree_snapshot();
    let selected_anodes = (0..edit_count)
        .map(|index| {
            let root = sources[index * sources.len() / edit_count].1;
            snapshot.node(root).expect("selected Constant should exist").uuid
        })
        .collect::<Vec<_>>();
    let params = (0..edit_count)
        .map(|index| {
            let root = sources[index * sources.len() / edit_count].1;
            let config = snapshot.find_child_by_decl_id(root, "config").expect("Constant config should exist");
            let param = snapshot
                .find_child_by_decl_id(config, "config/value")
                .expect("Constant value parameter should exist");
            let before = snapshot.node(param).and_then(|node| node.param_value.clone())
                .expect("Constant value should have a parameter value");
            let after = match &before {
                ParamValue::Float(value) => ParamValue::Float(value + 1.0),
                ParamValue::Int(value) => ParamValue::Int(value + 1),
                other => panic!("Constant fixture value should be numeric, got {other:?}"),
            };
            (param, before, after)
        })
        .collect::<Vec<_>>();
    drop(snapshot);
    let materializations_before = manager_formula_materializations(&engine);
    let phases_before = manager_live_edit_phase_ns(&engine);

    let session_id = "authored-constant-value-batch";
    engine.edits.push(Edit::BeginEditSession {
        origin: EditOrigin::Ui,
        label: Some("Change authored Constant values".into()),
        client_edit_id: session_id.into(),
        ui_client_instance_id: None,
    });
    for (param, _, after) in &params {
        engine.edits.push(Edit::SetParam {
            node: *param,
            value: after.clone(),
            behaviour: ParameterEventBehaviour::Coalesce,
        });
    }
    engine.edits.push(Edit::EndEditSession { client_edit_id: session_id.into() });
    let started = Instant::now();
    engine.apply_edits().expect("one parameter edit transaction should apply");
    let edit_ms = started.elapsed().as_millis();
    assert_eq!(engine.undo_len(), 1, "parameter batch should be one undo transaction");
    assert_eq!(engine.nodes.iter().count(), base_nodes);
    assert_parameter_values(&engine, &params, true);
    let started = Instant::now();
    engine.run_tick(Duration::from_millis(8)).expect("changed Constants should tick");
    let edit_tick_ms = started.elapsed().as_millis();
    assert_parameter_values(&engine, &params, true);
    let started = Instant::now();
    engine.run_tick(Duration::from_millis(8)).expect("changed Formula should refresh");
    let edit_refresh_tick_ms = started.elapsed().as_millis();
    let edit_refresh_stats = engine.tick_stats();
    let phases_after_edit = manager_live_edit_phase_ns(&engine);
    let edit_refresh_phase_ns: [u64; 3] = std::array::from_fn(|index| {
        phases_after_edit[index] - phases_before[index]
    });
    let materializations_after_edit = manager_formula_materializations(&engine);
    assert!(
        materializations_after_edit > materializations_before,
        "changed Constant values should refresh the active Formula"
    );
    assert_runtime_constant_values(&engine, &selected_anodes, &params, true);

    let started = Instant::now();
    assert!(engine.undo().expect("undo should succeed"));
    let undo_ms = started.elapsed().as_millis();
    assert_parameter_values(&engine, &params, false);
    let started = Instant::now();
    engine.run_tick(Duration::from_millis(8)).expect("undone Constants should tick");
    let undo_tick_ms = started.elapsed().as_millis();
    assert_parameter_values(&engine, &params, false);
    let started = Instant::now();
    engine.run_tick(Duration::from_millis(8)).expect("undone Formula should refresh");
    let undo_refresh_tick_ms = started.elapsed().as_millis();
    let materializations_after_undo = manager_formula_materializations(&engine);
    assert!(materializations_after_undo > materializations_after_edit);
    assert_runtime_constant_values(&engine, &selected_anodes, &params, false);

    let started = Instant::now();
    assert!(engine.redo().expect("redo should succeed"));
    let redo_ms = started.elapsed().as_millis();
    assert_parameter_values(&engine, &params, true);
    let started = Instant::now();
    engine.run_tick(Duration::from_millis(8)).expect("redone Constants should tick");
    let redo_tick_ms = started.elapsed().as_millis();
    assert_parameter_values(&engine, &params, true);
    let started = Instant::now();
    engine.run_tick(Duration::from_millis(8)).expect("redone Formula should refresh");
    let redo_refresh_tick_ms = started.elapsed().as_millis();
    assert!(manager_formula_materializations(&engine) > materializations_after_undo);
    assert_eq!(engine.nodes.iter().count(), base_nodes);
    assert_runtime_constant_values(&engine, &selected_anodes, &params, true);

    let saved = to_sparse_project_json_pretty(&engine).expect("edited project should save");
    drop(engine);
    let mut reloaded = from_sparse_project_json::<AppNode>(&saved).expect("edited project should reload");
    let reloaded_snapshot = reloaded.process_tree_snapshot();
    let reloaded_params = selected_anodes
        .iter()
        .zip(&params)
        .map(|(anode_uuid, (_, before, after))| {
            let anode = reloaded_snapshot.node_id_by_uuid(*anode_uuid).expect("edited Constant root should reload");
            let config = reloaded_snapshot.find_child_by_decl_id(anode, "config")
                .expect("reloaded Constant config should exist");
            let param = reloaded_snapshot.find_child_by_decl_id(config, "config/value")
                .expect("reloaded Constant value should exist");
            assert_eq!(
                reloaded_snapshot.node(param).and_then(|node| node.param_value.as_ref()),
                Some(after),
                "edited Constant value should survive save/reload"
            );
            (param, before.clone(), after.clone())
        })
        .collect::<Vec<_>>();
    drop(reloaded_snapshot);
    configure_loaded_engine(&mut reloaded).expect("reloaded edited project should configure");
    prepare_engine_for_runtime(&mut reloaded).expect("reloaded edited project should prepare");
    assert_runtime_constant_values(&reloaded, &selected_anodes, &reloaded_params, true);
    reloaded.run_tick(Duration::from_millis(8)).expect("reloaded edited project should tick");
    assert_parameter_values(&reloaded, &reloaded_params, true);
    assert_runtime_constant_values(&reloaded, &selected_anodes, &reloaded_params, true);

    println!(
        "AUTHORED_PARAMETER_EDIT_RESULT={}",
        serde_json::json!({
            "base_nodes": base_nodes,
            "graph_roots": sources.len(),
            "edited_params": params.len(),
            "reloaded_params": reloaded_params.len(),
            "reloaded_runtime_constants": reloaded_params.len(),
            "edit_ms": edit_ms,
            "edit_tick_ms": edit_tick_ms,
            "edit_refresh_tick_ms": edit_refresh_tick_ms,
            "edit_refresh_snapshot_builds": edit_refresh_stats.snapshot_builds,
            "edit_refresh_snapshot_nodes_cloned": edit_refresh_stats.snapshot_nodes_cloned,
            "edit_refresh_formula_cache_ns": edit_refresh_phase_ns[0],
            "edit_refresh_formula_catalog_ns": edit_refresh_phase_ns[1],
            "edit_refresh_runtime_rebuild_ns": edit_refresh_phase_ns[2],
            "undo_ms": undo_ms,
            "undo_tick_ms": undo_tick_ms,
            "undo_refresh_tick_ms": undo_refresh_tick_ms,
            "redo_ms": redo_ms,
            "redo_tick_ms": redo_tick_ms,
            "redo_refresh_tick_ms": redo_refresh_tick_ms,
        })
    );
}

#[test]
#[ignore = "manual T19 active authored-graph duplication qualification"]
fn authored_graph_duplicates_and_replays_one_live_edit() {
    let _performance_guard = lock_performance_test();
    let fixture = PathBuf::from(
        std::env::var_os("CHATAIGNE_AUTHORED_SCALE_FIXTURE")
            .expect("set CHATAIGNE_AUTHORED_SCALE_FIXTURE to a generated project path"),
    );
    let duplicate_count = std::env::var("CHATAIGNE_AUTHORED_SCALE_DUPLICATES")
        .expect("set CHATAIGNE_AUTHORED_SCALE_DUPLICATES")
        .parse::<usize>()
        .expect("duplicate count must be an integer");
    let mut engine = load_sparse_project_file::<AppNode, _>(&fixture).expect("authored project should load");
    configure_loaded_engine(&mut engine).expect("authored project should configure");
    prepare_engine_for_runtime(&mut engine).expect("authored project should prepare");
    engine.run_tick(Duration::from_millis(8)).expect("authored project should warm");
    let mut sources = engine
        .nodes
        .iter()
        .filter(|(_, node)| node.node_data().meta.decl_id.0.starts_with("scale_constant_"))
        .map(|(id, node)| (node.node_data().meta.decl_id.0.clone(), id, node.node_data().parent))
        .collect::<Vec<_>>();
    sources.sort_by(|left, right| left.0.cmp(&right.0));
    assert!(sources.len() >= duplicate_count);
    let formula = sources[0].2.expect("graph root should belong to a formula");
    let children_before = direct_child_uuids(&engine, formula);
    let roots_before = graph_root_uuids(&engine);
    let live_nodes_before = engine.nodes.iter().count();
    let phase_ns_before = manager_live_edit_phase_ns(&engine);
    let specs = sources
        .iter()
        .take(duplicate_count)
        .map(|(_, source, parent)| UiDuplicateNodeSpec {
            source: *source,
            new_parent: parent.expect("graph root should belong to a formula"),
            new_prev_sibling: None,
            initial_params: Vec::new(),
        })
        .collect();

    let started = Instant::now();
    let duplicates = engine
        .ui_apply_duplicate_nodes_with_dependent_user_items(
            specs,
            Vec::new(),
            Vec::new(),
            |node| node.project_encode_data(),
            AppNode::project_decode_node,
        )
        .expect("one batch should duplicate every graph root");
    let duplicate_ms = started.elapsed().as_millis();
    assert_eq!(duplicates.len(), duplicate_count);
    assert_eq!(engine.undo_len(), 1, "paste should create one undo transaction");
    let live_nodes_after = engine.nodes.iter().count();
    let children_after = direct_child_uuids(&engine, formula);
    assert_eq!(graph_root_uuids(&engine), roots_before, "original graph roots must remain stable");
    let started = Instant::now();
    engine.run_tick(Duration::from_millis(8)).expect("duplicated graph should tick");
    let duplicate_tick_ms = started.elapsed().as_millis();
    let phase_ns_after_duplicate = manager_live_edit_phase_ns(&engine);

    let started = Instant::now();
    assert!(engine.undo().expect("undo should succeed"));
    let undo_ms = started.elapsed().as_millis();
    assert_eq!(engine.nodes.iter().count(), live_nodes_before);
    assert_eq!(direct_child_uuids(&engine, formula), children_before);
    let started = Instant::now();
    engine.run_tick(Duration::from_millis(8)).expect("undone graph should tick");
    let undo_tick_ms = started.elapsed().as_millis();
    let phase_ns_after_undo = manager_live_edit_phase_ns(&engine);
    let started = Instant::now();
    assert!(engine.redo().expect("redo should succeed"));
    let redo_ms = started.elapsed().as_millis();
    assert_eq!(engine.nodes.iter().count(), live_nodes_after);
    assert_eq!(direct_child_uuids(&engine, formula), children_after);
    let started = Instant::now();
    engine.run_tick(Duration::from_millis(8)).expect("redone graph should tick");
    let redo_tick_ms = started.elapsed().as_millis();
    let phase_ns_after_redo = manager_live_edit_phase_ns(&engine);
    println!(
        "AUTHORED_LIVE_EDIT_RESULT={}",
        serde_json::json!({
            "base_nodes": live_nodes_before,
            "duplicate_roots": duplicate_count,
            "inserted_nodes": live_nodes_after - live_nodes_before,
            "duplicate_ms": duplicate_ms,
            "duplicate_tick_ms": duplicate_tick_ms,
            "undo_ms": undo_ms,
            "undo_tick_ms": undo_tick_ms,
            "redo_ms": redo_ms,
            "redo_tick_ms": redo_tick_ms,
            "manager_phase_ns_before": phase_ns_before,
            "manager_phase_ns_after_duplicate": phase_ns_after_duplicate,
            "manager_phase_ns_after_undo": phase_ns_after_undo,
            "manager_phase_ns_after_redo": phase_ns_after_redo,
        })
    );
}

#[test]
#[ignore = "manual T19 active authored-graph multi-root removal qualification"]
fn authored_graph_removes_and_replays_one_live_edit() {
    let _performance_guard = lock_performance_test();
    let fixture = PathBuf::from(
        std::env::var_os("CHATAIGNE_AUTHORED_SCALE_FIXTURE")
            .expect("set CHATAIGNE_AUTHORED_SCALE_FIXTURE to a generated project path"),
    );
    let remove_count = std::env::var("CHATAIGNE_AUTHORED_SCALE_REMOVALS")
        .expect("set CHATAIGNE_AUTHORED_SCALE_REMOVALS")
        .parse::<usize>()
        .expect("removal count must be an integer");
    let mut engine = load_sparse_project_file::<AppNode, _>(&fixture).expect("authored project should load");
    configure_loaded_engine(&mut engine).expect("authored project should configure");
    prepare_engine_for_runtime(&mut engine).expect("authored project should prepare");
    engine.run_tick(Duration::from_millis(8)).expect("authored project should warm");

    let mut sources = engine
        .nodes
        .iter()
        .filter(|(_, node)| node.node_data().meta.decl_id.0.starts_with("scale_constant_"))
        .map(|(id, node)| (node.node_data().meta.decl_id.0.clone(), id, node.node_data().parent))
        .collect::<Vec<_>>();
    sources.sort_by(|left, right| left.0.cmp(&right.0));
    assert!(sources.len() >= remove_count);
    let formula = sources[0].2.expect("graph root should belong to a formula");
    let children_before = direct_child_uuids(&engine, formula);
    let roots_before = graph_root_uuids(&engine);
    let live_nodes_before = engine.nodes.iter().count();
    let nodes = sources.iter().take(remove_count).map(|(_, id, _)| *id).collect::<Vec<_>>();
    assert!(sources.iter().take(remove_count).all(|(_, _, parent)| *parent == Some(formula)));

    let started = Instant::now();
    let acknowledgement = engine.apply_ui_intent(UiEditIntent::RemoveNodes { nodes });
    let remove_ms = started.elapsed().as_millis();
    assert!(acknowledgement.success, "remove intent should succeed: {acknowledgement:?}");
    assert_eq!(engine.undo_len(), 1, "multi-select delete should create one undo transaction");
    let live_nodes_after = engine.nodes.iter().count();
    let children_after = direct_child_uuids(&engine, formula);
    assert_eq!(children_after.len() + remove_count, children_before.len());
    assert_eq!(graph_root_uuids(&engine).len() + remove_count, roots_before.len());
    let started = Instant::now();
    engine.run_tick(Duration::from_millis(8)).expect("removed graph should tick");
    let remove_tick_ms = started.elapsed().as_millis();

    let started = Instant::now();
    assert!(engine.undo().expect("undo should succeed"));
    let undo_ms = started.elapsed().as_millis();
    assert_eq!(engine.nodes.iter().count(), live_nodes_before);
    assert_eq!(direct_child_uuids(&engine, formula), children_before);
    assert_eq!(graph_root_uuids(&engine), roots_before);
    let started = Instant::now();
    engine.run_tick(Duration::from_millis(8)).expect("restored graph should tick");
    let undo_tick_ms = started.elapsed().as_millis();

    let started = Instant::now();
    assert!(engine.redo().expect("redo should succeed"));
    let redo_ms = started.elapsed().as_millis();
    assert_eq!(engine.nodes.iter().count(), live_nodes_after);
    assert_eq!(direct_child_uuids(&engine, formula), children_after);
    let started = Instant::now();
    engine.run_tick(Duration::from_millis(8)).expect("removed graph should tick after redo");
    let redo_tick_ms = started.elapsed().as_millis();

    println!(
        "AUTHORED_LIVE_REMOVE_RESULT={}",
        serde_json::json!({
            "base_nodes": live_nodes_before,
            "removed_roots": remove_count,
            "removed_nodes": live_nodes_before - live_nodes_after,
            "remove_ms": remove_ms,
            "remove_tick_ms": remove_tick_ms,
            "undo_ms": undo_ms,
            "undo_tick_ms": undo_tick_ms,
            "redo_ms": redo_ms,
            "redo_tick_ms": redo_tick_ms,
        })
    );
}

#[test]
#[ignore = "manual T19 active authored-graph mixed-parent removal qualification"]
fn authored_graph_removes_mixed_parents_and_selected_descendant() {
    let _performance_guard = lock_performance_test();
    let fixture = PathBuf::from(
        std::env::var_os("CHATAIGNE_AUTHORED_SCALE_FIXTURE")
            .expect("set CHATAIGNE_AUTHORED_SCALE_FIXTURE to a generated project path"),
    );
    let remove_count = std::env::var("CHATAIGNE_AUTHORED_SCALE_REMOVALS")
        .expect("set CHATAIGNE_AUTHORED_SCALE_REMOVALS")
        .parse::<usize>()
        .expect("removal count must be an integer");
    let mut engine = load_sparse_project_file::<AppNode, _>(&fixture).expect("authored project should load");
    configure_loaded_engine(&mut engine).expect("authored project should configure");
    prepare_engine_for_runtime(&mut engine).expect("authored project should prepare");

    let extra_parent = Folder::new("Mixed-parent qualifier");
    let extra_parent_uuid = extra_parent.node_data().meta.uuid;
    engine.add_node(extra_parent.into(), None);
    engine.apply_edits().expect("second parent should attach");
    let extra_parent_id = engine.node_id_by_uuid(extra_parent_uuid).expect("second parent should exist");
    let extra_child = Folder::new("Mixed-parent leaf");
    let extra_child_uuid = extra_child.node_data().meta.uuid;
    engine.add_node(extra_child.into(), Some(extra_parent_id));
    engine.apply_edits().expect("second-parent leaf should attach");
    let extra_child_id = engine.node_id_by_uuid(extra_child_uuid).expect("second-parent leaf should exist");
    engine.run_tick(Duration::from_millis(8)).expect("authored project should warm");
    engine.clear_history();

    let mut sources = engine
        .nodes
        .iter()
        .filter(|(_, node)| node.node_data().meta.decl_id.0.starts_with("scale_constant_"))
        .map(|(id, node)| (node.node_data().meta.decl_id.0.clone(), id, node.node_data().parent))
        .collect::<Vec<_>>();
    sources.sort_by(|left, right| left.0.cmp(&right.0));
    assert!(sources.len() >= remove_count);
    let formula = sources[0].2.expect("graph root should belong to a formula");
    assert!(sources.iter().take(remove_count).all(|(_, _, parent)| *parent == Some(formula)));
    let selected_descendant = engine
        .nodes
        .get(sources[0].1)
        .and_then(|node| node.node_data().first_child)
        .expect("constant ANode should have a declared child");
    let formula_children_before = direct_child_uuids(&engine, formula);
    let extra_children_before = direct_child_uuids(&engine, extra_parent_id);
    let graph_roots_before = graph_root_uuids(&engine);
    let removed_root_uuids = sources
        .iter()
        .take(remove_count)
        .map(|(_, id, _)| engine.nodes.get(*id).expect("selected root should exist").node_data().meta.uuid)
        .collect::<HashSet<_>>();
    let expected_formula_children_after = formula_children_before
        .iter()
        .copied()
        .filter(|uuid| !removed_root_uuids.contains(uuid))
        .collect::<Vec<_>>();
    let removed_graph_root_uuids = removed_root_uuids.iter().map(|uuid| uuid.0.to_string()).collect::<HashSet<_>>();
    let expected_graph_roots_after = graph_roots_before
        .iter()
        .filter(|uuid| !removed_graph_root_uuids.contains(*uuid))
        .cloned()
        .collect::<HashSet<_>>();
    let live_nodes_before = engine.nodes.iter().count();
    let mut selection = vec![selected_descendant, extra_child_id];
    selection.extend(sources.iter().take(remove_count).map(|(_, id, _)| *id));

    let started = Instant::now();
    let acknowledgement = engine.apply_ui_intent(UiEditIntent::RemoveNodes { nodes: selection });
    let remove_ms = started.elapsed().as_millis();
    assert!(acknowledgement.success, "mixed remove should succeed: {acknowledgement:?}");
    assert_eq!(engine.undo_len(), 1, "mixed delete should create one undo transaction");
    let live_nodes_after = engine.nodes.iter().count();
    let formula_children_after = direct_child_uuids(&engine, formula);
    let extra_children_after = direct_child_uuids(&engine, extra_parent_id);
    assert_eq!(formula_children_after, expected_formula_children_after);
    assert!(extra_children_after.is_empty());
    assert_eq!(graph_root_uuids(&engine), expected_graph_roots_after);
    let started = Instant::now();
    engine.run_tick(Duration::from_millis(8)).expect("mixed-removed graph should tick");
    let remove_tick_ms = started.elapsed().as_millis();

    let started = Instant::now();
    assert!(engine.undo().expect("undo should succeed"));
    let undo_ms = started.elapsed().as_millis();
    assert_eq!(engine.nodes.iter().count(), live_nodes_before);
    assert_eq!(direct_child_uuids(&engine, formula), formula_children_before);
    assert_eq!(direct_child_uuids(&engine, extra_parent_id), extra_children_before);
    assert_eq!(graph_root_uuids(&engine), graph_roots_before);
    let started = Instant::now();
    engine.run_tick(Duration::from_millis(8)).expect("mixed-restored graph should tick");
    let undo_tick_ms = started.elapsed().as_millis();

    let started = Instant::now();
    assert!(engine.redo().expect("redo should succeed"));
    let redo_ms = started.elapsed().as_millis();
    assert_eq!(engine.nodes.iter().count(), live_nodes_after);
    assert_eq!(direct_child_uuids(&engine, formula), formula_children_after);
    assert_eq!(direct_child_uuids(&engine, extra_parent_id), extra_children_after);
    assert_eq!(graph_root_uuids(&engine), expected_graph_roots_after);
    let started = Instant::now();
    engine.run_tick(Duration::from_millis(8)).expect("mixed-removed graph should tick after redo");
    let redo_tick_ms = started.elapsed().as_millis();

    println!(
        "AUTHORED_LIVE_MIXED_REMOVE_RESULT={}",
        serde_json::json!({
            "base_nodes": live_nodes_before,
            "removed_roots": remove_count + 1,
            "removed_nodes": live_nodes_before - live_nodes_after,
            "remove_ms": remove_ms,
            "remove_tick_ms": remove_tick_ms,
            "undo_ms": undo_ms,
            "undo_tick_ms": undo_tick_ms,
            "redo_ms": redo_ms,
            "redo_tick_ms": redo_tick_ms,
        })
    );
}
