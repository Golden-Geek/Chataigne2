use std::time::Duration;

use golden_core::{
    node::{Folder, Node, NodeId},
    parameter::ParamValue,
    process_ctx::ExecutionPhase,
    script::ScriptSource,
};

use super::MouseModule;
mod service;

#[test]
fn mouse_events_update_values() {
    super::input::tests::assert_events_update_values();
}

#[test]
fn mouse_script_template_scaffolds_mouse_functions_and_callbacks() {
    let config = crate::app::module::script_api::module_script_config(MouseModule::NODE_TYPE);
    let ScriptSource::Inline { text: source } = config.source else {
        panic!("mouse module script template should resolve to inline source");
    };

    assert!(source.contains("local.moveMouse(x, y, coordinate = \"absolute\", units = \"pixels\")"));
    assert!(source.contains("function mouseMoved"));
    assert!(source.contains("function mouseButtonPressed"));
    assert!(source.contains("function mouseButtonReleased"));
}

#[test]
fn idle_mouse_has_no_periodic_engine_callbacks_or_snapshots() {
    let (mut engine, _) = create_mouse_module();
    for _ in 0..5 {
        run_mouse_tick(&mut engine);
    }
    for _ in 0..100 {
        run_mouse_tick(&mut engine);
        let stats = engine.tick_stats();
        assert_eq!(stats.nodes_due, 0);
        assert_eq!(stats.callbacks_fired, 0);
        assert_eq!(stats.wake_callbacks_fired, 0);
        assert_eq!(stats.snapshot_builds, 0);
    }
}

#[test]
fn mouse_device_selection_prefers_exact_device_and_auto_picks_first() {
    let devices = vec![
        super::mouse_runtime::DiscoveredMouseDevice {
            index: 0,
            variant_id: "raw:a".to_string(),
            label: "Mouse A".to_string(),
            details: "first".to_string(),
        },
        super::mouse_runtime::DiscoveredMouseDevice {
            index: 1,
            variant_id: "raw:b".to_string(),
            label: "Mouse B".to_string(),
            details: "second".to_string(),
        },
    ];

    assert_eq!(
        super::selected_mouse_device(super::AUTO_MOUSE_VARIANT, devices.as_slice()).map(|device| device.variant_id),
        Some("raw:a".to_string())
    );
    assert_eq!(
        super::selected_mouse_device("raw:b", devices.as_slice()).map(|device| device.variant_id),
        Some("raw:b".to_string())
    );
    assert!(super::selected_mouse_device(super::NO_MOUSE_VARIANT, devices.as_slice()).is_none());
}

#[test]
fn mouse_device_selection_accepts_legacy_label_suffixed_values() {
    let devices = vec![super::mouse_runtime::DiscoveredMouseDevice {
        index: 0,
        variant_id: r"\\?\HID#FTCS0038&Col01#5&4ea287c&0&0000#{378de44c-56ef-11d1-bc8c-00a0c91405dd}".to_string(),
        label: "Mouse FTCS0038".to_string(),
        details: "first".to_string(),
    }];

    assert_eq!(
        super::selected_mouse_device(
            r"\\?\HID#FTCS0038&Col01#5&4ea287c&0&0000#{378de44c-56ef-11d1-bc8c-00a0c91405dd}|Mouse Device 3",
            devices.as_slice(),
        )
        .map(|device| device.variant_id),
        Some(r"\\?\HID#FTCS0038&Col01#5&4ea287c&0&0000#{378de44c-56ef-11d1-bc8c-00a0c91405dd}".to_string())
    );
}

#[cfg(windows)]
#[test]
fn mouse_capture_only_swallows_non_injected_events_while_active() {
    assert!(super::mouse_runtime::should_swallow_captured_mouse_input(0, true));
    assert!(!super::mouse_runtime::should_swallow_captured_mouse_input(
        0x00000001, true
    ));
    assert!(!super::mouse_runtime::should_swallow_captured_mouse_input(0, false));
}

#[cfg(windows)]
#[test]
fn mouse_capture_uses_raw_relative_motion() {
    assert_eq!(
        super::mouse_runtime::next_mouse_position_from_raw_input(0, 7, -3, (10, 20)),
        Some((17, 17, 7, -3))
    );
    assert_eq!(
        super::mouse_runtime::next_mouse_position_from_raw_input(0, 0, 0, (10, 20)),
        None
    );
}

#[cfg(windows)]
#[test]
fn mouse_capture_targets_selected_device() {
    let devices = vec![
        super::mouse_runtime::DiscoveredMouseDevice {
            index: 0,
            variant_id: "raw:a".to_string(),
            label: "Mouse A".to_string(),
            details: "first".to_string(),
        },
        super::mouse_runtime::DiscoveredMouseDevice {
            index: 1,
            variant_id: "raw:b".to_string(),
            label: "Mouse B".to_string(),
            details: "second".to_string(),
        },
    ];

    assert!(super::mouse_runtime::capture_target_matches_selection(
        "auto",
        devices.as_slice(),
        "raw:a",
    ));
    assert!(!super::mouse_runtime::capture_target_matches_selection(
        "auto",
        devices.as_slice(),
        "raw:b",
    ));
    assert!(super::mouse_runtime::capture_target_matches_selection(
        "raw:b|Mouse B",
        devices.as_slice(),
        "raw:b",
    ));
}

fn create_mouse_module() -> (crate::app::AppEngine, NodeId) {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    let mut module = MouseModule::create();
    module.disable_backends_for_test();
    engine.add_node(module.into(), None);
    engine.apply_edits().expect("mouse module should attach");
    for _ in 0..4 {
        engine.apply_edits().expect("mouse module defaults should materialize");
    }
    engine.resolve().expect("mouse module schedule should resolve");

    let module_id = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("mouse module should be attached under root");

    (engine, module_id)
}

fn run_mouse_tick(engine: &mut crate::app::AppEngine) {
    engine
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .expect("mouse inbox should dispatch");
    engine
        .run_tick(Duration::from_millis(20))
        .expect("mouse tick should run");
    engine.apply_edits().expect("mouse tick edits should apply");
}

pub(super) fn find_path(engine: &crate::app::AppEngine, start: NodeId, path: &str) -> Option<NodeId> {
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
