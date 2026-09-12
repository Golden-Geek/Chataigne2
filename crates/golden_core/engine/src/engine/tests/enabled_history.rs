use std::sync::Mutex;

use super::*;
use crate::{
    edit::Edit,
    node::{Folder, Node, NodeMetaPatch},
    process_ctx::ProcessCtx,
};

static ENABLED_CALLBACKS: Mutex<Vec<bool>> = Mutex::new(Vec::new());
static ENABLED_HISTORY_TEST_LOCK: Mutex<()> = Mutex::new(());

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

    assert!(engine.undo().expect("undo should apply"));
    assert!(engine.nodes.get(parent).unwrap().node_data().effective_enabled);
    assert!(engine.nodes.get(child).unwrap().node_data().effective_enabled);

    assert!(engine.redo().expect("redo should apply"));
    assert!(!engine.nodes.get(parent).unwrap().node_data().effective_enabled);
    assert!(!engine.nodes.get(child).unwrap().node_data().effective_enabled);
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

    assert!(engine.undo().expect("undo should apply"));
    assert!(engine.nodes.get(child).unwrap().node_data().effective_enabled);

    assert!(engine.redo().expect("redo should apply"));
    assert!(!engine.nodes.get(child).unwrap().node_data().effective_enabled);
    assert_eq!(
        *ENABLED_CALLBACKS.lock().expect("callback log poisoned"),
        [false, true, false],
    );
}
