use std::time::Duration;

use golden_core::{
    edit::Edit,
    node::{Folder, Node, NodeId, NodeMetaPatch},
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
    engine.add_node(UdpModule::create().into(), None);
    engine.apply_edits().expect("UDP module should attach");
    for _ in 0..4 {
        engine.apply_edits().expect("UDP defaults should materialize");
    }
    engine.resolve().expect("UDP runtime schedule should resolve");

    let module_id = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("UDP module should be attached under root");

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
