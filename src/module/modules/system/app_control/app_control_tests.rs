use std::time::Duration;

use golden_core::{
    node::{Folder, Node, NodeId},
    parameter::{ParamValue, Parameter, ParameterChangeCheck},
    process_ctx::ExecutionPhase,
    script::ScriptSource,
};

use crate::app::module::common::app_control::ProcessMatchMode;

use super::AppControlModule;

#[test]
fn app_control_module_initializes_watch_roots() {
    let (engine, module_id) = create_module();

    assert!(find_path(&engine, module_id, "parameters/watched_apps_targets").is_some());
    assert!(find_path(&engine, module_id, "parameters/watched_folders_targets").is_some());
    assert!(find_path(&engine, module_id, "values/watched_apps_values").is_some());
    assert!(find_path(&engine, module_id, "values/watched_folders_values").is_some());
}

#[test]
fn app_control_module_stays_idle_without_watch_entries() {
    let (engine, module_id) = create_module();

    assert!(!module_needs_update(&engine, module_id));
}

#[test]
fn watched_app_configuration_changes_drive_app_control_tick_need() {
    let (mut engine, module_id) = create_module();
    let watched_apps_id =
        find_path(&engine, module_id, "parameters/watched_apps_targets").expect("watched apps root should exist");

    assert!(!module_needs_update(&engine, module_id));

    engine.add_user_item(create_test_watched_app().into(), Some(watched_apps_id));
    engine
        .apply_edits()
        .expect("watched app parameter should attach under the watched apps root");
    engine
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .expect("watched app add should dispatch App Control inbox callbacks");

    assert!(module_needs_update(&engine, module_id));

    run_app_control_ticks(&mut engine, 1);
    assert!(!module_needs_update(&engine, module_id));

    let watched_app_id = engine
        .nodes
        .get(watched_apps_id)
        .and_then(|root| root.node_data().first_child)
        .expect("watched app parameter should be present");

    set_param(
        &mut engine,
        watched_app_id,
        ParamValue::File("C:/Program Files/Chataigne/Chataigne.exe".to_string()),
    );
    engine
        .apply_edits()
        .expect("watched app file parameter should accept a target path");
    engine
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .expect("watched app target change should dispatch App Control inbox callbacks");

    assert!(module_needs_update(&engine, module_id));

    run_app_control_ticks(&mut engine, 1);
    assert!(module_needs_update(&engine, module_id));

    set_param(&mut engine, watched_app_id, ParamValue::File(String::new()));
    engine
        .apply_edits()
        .expect("watched app file parameter should accept clearing the target path");
    engine
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .expect("watched app target clear should dispatch App Control inbox callbacks");

    assert!(module_needs_update(&engine, module_id));

    run_app_control_ticks(&mut engine, 1);

    assert!(!module_needs_update(&engine, module_id));
}

#[test]
fn app_control_script_template_scaffolds_functions_and_callbacks() {
    let config = crate::app::module::script_api::module_script_config(AppControlModule::NODE_TYPE);
    let ScriptSource::Inline(source) = config.source else {
        panic!("App Control module script template should resolve to inline source");
    };

    assert!(source.contains("local.launchWatchedApp"));
    assert!(source.contains("local.controlWindow"));
    assert!(source.contains("function watchFolderChanged"));
    assert!(source.contains("function appControlCommandFailed"));
}

#[test]
fn process_match_mode_accepts_requested_aliases() {
    assert_eq!(
        ProcessMatchMode::from_variant("startwith"),
        Some(ProcessMatchMode::StartsWith)
    );
    assert_eq!(
        ProcessMatchMode::from_variant("endwidth"),
        Some(ProcessMatchMode::EndsWith)
    );
}

