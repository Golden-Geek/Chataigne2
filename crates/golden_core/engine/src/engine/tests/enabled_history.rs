use std::sync::Mutex;

use super::*;
use crate::{
    edit::Edit,
    node::{Folder, Node, NodeMetaPatch},
    process_ctx::ProcessCtx,
};

static ENABLED_CALLBACKS: Mutex<Vec<bool>> = Mutex::new(Vec::new());
static ENABLED_HISTORY_TEST_LOCK: Mutex<()> = Mutex::new(());

fn assert_enabled_snapshot_matches_reference<T: Node>(engine: &Engine<T>) {
    let snapshot = engine.build_process_tree_snapshot();
    for (node_id, node) in engine.nodes.iter() {
        let expected = engine.is_effectively_enabled(node_id);
        assert_eq!(
            node.node_data().effective_enabled,
            expected,
            "cached enabled state differs for {node_id:?}",
        );
        assert_eq!(
            snapshot.node(node_id).map(|node| node.enabled),
            Some(expected),
            "snapshot enabled state differs for {node_id:?}",
        );
    }
}

#[crate::node("enabled_history_probe")]
struct EnabledHistoryProbe {}

#[crate::node("enabled_history_probe", from_struct)]
impl Node for EnabledHistoryProbe {
    fn on_effective_enabled_changed(&mut self, ctx: &mut ProcessCtx, enabled: bool) {
        assert_eq!(
            ctx.tree_snapshot()
                .and_then(|snapshot| snapshot.node(self.id()))
                .map(|node| node.enabled),
            Some(enabled),
        );
        ENABLED_CALLBACKS.lock().expect("callback log poisoned").push(enabled);
    }
}

crate::define_node_enum!(
    enum EnabledHistoryNode {
        EnabledHistoryProbe,
    }
);

#[test]
fn metadata_history_replays_effective_enabled_state_and_callbacks() {
    let _test_guard = ENABLED_HISTORY_TEST_LOCK.lock().expect("test lock poisoned");
    ENABLED_CALLBACKS.lock().expect("callback log poisoned").clear();
    let root: EnabledHistoryNode = Folder::new("root").into();
    let mut engine = Engine::new(root);
    engine.add_node(Folder::new("parent").into(), None);
    engine.apply_edits().expect("parent should attach");
    let parent = engine.nodes.get(engine.root).unwrap().node_data().first_child.unwrap();
    engine.add_node(EnabledHistoryProbe::new().into(), Some(parent));
    engine.apply_edits().expect("child should attach");
    let child = engine.nodes.get(parent).unwrap().node_data().first_child.unwrap();
    engine.clear_history();

    engine.edits.push(Edit::PatchMeta {
        node: parent,
        patch: NodeMetaPatch {
            enabled: Some(false),
            ..Default::default()
        },
    });
    engine.apply_edits().expect("disable should apply");
    assert!(!engine.nodes.get(parent).unwrap().node_data().effective_enabled);
    assert!(!engine.nodes.get(child).unwrap().node_data().effective_enabled);
    assert_enabled_snapshot_matches_reference(&engine);

    assert!(engine.undo().expect("undo should apply"));
    assert!(engine.nodes.get(parent).unwrap().node_data().effective_enabled);
    assert!(engine.nodes.get(child).unwrap().node_data().effective_enabled);
    assert_enabled_snapshot_matches_reference(&engine);

    assert!(engine.redo().expect("redo should apply"));
    assert!(!engine.nodes.get(parent).unwrap().node_data().effective_enabled);
    assert!(!engine.nodes.get(child).unwrap().node_data().effective_enabled);
    assert_enabled_snapshot_matches_reference(&engine);
    assert_eq!(
        *ENABLED_CALLBACKS.lock().expect("callback log poisoned"),
        [false, true, false],
    );
}

