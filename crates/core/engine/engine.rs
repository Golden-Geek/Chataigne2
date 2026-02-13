use crate::edit::{Edit, EditQueue};
use crate::events::Inbox;
use crate::node::*;
use crate::process_ctx::ProcessCtx;
use serde::{Deserialize, Serialize};
pub mod node_store;
use node_store::NodeStore;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EngineTime {
    /// Monotonic engine tick counter. Increments only on EngineTick.
    pub tick: u64,

    /// Micro-step index within the same tick.
    /// 0 = main tick pass, 1.. = stabilisation rounds or flushImmediate rounds within that same tick.
    pub micro: u32,

    /// Total ordering within the same (tick, micro).
    pub seq: u32,
}

pub struct Engine<T: Node> {
    pub nodes: NodeStore<T>,
    pub root: NodeId,
    pub time: EngineTime,
    pub inbox: Inbox,
    pub edits: EditQueue,
}

impl<T: Node> Engine<T> {
    pub fn new(root: T) -> Self {
        let mut nodes: NodeStore<T> = NodeStore::new();
        let root = nodes.insert(root);

        Self {
            nodes,
            root,
            time: EngineTime { tick: 0, micro: 0, seq: 0 },
            inbox: Inbox::new(),
            edits: EditQueue::new(),
        }
    }

    pub fn add_node(&mut self, node: T, parent: Option<NodeId>) {
        self.edits.push(Edit::AddNode {
            parent: parent.unwrap_or(self.root),
            node: Box::new(node),
            prev_sibling: None,
        });
    }

    pub fn add_node_after(&mut self, node: T, sibling: NodeId) {
        let parent = self.nodes.get(sibling).and_then(|n| n.node_data().parent).unwrap_or(self.root);
        self.edits.push(Edit::AddNode {
            parent,
            prev_sibling: Some(sibling),
            node: Box::new(node),
        });
    }

    pub fn replace_node(&mut self, node: NodeId, new_node: T) {
        self.edits.push(Edit::ReplaceNode {
            node,
            new_node: Box::new(new_node),
        });
    }

    pub fn absorb_edits(&mut self, ctx: &mut ProcessCtx) {
        self.edits.pending.extend(ctx.edits.drain());
    }
}
