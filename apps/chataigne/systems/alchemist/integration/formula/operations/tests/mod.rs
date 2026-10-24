use super::*;
use golden_core::{
    application::apply_project_intent,
    ui_sync::{UiAck, UiCreateUserItemInitialParam, UiEditIntent},
};
use serde::{Serialize, de::DeserializeOwned};

mod callers;
mod connection;
mod contexts;
mod script;
mod transport;

fn invoke<R: DeserializeOwned>(
    engine: &mut crate::app::AppEngine,
    operation: &str,
    payload: impl Serialize,
) -> Result<R, Box<UiAck>> {
    let ack = apply_project_intent(
        engine,
        UiEditIntent::InvokeAppOperation {
            namespace: FORMULA_OPERATION_NAMESPACE.into(),
            operation: operation.into(),
            payload: serde_json::to_value(payload).unwrap(),
        },
    );
    if !ack.success {
        return Err(Box::new(ack));
    }
    Ok(serde_json::from_value(ack.result.unwrap().value).unwrap())
}

fn fixture() -> (crate::app::AppEngine, FormulaOwnerDto) {
    let mut engine = crate::app::AppEngine::new(Folder::new("root").into());
    let formula = AlchemistFormulaDefinition::new();
    let uuid = formula.node_data().meta.uuid;
    engine.add_node(formula.into(), None);
    engine.apply_edits().unwrap();
    let node = engine.process_tree_snapshot().node_id_by_uuid(uuid).unwrap();
    (engine, FormulaOwnerDto { node, uuid })
}

fn anode(engine: &mut crate::app::AppEngine, formula: &FormulaOwnerDto, kind: &str) -> NodeId {
    let tree = create_anode_user_item_tree(&format!("{ANODE_CREATE_PREFIX}{kind}")).unwrap();
    let uuid = tree.node.node_data().meta.uuid;
    engine.add_user_item_tree(tree, Some(formula.node));
    engine.apply_edits().unwrap();
    engine.process_tree_snapshot().node_id_by_uuid(uuid).unwrap()
}

fn copy(engine: &mut crate::app::AppEngine, formula: &FormulaOwnerDto, nodes: Vec<NodeId>) -> FormulaFragment {
    invoke(
        engine,
        "copyFragment",
        FormulaCopyRequest {
            formula: formula.clone(),
            nodes,
        },
    )
    .unwrap()
}

fn connected_fixture() -> (crate::app::AppEngine, FormulaOwnerDto, NodeId, NodeId) {
    let (mut engine, formula) = fixture();
    let source = anode(&mut engine, &formula, "constant");
    let target = anode(&mut engine, &formula, "math");
    let snapshot = engine.process_tree_snapshot();
    let source_uuid = snapshot.node(source).unwrap().uuid;
    let target_uuid = snapshot.node(target).unwrap().uuid;
    let params = [
        ("source_node", ParamValue::Reference(NodeReference::new(source_uuid))),
        ("source_socket", ParamValue::Str("value".into())),
        ("target_node", ParamValue::Reference(NodeReference::new(target_uuid))),
        ("target_socket", ParamValue::Str("value1".into())),
    ]
    .into_iter()
    .map(|(decl_id, value)| UiCreateUserItemInitialParam {
        decl_id: DeclId(decl_id.into()),
        value,
    })
    .collect();
    assert!(
        engine
            .apply_ui_intent(UiEditIntent::CreateUserItem {
                parent: formula.node,
                node_type: CONNECTION_NODE_TYPE.into(),
                label: Some("source edge".into()),
                initial_params: params
            })
            .success
    );
    (engine, formula, source, target)
}

