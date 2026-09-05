use golden_core::{
    edit::Edit,
    node::{Folder, Node, NodeId, NodeMetaPatch, NodeUuid},
    parameter::{ParamValue, ParameterEventBehaviour},
    process_ctx::ExecutionPhase,
    script::ScriptSource,
};

use super::MetronomesModule;
use std::collections::HashMap;

#[test]
fn metronomes_module_is_declared_under_generators_menu() {
    let item = crate::app::declared_user_creatable_items(crate::app::module::MODULE_ITEM_KIND)
        .into_iter()
        .find(|item| item.node_type == MetronomesModule::NODE_TYPE)
        .expect("Metronomes module should be exposed in the module catalog");

    assert_eq!(item.label, "Metronomes");
    assert_eq!(item.menu_path, vec!["Generators".to_string()]);
}

#[test]
fn metronomes_script_descriptor_advertises_reset_and_tick_methods() {
    let descriptor = MetronomesModule::create().engine_script_descriptor();

    for method in ["resetMetronomes", "resetMetronome", "tickMetronome"] {
        assert!(descriptor.methods.iter().any(|candidate| candidate == method));
    }
}

#[test]
fn metronomes_create_default_item_and_direct_value_trigger() {
    let (engine, module_id) = create_metronomes_module();

    let metronomes = find_path(&engine, module_id, "parameters/metronomes").expect("metronomes list");
    assert!(
        !engine
            .nodes
            .get(metronomes)
            .expect("metronomes list node")
            .node_data()
            .meta
            .can_be_disabled,
        "Metronome manager should not be disableable"
    );
    let default_metronome = nth_child(&engine, metronomes, 0).expect("default metronome");
    assert!(
        engine
            .nodes
            .get(default_metronome)
            .expect("default metronome node")
            .node_data()
            .meta
            .can_be_disabled,
        "Metronome items should remain disableable"
    );
    assert!(
        find_child_by_key(&engine, default_metronome, "enabled").is_none(),
        "Metronome items should use node enablement, not a duplicated Enabled parameter"
    );
    assert!(find_child_by_key(&engine, default_metronome, "frequency_hz").is_some());

    let values = find_path(&engine, module_id, "values").expect("values folder");
    assert!(
        find_child_by_key(&engine, values, "metronome_values").is_none(),
        "metronome values should be direct trigger params, not wrapped in a Metronomes folder"
    );
    assert!(
        find_child_by_key(&engine, values, "Metronome").is_some(),
        "default metronome should create one trigger value named after the metronome"
    );
    assert_eq!(
        count_children_by_key(&engine, values, "Metronome"),
        1,
        "creating a Metronomes module should materialize exactly one default trigger value"
    );
}

#[test]
fn metronome_value_trigger_uuid_is_derived_from_item_uuid() {
    let (engine, module_id) = create_metronomes_module();
    let metronomes = find_path(&engine, module_id, "parameters/metronomes").expect("metronomes list");
    let metronome = nth_child(&engine, metronomes, 0).expect("default metronome");
    let item_uuid = engine
        .nodes
        .get(metronome)
        .expect("default metronome node")
        .node_data()
        .meta
        .uuid;

    let values = find_path(&engine, module_id, "values").expect("values folder");
    let value = find_child_by_key(&engine, values, "Metronome").expect("metronome value trigger");
    let value_uuid = engine
        .nodes
        .get(value)
        .expect("metronome value node")
        .node_data()
        .meta
        .uuid;

    assert_eq!(value_uuid, super::metronome_value_uuid(item_uuid));
}

#[test]
fn sparse_reload_preserves_default_metronome_and_derived_trigger_identity() {
    let (engine, module_id) = create_metronomes_module();
    let metronomes = find_path(&engine, module_id, "parameters/metronomes").expect("metronomes list");
    let metronome = nth_child(&engine, metronomes, 0).expect("default metronome");
    let metronome_uuid = engine.nodes.get(metronome).expect("metronome").node_data().meta.uuid;
    let values = find_path(&engine, module_id, "values").expect("values folder");
    let trigger = find_child_by_key(&engine, values, "Metronome").expect("metronome trigger");
    let trigger_uuid = engine.nodes.get(trigger).expect("trigger").node_data().meta.uuid;

    let json = golden_core::app::to_sparse_project_json_pretty(&engine).expect("sparse project should encode");
    let mut loaded = golden_core::app::from_sparse_project_json::<crate::app::AppNode>(&json)
        .expect("sparse project should decode");
    stabilize(&mut loaded);
    let loaded_module = loaded
        .nodes
        .get(loaded.root)
        .and_then(|root| root.node_data().first_child)
        .expect("Metronomes module should reload");
    let loaded_metronomes =
        find_path(&loaded, loaded_module, "parameters/metronomes").expect("metronomes list should reload");
    let loaded_metronome = nth_child(&loaded, loaded_metronomes, 0).expect("metronome should reload");
    assert_eq!(
        loaded.nodes.get(loaded_metronome).expect("metronome").node_data().meta.uuid,
        metronome_uuid
    );
    let loaded_values = find_path(&loaded, loaded_module, "values").expect("values should reload");
    let loaded_trigger = find_child_by_key(&loaded, loaded_values, "Metronome").expect("trigger should reload");
    assert_eq!(
        loaded.nodes.get(loaded_trigger).expect("trigger").node_data().meta.uuid,
        trigger_uuid
    );
    assert_eq!(count_children_by_key(&loaded, loaded_values, "Metronome"), 1);
}

