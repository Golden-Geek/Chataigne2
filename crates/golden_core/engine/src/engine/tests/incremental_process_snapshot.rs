use super::*;
use crate::{
    node::Folder,
    parameter::{Parameter, ParameterChangeCheck, ParameterEventBehaviour},
};

crate::define_node_enum!(
    enum SnapshotNode {}
);

fn fixture() -> Engine<SnapshotNode> {
    let mut engine = Engine::new(SnapshotNode::from(Folder::new("root")));
    let trees = (0..1024)
        .map(|i| {
            NodeTree::new(Folder::new(format!("folder {i}"))).with_child(NodeTree::new(Parameter::new(
                "value",
                ParamValue::Int(0),
                ParameterChangeCheck::None,
            )))
        })
        .collect();
    engine.apply_project_load_node_trees(trees, engine.root, None).unwrap();
    engine
}

fn child(engine: &Engine<SnapshotNode>, parent: NodeId) -> NodeId {
    engine.nodes.get(parent).unwrap().node_data().first_child.unwrap()
}

fn check_fresh(engine: &mut Engine<SnapshotNode>) -> Arc<ProcessTreeSnapshot> {
    let refreshed = engine.canonical_process_snapshot();
    let full = engine.build_process_tree_snapshot();
    assert_eq!(refreshed.root(), full.root());
    assert_eq!(refreshed.node_count(), full.node_count());
    for id in engine.nodes.keys() {
        let actual = refreshed.node(id).unwrap();
        assert_eq!(actual, full.node(id).unwrap());
        assert_eq!(refreshed.child_ids_slice(id), full.child_ids_slice(id));
        assert_eq!(refreshed.subtree_revision(id), full.subtree_revision(id));
        assert_eq!(
            refreshed.subtree_structure_revision(id),
            full.subtree_structure_revision(id)
        );
        assert_eq!(
            refreshed.node_id_by_uuid(actual.uuid),
            full.node_id_by_uuid(actual.uuid)
        );
        if let Some(parent) = actual.parent {
            for alias in [&actual.decl_id, &actual.short_name, &actual.label] {
                assert_eq!(refreshed.find_child(parent, alias), full.find_child(parent, alias));
            }
            assert_eq!(
                refreshed.find_child_by_decl_id(parent, &actual.decl_id),
                full.find_child_by_decl_id(parent, &actual.decl_id)
            );
        }
    }
    refreshed
}

#[test]
fn reference_cache_reads_do_not_invalidate_unrelated_value_records() {
    let mut engine = fixture();
    let before = check_fresh(&mut engine);
    let cloned = engine.tick_stats().snapshot_nodes_cloned;
    assert_eq!(engine.resolve_reference_caches(), 0);
    assert!(engine.nodes.snapshot_changes().unwrap().is_empty());
    assert!(Arc::ptr_eq(&before, &engine.canonical_process_snapshot()));
    assert_eq!(engine.tick_stats().snapshot_nodes_cloned, cloned);
}

#[test]
fn sparse_parameter_writes_share_records_and_preserve_old_leased_views_across_compaction() {
    let mut engine = fixture();
    let parent = child(&engine, engine.root);
    let param = child(&engine, parent);
    let untouched = engine.nodes.get(parent).unwrap().node_data().next_sibling.unwrap();
    let first = check_fresh(&mut engine);
    let retained = first.try_lease().unwrap();
    let builds = engine.tick_stats().snapshot_builds;
    let cloned = engine.tick_stats().snapshot_nodes_cloned;
    engine.clear_tick_tree_snapshot();
    assert!(Arc::ptr_eq(&first, &engine.canonical_process_snapshot()));
    for value in 1..=20 {
        engine.edits.push(Edit::SetParam {
            node: param,
            value: ParamValue::Int(value),
            behaviour: ParameterEventBehaviour::Coalesce,
        });
        engine.apply_edits().unwrap();
        let next = check_fresh(&mut engine);
        assert_eq!(next.node(param).unwrap().param_value, Some(ParamValue::Int(value)));
        if value == 1 {
            assert!(std::ptr::eq(
                first.node(untouched).unwrap(),
                next.node(untouched).unwrap()
            ));
            assert!(engine.tick_stats().snapshot_nodes_cloned - cloned < 16);
        }
        assert_eq!(retained.node(param).unwrap().param_value, Some(ParamValue::Int(0)));
    }
    assert_eq!(engine.tick_stats().snapshot_builds, builds);
    assert_eq!(engine.tick_stats().snapshot_refreshes, 20);
}