#[test]
fn watched_app_file_parameter_populates_values_and_running_control() {
    let (mut engine, module_id) = create_module();
    let watched_apps_id =
        find_path(&engine, module_id, "parameters/watched_apps_targets").expect("watched apps root should exist");

    engine.add_user_item(create_test_watched_app().into(), Some(watched_apps_id));
    engine
        .apply_edits()
        .expect("watched app parameter should attach under the watched apps root");

    let watched_app_id = engine
        .nodes
        .get(watched_apps_id)
        .and_then(|root| root.node_data().first_child)
        .expect("watched app parameter should be present");

    set_param(
        &mut engine,
        watched_app_id,
        ParamValue::File("C:/Program Files/Chataigne/Chataigne.exe".to_string()),
    );
    engine
        .apply_edits()
        .expect("watched app file parameter should accept a target path");
    engine
        .apply_edits()
        .expect("watched app auto-rename edits should apply");
    engine
        .resolve()
        .expect("App Control schedule should resolve after watched app rename");

    run_app_control_ticks(&mut engine, 3);

    let watched_app = engine
        .nodes
        .get(watched_app_id)
        .expect("watched app parameter should still exist");
    assert_eq!(watched_app.node_data().meta.label, "Chataigne");

    let values_folder_id = find_path(&engine, module_id, "values/watched_apps_values/Chataigne")
        .expect("watched app values should mirror the file parameter label");
    let running_snapshot = param_snapshot(&engine, values_folder_id, "running")
        .expect("watched app values should expose a running control");
    assert!(!running_snapshot.read_only, "running control should be writable");
    assert_eq!(running_snapshot.value, ParamValue::Bool(false));
    assert_eq!(
        param_snapshot(&engine, values_folder_id, "cpu_ratio").map(|snapshot| snapshot.value),
        Some(ParamValue::Float(0.0))
    );
    assert_eq!(
        param_snapshot(&engine, values_folder_id, "memory_mb").map(|snapshot| snapshot.value),
        Some(ParamValue::Float(0.0))
    );
    assert_eq!(
        param_snapshot(&engine, values_folder_id, "uptime_seconds").map(|snapshot| snapshot.value),
        Some(ParamValue::Float(0.0))
    );
    assert_eq!(
        param_snapshot(&engine, values_folder_id, "uptime_seconds")
            .and_then(|snapshot| snapshot.ui_hints.widget),
        Some("time".to_string())
    );
    assert_eq!(
        string_param_value(&engine, module_id, "values/watched_apps_values/Chataigne/name"),
        Some("Chataigne".to_string())
    );
}

#[test]
fn watched_app_value_folder_survives_sibling_app_addition() {
    let (mut engine, module_id) = create_module();
    let watched_apps_id =
        find_path(&engine, module_id, "parameters/watched_apps_targets").expect("watched apps root should exist");

    engine.add_user_item(create_test_watched_app().into(), Some(watched_apps_id));
    engine
        .apply_edits()
        .expect("first watched app parameter should attach under the watched apps root");

    let first_watched_app_id = engine
        .nodes
        .get(watched_apps_id)
        .and_then(|root| root.node_data().first_child)
        .expect("first watched app parameter should be present");
    set_param(
        &mut engine,
        first_watched_app_id,
        ParamValue::File("C:/Program Files/Chataigne/Chataigne.exe".to_string()),
    );
    engine
        .apply_edits()
        .expect("first watched app file parameter should accept a target path");

    run_app_control_ticks(&mut engine, 2);

    let first_value_folder_id = find_path(&engine, module_id, "values/watched_apps_values/Chataigne")
        .expect("first watched app values should exist");

    engine.add_user_item(create_test_watched_app().into(), Some(watched_apps_id));
    engine
        .apply_edits()
        .expect("second watched app parameter should attach under the watched apps root");

    let second_watched_app_id = engine
        .nodes
        .get(first_watched_app_id)
        .and_then(|node| node.node_data().next_sibling)
        .expect("second watched app parameter should be present");
    set_param(
        &mut engine,
        second_watched_app_id,
        ParamValue::File("C:/Program Files/Other/Other.exe".to_string()),
    );
    engine
        .apply_edits()
        .expect("second watched app file parameter should accept a target path");

    run_app_control_ticks(&mut engine, 1);

    let current_first_value_folder_id = find_path(&engine, module_id, "values/watched_apps_values/Chataigne")
        .expect("first watched app values should still exist after adding a sibling app");
    assert_eq!(current_first_value_folder_id, first_value_folder_id);
}