#[test]
fn complete_fragment_paste_returns_exact_identities_cross_root_references_and_placement() {
    let (mut engine, formula, source, target) = connected_fixture();
    let snapshot = engine.process_tree_snapshot();
    let source_uuid = snapshot.node(source).unwrap().uuid;
    let target_uuid = snapshot.node(target).unwrap().uuid;
    let fragment = copy(&mut engine, &formula, vec![source, target]);
    assert_eq!(fragment.items.len(), 3);
    let source_ids = fragment
        .items
        .iter()
        .flat_map(|item| {
            let mut ids = Vec::new();
            let mut nodes = vec![&item.document.root];
            while let Some(node) = nodes.pop() {
                ids.push(node.uuid);
                nodes.extend(&node.children);
            }
            ids
        })
        .collect::<HashSet<_>>();
    let result: FormulaPasteResult = invoke(
        &mut engine,
        "pasteFragment",
        FormulaPasteRequest {
            formula: formula.clone(),
            fragment,
            offset: Some([5.0, 7.0]),
        },
    )
    .unwrap();
    assert_eq!(result.selected_nodes.len(), 2);
    assert_eq!(result.identities.len(), source_ids.len());
    let snapshot = engine.process_tree_snapshot();
    let remap = result
        .identities
        .iter()
        .map(|identity| (identity.source_uuid, identity))
        .collect::<HashMap<_, _>>();
    for identity in &result.identities {
        assert!(source_ids.contains(&identity.source_uuid));
        assert_ne!(identity.source_uuid, identity.uuid);
        assert_eq!(snapshot.node(identity.node).unwrap().uuid, identity.uuid);
    }
    let connection = result
        .identities
        .iter()
        .find(|identity| snapshot.node(identity.node).unwrap().node_type == CONNECTION_NODE_TYPE)
        .unwrap()
        .node;
    assert_eq!(
        child_reference_uuid(&snapshot, connection, "source_node"),
        Some(remap[&source_uuid].uuid)
    );
    assert_eq!(
        child_reference_uuid(&snapshot, connection, "target_node"),
        Some(remap[&target_uuid].uuid)
    );
    for id in &result.selected_nodes {
        assert_eq!(child_vec2(&snapshot, *id, "position"), Some([5.0, 7.0]));
    }
    assert_ne!(
        snapshot.node(source).unwrap().label,
        snapshot.node(remap[&source_uuid].node).unwrap().label
    );
    drop(snapshot);
    assert!(apply_project_intent(&mut engine, UiEditIntent::Undo).success);
    for identity in &result.identities {
        assert!(engine.nodes.get(identity.node).is_none());
    }
    assert!(apply_project_intent(&mut engine, UiEditIntent::Redo).success);
    for identity in result.identities {
        assert_eq!(
            engine.nodes.get(identity.node).unwrap().node_data().meta.uuid,
            identity.uuid
        );
    }
}

#[test]
fn duplicate_identities_and_invalid_nested_app_nodes_reject_without_partial_history_or_publication() {
    let (mut engine, formula) = fixture();
    let node = anode(&mut engine, &formula, "constant");
    let fragment = copy(&mut engine, &formula, vec![node]);
    let count = engine.nodes.len();
    let history = engine.undo_len();
    let mut duplicate = fragment.clone();
    duplicate.items.push(duplicate.items[0].clone());
    let ack = invoke::<FormulaPasteResult>(
        &mut engine,
        "pasteFragment",
        FormulaPasteRequest {
            formula: formula.clone(),
            fragment: duplicate,
            offset: None,
        },
    )
    .unwrap_err();
    assert!(ack.result.is_none());
    assert_eq!(engine.nodes.len(), count);
    assert_eq!(engine.undo_len(), history);
    let mut bad = fragment;
    bad.items[0].document.root.children[0].node_type = "artnet_module".into();
    let ack = invoke::<FormulaPasteResult>(
        &mut engine,
        "pasteFragment",
        FormulaPasteRequest {
            formula,
            fragment: bad,
            offset: None,
        },
    )
    .unwrap_err();
    assert!(ack.result.is_none());
    assert_eq!(engine.nodes.len(), count);
    assert_eq!(engine.undo_len(), history);
}

#[test]
fn saved_fragment_pastes_after_source_removal_and_does_not_depend_on_visible_projection() {
    let (mut engine, formula) = fixture();
    let node = anode(&mut engine, &formula, "constant");
    let fragment = copy(&mut engine, &formula, vec![node]);
    let uuid = fragment.items[0].document.root.uuid;
    assert!(engine.apply_ui_intent(UiEditIntent::RemoveNode { node }).success);
    let result: FormulaPasteResult = invoke(
        &mut engine,
        "pasteFragment",
        FormulaPasteRequest {
            formula,
            fragment,
            offset: None,
        },
    )
    .unwrap();
    let identity = result
        .identities
        .iter()
        .find(|identity| identity.source_uuid == uuid)
        .unwrap();
    assert!(result.selected_nodes.contains(&identity.node));
    assert_ne!(identity.uuid, uuid);
    assert_eq!(
        engine.nodes.get(identity.node).unwrap().node_data().meta.uuid,
        identity.uuid
    );
}

#[test]
fn external_property_reference_survives_fragment_import_without_remapping_the_target() {
    let (mut engine, formula) = fixture();
    let properties = engine
        .process_tree_snapshot()
        .find_child_by_decl_id(formula.node, PROPERTIES_DECL_ID)
        .unwrap();
    let tree = engine
        .nodes
        .get(properties)
        .unwrap()
        .create_user_item_tree(&format!("{PROPERTY_CREATE_PREFIX}float"))
        .unwrap();
    let property_uuid = tree.node.node_data().meta.uuid;
    engine.add_user_item_tree(tree, Some(properties));
    engine.apply_edits().unwrap();
    let getter = anode(&mut engine, &formula, "property");
    let snapshot = engine.process_tree_snapshot();
    let config = snapshot.find_child_by_decl_id(getter, "config").unwrap();
    let target = snapshot.find_child_by_decl_id(config, "config/property_id").unwrap();
    assert!(
        engine
            .apply_ui_intent(UiEditIntent::SetParam {
                node: target,
                value: ParamValue::Reference(NodeReference::new(property_uuid)),
                behaviour: ParameterEventBehaviour::Append
            })
            .success
    );
    let fragment = copy(&mut engine, &formula, vec![getter]);
    let result: FormulaPasteResult = invoke(
        &mut engine,
        "pasteFragment",
        FormulaPasteRequest {
            formula,
            fragment,
            offset: None,
        },
    )
    .unwrap();
    assert!(
        !result
            .identities
            .iter()
            .any(|identity| identity.source_uuid == property_uuid)
    );
    let snapshot = engine.process_tree_snapshot();
    let config = snapshot
        .find_child_by_decl_id(result.selected_nodes[0], "config")
        .unwrap();
    assert_eq!(
        child_reference_uuid(&snapshot, config, "config/property_id"),
        Some(property_uuid)
    );
}

