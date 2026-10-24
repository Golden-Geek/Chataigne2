use super::*;
use crate::{
    app::ProjectNode,
    edit::{Edit, NodeTree},
    node::Folder,
    parameter::{Parameter, ParameterChangeCheck},
    process_ctx::ProcessCtx,
};

#[crate::node("import_ready_failure")]
struct ImportReadyFailure {}

#[crate::node("import_ready_failure", from_struct)]
impl Node for ImportReadyFailure {
    fn on_node_ready(&mut self, ctx: &mut ProcessCtx, _: NodeCreationContext) {
        ctx.set_param(NodeId(u64::MAX), ParamValue::Int(1));
    }
}
crate::define_node_enum!(
    enum ImportNode {
        ImportReadyFailure,
    }
);

fn spec(project: ProjectFile) -> ProjectImportSpec {
    ProjectImportSpec {
        project,
        initial_params: vec![],
    }
}

#[test]
fn forest_import_remaps_cross_root_references_preserves_external_uuids_and_input_order() {
    let mut engine = Engine::<ImportNode>::new(Folder::new("root").into());
    let external = Folder::new("External");
    let external_uuid = external.node_data().meta.uuid;
    let b = Folder::new("B");
    let b_uuid = b.node_data().meta.uuid;
    let a = Folder::new("A");
    let a_uuid = a.node_data().meta.uuid;
    let peer = Parameter::new(
        "Peer",
        ParamValue::Reference(NodeReference::new(b_uuid)),
        ParameterChangeCheck::None,
    );
    let peer_uuid = peer.node_data().meta.uuid;
    let outside = Parameter::new(
        "Outside",
        ParamValue::Reference(NodeReference::new(external_uuid)),
        ParameterChangeCheck::None,
    );
    let outside_uuid = outside.node_data().meta.uuid;
    engine.edits.push(Edit::AddNodeTree {
        tree: NodeTree::new(external),
        parent: engine.root,
        prev_sibling: None,
    });
    engine.edits.push(Edit::AddNodeTree {
        tree: NodeTree::new(a)
            .with_child(NodeTree::new(peer))
            .with_child(NodeTree::new(outside)),
        parent: engine.root,
        prev_sibling: None,
    });
    engine.edits.push(Edit::AddNodeTree {
        tree: NodeTree::new(b),
        parent: engine.root,
        prev_sibling: None,
    });
    engine.apply_edits().unwrap();
    let a = engine.uuid_index[&a_uuid];
    let b = engine.uuid_index[&b_uuid];
    let docs = [a, b]
        .into_iter()
        .map(|node| {
            spec(
                engine
                    .capture_project_subtree_with(node, Node::project_encode_data)
                    .unwrap(),
            )
        })
        .collect();
    let imported = engine
        .import_project_subtrees_with(docs, engine.root, Some(b), ImportNode::project_decode_node, |_, _| {
            Ok(())
        })
        .unwrap();
    let ids = imported
        .identities
        .iter()
        .map(|id| (id.source_uuid, id))
        .collect::<HashMap<_, _>>();
    assert_eq!(imported.roots, vec![ids[&a_uuid].node, ids[&b_uuid].node]);
    assert_eq!(
        engine.nodes.get(imported.roots[0]).unwrap().node_data().next_sibling,
        Some(imported.roots[1])
    );
    let value = engine
        .nodes
        .get(ids[&peer_uuid].node)
        .unwrap()
        .engine_param_snapshot()
        .unwrap()
        .value;
    let ParamValue::Reference(reference) = value else {
        panic!("reference")
    };
    assert_eq!(reference.uuid(), ids[&b_uuid].uuid);
    let value = engine
        .nodes
        .get(ids[&outside_uuid].node)
        .unwrap()
        .engine_param_snapshot()
        .unwrap()
        .value;
    let ParamValue::Reference(reference) = value else {
        panic!("reference")
    };
    assert_eq!(reference.uuid(), external_uuid);
    assert_eq!(
        engine.nodes.get(imported.roots[0]).unwrap().node_data().meta.label,
        "A 2"
    );
}

#[test]
fn duplicate_identity_codec_admission_and_lifecycle_failures_have_no_partial_import() {
    let mut source = Engine::<ImportNode>::new(Folder::new("source").into());
    source.add_node(Folder::new("good").into(), None);
    source.apply_edits().unwrap();
    let good = source.nodes.get(source.root).unwrap().node_data().first_child.unwrap();
    let project = source
        .capture_project_subtree_with(good, Node::project_encode_data)
        .unwrap();
    for mode in ["duplicate", "codec", "admission", "ready"] {
        let mut engine = Engine::<ImportNode>::new(Folder::new("destination").into());
        let mut other = project.clone();
        other.root.uuid = NodeUuid(Uuid::new_v4());
        if mode == "duplicate" {
            other.root.uuid = project.root.uuid;
        }
        if mode == "codec" {
            other.root.node_type = "unavailable".into();
        }
        if mode == "ready" {
            other.root.node_type = "import_ready_failure".into();
            other.root.data = Some(serde_json::json!({}));
        }
        let cursor = engine.ui_event_log().len();
        let result = engine.import_project_subtrees_with(
            vec![spec(project.clone()), spec(other)],
            engine.root,
            None,
            ImportNode::project_decode_node,
            |_, _| {
                if mode == "admission" {
                    Err("not admitted".into())
                } else {
                    Ok(())
                }
            },
        );
        assert!(result.is_err(), "{mode}");
        assert_eq!(engine.nodes.len(), 1, "{mode}");
        assert_eq!(engine.ui_event_log().len(), cursor, "{mode}");
        assert_eq!(engine.undo_len(), 0, "{mode}");
        assert!(engine.edits.pending.is_empty(), "{mode}");
    }
}