#[test]
fn existing_command_updates_watched_app_enum_after_inbox_dispatch_without_periodic_update() {
    let (mut engine, module_id) = create_module();
    let command_tester_id =
        find_path(&engine, module_id, "command_tester").expect("command tester should exist");
    engine.edits.push(golden_core::edit::Edit::AddUserItem {
        parent: command_tester_id,
        prev_sibling: None,
        node: crate::app::create_declared_user_item(
            super::commands::APP_CONTROL_LAUNCH_PROCESS_COMMAND_NODE_TYPE,
            crate::app::module_command::MODULE_COMMAND_ITEM_KIND,
        )
        .expect("launch command should be creatable as a declared module command"),
    });
    engine
        .apply_edits()
        .expect("launch command should attach under the command tester");

    let watched_apps_id =
        find_path(&engine, module_id, "parameters/watched_apps_targets").expect("watched apps root should exist");
    engine.add_user_item(create_test_watched_app().into(), Some(watched_apps_id));
    engine
        .apply_edits()
        .expect("watched app parameter should attach under the watched apps root");

    let watched_app_id = engine
        .nodes
        .get(watched_apps_id)
        .and_then(|root| root.node_data().first_child)
        .expect("watched app parameter should be present");
    set_param(
        &mut engine,
        watched_app_id,
        ParamValue::File("C:/Program Files/Chataigne/Chataigne.exe".to_string()),
    );
    engine
        .apply_edits()
        .expect("watched app file parameter should accept a target path");
    engine
        .dispatch_inbox(ExecutionPhase::EndOfTickStabilization)
        .expect("watched app param change should dispatch App Control inbox callbacks");
    engine
        .apply_edits()
        .expect("runtime enum sync edits should flush after inbox dispatch");

    let command_id = engine
        .nodes
        .get(command_tester_id)
        .and_then(|tester| tester.node_data().first_child)
        .expect("command tester should contain the new launch command");
    let watched_app_snapshot = param_snapshot(&engine, command_id, "watched_app")
        .expect("launch command should expose a watched-app selector");
    let variants = watched_app_snapshot
        .constraints
        .enum_options
        .into_iter()
        .map(|option| option.variant_id)
        .collect::<Vec<_>>();
    assert_eq!(variants, vec!["Chataigne".to_string()]);
    assert_eq!(watched_app_snapshot.value, ParamValue::Enum("Chataigne".to_string()));
}

#[test]
fn command_added_after_watched_app_populates_enum_without_waiting_for_tick() {
    let (mut engine, module_id) = create_module();
    let watched_apps_id =
        find_path(&engine, module_id, "parameters/watched_apps_targets").expect("watched apps root should exist");

    engine.add_user_item(create_test_watched_app().into(), Some(watched_apps_id));
    engine
        .apply_edits()
        .expect("watched app parameter should attach under the watched apps root");

    let watched_app_id = engine
        .nodes
        .get(watched_apps_id)
        .and_then(|root| root.node_data().first_child)
        .expect("watched app parameter should be present");
    set_param(
        &mut engine,
        watched_app_id,
        ParamValue::File("C:/Program Files/Chataigne/Chataigne.exe".to_string()),
    );
    engine
        .apply_edits()
        .expect("watched app file parameter should accept a target path");

    run_app_control_ticks(&mut engine, 1);

    let command_tester_id =
        find_path(&engine, module_id, "command_tester").expect("command tester should exist");
    engine.edits.push(golden_core::edit::Edit::AddUserItem {
        parent: command_tester_id,
        prev_sibling: None,
        node: crate::app::create_declared_user_item(
            super::commands::APP_CONTROL_LAUNCH_PROCESS_COMMAND_NODE_TYPE,
            crate::app::module_command::MODULE_COMMAND_ITEM_KIND,
        )
        .expect("launch command should be creatable as a declared module command"),
    });
    engine
        .apply_edits()
        .expect("launch command should attach under the command tester");
    engine
        .dispatch_inbox(ExecutionPhase::EndOfTickStabilization)
        .expect("command add should dispatch App Control inbox callbacks");
    engine
        .apply_edits()
        .expect("runtime enum sync edits should flush after inbox dispatch");

    let command_id = engine
        .nodes
        .get(command_tester_id)
        .and_then(|tester| tester.node_data().first_child)
        .expect("command tester should contain the new launch command");
    let watched_app_snapshot = param_snapshot(&engine, command_id, "watched_app")
        .expect("launch command should expose a watched-app selector");
    let variants = watched_app_snapshot
        .constraints
        .enum_options
        .into_iter()
        .map(|option| option.variant_id)
        .collect::<Vec<_>>();
    assert_eq!(variants, vec!["Chataigne".to_string()]);
    assert_eq!(watched_app_snapshot.value, ParamValue::Enum("Chataigne".to_string()));
}

