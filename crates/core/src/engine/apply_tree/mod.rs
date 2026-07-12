pub(super) use std::collections::HashSet;
pub(super) use std::sync::Arc;

pub(super) use crate::edit::NodeTree;
pub(super) use crate::events::EventKind;
pub(super) use crate::node::{DeclId, Node, NodeCreationContext, NodeId, NodeUserPermissions, UserNodeRole};
pub(super) use crate::process_ctx::{ExecutionPhase, ProcessCtx};
pub(super) use crate::ui_sync::UiGraphOp;

pub(super) use super::history::{AddNodeEffect, MoveNodeEffect, RemoveNodeEffect, ReplaceNodeEffect};
pub(super) use super::{Engine, EngineEditError};

pub(super) struct PendingNodeTree<T: Node> {
    pub(super) node: T,
    pub(super) children: Vec<PendingNodeTree<T>>,
}

pub(super) struct InsertedNode {
    pub(super) id: NodeId,
    pub(super) parent: NodeId,
    pub(super) decl_id: DeclId,
}

mod insert;
mod lifecycle;
mod r#move;
mod remove;
mod replace;
mod structure;
mod validation;
