use crate::edit::{BuildEdit, BuildEditQueue, EditOrigin, Propagation};
use crate::events::Inbox;
use crate::node::*;
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
    pub build_edits: BuildEditQueue<T>,
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
            build_edits: BuildEditQueue::new(),
        }
    }
    
    pub fn add_node(&mut self, parent: NodeId, node: T, propagation: Propagation, origin: EditOrigin) {
        self.build_edits.push(BuildEdit::AddNode { parent, node }, propagation, origin);
    }

    pub fn replace_node(&mut self, node: NodeId, new_node: T, propagation: Propagation, origin: EditOrigin) {
        self.build_edits.push(BuildEdit::ReplaceNode { node, new_node }, propagation, origin);
    }
}