#[test]
fn move_history_replays_effective_enabled_state_and_callbacks() {
    let _test_guard = ENABLED_HISTORY_TEST_LOCK.lock().expect("test lock poisoned");
    ENABLED_CALLBACKS.lock().expect("callback log poisoned").clear();
    let root: EnabledHistoryNode = Folder::new("root").into();
    let mut engine = Engine::new(root);
    engine.add_node(Folder::new("enabled parent").into(), None);
    let mut disabled_parent: EnabledHistoryNode = Folder::new("disabled parent").into();
    disabled_parent.node_data_mut().meta.enabled = false;
    engine.add_node(disabled_parent, None);
    engine.apply_edits().expect("parents should attach");
    let enabled_parent = engine.nodes.get(engine.root).unwrap().node_data().first_child.unwrap();
    let disabled_parent = engine
        .nodes
        .get(enabled_parent)
        .unwrap()
        .node_data()
        .next_sibling
        .unwrap();
    engine.add_node(EnabledHistoryProbe::new().into(), Some(enabled_parent));
    engine.apply_edits().expect("child should attach");
    let child = engine
        .nodes
        .get(enabled_parent)
        .unwrap()
        .node_data()
        .first_child
        .unwrap();
    engine.clear_history();

    engine.edits.push(Edit::MoveNode {
        node: child,
        new_parent: disabled_parent,
        new_prev_sibling: None,
    });
    engine.apply_edits().expect("move should apply");
    assert!(!engine.nodes.get(child).unwrap().node_data().effective_enabled);
    assert_enabled_snapshot_matches_reference(&engine);

    assert!(engine.undo().expect("undo should apply"));
    assert!(engine.nodes.get(child).unwrap().node_data().effective_enabled);
    assert_enabled_snapshot_matches_reference(&engine);

    assert!(engine.redo().expect("redo should apply"));
    assert!(!engine.nodes.get(child).unwrap().node_data().effective_enabled);
    assert_enabled_snapshot_matches_reference(&engine);
    assert_eq!(
        *ENABLED_CALLBACKS.lock().expect("callback log poisoned"),
        [false, true, false],
    );
}

#[test]
fn replacement_reconciles_descendant_enabled_state_through_history() {
    let _test_guard = ENABLED_HISTORY_TEST_LOCK.lock().expect("test lock poisoned");
    ENABLED_CALLBACKS.lock().expect("callback log poisoned").clear();
    let mut engine = Engine::new(EnabledHistoryNode::from(Folder::new("root")));
    engine.add_node(Folder::new("parent").into(), None);
    engine.apply_edits().expect("parent should attach");
    let parent = engine.nodes.get(engine.root).unwrap().node_data().first_child.unwrap();
    engine.add_node(EnabledHistoryProbe::new().into(), Some(parent));
    engine.apply_edits().expect("child should attach");
    let child = engine.nodes.get(parent).unwrap().node_data().first_child.unwrap();
    engine.clear_history();

    let mut disabled_replacement: EnabledHistoryNode = Folder::new("replacement").into();
    disabled_replacement.node_data_mut().meta.enabled = false;
    engine.replace_node(parent, disabled_replacement);
    engine.apply_edits().expect("replacement should apply");
    assert!(!engine.nodes.get(parent).unwrap().node_data().effective_enabled);
    assert!(!engine.nodes.get(child).unwrap().node_data().effective_enabled);
    assert!(!engine.build_process_tree_snapshot().node(child).unwrap().enabled);
    assert_enabled_snapshot_matches_reference(&engine);

    assert!(engine.undo().expect("undo should apply"));
    assert!(engine.nodes.get(parent).unwrap().node_data().effective_enabled);
    assert!(engine.nodes.get(child).unwrap().node_data().effective_enabled);
    assert!(engine.build_process_tree_snapshot().node(child).unwrap().enabled);
    assert_enabled_snapshot_matches_reference(&engine);

    assert!(engine.redo().expect("redo should apply"));
    assert!(!engine.nodes.get(parent).unwrap().node_data().effective_enabled);
    assert!(!engine.nodes.get(child).unwrap().node_data().effective_enabled);
    assert!(!engine.build_process_tree_snapshot().node(child).unwrap().enabled);
    assert_enabled_snapshot_matches_reference(&engine);
    assert_eq!(
        *ENABLED_CALLBACKS.lock().expect("callback log poisoned"),
        [false, true, false],
    );
}

#[test]
fn replacement_under_disabled_parent_inherits_effective_enabled_state() {
    let _test_guard = ENABLED_HISTORY_TEST_LOCK.lock().expect("test lock poisoned");
    ENABLED_CALLBACKS.lock().expect("callback log poisoned").clear();
    let mut engine = Engine::new(EnabledHistoryNode::from(Folder::new("root")));
    let mut disabled_parent: EnabledHistoryNode = Folder::new("disabled parent").into();
    disabled_parent.node_data_mut().meta.enabled = false;
    engine.add_node(disabled_parent, None);
    engine.apply_edits().expect("parent should attach");
    let parent = engine.nodes.get(engine.root).unwrap().node_data().first_child.unwrap();
    engine.add_node(EnabledHistoryProbe::new().into(), Some(parent));
    engine.apply_edits().expect("child should attach");
    let child = engine.nodes.get(parent).unwrap().node_data().first_child.unwrap();
    engine.clear_history();

    engine.replace_node(child, EnabledHistoryProbe::new().into());
    engine.apply_edits().expect("replacement should apply");
    assert!(!engine.nodes.get(child).unwrap().node_data().effective_enabled);
    assert!(!engine.build_process_tree_snapshot().node(child).unwrap().enabled);
    assert_enabled_snapshot_matches_reference(&engine);
    assert!(engine.undo().expect("undo should apply"));
    assert!(!engine.nodes.get(child).unwrap().node_data().effective_enabled);
    assert_enabled_snapshot_matches_reference(&engine);
    assert!(engine.redo().expect("redo should apply"));
    assert!(!engine.nodes.get(child).unwrap().node_data().effective_enabled);
    assert_enabled_snapshot_matches_reference(&engine);
    assert!(ENABLED_CALLBACKS.lock().expect("callback log poisoned").is_empty());
}

