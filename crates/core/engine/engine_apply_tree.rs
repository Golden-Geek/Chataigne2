use crate::events::EventKind;
use crate::node::{Node, NodeId};

use super::engine_history::{AddNodeEffect, MoveNodeEffect, RemoveNodeEffect, ReplaceNodeEffect};
use super::{Engine, EngineEditError};

impl<T: Node> Engine<T> {
    /// Attaches `node` under `parent`, resolving insertion position from `prev_sibling`.
    ///
    /// When `prev_sibling` is `None`, insertion defaults to the end of the parent's child list.
    pub(crate) fn attach_node(&mut self, edit_index: usize, operation: &'static str, node: NodeId, parent: NodeId, prev_sibling: Option<NodeId>) -> Result<(), EngineEditError> {
        if !self.nodes.contains(parent) {
            return Err(EngineEditError::ParentNotFound { edit_index, operation, parent });
        }

        let effective_prev_sibling = match prev_sibling {
            Some(sibling) => {
                if sibling == node {
                    return Err(EngineEditError::InvalidSiblingReference { edit_index, operation, node, sibling });
                }

                let sibling_parent = self.nodes.get(sibling).ok_or(EngineEditError::SiblingNotFound { edit_index, operation, sibling })?.node_data().parent;

                if sibling_parent != Some(parent) {
                    return Err(EngineEditError::InvalidSiblingParent { edit_index, operation, parent, sibling, sibling_parent });
                }

                Some(sibling)
            }
            None => self.nodes.get(parent).and_then(|n| n.node_data().last_child),
        };

        let effective_next_sibling = match effective_prev_sibling {
            Some(prev) => self.nodes.get(prev).and_then(|n| n.node_data().next_sibling),
            None => self.nodes.get(parent).and_then(|n| n.node_data().first_child),
        };

        self.attach_node_between(edit_index, operation, node, parent, effective_prev_sibling, effective_next_sibling)
    }

    /// Attaches `node` under `parent` between explicitly provided sibling boundaries.
    ///
    /// Both `prev_sibling` and `next_sibling` are validated when present.
    pub(crate) fn attach_node_between(&mut self, edit_index: usize, operation: &'static str, node: NodeId, parent: NodeId, prev_sibling: Option<NodeId>, next_sibling: Option<NodeId>) -> Result<(), EngineEditError> {
        if !self.nodes.contains(parent) {
            return Err(EngineEditError::ParentNotFound { edit_index, operation, parent });
        }

        if let Some(prev) = prev_sibling {
            if prev == node {
                return Err(EngineEditError::InvalidSiblingReference { edit_index, operation, node, sibling: prev });
            }

            let prev_parent = self.nodes.get(prev).ok_or(EngineEditError::SiblingNotFound { edit_index, operation, sibling: prev })?.node_data().parent;
            if prev_parent != Some(parent) {
                return Err(EngineEditError::InvalidSiblingParent {
                    edit_index,
                    operation,
                    parent,
                    sibling: prev,
                    sibling_parent: prev_parent,
                });
            }
        }

        if let Some(next) = next_sibling {
            if next == node {
                return Err(EngineEditError::InvalidSiblingReference { edit_index, operation, node, sibling: next });
            }

            let next_parent = self.nodes.get(next).ok_or(EngineEditError::SiblingNotFound { edit_index, operation, sibling: next })?.node_data().parent;
            if next_parent != Some(parent) {
                return Err(EngineEditError::InvalidSiblingParent {
                    edit_index,
                    operation,
                    parent,
                    sibling: next,
                    sibling_parent: next_parent,
                });
            }
        }

        {
            let node_data = self.nodes.get_mut(node).ok_or(EngineEditError::NodeNotFound { edit_index, operation, node })?.node_data_mut();
            node_data.parent = Some(parent);
            node_data.prev_sibling = prev_sibling;
            node_data.next_sibling = next_sibling;
        }

        if let Some(prev) = prev_sibling {
            self.nodes.get_mut(prev).ok_or(EngineEditError::SiblingNotFound { edit_index, operation, sibling: prev })?.node_data_mut().next_sibling = Some(node);
        } else {
            self.nodes.get_mut(parent).ok_or(EngineEditError::ParentNotFound { edit_index, operation, parent })?.node_data_mut().first_child = Some(node);
        }

        if let Some(next) = next_sibling {
            self.nodes.get_mut(next).ok_or(EngineEditError::SiblingNotFound { edit_index, operation, sibling: next })?.node_data_mut().prev_sibling = Some(node);
        } else {
            self.nodes.get_mut(parent).ok_or(EngineEditError::ParentNotFound { edit_index, operation, parent })?.node_data_mut().last_child = Some(node);
        }

        self.mark_schedule_dirty();
        Ok(())
    }

