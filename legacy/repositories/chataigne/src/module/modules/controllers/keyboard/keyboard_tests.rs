use std::time::Duration;

use golden_core::{
    node::{Folder, Node, NodeId},
    parameter::ParamValue,
    process_ctx::ExecutionPhase,
    script::ScriptSource,
};

use super::{keyboard_runtime::KeyboardInputEvent, KeyboardModule};
use crate::app::module::common::keyboard::KeyboardKey;

#[test]
fn keyboard_events_update_values() {
    let (mut engine, module_id) = create_keyboard_module();

    enqueue_event(
        &mut engine,
        module_id,
        KeyboardInputEvent::KeyChanged {
            key: KeyboardKey::LeftShift,
            pressed: true,
        },
    );
    enqueue_event(
        &mut engine,
        module_id,
        KeyboardInputEvent::KeyChanged {
            key: KeyboardKey::A,
            pressed: true,
        },
    );

    run_keyboard_tick(&mut engine);

    assert_eq!(
        bool_param_value(&engine, module_id, "values/modifiers/left_shift"),
        Some(true),
        "left shift state should update from incoming keyboard events"
    );
    assert_eq!(
        string_param_value(&engine, module_id, "values/info/last_key"),
        Some("a".to_string()),
        "last key should store the most recent keyboard key id"
    );
    assert_eq!(
        bool_param_value(&engine, module_id, "values/info/last_pressed"),
        Some(true),
        "last pressed should reflect the most recent keyboard event"
    );
    assert_eq!(
        int_param_value(&engine, module_id, "values/info/held_key_count"),
        Some(2),
        "held key count should track supported pressed keys"
    );
    let held_keys = string_param_value(&engine, module_id, "values/info/held_keys")
        .expect("held keys value should exist");
    assert!(held_keys.contains("a"));
    assert!(held_keys.contains("left_shift"));
}

#[test]
fn keyboard_script_template_scaffolds_keyboard_functions_and_callbacks() {
    let config = crate::app::module::script_api::module_script_config(KeyboardModule::NODE_TYPE);
    let ScriptSource::Inline(source) = config.source else {
        panic!("keyboard module script template should resolve to inline source");
    };

    assert!(source.contains("local.tapKey(key = \"space\")"));
    assert!(source.contains("function keyboardKeyPressed"));
    assert!(source.contains("function keyboardKeyReleased"));
}

#[test]
fn keyboard_device_selection_prefers_exact_device_and_auto_picks_first() {
    let devices = vec![
        super::keyboard_runtime::DiscoveredKeyboardDevice {
            index: 0,
            variant_id: "raw:a".to_string(),
            label: "Keyboard A".to_string(),
            details: "first".to_string(),
        },
        super::keyboard_runtime::DiscoveredKeyboardDevice {
            index: 1,
            variant_id: "raw:b".to_string(),
            label: "Keyboard B".to_string(),
            details: "second".to_string(),
        },
    ];

    assert_eq!(
        super::selected_keyboard_device(super::AUTO_KEYBOARD_VARIANT, devices.as_slice())
            .map(|device| device.variant_id),
        Some("raw:a".to_string())
    );
    assert_eq!(
        super::selected_keyboard_device("raw:b", devices.as_slice()).map(|device| device.variant_id),
        Some("raw:b".to_string())
    );
    assert!(super::selected_keyboard_device(super::NO_KEYBOARD_VARIANT, devices.as_slice()).is_none());
}

#[test]
fn keyboard_device_selection_accepts_legacy_label_suffixed_values() {
    let devices = vec![super::keyboard_runtime::DiscoveredKeyboardDevice {
        index: 0,
        variant_id: r"\\?\HID#FTCS0038&Col01#5&4ea287c&0&0000#{884b96c3-56ef-11d1-bc8c-00a0c91405dd}".to_string(),
        label: "Keyboard FTCS0038".to_string(),
        details: "first".to_string(),
    }];

    assert_eq!(
        super::selected_keyboard_device(
            r"\\?\HID#FTCS0038&Col01#5&4ea287c&0&0000#{884b96c3-56ef-11d1-bc8c-00a0c91405dd}|Keyboard Device 1",
            devices.as_slice(),
        )
        .map(|device| device.variant_id),
        Some(r"\\?\HID#FTCS0038&Col01#5&4ea287c&0&0000#{884b96c3-56ef-11d1-bc8c-00a0c91405dd}".to_string())
    );
}

fn create_keyboard_module() -> (crate::app::AppEngine, NodeId) {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    let mut module = KeyboardModule::create();
    module.disable_backends_for_test();
    engine.add_node(module.into(), None);
    engine.apply_edits().expect("keyboard module should attach");
    for _ in 0..4 {
        engine
            .apply_edits()
            .expect("keyboard module defaults should materialize");
    }
    engine
        .resolve()
        .expect("keyboard module schedule should resolve");

    let module_id = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("keyboard module should be attached under root");

    (engine, module_id)
}

fn enqueue_event(engine: &mut crate::app::AppEngine, module_id: NodeId, event: KeyboardInputEvent) {
    let crate::app::AppNode::KeyboardModule(module) =
        engine.nodes.get_mut(module_id).expect("keyboard module should exist")
    else {
        panic!("expected KeyboardModule node");
    };
    module.enqueue_input_event_for_test(event);
}

fn run_keyboard_tick(engine: &mut crate::app::AppEngine) {
    engine
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .expect("keyboard inbox should dispatch");
    engine
        .run_tick(Duration::from_millis(20))
        .expect("keyboard tick should run");
    engine
        .apply_edits()
        .expect("keyboard tick edits should apply");
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

fn int_param_value(engine: &crate::app::AppEngine, start: NodeId, path: &str) -> Option<i32> {
    match param_value(engine, start, path)? {
        ParamValue::Int(value) => Some(value),
        _ => None,
    }
}

fn string_param_value(engine: &crate::app::AppEngine, start: NodeId, path: &str) -> Option<String> {
    match param_value(engine, start, path)? {
        ParamValue::Str(value) => Some(value),
        _ => None,
    }
}