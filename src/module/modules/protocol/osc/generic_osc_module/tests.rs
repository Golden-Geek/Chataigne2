use std::time::Duration;

use golden_core::{
    node::{Folder, Node, NodeId},
    parameter::ParamValue,
};

use super::GenericOscModule;
use crate::app::{OscDecodedMessage, OscValuePayload};

#[test]
fn incoming_message_auto_adds_value_nodes_under_values_folder() {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(GenericOscModule::create().into(), None);
    engine.apply_edits().expect("generic osc module should attach");
    engine.resolve().expect("runtime schedule should resolve");

    let module_id = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("module should be attached under root");

    let crate::app::AppNode::GenericOscModule(module) = engine
        .nodes
        .get_mut(module_id)
        .expect("module should exist")
    else {
        panic!("expected GenericOscModule node");
    };
    module.base.stop_transport();
    module.base.set_transport_dirty(false);
    module.base.enqueue_incoming_message(OscDecodedMessage {
        address: "/foo".to_string(),
        payload: OscValuePayload::Single(ParamValue::Int(42)),
    });

    engine
        .run_tick(Duration::from_millis(20))
        .expect("runtime tick should process queued incoming OSC message");

    let crate::app::AppNode::GenericOscModule(module) = engine
        .nodes
        .get(module_id)
        .expect("module should still exist")
    else {
        panic!("expected GenericOscModule node");
    };
    assert!(module.base.auto_add_enabled(), "auto-add should remain enabled by default");
    assert!(
        !module.base.has_pending_incoming_messages(),
        "incoming message queue should be drained after runtime tick"
    );

    let values_folder = find_child_by_key(&engine, module_id, "values").expect("module should contain a Values folder");
    let value_child_labels = child_labels(&engine, values_folder);
    let values_param = find_child_by_key(&engine, values_folder, "foo").unwrap_or_else(|| {
        panic!("incoming OSC address should auto-create a parameter under Values; children were {value_child_labels:?}")
    });
    let values_node = engine.nodes.get(values_param).expect("auto-created value node should exist");

    assert_eq!(values_node.node_data().meta.label, "foo");
    assert_eq!(
        values_node.engine_param_snapshot().map(|snapshot| snapshot.value),
        Some(ParamValue::Int(42))
    );
}

fn find_child_by_key(engine: &crate::app::AppEngine, parent: NodeId, key: &str) -> Option<NodeId> {
    let mut child = engine.nodes.get(parent)?.node_data().first_child;
    while let Some(child_id) = child {
        let node = engine.nodes.get(child_id)?;
        let meta = &node.node_data().meta;
        if meta.decl_id.0 == key || meta.short_name == key || meta.label == key {
            return Some(child_id);
        }
        child = node.node_data().next_sibling;
    }

    None
}

fn child_labels(engine: &crate::app::AppEngine, parent: NodeId) -> Vec<String> {
    let mut labels = Vec::new();
    let mut child = engine.nodes.get(parent).and_then(|node| node.node_data().first_child);
    while let Some(child_id) = child {
        let node = engine.nodes.get(child_id).expect("child node should exist");
        labels.push(node.node_data().meta.label.clone());
        child = node.node_data().next_sibling;
    }
    labels
}