    /// Detaches `node` from its parent sibling chain and returns the previous parent id.
    pub(crate) fn detach_node(&mut self, edit_index: usize, operation: &'static str, node: NodeId) -> Result<NodeId, EngineEditError> {
        let (parent, prev_sibling, next_sibling) = self.node_position(edit_index, operation, node)?;

        if let Some(prev) = prev_sibling {
            let prev_data = self.nodes.get_mut(prev).ok_or(EngineEditError::NodeNotFound { edit_index, operation, node: prev })?.node_data_mut();
            prev_data.next_sibling = next_sibling;
        } else {
            let parent_data = self.nodes.get_mut(parent).ok_or(EngineEditError::ParentNotFound { edit_index, operation, parent })?.node_data_mut();
            parent_data.first_child = next_sibling;
        }

        if let Some(next) = next_sibling {
            let next_data = self.nodes.get_mut(next).ok_or(EngineEditError::NodeNotFound { edit_index, operation, node: next })?.node_data_mut();
            next_data.prev_sibling = prev_sibling;
        } else {
            let parent_data = self.nodes.get_mut(parent).ok_or(EngineEditError::ParentNotFound { edit_index, operation, parent })?.node_data_mut();
            parent_data.last_child = prev_sibling;
        }

        {
            let node_data = self.nodes.get_mut(node).ok_or(EngineEditError::NodeNotFound { edit_index, operation, node })?.node_data_mut();
            node_data.parent = None;
            node_data.prev_sibling = None;
            node_data.next_sibling = None;
        }

        self.mark_schedule_dirty();
        Ok(parent)
    }

    /// Returns structural position information for `node` as `(parent, prev_sibling, next_sibling)`.
    ///
    /// Fails when the node is root or missing.
    pub(crate) fn node_position(&self, edit_index: usize, operation: &'static str, node: NodeId) -> Result<(NodeId, Option<NodeId>, Option<NodeId>), EngineEditError> {
        let data = self.nodes.get(node).ok_or(EngineEditError::NodeNotFound { edit_index, operation, node })?.node_data();

        let Some(parent) = data.parent else {
            return Err(EngineEditError::CannotMutateRoot { edit_index, operation, node });
        };

        Ok((parent, data.prev_sibling, data.next_sibling))
    }

    /// Collects all node ids in the subtree rooted at `root` using depth-first traversal.
    pub(crate) fn collect_subtree(&self, edit_index: usize, operation: &'static str, root: NodeId) -> Result<Vec<NodeId>, EngineEditError> {
        if !self.nodes.contains(root) {
            return Err(EngineEditError::NodeNotFound { edit_index, operation, node: root });
        }

        let mut out = Vec::new();
        let mut stack = vec![root];

        while let Some(node) = stack.pop() {
            out.push(node);

            let mut child = self.nodes.get(node).ok_or(EngineEditError::NodeNotFound { edit_index, operation, node })?.node_data().first_child;

            while let Some(child_id) = child {
                stack.push(child_id);
                child = self.nodes.get(child_id).ok_or(EngineEditError::NodeNotFound { edit_index, operation, node: child_id })?.node_data().next_sibling;
            }
        }

        Ok(out)
    }

