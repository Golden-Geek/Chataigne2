use golden_core::node::{Folder, Node, NodeId};

const COMMON_PROCESSING_PARAM_DECL_IDS: &[&str] = &[
    "auto_add",
    "parse_mode",
    "name_separator",
    "value_separator",
    "hierarchy_separator",
];

const DISABLEABLE_SEPARATOR_PARAM_DECL_IDS: &[&str] = &["name_separator", "value_separator", "hierarchy_separator"];

#[test]
fn concrete_stream_modules_materialize_common_processing_params_once() {
    assert_common_processing_params_once(crate::app::SerialModule::create().into(), "Serial");
    assert_common_processing_params_once(crate::app::TcpServerModule::create().into(), "TCP Server");
    assert_common_processing_params_once(crate::app::TcpClientModule::create().into(), "TCP");
    assert_common_processing_params_once(crate::app::UdpModule::create().into(), "UDP");
    assert_common_processing_params_once(
        crate::app::WebSocketServerModule::create().into(),
        "WebSocket Server",
    );
    assert_common_processing_params_once(
        crate::app::WebSocketClientModule::create().into(),
        "WebSocket Client",
    );
}

fn assert_common_processing_params_once(module: crate::app::AppNode, label: &str) {
    let (engine, module_id) = create_engine_with_module(module);
    let parameters_id = find_child_by_key(&engine, module_id, "parameters")
        .unwrap_or_else(|| panic!("{label} module should have a parameters folder"));
    let processing_id = find_child_by_key(&engine, parameters_id, "processing")
        .unwrap_or_else(|| panic!("{label} module should have a processing folder"));

    for decl_id in COMMON_PROCESSING_PARAM_DECL_IDS {
        let count = count_direct_children_by_key(&engine, processing_id, decl_id);
        assert_eq!(
            count, 1,
            "{label} processing should materialize exactly one '{decl_id}' parameter"
        );
    }

    for decl_id in DISABLEABLE_SEPARATOR_PARAM_DECL_IDS {
        let param_id = find_child_by_key(&engine, processing_id, decl_id)
            .unwrap_or_else(|| panic!("{label} processing should contain '{decl_id}'"));
        let param = engine
            .nodes
            .get(param_id)
            .unwrap_or_else(|| panic!("{label} processing '{decl_id}' parameter should exist"));
        assert!(
            param.node_data().meta.can_be_disabled,
            "{label} processing '{decl_id}' parameter should be disableable"
        );
    }
}

fn create_engine_with_module(module: crate::app::AppNode) -> (crate::app::AppEngine, NodeId) {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(module, None);
    engine.apply_edits().expect("stream module should attach");
    for _ in 0..4 {
        engine.apply_edits().expect("stream module defaults should materialize");
    }

    let module_id = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("stream module should be attached under root");

    (engine, module_id)
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

fn count_direct_children_by_key(engine: &crate::app::AppEngine, parent: NodeId, key: &str) -> usize {
    let mut count = 0;
    let mut current = engine.nodes.get(parent).and_then(|node| node.node_data().first_child);
    while let Some(child_id) = current {
        let child = engine
            .nodes
            .get(child_id)
            .expect("child id from node links should exist");
        if node_key_matches(child.node_data(), key) {
            count += 1;
        }
        current = child.node_data().next_sibling;
    }
    count
}

fn node_key_matches(node: &golden_core::node::NodeData, key: &str) -> bool {
    node.meta.decl_id.0 == key
        || node.meta.decl_id.0.rsplit('/').next() == Some(key)
        || node.meta.short_name == key
        || node.meta.label == key
}
