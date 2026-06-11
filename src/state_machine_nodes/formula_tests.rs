use golden_core::{
    node::{DeclId, Folder, Node, NodeId, NodeReference},
    parameter::{ParamValue, ParameterEventBehaviour},
    process_ctx::ExecutionPhase,
    ui_sync::{UiCreateUserItemInitialParam, UiEditIntent},
};

use super::{
    ANODE_CREATE_PREFIX, ANODE_ITEM_KIND, AlchemistANode,
    AlchemistConnection, AlchemistFormulaDefinition, FORMULA_ITEM_KIND,
    FormulaLibrary, formula_from_snapshot,
};
use crate::app::{AppEngine, AppNode};

#[test]
fn formula_library_accepts_real_formula_nodes() {
    let library = FormulaLibrary::new();
    assert!(library.user_container_accepts_item(
        AlchemistFormulaDefinition::NODE_TYPE,
        FORMULA_ITEM_KIND
    ));
}

#[test]
fn formula_exposes_anode_catalog_as_real_user_items() {
    let formula = AlchemistFormulaDefinition::new();
    let items = formula.user_creatable_items();

    assert!(items.iter().any(|item| {
        item.node_type == format!("{ANODE_CREATE_PREFIX}constant")
            && item.item_kind == ANODE_ITEM_KIND
    }));
    assert!(items.iter().any(|item| {
        item.node_type == AlchemistConnection::NODE_TYPE
    }));
}

#[test]
fn library_created_formula_has_no_opaque_document_parameter() {
    let (engine, formula) = engine_with_formula();

    assert!(
        find_descendant_by_decl(&engine, formula, "formula_json").is_none(),
        "Formula source of truth must be its visible node subtree"
    );
    assert!(find_child_by_decl(&engine, formula, "is_valid").is_some());
    assert!(find_child_by_decl(&engine, formula, "diagnostics_json").is_some());
}

#[test]
fn anode_creation_materializes_visible_config_and_socket_nodes() {
    let (mut engine, formula) = engine_with_formula();
    create_anode(&mut engine, formula, "constant", 2.0, 3.0);

    let anode = direct_children(&engine, formula)
        .into_iter()
        .find(|node| {
            engine.nodes.get(*node).is_some_and(|node| {
                node.get_type() == AlchemistANode::NODE_TYPE
            })
        })
        .expect("Constant ANode should be visible under the Formula");

    assert_eq!(
        parameter_value(&engine, anode, "anode_type"),
        ParamValue::Str("constant".into())
    );
    assert_eq!(
        parameter_value(&engine, anode, "position"),
        ParamValue::Vec2(2.0, 3.0)
    );
    let config = find_child_by_decl(&engine, anode, "config")
        .expect("Config folder should be visible");
    let outputs = find_child_by_decl(&engine, anode, "outputs")
        .expect("Outputs folder should be visible");
    assert!(
        find_child_by_decl(&engine, config, "config/value").is_some(),
        "Constant value must be a real parameter"
    );
    assert!(
        direct_children(&engine, outputs).into_iter().any(|node| {
            engine.nodes.get(node).is_some_and(|node| {
                node.get_type() == "alchemist_output_socket"
            })
        }),
        "Constant output socket must be a real node"
    );
}

