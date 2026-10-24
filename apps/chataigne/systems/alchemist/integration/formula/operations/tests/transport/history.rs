use super::*;
use golden_core::parameter::ParameterEventBehaviour;

fn parameter_value(snapshot: &UiSnapshot, uuid: NodeUuid) -> &ParamValue {
    let node = snapshot.nodes.iter().find(|node| node.uuid == uuid).unwrap();
    match &node.data {
        UiNodeDataDto::Parameter { param } => &param.value,
        _ => panic!("expected a position parameter"),
    }
}

#[test]
#[ignore = "requires current cargo build --locked -p Chataigne2; isolated authenticated headless child"]
fn real_selection_drag_delete_undo_redo_preserves_geometry_edges_and_identities() {
    let (engine, formula, source, target) = connected_fixture();
    let tree = engine.process_tree_snapshot();
    let selected = [source, target];
    let root_uuids = selected.map(|id| tree.node(id).unwrap().uuid);
    let positions = selected.map(|id| {
        tree.node(tree.find_child_by_decl_id(id, "position").unwrap())
            .unwrap()
            .uuid
    });
    let edge_uuid = tree
        .child_ids_slice(formula.node)
        .iter()
        .find_map(|id| {
            let child = tree.node(*id).unwrap();
            (child.node_type == CONNECTION_NODE_TYPE).then_some(child.uuid)
        })
        .unwrap();
    let host = Host::start();
    host.upload(engine.to_project_json_with(Node::project_encode_data).unwrap());
    let initial = host.snapshot();
    let initial_values = positions.map(|uuid| parameter_value(&initial, uuid).clone());
    let identities = initial
        .nodes
        .iter()
        .map(|node| (node.node_id, node.uuid))
        .collect::<std::collections::HashMap<_, _>>();
    let nodes = root_uuids.map(|uuid| owner(&initial, uuid).node);
    assert!(
        host.intent(&UiEditIntent::BeginEdit {
            client_edit_id: "p09-selection-drag".into(),
            label: Some("Move selection".into()),
        })
        .success
    );
    let moved_values = [ParamValue::Vec2(20.0, 30.0), ParamValue::Vec2(40.0, 50.0)];
    for (uuid, value) in positions.iter().zip(&moved_values) {
        assert!(
            host.intent(&UiEditIntent::SetParam {
                node: owner(&initial, *uuid).node,
                value: value.clone(),
                behaviour: ParameterEventBehaviour::Append,
            })
            .success
        );
    }
    let moved = host.intent(&UiEditIntent::EndEdit {
        client_edit_id: "p09-selection-drag".into(),
    });
    assert!(moved.success);
    assert_eq!(moved.history.undo_len, initial.history.undo_len + 1);
    for (uuid, value) in positions.iter().zip(&moved_values) {
        assert_eq!(parameter_value(&host.snapshot(), *uuid), value);
    }
    assert!(host.intent(&UiEditIntent::Undo).success);
    for (uuid, value) in positions.iter().zip(&initial_values) {
        assert_eq!(parameter_value(&host.snapshot(), *uuid), value);
    }
    assert!(host.intent(&UiEditIntent::Redo).success);
    let deleted = host.intent(&UiEditIntent::RemoveNodes { nodes: nodes.to_vec() });
    assert!(deleted.success);
    assert_eq!(deleted.history.undo_len, initial.history.undo_len + 2);
    let snapshot = host.snapshot();
    for uuid in root_uuids.into_iter().chain([edge_uuid]) {
        assert!(!snapshot.nodes.iter().any(|node| node.uuid == uuid));
    }
    assert!(host.intent(&UiEditIntent::Undo).success);
    let restored = host.snapshot();
    assert_eq!(
        restored
            .nodes
            .iter()
            .map(|node| (node.node_id, node.uuid))
            .collect::<std::collections::HashMap<_, _>>(),
        identities
    );
    for (uuid, value) in positions.iter().zip(&moved_values) {
        assert_eq!(parameter_value(&restored, *uuid), value);
    }
    assert!(host.intent(&UiEditIntent::Redo).success);
    let deleted_again = host.snapshot();
    for uuid in root_uuids.into_iter().chain([edge_uuid]) {
        assert!(!deleted_again.nodes.iter().any(|node| node.uuid == uuid));
    }
}
