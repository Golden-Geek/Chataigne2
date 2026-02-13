use crate::node::{Node, NodeId, NodeMetaPatch};
use crate::events::CustomEvent;
use crate::parameter::ParamValue;


pub enum Edit {
    SetParam { node: NodeId, value: ParamValue },
    AddNode { node: Box<dyn Node>, parent: NodeId, prev_sibling: Option<NodeId> },
    ReplaceNode { node: NodeId, new_node: Box<dyn Node> },
    RemoveNode { node: NodeId },
    MoveNode { node: NodeId, new_parent: NodeId, new_prev_sibling: Option<NodeId> },
    PatchMeta { node: NodeId, patch: NodeMetaPatch },
    EmitCustomEvent { event: CustomEvent },
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