    /// Rewrites `parent` for every node in a sibling chain starting at `first_child`.
    pub(crate) fn reparent_child_chain(&mut self, edit_index: usize, operation: &'static str, first_child: Option<NodeId>, parent: NodeId) -> Result<(), EngineEditError> {
        let mut child = first_child;
        while let Some(child_id) = child {
            let next = self.nodes.get(child_id).ok_or(EngineEditError::NodeNotFound { edit_index, operation, node: child_id })?.node_data().next_sibling;

            self.nodes.get_mut(child_id).ok_or(EngineEditError::NodeNotFound { edit_index, operation, node: child_id })?.node_data_mut().parent = Some(parent);

            child = next;
        }

        Ok(())
    }

    /// Returns `true` when `node` is equal to or under `ancestor` in the parent chain.
    fn is_descendant_of(&self, node: NodeId, ancestor: NodeId) -> bool {
        let mut cursor = Some(node);
        while let Some(current) = cursor {
            if current == ancestor {
                return true;
            }
            cursor = self.nodes.get(current).and_then(|n| n.node_data().parent);
        }
        false
    }

    /// Applies an add-node edit and returns history data required for undo/redo.
    pub(crate) fn apply_add_node(&mut self, edit_index: usize, node: Box<dyn Node>, parent: NodeId, prev_sibling: Option<NodeId>) -> Result<AddNodeEffect, EngineEditError> {
        const OP: &str = "AddNode";

        let mut node = self.coerce_node_for_engine(edit_index, OP, node)?;
        {
            let node_data = node.node_data_mut();
            node_data.parent = None;
            node_data.first_child = None;
            node_data.last_child = None;
            node_data.prev_sibling = None;
            node_data.next_sibling = None;
        }

        let child_id = self.nodes.insert(node);
        self.attach_node(edit_index, OP, child_id, parent, prev_sibling)?;

        let (attached_prev_sibling, attached_next_sibling) = {
            let attached_data = self.nodes.get(child_id).ok_or(EngineEditError::NodeNotFound { edit_index, operation: OP, node: child_id })?.node_data();
            (attached_data.prev_sibling, attached_data.next_sibling)
        };

        let decl_id = self.nodes.get(child_id).ok_or(EngineEditError::NodeNotFound { edit_index, operation: OP, node: child_id })?.node_data().meta.decl_id.clone();

        self.emit_event(EventKind::NodeCreated { node: child_id });
        self.emit_event(EventKind::ChildAdded {
            parent,
            child: child_id,
            decl_id,
        });

        Ok(AddNodeEffect {
            node: child_id,
            parent,
            prev_sibling: attached_prev_sibling,
            next_sibling: attached_next_sibling,
        })
    }

    /// Applies a replace-node edit and returns history data required for undo/redo.
    pub(crate) fn apply_replace_node(&mut self, edit_index: usize, node: NodeId, new_node: Box<dyn Node>) -> Result<ReplaceNodeEffect<T>, EngineEditError> {
        const OP: &str = "ReplaceNode";

        let old_data = self.nodes.get(node).ok_or(EngineEditError::NodeNotFound { edit_index, operation: OP, node })?.node_data().clone();

        let Some(parent) = old_data.parent else {
            return Err(EngineEditError::CannotMutateRoot { edit_index, operation: OP, node });
        };

        let mut replacement = self.coerce_node_for_engine(edit_index, OP, new_node)?;
        {
            let replacement_data = replacement.node_data_mut();
            replacement_data.id = node;
            replacement_data.parent = old_data.parent;
            replacement_data.first_child = old_data.first_child;
            replacement_data.last_child = old_data.last_child;
            replacement_data.prev_sibling = old_data.prev_sibling;
            replacement_data.next_sibling = old_data.next_sibling;
            replacement_data.meta.decl_id = old_data.meta.decl_id.clone();
        }

        let old_node = self.nodes.detach(node).ok_or(EngineEditError::NodeNotFound { edit_index, operation: OP, node })?;
        self.nodes.reattach(node, replacement);
        self.mark_schedule_dirty();

        self.emit_event(EventKind::ChildReplaced {
            parent,
            old: node,
            new: node,
            decl_id: old_data.meta.decl_id.clone(),
        });

        Ok(ReplaceNodeEffect {
            parent,
            old_id: node,
            new_id: node,
            prev_sibling: old_data.prev_sibling,
            next_sibling: old_data.next_sibling,
            old_node,
        })
    }

