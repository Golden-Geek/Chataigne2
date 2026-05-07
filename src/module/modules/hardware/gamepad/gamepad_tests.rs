use std::time::Duration;

use golden_core::{
    edit::Edit,
    node::{Folder, Node, NodeId},
    parameter::{ParamValue, ParameterEventBehaviour},
    process_ctx::ExecutionPhase,
    script::ScriptSource,
};

use super::{
    gamepad_connection_warning, gamepad_runtime::DiscoveredGamepad, gamepad_runtime::GamepadRuntimeEvent,
    process_axis_value, GamepadAxis, GamepadButton, GamepadModule, GAMEPAD_UDEV_HELP_URL,
};

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
    let (mut engine, module_id) = create_gamepad_module();
    let device = test_device();

    enqueue_event(
        &mut engine,
        module_id,
        GamepadRuntimeEvent::Connected {
            device: device.clone(),
        },
    );
    enqueue_event(
        &mut engine,
        module_id,
        GamepadRuntimeEvent::AxisChanged {
            device: device.clone(),
            axis: GamepadAxis::LeftStickX,
            value: 0.55,
        },
    );
    enqueue_event(
        &mut engine,
        module_id,
        GamepadRuntimeEvent::ButtonPressed {
            device,
            button: GamepadButton::South,
        },
    );

    run_gamepad_tick(&mut engine);

    // assert_eq!(
    //     bool_param_value(&engine, module_id, "values/info/active"),
    //     Some(true),
    //     "connected test device should become active"
    // );
    assert!(
        (float_param_value(&engine, module_id, "values/axes/left_stick_x").unwrap() - 0.5).abs()
            < 0.000_001,
        "raw 0.55 with default 0.1 dead zone should rescale to 0.5"
    );
    let Some(ParamValue::Vec2(left_x, left_y)) = param_value(&engine, module_id, "values/axes/left_stick") else {
        panic!("left stick vec2 should exist");
    };
    assert!(
        (left_x - 0.5).abs() < 0.000_001 && left_y.abs() < 0.000_001,
        "left stick vec2 should mirror processed X/Y values, got ({left_x}, {left_y})"
    );
    assert_eq!(
        bool_param_value(&engine, module_id, "values/buttons/south"),
        Some(true),
        "button press should update the South button value"
    );
}

#[test]
fn gamepad_script_template_scaffolds_gamepad_callbacks_only() {
    let config = crate::app::module::script_api::module_script_config(GamepadModule::NODE_TYPE);
    let ScriptSource::Inline(source) = config.source else {
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
        engine
            .apply_edits()
            .expect("gamepad defaults should materialize");
    }
    engine
        .resolve()
        .expect("gamepad runtime schedule should resolve");

    let module_id = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("gamepad module should be attached under root");

    (engine, module_id)
}

fn enqueue_event(engine: &mut crate::app::AppEngine, module_id: NodeId, event: GamepadRuntimeEvent) {
    let crate::app::AppNode::GamepadModule(module) =
        engine.nodes.get_mut(module_id).expect("gamepad module should exist")
    else {
        panic!("expected GamepadModule node");
    };
    module.enqueue_runtime_event_for_test(event);
}

fn test_device() -> DiscoveredGamepad {
    DiscoveredGamepad {
        index: 0,
        variant_id: "gamepad:0|Test Pad".to_string(),
        label: "Test Pad".to_string(),
        details: "index=0, vid=1234, pid=5678".to_string(),
    }
}

fn run_gamepad_tick(engine: &mut crate::app::AppEngine) {
    engine
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .expect("gamepad inbox should dispatch");
    engine
        .run_tick(Duration::from_millis(20))
        .expect("gamepad tick should run");
    engine
        .apply_edits()
        .expect("gamepad tick edits should apply");
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

fn param_value(engine: &crate::app::AppEngine, start: NodeId, path: &str) -> Option<ParamValue> {
    let param_id = find_path(engine, start, path)?;
    engine
        .nodes
        .get(param_id)?
        .engine_param_snapshot()
        .map(|snapshot| snapshot.value)
}

fn bool_param_value(engine: &crate::app::AppEngine, start: NodeId, path: &str) -> Option<bool> {
    match param_value(engine, start, path)? {
        ParamValue::Bool(value) => Some(value),
        _ => None,
    }
}

fn float_param_value(engine: &crate::app::AppEngine, start: NodeId, path: &str) -> Option<f64> {
    match param_value(engine, start, path)? {
        ParamValue::Float(value) => Some(value),
        _ => None,
    }
}

#[allow(dead_code)]
fn set_param(engine: &mut crate::app::AppEngine, node: NodeId, value: ParamValue) {
    engine.edits.push(Edit::SetParam {
        node,
        value,
        behaviour: ParameterEventBehaviour::Coalesce,
    });
}