#[test]
fn thousand_and_ten_thousand_root_imports_share_lifecycle_snapshots() {
    let source = Engine::<ImportNode>::new(Folder::new("Gain").into());
    let template = source
        .capture_project_subtree_with(source.root, Node::project_encode_data)
        .unwrap();
    for count in [1_000, 10_000] {
        let mut engine = Engine::<ImportNode>::new(Folder::new("destination").into());
        let imports = (0..count)
            .map(|_| {
                let mut document = template.clone();
                document.root.uuid = NodeUuid(Uuid::new_v4());
                spec(document)
            })
            .collect();
        let before = engine.tick_stats().snapshot_builds;
        let started = std::time::Instant::now();
        let result = engine
            .import_project_subtrees_with(imports, engine.root, None, ImportNode::project_decode_node, |_, _| {
                Ok(())
            })
            .unwrap();
        let elapsed = started.elapsed();
        let snapshots = engine.tick_stats().snapshot_builds - before;
        assert!(snapshots <= 3, "{count} roots built {snapshots} full snapshots");
        assert_eq!(result.roots.len(), count);
        assert_eq!(result.identities.len(), count);
        assert_eq!(engine.nodes.len(), count + 1);
        assert_eq!(
            engine
                .nodes
                .get(*result.roots.last().unwrap())
                .unwrap()
                .node_data()
                .meta
                .label,
            format!("Gain {count}")
        );
        assert_eq!(engine.process_tree_snapshot().child_ids(engine.root), result.roots);
        let transactions = engine
            .ui_event_log()
            .iter()
            .filter_map(|event| {
                if let crate::events::EventKind::GraphTransaction { transaction } = &event.kind {
                    Some(transaction)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(transactions.len(), 1);
        let orders = transactions[0]
            .ops
            .iter()
            .filter(|op| {
                matches!(
                    op,
                    crate::ui_sync::UiGraphOp::ChildrenInserted { .. }
                        | crate::ui_sync::UiGraphOp::ChildrenReordered { .. }
                        | crate::ui_sync::UiGraphOp::SubtreeInserted {
                            parent_children_after: Some(_),
                            ..
                        }
                )
            })
            .count();
        assert_eq!(orders, 1, "{count} roots must publish one parent order");
        eprintln!(
            "complete forest: {count} roots, {} ms, {snapshots} full snapshots",
            elapsed.as_millis()
        );
    }
}

#[test]
fn root_placement_initializer_cannot_target_a_nested_curve_position() {
    let mut source = Engine::<ImportNode>::new(Folder::new("source").into());
    let root = Folder::new("ANode fixture");
    let uuid = root.node_data().meta.uuid;
    let mut position = Parameter::new("Position", ParamValue::Vec2(0.0, 0.0), ParameterChangeCheck::None);
    position.node_data_mut().meta.decl_id = DeclId("position".into());
    let position_uuid = position.node_data().meta.uuid;
    let mut nested = Parameter::new("Position", ParamValue::Float(0.5), ParameterChangeCheck::None);
    nested.node_data_mut().meta.decl_id = DeclId("position".into());
    let nested_uuid = nested.node_data().meta.uuid;
    source.edits.push(Edit::AddNodeTree {
        tree: NodeTree::new(root)
            .with_child(NodeTree::new(position))
            .with_child(NodeTree::new(Folder::new("curve")).with_child(NodeTree::new(nested))),
        parent: source.root,
        prev_sibling: None,
    });
    source.apply_edits().unwrap();
    let document = source
        .capture_project_subtree_with(source.uuid_index[&uuid], Node::project_encode_data)
        .unwrap();
    let mut destination = Engine::<ImportNode>::new(Folder::new("destination").into());
    let result = destination
        .import_project_subtrees_with(
            vec![ProjectImportSpec {
                project: document,
                initial_params: vec![(DeclId("position".into()), ParamValue::Vec2(12.0, 15.0))],
            }],
            destination.root,
            None,
            ImportNode::project_decode_node,
            |_, _| Ok(()),
        )
        .unwrap();
    let mapping = result
        .identities
        .iter()
        .map(|identity| (identity.source_uuid, identity.node))
        .collect::<HashMap<_, _>>();
    assert_eq!(
        destination
            .nodes
            .get(mapping[&position_uuid])
            .unwrap()
            .engine_param_snapshot()
            .unwrap()
            .value,
        ParamValue::Vec2(12.0, 15.0)
    );
    assert_eq!(
        destination
            .nodes
            .get(mapping[&nested_uuid])
            .unwrap()
            .engine_param_snapshot()
            .unwrap()
            .value,
        ParamValue::Float(0.5)
    );
}