    /// Applies a remove-node edit and returns history data required for undo/redo.
    pub(crate) fn apply_remove_node(&mut self, edit_index: usize, node: NodeId) -> Result<RemoveNodeEffect<T>, EngineEditError> {
        const OP: &str = "RemoveNode";

        if node == self.root {
            return Err(EngineEditError::CannotMutateRoot { edit_index, operation: OP, node });
        }

        let (parent, prev_sibling, next_sibling) = self.node_position(edit_index, OP, node)?;
        let subtree = self.collect_subtree(edit_index, OP, node)?;
        self.detach_node(edit_index, OP, node)?;

        let mut detached_nodes = Vec::with_capacity(subtree.len());
        for removed in subtree.into_iter().rev() {
            let detached_node = self.nodes.detach(removed).ok_or(EngineEditError::NodeNotFound { edit_index, operation: OP, node: removed })?;
            detached_nodes.push((removed, detached_node));
            self.emit_event(EventKind::NodeDeleted { node: removed });
        }

        self.emit_event(EventKind::ChildRemoved { parent, child: node });

        Ok(RemoveNodeEffect {
            node,
            parent,
            prev_sibling,
            next_sibling,
            detached_nodes,
        })
    }

    /// Applies a move-node edit and returns history data required for undo/redo.
    pub(crate) fn apply_move_node(&mut self, edit_index: usize, node: NodeId, new_parent: NodeId, new_prev_sibling: Option<NodeId>) -> Result<MoveNodeEffect, EngineEditError> {
        const OP: &str = "MoveNode";

        if node == self.root {
            return Err(EngineEditError::CannotMutateRoot { edit_index, operation: OP, node });
        }

        if !self.nodes.contains(node) {
            return Err(EngineEditError::NodeNotFound { edit_index, operation: OP, node });
        }

        if !self.nodes.contains(new_parent) {
            return Err(EngineEditError::ParentNotFound { edit_index, operation: OP, parent: new_parent });
        }

        if node == new_parent || self.is_descendant_of(new_parent, node) {
            return Err(EngineEditError::CycleDetected { edit_index, operation: OP, node, new_parent });
        }

        if let Some(sibling) = new_prev_sibling {
            if sibling == node {
                return Err(EngineEditError::InvalidSiblingReference { edit_index, operation: OP, node, sibling });
            }
        }

        let (old_parent, old_prev_sibling, old_next_sibling) = self.node_position(edit_index, OP, node)?;

        let detached_parent = self.detach_node(edit_index, OP, node)?;
        self.attach_node(edit_index, OP, node, new_parent, new_prev_sibling)?;

        let new_node_data = self.nodes.get(node).ok_or(EngineEditError::NodeNotFound { edit_index, operation: OP, node })?.node_data();
        let new_prev_sibling = new_node_data.prev_sibling;
        let new_next_sibling = new_node_data.next_sibling;

        if detached_parent == new_parent {
            self.emit_event(EventKind::ChildReordered { parent: new_parent, child: node });
        } else {
            self.emit_event(EventKind::ChildMoved { child: node, old_parent: detached_parent, new_parent });
        }

        Ok(MoveNodeEffect {
            node,
            old_parent,
            old_prev_sibling,
            old_next_sibling,
            new_parent,
            new_prev_sibling,
            new_next_sibling,
        })
    }
}
