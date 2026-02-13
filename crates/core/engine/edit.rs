use crate::node::{Node, NodeId, NodeMetaPatch};
use crate::parameter::ParamValue;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Propagation {
    Immediate,
    EndOfTick,
    NextTick,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditOrigin {
    UI,
    Network,
    Script,
    Internal,
}

pub enum Edit {
    SetParam { node: NodeId, value: ParamValue },
    RemoveNode { node: NodeId },
    MoveNode { node: NodeId, new_parent: NodeId, new_prev_sibling: Option<NodeId> },
    PatchMeta { node: NodeId, patch: NodeMetaPatch },
}

pub struct EditRequest {
    pub edit: Edit,
    pub propagation: Propagation,
    pub origin: EditOrigin,
}

#[derive(Default)]
pub struct EditQueue {
    pub pending: Vec<EditRequest>,
}

impl EditQueue {
    pub fn new() -> Self {
        Self { pending: Vec::new() }
    }

    pub fn push(&mut self, edit: Edit, propagation: Propagation, origin: EditOrigin) {
        self.pending.push(EditRequest { edit, propagation, origin });
    }

    pub fn drain(&mut self) -> Vec<EditRequest> {
        std::mem::take(&mut self.pending)
    }
}

pub enum BuildEdit<T: Node> {
    AddNode { parent: NodeId, node: T },
    ReplaceNode { node: NodeId, new_node: T },
}

pub struct BuildEditRequest<T: Node> {
    pub edit: BuildEdit<T>,
    pub propagation: Propagation,
    pub origin: EditOrigin,
}

#[derive(Default)]
pub struct BuildEditQueue<T: Node> {
    pub pending: Vec<BuildEditRequest<T>>,
}

impl<T: Node> BuildEditQueue<T> {
    pub fn new() -> Self {
        Self { pending: Vec::new() }
    }

    pub fn push(&mut self, edit: BuildEdit<T>, propagation: Propagation, origin: EditOrigin) {
        self.pending.push(BuildEditRequest {
            edit,
            propagation,
            origin,
        });
    }

    pub fn drain(&mut self) -> Vec<BuildEditRequest<T>> {
        std::mem::take(&mut self.pending)
    }
}
