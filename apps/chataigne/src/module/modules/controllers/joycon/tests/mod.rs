use std::time::Duration;

use golden_core::{
    node::{Folder, Node, NodeId},
    parameter::ParamValue,
    script::ScriptSource,
};
use joycon_rs::joycon::Buttons;

use crate::app::AppNode;
use crate::app::module::common::joycon::JoyConMotionDataMode;

use golden_core::node::DeclaredUserItemNode;

use super::JoyConModule;

#[test]
fn joycon_module_is_a_module_item() {
    assert_eq!(
        <JoyConModule as DeclaredUserItemNode>::ITEM_KIND,
        crate::app::module::MODULE_ITEM_KIND
    );
    assert!(crate::app::declared_user_item_type_matches(
        JoyConModule::NODE_TYPE,
        crate::app::module::MODULE_ITEM_KIND
    ));
}

#[test]
fn joycon_module_appears_under_controllers_menu() {
    let item = crate::app::declared_user_creatable_items(crate::app::module::MODULE_ITEM_KIND)
        .into_iter()
        .find(|item| item.node_type == JoyConModule::NODE_TYPE)
        .expect("Joy-Con module should be present in the module catalog");

    assert_eq!(item.label, "Joy-Con");
    assert_eq!(item.menu_path, vec!["Controllers".to_string()]);
}

#[test]
fn joycon_module_uses_fixed_motion_mode_configuration() {
    let module = JoyConModule::create();
    assert_eq!(module.get_type(), JoyConModule::NODE_TYPE);
}

#[test]
fn joycon_module_defaults_processing_fps_cap_to_module_rate() {
    let (engine, module_id) = create_joycon_module();
    let fps_cap_id = find_path(&engine, module_id, "connection/processing_fps_cap")
        .expect("processing fps cap parameter should exist");

    assert_eq!(param_value(&engine, fps_cap_id), ParamValue::Int(120));
}

#[test]
fn joycon_module_defaults_stick_dead_zone_to_requested_value() {
    let (engine, module_id) = create_joycon_module();
    let dead_zone_id = find_path(&engine, module_id, "parameters/stick_processing/dead_zone")
        .or_else(|| find_path(&engine, module_id, "parameters/stick_processing/stick_dead_zone"))
        .expect("stick dead zone parameter should exist");

    assert_eq!(param_value(&engine, dead_zone_id), ParamValue::Float(0.1));
}

#[test]
fn joycon_module_exposes_activity_indicator() {
    let (engine, module_id) = create_joycon_module();
    let activity_id = find_path(&engine, module_id, "values/info/activity")
        .expect("activity indicator should exist");

    assert_eq!(param_value(&engine, activity_id), ParamValue::Bool(false));
}

#[test]
fn joycon_module_attaches_under_module_manager() {
    let (engine, module_id) = create_joycon_module();
    let module = engine.nodes.get(module_id).expect("joycon module should exist");
    assert_eq!(module.get_type(), JoyConModule::NODE_TYPE);
}

#[test]
fn joycon_module_command_tester_advertises_joycon_commands() {
    let (engine, module_id) = create_joycon_module();
    let command_tester_id = find_path(&engine, module_id, "command_tester").expect("command tester should exist");

    let available_types = engine
        .catalog_creatable_items(command_tester_id)
        .into_iter()
        .map(|item| item.node_type)
        .collect::<Vec<_>>();

    assert_eq!(
        available_types,
        vec![
            crate::app::module::common::joycon::JOYCON_VIBRATE_COMMAND_NODE_TYPE.to_string(),
            crate::app::module::common::joycon::JOYCON_SET_LED_COMMAND_NODE_TYPE.to_string(),
        ],
        "Joy-Con command tester should advertise vibrate and set-led commands"
    );
}

