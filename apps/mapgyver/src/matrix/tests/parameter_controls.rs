use super::*;
use golden_core::{
    node::{Node, NodeId},
    parameter::{
        ParamValue, ParameterControlMode, ParameterControlSpec, ParameterControlState, ParameterEventBehaviour,
    },
};

fn child(engine: &AppEngine, parent: NodeId, decl: &str) -> NodeId {
    engine
        .nodes
        .iter()
        .find(|(_, node)| node.node_data().parent == Some(parent) && node.node_data().meta.decl_id.0 == decl)
        .unwrap()
        .0
}

fn apply_ui(engine: &mut AppEngine, intent: UiEditIntent) -> golden_core::ui_sync::UiAck {
    let ack = apply_project_intent(engine, intent);
    assert!(ack.success, "{ack:?}");
    ack
}

fn curve(engine: &mut AppEngine) -> (ChannelReference, NodeId, NodeId, NodeId) {
    let matrix = create(engine, MediaKind::Matrix);
    let item = layers(engine, matrix.media, None, 1)[0];
    let owner = engine.node_id_by_uuid(layer(item).0).unwrap();
    let folder = child(engine, owner, "parameters");
    let parameter = child(engine, folder, "parameters/opacity");
    apply_ui(
        engine,
        UiEditIntent::SetParamControlState {
            node: parameter,
            state: ParameterControlState::new(ParameterControlMode::Animation, ParameterControlSpec::Animation).into(),
        },
    );
    let animation = engine.nodes.get(parameter).unwrap().node_data().first_child.unwrap();
    let waveform = child(engine, animation, "waveform");
    apply_ui(
        engine,
        UiEditIntent::SetParam {
            node: waveform,
            value: ParamValue::Enum("curve".into()),
            behaviour: ParameterEventBehaviour::Append,
        },
    );
    engine.run_tick(std::time::Duration::from_millis(1)).unwrap();
    (matrix, owner, parameter, child(engine, animation, "curve"))
}

fn create_key(engine: &mut AppEngine, curve: NodeId) -> golden_core::ui_sync::UiCreatedNode {
    apply_ui(
        engine,
        UiEditIntent::CreateUserItem {
            parent: curve,
            node_type: "animation_curve_key".into(),
            label: None,
            initial_params: vec![],
        },
    )
    .created_node
    .unwrap()
}

#[test]
fn matrix_parameter_curve_creation_is_one_history_step_and_roundtrips_exactly() {
    let mut engine = fixture();
    let (_, _, _, curve) = curve(&mut engine);
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    apply_ui(
        &mut engine,
        UiEditIntent::BeginEdit {
            client_edit_id: "curve-key".into(),
            label: Some("Add Curve Key".into()),
        },
    );
    let created = create_key(&mut engine, curve);
    assert_eq!(
        engine.nodes.get(created.node_id).unwrap().node_data().meta.uuid,
        created.uuid
    );
    for (decl, value) in [("position", 0.4), ("value", 0.7)] {
        let node = child(&engine, created.node_id, decl);
        apply_ui(
            &mut engine,
            UiEditIntent::SetParam {
                node,
                value: ParamValue::Float(value),
                behaviour: ParameterEventBehaviour::Append,
            },
        );
    }
    assert_eq!(engine.undo_len(), history);
    apply_ui(
        &mut engine,
        UiEditIntent::EndEdit {
            client_edit_id: "curve-key".into(),
        },
    );
    assert_eq!(engine.undo_len(), history + 1);
    let after = to_sparse_project_json_pretty(&engine).unwrap();
    assert_ne!(after, before);
    assert!(engine.undo().unwrap());
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    assert!(engine.redo().unwrap());
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), after);
    let curve_uuid = engine.nodes.get(curve).unwrap().node_data().meta.uuid;
    let expected = golden_core::node::curve_from_snapshot(&engine.process_tree_snapshot(), curve).unwrap();
    let restored = from_sparse_project_json::<AppNode>(&after).unwrap();
    assert!(restored.node_id_by_uuid(created.uuid).is_some());
    let restored_curve = restored.node_id_by_uuid(curve_uuid).unwrap();
    let actual = golden_core::node::curve_from_snapshot(&restored.process_tree_snapshot(), restored_curve).unwrap();
    // Sparse reload rematerializes declared children; compare the complete typed curve model.
    assert_eq!(
        serde_json::to_value(actual).unwrap(),
        serde_json::to_value(expected).unwrap()
    );
    crate::media::validate_project(&restored).unwrap();
}

#[test]
fn generic_curve_crud_does_not_bypass_matrix_or_declared_parameter_ownership() {
    let mut engine = fixture();
    let (_, layer, parameter, curve) = curve(&mut engine);
    let key = create_key(&mut engine, curve);
    let original_ids = engine
        .nodes
        .iter()
        .map(|(id, _)| id)
        .collect::<std::collections::HashSet<_>>();
    apply_ui(
        &mut engine,
        UiEditIntent::DuplicateNode {
            source: key.node_id,
            new_parent: curve,
            new_prev_sibling: None,
            initial_params: vec![],
        },
    );
    let duplicates = engine
        .nodes
        .iter()
        .filter(|(id, node)| !original_ids.contains(id) && node.node_data().parent == Some(curve))
        .map(|(id, node)| (id, node.node_data().meta.uuid))
        .collect::<Vec<_>>();
    assert_eq!(duplicates.len(), 1);
    let (duplicate_id, duplicate_uuid) = duplicates[0];
    apply_ui(
        &mut engine,
        UiEditIntent::MoveNode {
            node: duplicate_id,
            new_parent: curve,
            new_prev_sibling: Some(key.node_id),
        },
    );
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    for intent in [
        UiEditIntent::RemoveNodes {
            nodes: vec![key.node_id, layer],
        },
        UiEditIntent::MoveNode {
            node: layer,
            new_parent: curve,
            new_prev_sibling: None,
        },
        UiEditIntent::RemoveNode { node: parameter },
        UiEditIntent::CreateUserItem {
            parent: parameter,
            node_type: "animation_curve_key".into(),
            label: None,
            initial_params: vec![],
        },
    ] {
        let ack = apply_project_intent(&mut engine, intent);
        assert!(!ack.success, "{ack:?}");
        assert_eq!(ack.error_code.as_deref(), Some("app_intent_rejected"));
        assert_eq!(engine.undo_len(), history);
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    }
    apply_ui(&mut engine, UiEditIntent::RemoveNode { node: key.node_id });
    apply_ui(
        &mut engine,
        UiEditIntent::RemoveNodes {
            nodes: vec![duplicate_id],
        },
    );
    assert!(engine.node_id_by_uuid(key.uuid).is_none());
    assert!(engine.node_id_by_uuid(duplicate_uuid).is_none());
    assert!(engine.nodes.get(layer).is_some());
    crate::media::validate_project(&engine).unwrap();
}