#[test]
fn anode_config_and_type_changes_reconcile_real_children() {
    let (mut engine, formula) = engine_with_formula();
    create_anode(&mut engine, formula, "constant", 2.0, 3.0);
    let anode = direct_children(&engine, formula)
        .into_iter()
        .find(|node| {
            engine.nodes.get(*node).is_some_and(|node| {
                node.get_type() == AlchemistANode::NODE_TYPE
            })
        })
        .expect("Constant ANode should exist");
    let config =
        find_child_by_decl(&engine, anode, "config").expect("Config should exist");
    let value_type = find_child_by_decl(
        &engine,
        config,
        "config/value__type",
    )
    .expect("runtime value type should be a real parameter");

    let ack = engine.apply_ui_intent(UiEditIntent::SetParam {
        node: value_type,
        value: ParamValue::Enum("vec3".into()),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    assert!(ack.success, "value type edit should succeed: {ack:?}");
    assert_eq!(
        parameter_value(&engine, config, "config/value"),
        ParamValue::Vec3(0.0, 0.0, 0.0)
    );
    let outputs =
        find_child_by_decl(&engine, anode, "outputs").expect("Outputs should exist");
    let output = direct_children(&engine, outputs)
        .into_iter()
        .next()
        .expect("Constant output should exist");
    assert_eq!(
        parameter_value(&engine, output, "outputs/value/value_type"),
        ParamValue::Str("vec3".into())
    );

    let anode_type = find_child_by_decl(&engine, anode, "anode_type")
        .expect("ANode type should be a real parameter");
    let ack = engine.apply_ui_intent(UiEditIntent::SetParam {
        node: anode_type,
        value: ParamValue::Str("debug_log".into()),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    assert!(ack.success, "ANode type edit should succeed: {ack:?}");
    assert!(
        direct_children(&engine, config).is_empty(),
        "stale Constant config should be removed"
    );
    assert!(
        direct_children(&engine, outputs).is_empty(),
        "stale Constant output should be removed"
    );
    let inputs =
        find_child_by_decl(&engine, anode, "inputs").expect("Inputs should exist");
    assert_eq!(direct_children(&engine, inputs).len(), 1);
}

#[test]
fn authored_formula_subtree_survives_reload_and_compiles() {
    let (mut engine, formula) = engine_with_formula();
    create_anode(&mut engine, formula, "constant", 2.0, 3.0);
    create_anode(&mut engine, formula, "debug_log", 18.0, 3.0);

    let anodes = direct_children(&engine, formula)
        .into_iter()
        .filter(|node| {
            engine.nodes.get(*node).is_some_and(|node| {
                node.get_type() == AlchemistANode::NODE_TYPE
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(anodes.len(), 2);
    let constant = anodes
        .iter()
        .copied()
        .find(|node| {
            parameter_value(&engine, *node, "anode_type")
                == ParamValue::Str("constant".into())
        })
        .expect("Constant should exist");
    let debug = anodes
        .iter()
        .copied()
        .find(|node| {
            parameter_value(&engine, *node, "anode_type")
                == ParamValue::Str("debug_log".into())
        })
        .expect("Debug Log should exist");
    create_connection(
        &mut engine,
        formula,
        constant,
        "value",
        debug,
        "value",
    );

    let materialized = formula_from_snapshot(
        &engine.process_tree_snapshot(),
        formula,
    )
    .expect("Formula subtree should materialize");
    assert_eq!(materialized.graph.nodes.len(), 2);
    assert_eq!(materialized.graph.edges.len(), 1);
    assert_eq!(
        parameter_value(&engine, formula, "is_valid"),
        ParamValue::Bool(true)
    );

    let json = golden_core::app::to_sparse_project_json_pretty(&engine)
        .expect("Formula project should encode");
    assert!(json.contains("alchemist_anode"));
    assert!(json.contains("alchemist_connection"));
    assert!(!json.contains("formula_json"));

    let loaded = golden_core::app::from_sparse_project_json::<AppNode>(&json)
        .expect("Formula project should decode");
    let loaded_formula = loaded
        .nodes
        .iter()
        .find(|(_, node)| {
            node.get_type() == AlchemistFormulaDefinition::NODE_TYPE
        })
        .map(|(id, _)| id)
        .expect("Formula should reload");
    let reloaded = formula_from_snapshot(
        &loaded.process_tree_snapshot(),
        loaded_formula,
    )
    .expect("reloaded Formula subtree should materialize");

    assert_eq!(reloaded.graph.nodes.len(), 2);
    assert_eq!(reloaded.graph.edges.len(), 1);

    let formula_asset =
        golden_core::app::to_sparse_subtree_json_pretty(&engine, formula)
            .expect("Formula subtree should encode as an asset");
    let asset: serde_json::Value =
        serde_json::from_str(&formula_asset).expect("Formula asset should be JSON");
    assert_eq!(asset["root"]["type"], "alchemist_formula");

    let root: AppNode = Folder::new("import root").into();
    let mut imported_engine = AppEngine::new(root);
    imported_engine.add_node(FormulaLibrary::new().into(), None);
    imported_engine
        .apply_edits()
        .expect("import Formula Library should attach");
    let imported_library = imported_engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == FormulaLibrary::NODE_TYPE)
        .map(|(id, _)| id)
        .expect("import Formula Library should exist");
    let imported_formula = golden_core::app::insert_sparse_subtree_json(
        &mut imported_engine,
        imported_library,
        None,
        &formula_asset,
    )
    .expect("Formula asset should import as a normal node subtree");
    assert_ne!(
        imported_engine
            .nodes
            .get(imported_formula)
            .expect("imported Formula should exist")
            .node_data()
            .meta
            .uuid,
        engine
            .nodes
            .get(formula)
            .expect("source Formula should exist")
            .node_data()
            .meta
            .uuid,
        "asset import should remap persistent identities"
    );
    let imported = formula_from_snapshot(
        &imported_engine.process_tree_snapshot(),
        imported_formula,
    )
    .expect("imported Formula subtree should materialize");
    assert_eq!(imported.graph.nodes.len(), 2);
    assert_eq!(imported.graph.edges.len(), 1);
}

#[test]
fn removing_anode_removes_its_dangling_connections() {
    let (mut engine, formula) = engine_with_formula();
    create_anode(&mut engine, formula, "constant", 2.0, 3.0);
    create_anode(&mut engine, formula, "debug_log", 18.0, 3.0);
    let anodes = direct_children(&engine, formula)
        .into_iter()
        .filter(|node| {
            engine.nodes.get(*node).is_some_and(|node| {
                node.get_type() == AlchemistANode::NODE_TYPE
            })
        })
        .collect::<Vec<_>>();
    create_connection(
        &mut engine,
        formula,
        anodes[0],
        "value",
        anodes[1],
        "value",
    );

    let ack = engine.apply_ui_intent(UiEditIntent::RemoveNode {
        node: anodes[0],
    });
    assert!(ack.success, "ANode removal should succeed: {ack:?}");
    engine
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .expect("Formula should receive the child removal event");
    engine
        .apply_edits()
        .expect("Formula connection cleanup should stabilize");
    assert!(
        direct_children(&engine, formula).into_iter().all(|node| {
            engine.nodes.get(node).is_some_and(|node| {
                node.get_type() != AlchemistConnection::NODE_TYPE
            })
        }),
        "Formula should own cleanup of connections whose endpoint was removed"
    );
}

fn engine_with_formula() -> (AppEngine, NodeId) {
    let root: AppNode = Folder::new("root").into();
    let mut engine = AppEngine::new(root);
    engine.add_node(FormulaLibrary::new().into(), None);
    engine.apply_edits().expect("Formula Library should attach");
    let library = engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == FormulaLibrary::NODE_TYPE)
        .map(|(id, _)| id)
        .expect("Formula Library should exist");
    let ack = engine.apply_ui_intent(UiEditIntent::CreateUserItem {
        parent: library,
        node_type: AlchemistFormulaDefinition::NODE_TYPE.to_owned(),
        label: Some("Custom Formula".into()),
        initial_params: Vec::new(),
    });
    assert!(ack.success, "Formula creation should succeed: {ack:?}");
    let formula = direct_children(&engine, library)
        .into_iter()
        .find(|node| {
            engine.nodes.get(*node).is_some_and(|node| {
                node.get_type() == AlchemistFormulaDefinition::NODE_TYPE
            })
        })
        .expect("Formula should exist");
    (engine, formula)
}

fn create_anode(
    engine: &mut AppEngine,
    formula: NodeId,
    type_id: &str,
    x: f64,
    y: f64,
) {
    let ack = engine.apply_ui_intent(UiEditIntent::CreateUserItem {
        parent: formula,
        node_type: format!("{ANODE_CREATE_PREFIX}{type_id}"),
        label: None,
        initial_params: vec![UiCreateUserItemInitialParam {
            decl_id: DeclId("position".into()),
            value: ParamValue::Vec2(x, y),
        }],
    });
    assert!(ack.success, "ANode creation should succeed: {ack:?}");
}

fn create_connection(
    engine: &mut AppEngine,
    formula: NodeId,
    source: NodeId,
    source_socket: &str,
    target: NodeId,
    target_socket: &str,
) {
    let source_uuid = engine
        .nodes
        .get(source)
        .expect("source should exist")
        .node_data()
        .meta
        .uuid;
    let target_uuid = engine
        .nodes
        .get(target)
        .expect("target should exist")
        .node_data()
        .meta
        .uuid;
    let ack = engine.apply_ui_intent(UiEditIntent::CreateUserItem {
        parent: formula,
        node_type: AlchemistConnection::NODE_TYPE.to_owned(),
        label: None,
        initial_params: vec![
            UiCreateUserItemInitialParam {
                decl_id: DeclId("source_node".into()),
                value: ParamValue::Reference(NodeReference::new(source_uuid)),
            },
            UiCreateUserItemInitialParam {
                decl_id: DeclId("source_socket".into()),
                value: ParamValue::Str(source_socket.into()),
            },
            UiCreateUserItemInitialParam {
                decl_id: DeclId("target_node".into()),
                value: ParamValue::Reference(NodeReference::new(target_uuid)),
            },
            UiCreateUserItemInitialParam {
                decl_id: DeclId("target_socket".into()),
                value: ParamValue::Str(target_socket.into()),
            },
        ],
    });
    assert!(ack.success, "Connection creation should succeed: {ack:?}");
}

fn direct_children(engine: &AppEngine, parent: NodeId) -> Vec<NodeId> {
    engine.process_tree_snapshot().child_ids(parent)
}

fn find_child_by_decl(
    engine: &AppEngine,
    parent: NodeId,
    decl_id: &str,
) -> Option<NodeId> {
    engine
        .process_tree_snapshot()
        .find_child_by_decl_id(parent, decl_id)
}

fn find_descendant_by_decl(
    engine: &AppEngine,
    root: NodeId,
    decl_id: &str,
) -> Option<NodeId> {
    let snapshot = engine.process_tree_snapshot();
    let mut stack = vec![root];
    while let Some(parent) = stack.pop() {
        for child in snapshot.child_ids(parent) {
            let child_snapshot = snapshot.node(child)?;
            if child_snapshot.decl_id == decl_id {
                return Some(child);
            }
            stack.push(child);
        }
    }
    None
}

fn parameter_value(
    engine: &AppEngine,
    parent: NodeId,
    decl_id: &str,
) -> ParamValue {
    let child = find_child_by_decl(engine, parent, decl_id)
        .unwrap_or_else(|| panic!("parameter `{decl_id}` should exist"));
    engine
        .nodes
        .get(child)
        .and_then(Node::engine_param_snapshot)
        .map(|snapshot| snapshot.value)
        .unwrap_or_else(|| panic!("`{decl_id}` should be a parameter"))
}
