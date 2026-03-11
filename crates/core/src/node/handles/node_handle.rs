use crate::edit::Edit;
use crate::node::{Node, NodeId, NodeMetaPatch};
use crate::process_ctx::ProcessCtx;

/// Handle to an existing node id with structural edit helpers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NodeHandle {
    node: NodeId,
}

impl NodeHandle {
    /// Creates a node handle from an existing node id.
    pub fn new(node: NodeId) -> Self {
        Self { node }
    }

    /// Returns the wrapped node id.
    pub fn id(&self) -> NodeId {
        self.node
    }

    /// Queues removal of this node.
    pub fn remove(&self, ctx: &mut ProcessCtx) {
        ctx.edits.push(Edit::RemoveNode { node: self.node });
    }

    /// Queues movement of this node under a new parent.
    pub fn move_to(&self, ctx: &mut ProcessCtx, new_parent: NodeId, after: Option<NodeId>) {
        ctx.edits.push(Edit::MoveNode {
            node: self.node,
            new_parent,
            new_prev_sibling: after,
        });
    }

    /// Queues replacement of this node id by a typed node value.
    pub fn replace_with<N: Node + 'static>(&self, ctx: &mut ProcessCtx, new_node: N) {
        ctx.replace_node(self.node, new_node);
    }

    /// Queues replacement of this node id by a boxed node value.
    pub fn replace_with_boxed(&self, ctx: &mut ProcessCtx, new_node: Box<dyn Node>) {
        ctx.replace_node_boxed(self.node, new_node);
    }

    /// Sets or replaces the default warning on this node.
    pub fn set_warning(&self, ctx: &mut ProcessCtx, message: impl Into<String>) {
        ctx.set_node_warning(self.node, message);
    }

    /// Sets or replaces one warning on this node.
    ///
    /// `warning_id = None` uses the default empty warning id.
    pub fn set_warning_with(
        &self,
        ctx: &mut ProcessCtx,
        warning_id: Option<&str>,
        message: impl Into<String>,
        detail: Option<&str>,
    ) {
        ctx.set_node_warning_with(self.node, warning_id, message, detail);
    }

    /// Clears one warning by id on this node.
    ///
    /// `warning_id = None` clears all warnings.
    pub fn clear_warning(&self, ctx: &mut ProcessCtx, warning_id: Option<&str>) {
        ctx.clear_node_warning(self.node, warning_id);
    }

    /// Clears all warnings on this node.
    pub fn clear_warnings(&self, ctx: &mut ProcessCtx) {
        ctx.clear_all_node_warnings(self.node);
    }

    /// Sets descendant warning surfacing depth on this node.
    pub fn set_child_warning_depth(&self, ctx: &mut ProcessCtx, max_depth: u32) {
        ctx.set_node_child_warning_depth(self.node, max_depth);
    }

    /// Queues a metadata patch on this node.
    pub fn patch_meta(&self, ctx: &mut ProcessCtx, patch: NodeMetaPatch) {
        ctx.patch_node_meta(self.node, patch);
    }

    /// Queues one typed mutation callback for this node.
    ///
    /// The callback executes during edit application and receives the concrete
    /// node value plus a mutable process context for queuing follow-up edits.
    pub fn with_mut<N, F>(&self, ctx: &mut ProcessCtx, callback: F)
    where
        N: Node + 'static,
        F: FnOnce(&mut N, &mut ProcessCtx) + Send + 'static,
    {
        let target = self.node;
        ctx.call_node_mutation(target, move |node, child_ctx| {
            let node_type = node.get_type().to_string();
            let Some(typed) = node.as_any_mut().downcast_mut::<N>() else {
                return Err(format!(
                    "node {:?} is type '{}' and cannot be downcast to {}",
                    target,
                    node_type,
                    std::any::type_name::<N>()
                ));
            };
            callback(typed, child_ctx);
            Ok(())
        });
    }
}