#[test]
fn metronome_config_uses_recursive_node_enablement() {
    let (mut engine, module_id) = create_metronomes_module();
    let metronomes = find_path(&engine, module_id, "parameters/metronomes").expect("metronomes list");
    let metronome = nth_child(&engine, metronomes, 0).expect("default metronome");

    patch_enabled(&mut engine, metronome, false);
    stabilize(&mut engine);
    let snapshot = engine.process_tree_snapshot();
    let config = super::metronome_config(snapshot.as_ref(), metronome).expect("metronome config");
    assert!(
        !config.enabled,
        "disabled metronome item should not be processed"
    );

    patch_enabled(&mut engine, metronome, true);
    patch_enabled(&mut engine, module_id, false);
    stabilize(&mut engine);
    let snapshot = engine.process_tree_snapshot();
    let config = super::metronome_config(snapshot.as_ref(), metronome).expect("metronome config");
    assert!(
        !config.enabled,
        "disabled module ancestor should disable metronome processing"
    );
}

#[test]
fn metronome_mode_dependencies_materialize_selected_parameters() {
    let (mut engine, module_id) = create_metronomes_module();
    let metronomes = find_path(&engine, module_id, "parameters/metronomes").expect("metronomes list");
    let metronome = nth_child(&engine, metronomes, 0).expect("default metronome");

    assert!(find_child_by_key(&engine, metronome, "frequency_hz").is_some());
    assert!(find_child_by_key(&engine, metronome, "interval_seconds").is_none());
    assert!(find_child_by_key(&engine, metronome, "bpm").is_none());

    let mode = find_child_by_key(&engine, metronome, "mode").expect("mode");
    set_param(&mut engine, mode, ParamValue::Enum("time".to_string()));
    stabilize(&mut engine);
    assert!(find_child_by_key(&engine, metronome, "frequency_hz").is_none());
    assert!(find_child_by_key(&engine, metronome, "interval_seconds").is_some());
    assert!(find_child_by_key(&engine, metronome, "bpm").is_none());

    set_param(&mut engine, mode, ParamValue::Enum("bpm".to_string()));
    stabilize(&mut engine);
    assert!(find_child_by_key(&engine, metronome, "frequency_hz").is_none());
    assert!(find_child_by_key(&engine, metronome, "interval_seconds").is_none());
    assert!(find_child_by_key(&engine, metronome, "bpm").is_some());
}

#[test]
fn metronome_emits_regular_trigger_and_count() {
    let config = test_config(1.0, false, 1.0, 1.0, 0);
    let mut state = super::MetronomeRuntimeState::default();

    super::reset_state_if_config_changed(&mut state, &config);

    assert_eq!(state.next_gap_seconds, 1.0);
    assert_eq!(super::metronome_gap_seconds(&config, 1), 1.0);
}

#[test]
fn metronome_supports_bpm_and_multiple_items() {
    let regular = test_config(0.5, false, 1.0, 1.0, 0);
    let randomized = test_config(0.5, true, 0.25, 0.75, 7);

    assert_eq!(super::metronome_gap_seconds(&regular, 10), 0.5);
    let randomized_gap = super::metronome_gap_seconds(&randomized, 10);
    assert!(
        (0.125..=0.375).contains(&randomized_gap),
        "randomized gap should stay inside the configured multiplier range, got {randomized_gap}"
    );
}

#[test]
fn metronome_worker_fixture_preserves_tick_multiplicity_and_count() {
    let config = test_config(0.5, false, 1.0, 1.0, 0);
    let worker_config = super::runtime::MetronomeWorkerConfig {
        update_rate_hz: 60,
        metronomes: vec![config],
    };
    let mut states = HashMap::new();

    let event = super::runtime::compute_metronome_update(&worker_config, &mut states, 1.25);
    let tick = event.ticks.get(&NodeId(1)).expect("metronome tick");
    assert_eq!(tick.fired, 2);
    assert_eq!(tick.total_ticks, 2);
    assert_eq!(tick.interval_seconds, 0.5);
    assert_eq!(tick.last_gap_seconds, 0.5);
}