#[test]
fn stale_owner_and_read_only_formula_reject_before_import() {
    let (mut engine, formula) = fixture();
    let node = anode(&mut engine, &formula, "constant");
    let fragment = copy(&mut engine, &formula, vec![node]);
    let before = engine.nodes.len();
    let mut stale = formula.clone();
    stale.uuid = NodeUuid(uuid::Uuid::new_v4());
    assert!(
        invoke::<FormulaPasteResult>(
            &mut engine,
            "pasteFragment",
            FormulaPasteRequest {
                formula: stale,
                fragment: fragment.clone(),
                offset: None
            }
        )
        .is_err()
    );
    engine
        .nodes
        .get_mut(formula.node)
        .unwrap()
        .node_data_mut()
        .meta
        .tags
        .push(FORMULA_EXTERNAL_READ_ONLY_TAG.into());
    assert!(
        invoke::<FormulaPasteResult>(
            &mut engine,
            "pasteFragment",
            FormulaPasteRequest {
                formula,
                fragment,
                offset: None
            }
        )
        .is_err()
    );
    assert_eq!(engine.nodes.len(), before);
}

#[test]
fn missing_declared_children_and_invalid_connections_reject_the_complete_action() {
    let (mut engine, formula, source, target) = connected_fixture();
    let fragment = copy(&mut engine, &formula, vec![source, target]);
    let before = engine.nodes.len();
    let history = engine.undo_len();
    let events = engine.ui_event_log().len();
    for mode in ["missing_declared_child", "missing_socket", "external_endpoint"] {
        let mut fragment = fragment.clone();
        if mode == "missing_declared_child" {
            fragment.items[0].document.root.children.retain(|child| {
                child
                    .meta
                    .decl_id
                    .as_ref()
                    .is_none_or(|id| id.0 != ANODE_POSITION_DECL_ID)
            });
        } else {
            let edge = fragment
                .items
                .iter_mut()
                .find(|item| item.document.root.node_type == CONNECTION_NODE_TYPE)
                .unwrap();
            let decl = if mode == "missing_socket" {
                "source_socket"
            } else {
                "source_node"
            };
            let parameter = edge
                .document
                .root
                .children
                .iter_mut()
                .find(|child| child.meta.decl_id.as_ref().is_some_and(|id| id.0 == decl))
                .unwrap();
            let value = if mode == "missing_socket" {
                ParamValue::Str("socket-does-not-exist".into())
            } else {
                ParamValue::Reference(NodeReference::new(formula.uuid))
            };
            parameter.data.as_mut().unwrap()["value"] = serde_json::to_value(value).unwrap();
        }
        let ack = invoke::<FormulaPasteResult>(
            &mut engine,
            "pasteFragment",
            FormulaPasteRequest {
                formula: formula.clone(),
                fragment,
                offset: None,
            },
        )
        .unwrap_err();
        assert!(ack.result.is_none(), "{mode}");
        assert_eq!(engine.nodes.len(), before, "{mode}");
        assert_eq!(engine.undo_len(), history, "{mode}");
        assert_eq!(engine.ui_event_log().len(), events, "{mode}");
        assert!(engine.edits.pending.is_empty(), "{mode}");
    }
}

#[test]
fn hosted_curve_and_gradient_capsules_preserve_authored_resources_and_record_identities() {
    let (mut engine, formula) = fixture();
    for kind in ["curve_remap", "gradient_sampler"] {
        let source = anode(&mut engine, &formula, kind);
        let before = anode_from_snapshot(&engine.process_tree_snapshot(), source).unwrap();
        let fragment = copy(&mut engine, &formula, vec![source]);
        let result: FormulaPasteResult = invoke(
            &mut engine,
            "pasteFragment",
            FormulaPasteRequest {
                formula: formula.clone(),
                fragment,
                offset: Some([8.0, 11.0]),
            },
        )
        .unwrap();
        let snapshot = engine.process_tree_snapshot();
        let copied = result.selected_nodes[0];
        let after = anode_from_snapshot(&snapshot, copied).unwrap();
        assert_eq!(before.config, after.config, "{kind}");
        assert_eq!(child_vec2(&snapshot, copied, "position"), Some([8.0, 11.0]));
        assert!(
            result.identities.iter().all(|identity| snapshot
                .node(identity.node)
                .is_some_and(|node| node.uuid == identity.uuid)),
            "{kind}"
        );
    }
}