#[test]
fn additions_aliases_moves_removal_and_history_match_full_capture() {
    let mut engine = fixture();
    let left = child(&engine, engine.root);
    let right = engine.nodes.get(left).unwrap().node_data().next_sibling.unwrap();
    let param = child(&engine, left);
    let old = check_fresh(&mut engine);
    engine.edits.push(Edit::PatchMeta {
        node: left,
        patch: NodeMetaPatch {
            label: Some("renamed".into()),
            short_name: Some("alias".into()),
            ..Default::default()
        },
    });
    engine.apply_edits().unwrap();
    check_fresh(&mut engine);
    engine.add_node(Folder::new("added").into(), Some(right));
    engine.apply_edits().unwrap();
    check_fresh(&mut engine);
    engine.edits.push(Edit::MoveNode {
        node: param,
        new_parent: right,
        new_prev_sibling: None,
    });
    engine.apply_edits().unwrap();
    check_fresh(&mut engine);
    assert!(engine.undo().unwrap());
    check_fresh(&mut engine);
    assert!(engine.redo().unwrap());
    check_fresh(&mut engine);
    engine.edits.push(Edit::RemoveNode { node: param });
    engine.apply_edits().unwrap();
    assert!(check_fresh(&mut engine).node(param).is_none());
    assert!(engine.undo().unwrap());
    check_fresh(&mut engine);
    assert_eq!(old.node(param).unwrap().parent, Some(left));
    assert_eq!(old.node(left).unwrap().label, "folder 0");
}

#[test]
fn direct_parent_mutation_counts_relations_without_trusting_sibling_links() {
    let mut engine = fixture();
    let left = child(&engine, engine.root);
    let right = engine.nodes.get(left).unwrap().node_data().next_sibling.unwrap();
    let param = child(&engine, left);
    engine.edits.push(Edit::SetParam {
        node: param,
        value: ParamValue::Int(99),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().unwrap();
    check_fresh(&mut engine);
    engine.nodes.get_mut(param).unwrap().node_data_mut().parent = Some(right);
    let next = check_fresh(&mut engine);
    assert_eq!(next.node(left).unwrap().child_count, 0);
    assert_eq!(next.node(right).unwrap().child_count, 2);
}

#[test]
fn counter_only_writes_refresh_ancestors_without_node_mutation_candidates() {
    let mut engine = fixture();
    let parent = child(&engine, engine.root);
    let param = child(&engine, parent);
    let before = check_fresh(&mut engine);
    let sequence = engine.tree_revisions.next_change();
    engine.tree_revisions.mark_structure(param, sequence);
    assert!(engine.nodes.snapshot_changes().unwrap().is_empty());
    let after = check_fresh(&mut engine);
    assert_ne!(
        after.subtree_revision(engine.root),
        before.subtree_revision(engine.root)
    );
    assert_ne!(
        after.subtree_structure_revision(parent),
        before.subtree_structure_revision(parent)
    );
    assert!(std::ptr::eq(before.node(param).unwrap(), after.node(param).unwrap()));
}

#[test]
fn deep_value_write_falls_back_to_exact_full_counter_folding() {
    let mut engine = Engine::new(SnapshotNode::from(Folder::new("root")));
    let mut tree = NodeTree::new(Parameter::new("value", ParamValue::Int(0), ParameterChangeCheck::None));
    for index in 0..128 {
        tree = NodeTree::new(Folder::new(format!("level {index}"))).with_child(tree);
    }
    engine
        .apply_project_load_node_trees(vec![tree], engine.root, None)
        .unwrap();
    let param = engine
        .nodes
        .iter()
        .find(|(_, node)| node.engine_param_snapshot().is_some())
        .unwrap()
        .0;
    let before = check_fresh(&mut engine);
    engine.edits.push(Edit::SetParam {
        node: param,
        value: ParamValue::Int(1),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().unwrap();
    let after = check_fresh(&mut engine);
    assert_ne!(
        after.subtree_revision(engine.root),
        before.subtree_revision(engine.root)
    );
    assert_eq!(before.node(param).unwrap().param_value, Some(ParamValue::Int(0)));
}

#[test]
fn duplicate_uuid_rebinding_and_replaced_stores_use_fresh_capture() {
    let mut engine = fixture();
    let left = child(&engine, engine.root);
    let param = child(&engine, left);
    check_fresh(&mut engine);
    let uuid = engine.nodes.get(left).unwrap().node_data().meta.uuid;
    engine.nodes.get_mut(param).unwrap().node_data_mut().meta.uuid = uuid;
    let builds = engine.tick_stats().snapshot_builds;
    check_fresh(&mut engine);
    assert_eq!(engine.tick_stats().snapshot_builds, builds + 1);
    let mut replacement = NodeStore::new();
    let root = replacement.insert(SnapshotNode::from(Folder::new("replacement")));
    engine.nodes = replacement;
    engine.root = root;
    let next = check_fresh(&mut engine);
    assert_eq!(next.node_count(), 1);
    assert_eq!(next.node(root).unwrap().label, "replacement");
}

#[test]
fn multiple_new_nodes_with_one_uuid_preserve_full_capture_first_owner() {
    let mut engine = fixture();
    check_fresh(&mut engine);
    let mut first = SnapshotNode::from(Folder::new("first"));
    let uuid = first.node_data().meta.uuid;
    first.node_data_mut().parent = Some(engine.root);
    let mut second = SnapshotNode::from(Folder::new("second"));
    second.node_data_mut().meta.uuid = uuid;
    second.node_data_mut().parent = Some(engine.root);
    engine.nodes.insert(first);
    engine.nodes.insert(second);
    let builds = engine.tick_stats().snapshot_builds;
    check_fresh(&mut engine);
    assert_eq!(engine.tick_stats().snapshot_builds, builds + 1);
}