#[test]
fn launch_command_watched_app_param_tracks_available_watched_apps() {
    let (mut engine, module_id) = create_module();
    let watched_apps_id =
        find_path(&engine, module_id, "parameters/watched_apps_targets").expect("watched apps root should exist");

    engine.add_user_item(create_test_watched_app().into(), Some(watched_apps_id));
    engine
        .apply_edits()
        .expect("watched app parameter should attach under the watched apps root");

    let watched_app_id = engine
        .nodes
        .get(watched_apps_id)
        .and_then(|root| root.node_data().first_child)
        .expect("watched app parameter should be present");
    set_param(
        &mut engine,
        watched_app_id,
        ParamValue::File("C:/Program Files/Chataigne/Chataigne.exe".to_string()),
    );
    engine
        .apply_edits()
        .expect("watched app file parameter should accept a target path");
    engine
        .apply_edits()
        .expect("watched app auto-rename edits should apply");
    engine
        .resolve()
        .expect("App Control schedule should resolve after watched app rename");

    let command_tester_id =
        find_path(&engine, module_id, "command_tester").expect("command tester should exist");
    engine.edits.push(golden_core::edit::Edit::AddUserItem {
        parent: command_tester_id,
        prev_sibling: None,
        node: crate::app::create_declared_user_item(
            super::commands::APP_CONTROL_LAUNCH_PROCESS_COMMAND_NODE_TYPE,
            crate::app::module_command::MODULE_COMMAND_ITEM_KIND,
        )
        .expect("launch command should be creatable as a declared module command"),
    });
    engine
        .apply_edits()
        .expect("launch command should attach under the command tester");

    run_app_control_ticks(&mut engine, 1);

    let command_id = engine
        .nodes
        .get(command_tester_id)
        .and_then(|tester| tester.node_data().first_child)
        .expect("command tester should contain the new launch command");
    let watched_app_snapshot = param_snapshot(&engine, command_id, "watched_app")
        .expect("launch command should expose a watched-app selector");
    let variants = watched_app_snapshot
        .constraints
        .enum_options
        .into_iter()
        .map(|option| option.variant_id)
        .collect::<Vec<_>>();
    assert_eq!(variants, vec!["Chataigne".to_string()]);
    assert_eq!(watched_app_snapshot.value, ParamValue::Enum("Chataigne".to_string()));
}

