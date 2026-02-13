use crate::node::{Node, NodeId, NodeMetaPatch};
use crate::parameter::ParamValue;


pub enum Edit {
    SetParam { node: NodeId, value: ParamValue },
    RemoveNode { node: NodeId },
    MoveNode { node: NodeId, new_parent: NodeId, new_prev_sibling: Option<NodeId> },
    PatchMeta { node: NodeId, patch: NodeMetaPatch },
}

pub struct EditRequest {
    pub edit: Edit
}

#[derive(Default)]
pub struct EditQueue {
    pub pending: Vec<EditRequest>,
}

impl EditQueue {
    pub fn new() -> Self {
        Self { pending: Vec::new() }
    }

    pub fn push(&mut self, edit: Edit) { 
        self.pending.push(EditRequest { edit });
    }

    pub fn drain(&mut self) -> Vec<EditRequest> {
        std::mem::take(&mut self.pending)
    }
}

pub enum BuildEdit<T: Node> {
    AddNode { node: T, parent: NodeId, prev_sibling: Option<NodeId> },
    ReplaceNode { node: NodeId, new_node: T },
}

pub struct BuildEditRequest<T: Node> {
    pub edit: BuildEdit<T>
}

#[derive(Default)]
pub struct BuildEditQueue<T: Node> {
    pub pending: Vec<BuildEditRequest<T>>,
}

impl<T: Node> BuildEditQueue<T> {
    pub fn new() -> Self {
        Self { pending: Vec::new() }
    }

    pub fn push(&mut self, edit: BuildEdit<T>) {
        self.pending.push(BuildEditRequest {
            edit,
        });
    }

    pub fn drain(&mut self) -> Vec<BuildEditRequest<T>> {
        std::mem::take(&mut self.pending)
    }
}