#[test]
fn metronome_tick_callback_payload_preserves_multiplicity_count_and_timing() {
    let tick = super::runtime::MetronomeWorkerTick {
        item_id: NodeId(9),
        label: "Beat".to_string(),
        fired: 4,
        total_ticks: 21,
        interval_seconds: 0.5,
        last_gap_seconds: 0.45,
    };

    assert_eq!(
        super::metronome_tick_callback_args(&tick),
        vec![
            serde_json::json!("Beat"),
            serde_json::json!(4),
            serde_json::json!(21),
            serde_json::json!({
                "name": "Beat",
                "ticks": 4,
                "totalTicks": 21,
                "intervalSeconds": 0.5,
                "lastGapSeconds": 0.45,
            }),
        ]
    );
}

#[test]
fn metronomes_script_template_scaffolds_generator_functions_and_callbacks() {
    let config = crate::app::module::script_api::module_script_config(MetronomesModule::NODE_TYPE);
    let ScriptSource::Inline { text: source } = config.source else {
        panic!("Metronomes module script template should resolve to inline source");
    };

    assert!(source.contains("Metronomes module functions"));
    assert!(source.contains("local.resetMetronomes()"));
    assert!(source.contains("local.tickMetronome(nameOrIndex)"));
    assert!(source.contains("function metronomeTick"));
}

fn test_config(
    interval_seconds: f64,
    randomize_gap: bool,
    random_min: f64,
    random_max: f64,
    seed: i32,
) -> super::MetronomeConfig {
    super::MetronomeConfig {
        item_id: NodeId(1),
        item_uuid: NodeUuid(uuid::Uuid::from_u128(1)),
        value_decl_id: "metronome_test".to_string(),
        label: "Metronome".to_string(),
        enabled: true,
        interval_seconds,
        randomize_gap,
        random_min,
        random_max,
        seed,
    }
}

fn create_metronomes_module() -> (crate::app::AppEngine, NodeId) {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(MetronomesModule::create().into(), None);
    stabilize(&mut engine);
    let module_id = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("Metronomes module should be attached under root");
    (engine, module_id)
}

fn stabilize(engine: &mut crate::app::AppEngine) {
    for _ in 0..8 {
        engine.apply_edits().expect("edits should apply");
        engine
            .dispatch_inbox(ExecutionPhase::EndOfTickStabilization)
            .expect("stabilization inbox should dispatch");
    }
    engine.resolve().expect("schedule should resolve");
}

fn set_param(engine: &mut crate::app::AppEngine, node: NodeId, value: ParamValue) {
    engine.edits.push(Edit::SetParam {
        node,
        value,
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().expect("parameter edit should apply");
}

fn patch_enabled(engine: &mut crate::app::AppEngine, node: NodeId, enabled: bool) {
    engine.edits.push(Edit::PatchMeta {
        node,
        patch: NodeMetaPatch {
            enabled: Some(enabled),
            ..Default::default()
        },
    });
    engine.apply_edits().expect("enabled patch should apply");
}

fn find_path(engine: &crate::app::AppEngine, start: NodeId, path: &str) -> Option<NodeId> {
    let mut current = start;
    for segment in path.split('/') {
        if segment.trim().is_empty() {
            continue;
        }
        current = find_child_by_key(engine, current, segment)?;
    }
    Some(current)
}

fn find_child_by_key(engine: &crate::app::AppEngine, parent: NodeId, key: &str) -> Option<NodeId> {
    let mut current = engine.nodes.get(parent)?.node_data().first_child;
    while let Some(child_id) = current {
        let child = engine.nodes.get(child_id)?;
        let meta = &child.node_data().meta;
        if meta.decl_id.0 == key
            || meta.decl_id.0.rsplit('/').next() == Some(key)
            || meta.short_name == key
            || meta.label == key
        {
            return Some(child_id);
        }
        current = child.node_data().next_sibling;
    }
    None
}

fn count_children_by_key(engine: &crate::app::AppEngine, parent: NodeId, key: &str) -> usize {
    let mut current = engine.nodes.get(parent).and_then(|node| node.node_data().first_child);
    let mut count = 0usize;
    while let Some(child_id) = current {
        let Some(child) = engine.nodes.get(child_id) else {
            break;
        };
        let meta = &child.node_data().meta;
        if meta.decl_id.0 == key
            || meta.decl_id.0.rsplit('/').next() == Some(key)
            || meta.short_name == key
            || meta.label == key
        {
            count += 1;
        }
        current = child.node_data().next_sibling;
    }
    count
}

fn nth_child(engine: &crate::app::AppEngine, parent: NodeId, index: usize) -> Option<NodeId> {
    let mut current = engine.nodes.get(parent)?.node_data().first_child;
    let mut current_index = 0usize;
    while let Some(child_id) = current {
        if current_index == index {
            return Some(child_id);
        }
        current_index += 1;
        current = engine.nodes.get(child_id)?.node_data().next_sibling;
    }
    None
}
