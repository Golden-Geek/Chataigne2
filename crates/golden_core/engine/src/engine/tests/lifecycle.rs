use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;
use crate::{
    edit::{Edit, NodeTree},
    node::{Folder, Node, NodeCreationContext},
    process_ctx::ProcessCtx,
};

static READY_WITH_EXTERNAL_SENDER_COUNT: AtomicUsize = AtomicUsize::new(0);

#[crate::node("ready_external_sender_probe")]
struct ReadyExternalSenderProbe {}

#[crate::node("ready_external_sender_probe", from_struct)]
impl Node for ReadyExternalSenderProbe {
    fn on_node_ready(&mut self, ctx: &mut ProcessCtx, _context: NodeCreationContext) {
        if ctx.external_edit_sender().is_some() {
            READY_WITH_EXTERNAL_SENDER_COUNT.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn lifecycle_requires_tree_snapshot(&self) -> bool {
        false
    }
}

crate::define_node_enum!(
    enum LifecycleTestNode {
        ReadyExternalSenderProbe,
    }
);

#[test]
fn batched_ready_callbacks_receive_external_edit_sender() {
    READY_WITH_EXTERNAL_SENDER_COUNT.store(0, Ordering::SeqCst);
    let root: LifecycleTestNode = Folder::new("root".to_owned()).into();
    let mut engine = Engine::new(root);

    engine.edits.push(Edit::AddNodeTree {
        parent: engine.root,
        prev_sibling: None,
        tree: NodeTree::new(ReadyExternalSenderProbe::new()),
    });
    engine.apply_edits().expect("batched node tree should be added");

    assert_eq!(READY_WITH_EXTERNAL_SENDER_COUNT.load(Ordering::SeqCst), 1);
}