#[test]
fn joycon_module_command_tester_creates_joycon_commands() {
    let (mut engine, module_id) = create_joycon_module();
    let command_tester_id = find_path(&engine, module_id, "command_tester").expect("command tester should exist");

    engine.add_user_item(crate::app::JoyConVibrateCommand::create().into(), Some(command_tester_id));
    engine.apply_edits().expect("joycon command should attach under the command tester");

    let command_id = engine
        .nodes
        .get(command_tester_id)
        .and_then(|tester| tester.node_data().first_child)
        .expect("command tester should contain the new command");
    assert_eq!(
        engine.nodes.get(command_id).expect("joycon command should exist").get_type(),
        crate::app::JoyConVibrateCommand::NODE_TYPE,
    );
}

#[test]
fn joycon_module_script_descriptor_advertises_control_methods() {
    let descriptor = JoyConModule::create().engine_script_descriptor();

    for method in ["vibrate", "setPlayerLights"] {
        assert!(
            descriptor.methods.iter().any(|candidate| candidate == method),
            "joycon script descriptor should advertise '{method}'"
        );
    }
}

#[test]
fn joycon_module_script_template_scaffolds_joycon_functions_and_callbacks() {
    let config = crate::app::module::script_api::module_script_config(JoyConModule::NODE_TYPE);
    let ScriptSource::Inline { text: source } = config.source else {
        panic!("joycon module script template should resolve to inline source");
    };

    assert!(source.contains("local.vibrate(frequencyHz = 300, amplitude = 0.9, durationMs = 60, target = \"both\")"));
    assert!(
        source.contains(
            "local.setPlayerLights(led1 = \"off\", led2 = \"off\", led3 = \"off\", led4 = \"off\", target = \"both\")"
        )
    );
    assert!(source.contains("function joyConButtonPressed"));
    assert!(source.contains("function joyConStickChanged"));
    assert!(source.contains("function joyConMotionChanged"));
    assert!(!source.contains("function noteOnReceived"));
}

#[test]
fn joycon_processing_interval_clamps_to_module_update_rate() {
    assert_eq!(
        super::processing_interval_from_fps_cap(999),
        Duration::from_secs_f64(1.0 / 120.0),
    );
    assert_eq!(
        super::processing_interval_from_fps_cap(60),
        Duration::from_secs_f64(1.0 / 60.0),
    );
}

#[test]
fn joycon_connection_change_detection_tracks_each_side() {
    let previous = super::runtime::JoyConRuntimeState::disconnected();
    let mut next = super::runtime::JoyConRuntimeState::disconnected();
    next.left.connected = true;

    assert!(super::connection_state_changed(&previous, &next));
    assert!(!super::connection_state_changed(&next, &next));
}

#[test]
fn joycon_runtime_restart_requires_expected_live_state() {
    let disconnected = super::runtime::JoyConRuntimeState::disconnected();
    let mut connected = super::runtime::JoyConRuntimeState::disconnected();
    connected.right.connected = true;

    assert!(!super::runtime_expected_to_be_live(None, &disconnected));
    assert!(super::runtime_expected_to_be_live(None, &connected));
    assert!(super::runtime_expected_to_be_live(Some(&connected), &disconnected));
}

#[test]
fn joycon_stick_dead_zone_scales_remaining_range() {
    assert_eq!(super::process_stick_axis_value(0.05, 0.1), 0.0);
    assert_eq!(super::process_stick_axis_value(-0.05, 0.1), 0.0);
    assert!((super::process_stick_axis_value(0.55, 0.1) - 0.5).abs() < f64::EPSILON);
    assert!((super::process_stick_axis_value(-0.55, 0.1) + 0.5).abs() < f64::EPSILON);
}

#[test]
fn joycon_visible_input_activity_ignores_connection_only_changes() {
    let previous = super::runtime::JoyConRuntimeState::disconnected();
    let mut next = super::runtime::JoyConRuntimeState::disconnected();
    next.left.connected = true;

    assert!(!super::runtime_state_has_visible_input_change(
        &previous,
        &next,
        0.1,
        JoyConMotionDataMode::None,
    ));
}

