use super::{DeclId, NodeId, NodeUuid};

#[test]
fn identities_keep_their_wire_shapes() {
    assert_eq!(serde_json::to_value(NodeId(42)).unwrap(), serde_json::json!(42));
    assert_eq!(
        serde_json::to_value(DeclId("gain".into())).unwrap(),
        serde_json::json!("gain")
    );

    let uuid = NodeUuid::nil();
    assert!(uuid.is_nil());
    assert_eq!(
        serde_json::from_value::<NodeUuid>(serde_json::to_value(uuid).unwrap()).unwrap(),
        uuid
    );
}