#[test]
fn existing_launch_command_updates_watched_app_enum_when_app_is_added() {
    let (mut engine, module_id) = create_module();
    let command_tester_id =
        find_path(&engine, module_id, "command_tester").expect("command tester should exist");
    engine.edits.push(golden_core::edit::Edit::AddUserItem {
        parent: command_tester_id,
        prev_sibling: None,
        node: crate::app::create_declared_user_item(
            super::commands::APP_CONTROL_LAUNCH_PROCESS_COMMAND_NODE_TYPE,
            crate::app::module_command::MODULE_COMMAND_ITEM_KIND,
        )
        .expect("launch command should be creatable as a declared module command"),
    });
    engine
        .apply_edits()
        .expect("launch command should attach under the command tester");

    let watched_apps_id =
        find_path(&engine, module_id, "parameters/watched_apps_targets").expect("watched apps root should exist");
    engine.add_user_item(create_test_watched_app().into(), Some(watched_apps_id));
    engine
        .apply_edits()
        .expect("watched app parameter should attach under the watched apps root");

    let watched_app_id = engine
        .nodes
        .get(watched_apps_id)
        .and_then(|root| root.node_data().first_child)
        .expect("watched app parameter should be present");
    set_param(
        &mut engine,
        watched_app_id,
        ParamValue::File("C:/Program Files/Chataigne/Chataigne.exe".to_string()),
    );
    engine
        .apply_edits()
        .expect("watched app file parameter should accept a target path");

    run_app_control_ticks(&mut engine, 1);

    let command_id = engine
        .nodes
        .get(command_tester_id)
        .and_then(|tester| tester.node_data().first_child)
        .expect("command tester should contain the new launch command");
    let watched_app_snapshot = param_snapshot(&engine, command_id, "watched_app")
        .expect("launch command should expose a watched-app selector");
    let variants = watched_app_snapshot
        .constraints
        .enum_options
        .into_iter()
        .map(|option| option.variant_id)
        .collect::<Vec<_>>();
    assert_eq!(variants, vec!["Chataigne".to_string()]);
    assert_eq!(watched_app_snapshot.value, ParamValue::Enum("Chataigne".to_string()));
}

fn create_module() -> (crate::app::AppEngine, NodeId) {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(AppControlModule::create().into(), None);
    engine.apply_edits().expect("App Control module should attach");
    for _ in 0..4 {
        engine
            .apply_edits()
            .expect("App Control module defaults should materialize");
    }
    engine
        .resolve()
        .expect("App Control module schedule should resolve");

    let module_id = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("App Control module should be attached under root");

    (engine, module_id)
}

fn create_test_watched_app() -> Parameter {
    Parameter::new(
        "Watched App",
        ParamValue::File(String::new()),
        ParameterChangeCheck::ValueChange,
    )
}

fn run_app_control_ticks(engine: &mut crate::app::AppEngine, count: usize) {
    for _ in 0..count {
        engine
            .dispatch_inbox(ExecutionPhase::EngineTick)
            .expect("pending App Control events should dispatch");
        engine
            .run_tick(Duration::from_millis(600))
            .expect("App Control runtime tick should succeed");
        engine
            .apply_edits()
            .expect("pending App Control edits should apply");
        engine
            .resolve()
            .expect("App Control schedule should resolve after tick");
    }
}

fn set_param(engine: &mut crate::app::AppEngine, node: NodeId, value: ParamValue) {
    engine.edits.push(golden_core::edit::Edit::SetParam {
        node,
        value,
        behaviour: golden_core::parameter::ParameterEventBehaviour::Coalesce,
    });
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
        if node_key_matches(child.node_data(), key) {
            return Some(child_id);
        }
        current = child.node_data().next_sibling;
    }
    None
}

fn node_key_matches(node: &golden_core::node::NodeData, key: &str) -> bool {
    node.meta.decl_id.0 == key
        || node.meta.decl_id.0.rsplit('/').next() == Some(key)
        || node.meta.short_name == key
        || node.meta.label == key
}

fn param_snapshot(
    engine: &crate::app::AppEngine,
    start: NodeId,
    path: &str,
) -> Option<golden_core::parameter::ParameterSnapshot> {
    let param_id = find_path(engine, start, path)?;
    engine.nodes.get(param_id)?.engine_param_snapshot()
}

fn string_param_value(engine: &crate::app::AppEngine, start: NodeId, path: &str) -> Option<String> {
    match param_snapshot(engine, start, path)?.value {
        ParamValue::Str(value) => Some(value),
        _ => None,
    }
}

fn module_needs_update(engine: &crate::app::AppEngine, module_id: NodeId) -> bool {
    engine
        .nodes
        .get(module_id)
        .expect("App Control module should be present")
        .needs_update()
}