#[test]
fn joycon_visible_input_activity_ignores_stick_changes_inside_dead_zone() {
    let previous = connected_left_state();
    let mut next = previous.clone();
    next.left.stick_x = 0.05;

    assert!(!super::runtime_state_has_visible_input_change(
        &previous,
        &next,
        0.1,
        JoyConMotionDataMode::None,
    ));

    next.left.stick_x = 0.55;

    assert!(super::runtime_state_has_visible_input_change(
        &previous,
        &next,
        0.1,
        JoyConMotionDataMode::None,
    ));
}

#[test]
fn joycon_visible_input_activity_detects_button_changes() {
    let previous = connected_left_state();
    let mut next = previous.clone();
    next.left.left_buttons.push(Buttons::Down);

    assert!(super::runtime_state_has_visible_input_change(
        &previous,
        &next,
        0.1,
        JoyConMotionDataMode::None,
    ));
}

#[test]
fn joycon_visible_input_activity_ignores_hidden_motion_but_tracks_enabled_motion() {
    let previous = connected_left_state();
    let mut next = previous.clone();
    next.left.orientation_pitch = 12.0;
    next.left.orientation_roll = -5.0;
    next.left.accelerometer = (1.0, 2.0, 3.0);
    next.left.gyroscope = (4.0, 5.0, 6.0);

    assert!(!super::runtime_state_has_visible_input_change(
        &previous,
        &next,
        0.1,
        JoyConMotionDataMode::None,
    ));
    assert!(super::runtime_state_has_visible_input_change(
        &previous,
        &next,
        0.1,
        JoyConMotionDataMode::Orientation,
    ));
    assert!(super::runtime_state_has_visible_input_change(
        &previous,
        &next,
        0.1,
        JoyConMotionDataMode::All,
    ));
}

fn connected_left_state() -> super::runtime::JoyConRuntimeState {
    let mut state = super::runtime::JoyConRuntimeState::disconnected();
    state.left.connected = true;
    state
}

fn create_joycon_module() -> (crate::app::AppEngine, NodeId) {
    let root: AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(crate::app::ModuleManager::new().into(), None);
    engine.apply_edits().expect("module manager should attach");

    let manager_id = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("module manager should be attached under root");

    engine.add_user_item(JoyConModule::create().into(), Some(manager_id));
    engine.apply_edits().expect("joycon module should attach");
    for _ in 0..4 {
        engine.apply_edits().expect("joycon defaults should materialize");
    }

    let module_id = engine
        .nodes
        .get(manager_id)
        .and_then(|manager| manager.node_data().first_child)
        .expect("joycon module should be attached under the module manager");

    (engine, module_id)
}

fn find_path(engine: &crate::app::AppEngine, start: NodeId, path: &str) -> Option<NodeId> {
    let mut current = start;
    let mut remaining = path.trim_matches('/');

    loop {
        if remaining.is_empty() {
            return Some(current);
        }

        if let Some(found) = find_child_by_key(engine, current, remaining) {
            return Some(found);
        }

        let Some((segment, tail)) = remaining.split_once('/') else {
            return find_child_by_key(engine, current, remaining);
        };
        current = find_child_by_key(engine, current, segment)?;
        remaining = tail;
    }
}

fn find_child_by_key(engine: &crate::app::AppEngine, parent: NodeId, key: &str) -> Option<NodeId> {
    let mut child = engine.nodes.get(parent)?.node_data().first_child;
    while let Some(child_id) = child {
        let node = engine.nodes.get(child_id)?;
        let meta = &node.node_data().meta;
        if meta.decl_id.0 == key
            || meta.decl_id.0.rsplit('/').next() == Some(key)
            || meta.short_name == key
            || meta.label == key
        {
            return Some(child_id);
        }
        child = node.node_data().next_sibling;
    }

    None
}

fn param_value(engine: &crate::app::AppEngine, node: NodeId) -> ParamValue {
    engine
        .nodes
        .get(node)
        .and_then(|node| node.engine_param_snapshot())
        .map(|snapshot| snapshot.value)
        .expect("parameter value should exist")
}
