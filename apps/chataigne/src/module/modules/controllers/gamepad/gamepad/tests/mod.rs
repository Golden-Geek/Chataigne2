use golden_core::{
    node::{Folder, Node, NodeId},
    script::ScriptSource,
};

use super::{gamepad_connection_warning, process_axis_value, GamepadAxis, GamepadModule, GAMEPAD_UDEV_HELP_URL};

#[test]
fn axis_processing_applies_offset_dead_zone_and_rescales() {
    assert_eq!(process_axis_value(0.05, 0.1, 0.0), 0.0);
    assert_eq!(process_axis_value(0.25, 0.2, -0.1), 0.0);
    assert!((process_axis_value(0.7, 0.2, -0.1) - 0.5).abs() < 0.000_001);
    assert!((process_axis_value(-0.7, 0.2, 0.1) + 0.5).abs() < 0.000_001);
}

#[test]
fn permission_denied_warning_mentions_linux_udev_rules_with_link() {
    let warning = gamepad_connection_warning("permission denied opening /dev/input/event7");

    assert!(
        warning.message.contains("udev rules"),
        "permission-denied warning should mention udev rules"
    );
    assert!(
        warning
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains(GAMEPAD_UDEV_HELP_URL)),
        "permission-denied warning should include the GilRs Linux evdev help link"
    );
}

#[test]
fn gamepad_module_materializes_axis_processing_for_every_axis() {
    let (engine, module_id) = create_gamepad_module();

    for axis in GamepadAxis::ALL {
        let base_path = format!("parameters/axis_processing/{}", axis.decl_id());
        assert!(
            find_path(&engine, module_id, base_path.as_str()).is_some(),
            "axis processing folder for {} should exist",
            axis.label()
        );
        assert!(
            find_path(
                &engine,
                module_id,
                format!("{base_path}/{}_dead_zone", axis.decl_id()).as_str()
            )
            .is_some(),
            "dead-zone parameter for {} should exist",
            axis.label()
        );
        assert!(
            find_path(
                &engine,
                module_id,
                format!("{base_path}/{}_offset", axis.decl_id()).as_str()
            )
            .is_some(),
            "offset parameter for {} should exist",
            axis.label()
        );
    }
}

#[test]
fn selected_gamepad_events_update_values_after_axis_processing() {
    super::service::tests::assert_selected_event_values();
}

#[test]
fn gamepad_script_template_scaffolds_gamepad_callbacks_only() {
    let config = crate::app::module::script_api::module_script_config(GamepadModule::NODE_TYPE);
    let ScriptSource::Inline { text: source } = config.source else {
        panic!("gamepad module script template should resolve to inline source");
    };

    assert!(source.contains("function gamepadAxisChanged"));
    assert!(source.contains("function gamepadButtonPressed"));
    assert!(!source.contains("function noteOnReceived"));
    assert!(!source.contains("function clientConnected"));
}

fn create_gamepad_module() -> (crate::app::AppEngine, NodeId) {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    let mut module = GamepadModule::create();
    module.disable_runtime_for_test();
    engine.add_node(module.into(), None);
    engine.apply_edits().expect("gamepad module should attach");
    for _ in 0..4 {
        engine.apply_edits().expect("gamepad defaults should materialize");
    }
    engine.resolve().expect("gamepad runtime schedule should resolve");

    let module_id = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("gamepad module should be attached under root");

    (engine, module_id)
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