#[test]
fn disabled_root_initializes_effective_enabled_state() {
    let mut root: EnabledHistoryNode = Folder::new("disabled root").into();
    root.node_data_mut().meta.enabled = false;
    let engine = Engine::new(root);
    assert!(!engine.nodes.get(engine.root).unwrap().node_data().effective_enabled);
    assert!(!engine.build_process_tree_snapshot().node(engine.root).unwrap().enabled);
    assert_enabled_snapshot_matches_reference(&engine);
}

#[test]
fn project_load_initializes_inherited_enabled_cache_before_lifecycle() {
    let mut root = Folder::new("disabled root");
    root.node_data_mut().meta.enabled = false;
    let mut original = Engine::new(root);
    original.add_node(Folder::new("parent"), None);
    original.apply_edits().expect("parent should attach");
    let parent = original
        .nodes
        .get(original.root)
        .unwrap()
        .node_data()
        .first_child
        .unwrap();
    original.add_node(Folder::new("child"), Some(parent));
    original.apply_edits().expect("child should attach");
    let project = original
        .to_project_file_with(|_| Ok(serde_json::Value::Null))
        .expect("project should serialize");

    let loaded = Engine::<Folder>::from_project_file_with(project, |_, _, _| Ok(Folder::new("decoded")))
        .expect("project should load");
    assert_enabled_snapshot_matches_reference(&loaded);
    for (node_id, node) in loaded.nodes.iter() {
        assert_eq!(
            node.node_data().effective_enabled,
            loaded.is_effectively_enabled(node_id),
            "loaded cache must match inherited state for {node_id:?}",
        );
    }
}

#[test]
fn imported_subtree_inherits_disabled_destination_cache() {
    let mut source = Engine::new(Folder::new("source root"));
    source.add_node(Folder::new("source child"), None);
    source.apply_edits().expect("source child should attach");
    let project = source
        .to_project_file_with(|_| Ok(serde_json::Value::Null))
        .expect("source should serialize");

    let mut target = Engine::new(Folder::new("target root"));
    let mut disabled_parent = Folder::new("disabled parent");
    disabled_parent.node_data_mut().meta.enabled = false;
    target.add_node(disabled_parent, None);
    target.apply_edits().expect("destination should attach");
    let parent = target.nodes.get(target.root).unwrap().node_data().first_child.unwrap();
    let imported = target
        .insert_project_subtree_with(project, parent, None, |_, _, _| Ok(Folder::new("decoded")))
        .expect("subtree should import");
    assert_enabled_snapshot_matches_reference(&target);
    for node_id in target.collect_subtree_node_ids(imported) {
        let node = target.nodes.get(node_id).unwrap();
        assert!(!node.node_data().effective_enabled);
        assert!(!target.build_process_tree_snapshot().node(node_id).unwrap().enabled);
        assert_eq!(
            node.node_data().effective_enabled,
            target.is_effectively_enabled(node_id)
        );
    }
}

#[test]
fn add_and_remove_history_preserve_disabled_ancestor_cache() {
    let mut engine = Engine::new(Folder::new("root"));
    let mut disabled_parent = Folder::new("disabled parent");
    disabled_parent.node_data_mut().meta.enabled = false;
    engine.add_node(disabled_parent, None);
    engine.apply_edits().expect("parent should attach");
    let parent = engine.nodes.get(engine.root).unwrap().node_data().first_child.unwrap();
    engine.clear_history();

    engine.add_node(Folder::new("child"), Some(parent));
    engine.apply_edits().expect("child should attach");
    assert_enabled_snapshot_matches_reference(&engine);
    let child = engine.nodes.get(parent).unwrap().node_data().first_child.unwrap();

    assert!(engine.undo().expect("add undo should apply"));
    assert_enabled_snapshot_matches_reference(&engine);
    assert!(engine.redo().expect("add redo should apply"));
    assert_enabled_snapshot_matches_reference(&engine);

    engine.clear_history();
    engine.edits.push(Edit::RemoveNode { node: child });
    engine.apply_edits().expect("remove should apply");
    assert_enabled_snapshot_matches_reference(&engine);
    assert!(engine.undo().expect("remove undo should apply"));
    assert_enabled_snapshot_matches_reference(&engine);
    assert!(engine.redo().expect("remove redo should apply"));
    assert_enabled_snapshot_matches_reference(&engine);
}
