use std::time::Duration;

use golden_core::{
    edit::Edit,
    node::{Folder, Node, NodeId, NodeMetaPatch},
    parameter::{ParamValue, ParameterEventBehaviour},
    process_ctx::ExecutionPhase,
};

use super::UdpModule;

#[test]
fn udp_module_root_enable_toggle_stops_and_restarts_transport() {
    let (mut engine, module_id) = create_udp_module();

    let module = udp_module(&engine, module_id);
    assert!(
        module.transport.is_some(),
        "UDP module should start a transport while enabled"
    );
    assert!(
        module.last_transport_config.is_some(),
        "UDP module should retain its transport config while enabled"
    );

    set_node_enabled(&mut engine, module_id, false);
    settle_transport_state(&mut engine);

    let module = udp_module(&engine, module_id);
    assert!(
        module.transport.is_none(),
        "UDP module should stop its transport when disabled"
    );
    assert!(
        module.last_transport_config.is_none(),
        "UDP module should clear cached transport config while disabled"
    );

    set_node_enabled(&mut engine, module_id, true);
    settle_transport_state(&mut engine);

    let module = udp_module(&engine, module_id);
    assert!(
        module.transport.is_some(),
        "UDP module should restart its transport when re-enabled"
    );
    assert!(
        module.last_transport_config.is_some(),
        "UDP module should restore transport config after re-enable"
    );
}

fn create_udp_module() -> (crate::app::AppEngine, NodeId) {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    let mut module = UdpModule::create();
    module.node_data_mut().meta.enabled = false;
    engine.add_node(module.into(), None);
    engine.apply_edits().expect("UDP module should attach");
    for _ in 0..4 {
        engine.apply_edits().expect("UDP defaults should materialize");
    }

    let module_id = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("UDP module should be attached under root");

    let connection_id = child_by_decl(&engine, module_id, "connection");
    let input_id = child_by_decl(&engine, connection_id, "input");
    let port_id = child_by_decl(&engine, input_id, "port");
    set_param(&mut engine, port_id, ParamValue::Int(0));
    set_node_enabled(&mut engine, module_id, true);
    engine.resolve().expect("UDP runtime schedule should resolve");
    settle_transport_state(&mut engine);

    (engine, module_id)
}

fn udp_module(engine: &crate::app::AppEngine, module_id: NodeId) -> &UdpModule {
    let crate::app::AppNode::UdpModule(module) = engine.nodes.get(module_id).expect("UDP module should exist") else {
        panic!("expected UdpModule node");
    };

    module
}

fn set_node_enabled(engine: &mut crate::app::AppEngine, node: NodeId, enabled: bool) {
    engine.edits.push(Edit::PatchMeta {
        node,
        patch: NodeMetaPatch {
            enabled: Some(enabled),
            ..Default::default()
        },
    });
}

fn set_param(engine: &mut crate::app::AppEngine, node: NodeId, value: ParamValue) {
    engine.edits.push(Edit::SetParam {
        node,
        value,
        behaviour: ParameterEventBehaviour::Coalesce,
    });
}

fn child_by_decl(engine: &crate::app::AppEngine, parent: NodeId, decl_id: &str) -> NodeId {
    let mut current = engine
        .nodes
        .get(parent)
        .and_then(|node| node.node_data().first_child);
    while let Some(child_id) = current {
        let child = engine.nodes.get(child_id).expect("UDP child should exist");
        let child_decl_id = child.node_data().meta.decl_id.0.as_str();
        if child_decl_id == decl_id || child_decl_id.rsplit('/').next() == Some(decl_id) {
            return child_id;
        }
        current = child.node_data().next_sibling;
    }
    panic!("UDP child '{decl_id}' should exist");
}

fn settle_transport_state(engine: &mut crate::app::AppEngine) {
    engine.apply_edits().expect("pending UDP edits should apply");
    engine
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .expect("pending UDP edits should dispatch");
    engine.apply_edits().expect("UDP event reactions should apply");
    engine
        .run_tick(Duration::from_millis(20))
        .expect("UDP transport tick should succeed");
    engine.apply_edits().expect("UDP transport edits should apply");
}
