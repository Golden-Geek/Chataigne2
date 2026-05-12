use golden_core::{
    node::{Folder, Node, NodeId},
    parameter::ParamValue,
    script::ScriptSource,
};

use super::OsModule;

#[test]
fn os_module_initializes_host_values() {
    let (engine, module_id) = create_os_module();

    assert_eq!(
        string_param_value(&engine, module_id, "values/info/os_type"),
        Some(super::os_runtime::host_os_type().to_string())
    );
    assert!(
        string_param_value(&engine, module_id, "values/info/architecture")
            .is_some_and(|value| !value.trim().is_empty())
    );
    assert!(
        float_param_value(&engine, module_id, "values/uptime/system_seconds")
            .is_some_and(|value| value >= 0.0)
    );
    assert!(
        float_param_value(&engine, module_id, "values/cpu/global_ratio")
            .is_some_and(|value| (0.0..=1.0).contains(&value))
    );
    assert!(
        float_param_value(&engine, module_id, "values/memory/system_total_mb")
            .is_some_and(|value| value >= 0.0)
    );
    assert_eq!(
        param_snapshot(&engine, module_id, "values/uptime/system_seconds")
            .and_then(|snapshot| snapshot.ui_hints.widget),
        Some("time".to_string())
    );
    assert_eq!(
        param_snapshot(&engine, module_id, "values/uptime/app_seconds")
            .and_then(|snapshot| snapshot.ui_hints.widget),
        Some("time".to_string())
    );
}

#[test]
fn os_script_template_scaffolds_functions_and_callbacks() {
    let config = crate::app::module::script_api::module_script_config(OsModule::NODE_TYPE);
    let ScriptSource::Inline(source) = config.source else {
        panic!("OS module script template should resolve to inline source");
    };

    assert!(source.contains("local.shutdown()"));
    assert!(source.contains("local.wakeOnLan(macAddress, broadcastHost = \"255.255.255.255\", port = 9)"));
    assert!(source.contains("function systemStatsUpdated"));
    assert!(source.contains("function systemCommandFailed"));
}

#[test]
fn wake_on_lan_mac_parser_accepts_common_formats() {
    let expected = [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF];

    assert_eq!(
        super::os_runtime::parse_mac_address("AA:BB:CC:DD:EE:FF"),
        Ok(expected)
    );
    assert_eq!(
        super::os_runtime::parse_mac_address("aa-bb-cc-dd-ee-ff"),
        Ok(expected)
    );
    assert_eq!(
        super::os_runtime::parse_mac_address("aabb.ccdd.eeff"),
        Ok(expected)
    );
    assert!(super::os_runtime::parse_mac_address("invalid").is_err());
}

#[test]
fn wake_on_lan_magic_packet_has_expected_shape() {
    let mac = [1, 2, 3, 4, 5, 6];
    let packet = super::os_runtime::build_magic_packet(mac);

    assert_eq!(packet.len(), 102);
    assert!(packet[..6].iter().all(|byte| *byte == 0xFF));
    for chunk in packet[6..].chunks(6) {
        assert_eq!(chunk, mac.as_slice());
    }
}

fn create_os_module() -> (crate::app::AppEngine, NodeId) {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(OsModule::create().into(), None);
    engine.apply_edits().expect("OS module should attach");
    for _ in 0..4 {
        engine
            .apply_edits()
            .expect("OS module defaults should materialize");
    }
    engine.resolve().expect("OS module schedule should resolve");

    let module_id = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("OS module should be attached under root");

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

fn param_value(engine: &crate::app::AppEngine, start: NodeId, path: &str) -> Option<ParamValue> {
    let param_id = find_path(engine, start, path)?;
    engine
        .nodes
        .get(param_id)?
        .engine_param_snapshot()
        .map(|snapshot| snapshot.value)
}

fn float_param_value(engine: &crate::app::AppEngine, start: NodeId, path: &str) -> Option<f64> {
    match param_value(engine, start, path)? {
        ParamValue::Float(value) => Some(value),
        _ => None,
    }
}

fn string_param_value(engine: &crate::app::AppEngine, start: NodeId, path: &str) -> Option<String> {
    match param_value(engine, start, path)? {
        ParamValue::Str(value) => Some(value),
        _ => None,
    }
}

fn param_snapshot(
    engine: &crate::app::AppEngine,
    start: NodeId,
    path: &str,
) -> Option<golden_core::parameter::ParameterSnapshot> {
    let param_id = find_path(engine, start, path)?;
    engine.nodes.get(param_id)?.engine_param_snapshot()
}