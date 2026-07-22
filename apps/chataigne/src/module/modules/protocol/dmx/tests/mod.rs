use golden_core::{
    node::{DeclaredUserItemNode, Folder, Node},
    parameter::ParamValue,
};

use crate::app::{
    module_modules_protocol_dmx::{
        parse_destination, script_blackout, script_send_frame, script_set_channel,
        DmxCommandRequest, DmxProtocol,
    },
    AppNode, ArtNetModule, ModuleManager, NodeModule, SacnModule,
};

#[test]
fn lighting_modules_are_distinct_catalog_items_with_distinct_kernels() {
    assert_eq!(
        <ArtNetModule as DeclaredUserItemNode>::ITEM_NODE_TYPE,
        "artnet_module"
    );
    assert_eq!(
        <SacnModule as DeclaredUserItemNode>::ITEM_NODE_TYPE,
        "sacn_module"
    );
    assert_ne!(
        ArtNetModule::create().execution_rule(),
        SacnModule::create().execution_rule()
    );
}

#[test]
fn script_requests_cover_channel_frame_and_blackout_semantics() {
    assert_eq!(
        script_set_channel(&[ParamValue::Int(512), ParamValue::Int(255)]).unwrap(),
        DmxCommandRequest::SetChannel {
            channel: 512,
            value: 255
        }
    );
    assert_eq!(
        script_send_frame(&[
            ParamValue::from_script_json(&serde_json::json!("[1,2,3]")).unwrap(),
        ])
        .unwrap(),
        DmxCommandRequest::SendFrame {
            slots: vec![1, 2, 3]
        }
    );
    assert_eq!(
        script_blackout(&[]).unwrap(),
        DmxCommandRequest::Blackout
    );
}

#[test]
fn protocol_destinations_preserve_multicast_and_broadcast_policy() {
    assert_eq!(
        parse_destination("", 5568, DmxProtocol::Sacn).unwrap(),
        None
    );
    assert_eq!(
        parse_destination("127.0.0.1", 6454, DmxProtocol::ArtNet).unwrap(),
        Some("127.0.0.1:6454".parse().unwrap())
    );
    assert!(parse_destination("", 6454, DmxProtocol::ArtNet).is_err());
}

#[test]
fn dmx_modules_round_trip_through_sparse_project_persistence() {
    let root: AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(ModuleManager::new().into(), None);
    engine
        .apply_edits()
        .expect("module manager should attach");
    let manager_id = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("module manager should be attached under root");

    engine.add_user_item(ArtNetModule::create().into(), Some(manager_id));
    engine.add_user_item(SacnModule::create().into(), Some(manager_id));
    engine.add_user_item(NodeModule::create().into(), Some(manager_id));
    engine
        .apply_edits()
        .expect("DMX modules should attach");
    for _ in 0..4 {
        engine
            .apply_edits()
            .expect("DMX defaults should materialize");
    }

    let json = golden_core::app::to_sparse_project_json_pretty(&engine)
        .expect("DMX project should encode");
    let reopened = golden_core::app::from_sparse_project_json::<AppNode>(&json)
        .expect("DMX project should decode");
    for node_type in ["artnet_module", "sacn_module", "node_module"] {
        assert_eq!(
            reopened
                .nodes
                .iter()
                .filter(|(_, node)| node.get_type() == node_type)
                .count(),
            1,
            "{node_type} should survive the sparse-project round trip"
        );
    }
    let round_tripped = golden_core::app::to_sparse_project_json_pretty(&reopened)
        .expect("reopened DMX project should encode");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&round_tripped).unwrap(),
        serde_json::from_str::<serde_json::Value>(&json).unwrap(),
        "saving, reopening, and saving DMX modules should be stable"
    );
}
