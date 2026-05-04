use std::{net::TcpListener, time::Duration};

use golden_core::{
    edit::Edit,
    node::{Folder, Node, NodeId, NodeMetaPatch},
    parameter::{ParamValue, ParameterEventBehaviour},
    process_ctx::ExecutionPhase,
};

use super::TcpClientModule;

#[test]
fn tcp_module_root_enable_toggle_stops_and_restarts_transport() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("TCP test listener should bind");
    let port = listener
        .local_addr()
        .expect("TCP test listener should expose a port")
        .port();
    let (mut engine, module_id) = create_tcp_module();
    let remote_port_id = tcp_module(&engine, module_id).remote_port.id();

    set_param(&mut engine, remote_port_id, ParamValue::Int(i32::from(port)));
    settle_transport_state(&mut engine);

    let module = tcp_module(&engine, module_id);
    assert!(
        module.transport.is_some(),
        "TCP module should start a transport while enabled"
    );
    assert!(
        module.last_transport_config.is_some(),
        "TCP module should retain its transport config while enabled"
    );

    set_node_enabled(&mut engine, module_id, false);
    settle_transport_state(&mut engine);

    let module = tcp_module(&engine, module_id);
    assert!(
        module.transport.is_none(),
        "TCP module should stop its transport when disabled"
    );
    assert!(
        module.last_transport_config.is_none(),
        "TCP module should clear cached transport config while disabled"
    );

    set_node_enabled(&mut engine, module_id, true);
    settle_transport_state(&mut engine);

    let module = tcp_module(&engine, module_id);
    assert!(
        module.transport.is_some(),
        "TCP module should restart its transport when re-enabled"
    );
    assert!(
        module.last_transport_config.is_some(),
        "TCP module should restore transport config after re-enable"
    );

    drop(listener);
}

fn create_tcp_module() -> (crate::app::AppEngine, NodeId) {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(TcpClientModule::create().into(), None);
    engine.apply_edits().expect("TCP module should attach");
    for _ in 0..4 {
        engine.apply_edits().expect("TCP defaults should materialize");
    }
    engine.resolve().expect("TCP runtime schedule should resolve");

    let module_id = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("TCP module should be attached under root");

    (engine, module_id)
}

fn tcp_module(engine: &crate::app::AppEngine, module_id: NodeId) -> &TcpClientModule {
    let crate::app::AppNode::TcpClientModule(module) = engine.nodes.get(module_id).expect("TCP module should exist")
    else {
        panic!("expected TcpClientModule node");
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

fn settle_transport_state(engine: &mut crate::app::AppEngine) {
    engine.apply_edits().expect("pending TCP edits should apply");
    engine
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .expect("pending TCP edits should dispatch");
    engine.apply_edits().expect("TCP event reactions should apply");
    engine
        .run_tick(Duration::from_millis(20))
        .expect("TCP transport tick should succeed");
    engine.apply_edits().expect("TCP transport edits should apply");
}
