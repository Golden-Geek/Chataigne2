use crate::edit::Edit;
use crate::events::EventKind;
use crate::node::{DeclId, Node, NodeId};
use crate::process_ctx::ProcessCtx;

use super::node_handle::NodeHandle;

/// Declared optional child slot identified by parent + `decl_id`.
///
/// The slot may currently map to a runtime node id (`current`) or be absent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PotentialNodeHandle {
    parent: NodeId,
    decl_id: DeclId,
    current: Option<NodeId>,
    pending_create: bool,
}

impl PotentialNodeHandle {
    /// Creates an empty potential slot under `parent`.
    pub fn new(parent: NodeId, decl_id: impl Into<String>) -> Self {
        Self {
            parent,
            decl_id: DeclId(decl_id.into()),
            current: None,
            pending_create: false,
        }
    }

    /// Creates a potential slot with a known current runtime node id.
    pub fn with_current(parent: NodeId, decl_id: impl Into<String>, current: NodeId) -> Self {
        Self {
            parent,
            decl_id: DeclId(decl_id.into()),
            current: Some(current),
            pending_create: false,
        }
    }

    /// Returns the parent node id for this slot.
    pub fn parent(&self) -> NodeId {
        self.parent
    }

    /// Rebinds the parent node for this slot.
    pub fn set_parent(&mut self, parent: NodeId) {
        self.parent = parent;
    }

    /// Returns the declared slot id.
    pub fn decl_id(&self) -> &DeclId {
        &self.decl_id
    }

    /// Returns `true` when a runtime node is currently bound to this slot.
    pub fn is_present(&self) -> bool {
        self.current.is_some()
    }

    /// Returns `true` when this slot has queued creation and is waiting for `ChildAdded`.
    pub fn is_pending_create(&self) -> bool {
        self.pending_create
    }

    /// Returns the currently bound node id, when present.
    pub fn current_id(&self) -> Option<NodeId> {
        self.current
    }

    /// Returns a concrete node handle when this slot is present.
    pub fn get(&self) -> Option<NodeHandle> {
        self.current.map(NodeHandle::new)
    }

    /// Queues one typed mutation callback when this slot is currently present.
    ///
    /// Returns `true` when a callback was queued, `false` when this slot has no
    /// materialized runtime node yet.
    pub fn with_mut<N, F>(&self, ctx: &mut ProcessCtx, callback: F) -> bool
    where
        N: Node + 'static,
        F: FnOnce(&mut N, &mut ProcessCtx) + Send + 'static,
    {
        let Some(handle) = self.get() else {
            return false;
        };
        handle.with_mut::<N, F>(ctx, callback);
        true
    }

    /// Binds an existing node id as the current materialized slot value.
    pub fn bind_existing(&mut self, node: NodeId) {
        self.current = Some(node);
        self.pending_create = false;
    }

    /// Detaches local knowledge of the current node id without queuing edits.
    pub fn detach_current(&mut self) -> Option<NodeId> {
        self.pending_create = false;
        self.current.take()
    }

    /// Moves an existing node under this slot parent and marks it as current.
    pub fn attach_existing(&mut self, ctx: &mut ProcessCtx, node: NodeId, after: Option<NodeId>) {
        ctx.edits.push(Edit::MoveNode {
            node,
            new_parent: self.parent,
            new_prev_sibling: after,
        });
        self.current = Some(node);
        self.pending_create = false;
    }

    /// Removes the current slot node if present and clears local binding.
    pub fn clear(&mut self, ctx: &mut ProcessCtx) -> Option<NodeId> {
        let removed = self.current.take();
        if let Some(node) = removed {
            ctx.edits.push(Edit::RemoveNode { node });
        }
        self.pending_create = false;
        removed
    }

    /// Queues replacement (or creation) of this slot content.
    ///
    /// If a current node is known, `ReplaceNode` is queued. If absent, `AddNode` is
    /// queued under the slot parent.
    ///
    /// Replacement preserves the node id, so `current` stays bound when present.
    /// Creation still requires runtime id allocation, so `current` remains `None`.
    pub fn replace_with<N: Node + 'static>(&mut self, ctx: &mut ProcessCtx, new_node: N) {
        self.replace_with_boxed(ctx, Box::new(new_node));
    }

    /// Boxed variant of [`Self::replace_with`].
    pub fn replace_with_boxed(&mut self, ctx: &mut ProcessCtx, new_node: Box<dyn Node>) {
        if let Some(current) = self.current {
            ctx.replace_node_boxed(current, new_node);
            self.pending_create = false;
        } else {
            let mut new_node = new_node;
            new_node.node_data_mut().meta.decl_id = self.decl_id.clone();
            ctx.add_child_boxed(self.parent, new_node, None);
            self.current = None;
            self.pending_create = true;
        }
    }

    /// Reconciles a `ChildAdded` event and binds this slot when `decl_id` matches.
    pub fn reconcile_child_added(&mut self, parent: NodeId, child: NodeId, decl_id: &DeclId) -> bool {
        if parent == self.parent && decl_id == &self.decl_id {
            self.current = Some(child);
            self.pending_create = false;
            return true;
        }
        false
    }

    /// Reconciles a `ChildReplaced` event and binds this slot when `decl_id` matches.
    pub fn reconcile_child_replaced(&mut self, parent: NodeId, _old: NodeId, new: NodeId, decl_id: &DeclId) -> bool {
        if parent == self.parent && decl_id == &self.decl_id {
            self.current = Some(new);
            self.pending_create = false;
            return true;
        }
        false
    }

    /// Reconciles a `ChildRemoved` event and clears this slot when its current node is removed.
    pub fn reconcile_child_removed(&mut self, parent: NodeId, child: NodeId) -> bool {
        if parent == self.parent && self.current == Some(child) {
            self.current = None;
            self.pending_create = false;
            return true;
        }
        false
    }

    /// Reconciles this slot from a generic engine event.
    pub fn reconcile_event(&mut self, event: &EventKind) -> bool {
        match event {
            EventKind::ChildAdded { parent, child, decl_id } => self.reconcile_child_added(*parent, *child, decl_id),
            EventKind::ChildReplaced {
                parent,
                old,
                new,
                decl_id,
            } => self.reconcile_child_replaced(*parent, *old, *new, decl_id),
            EventKind::ChildRemoved { parent, child } => self.reconcile_child_removed(*parent, *child),
            _ => false,
        }
    }
}
