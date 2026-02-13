use crate::events::EventKind;
use crate::node::{Node, NodeId};

use super::{Engine, EngineEditError};

impl<T: Node> Engine<T> {
    fn attach_node(
        &mut self,
        edit_index: usize,
        operation: &'static str,
        node: NodeId,
        parent: NodeId,
        prev_sibling: Option<NodeId>,
    ) -> Result<(), EngineEditError> {
        if !self.nodes.contains(parent) {
            return Err(EngineEditError::ParentNotFound {
                edit_index,
                operation,
                parent,
            });
        }

        if let Some(sibling) = prev_sibling {
            if sibling == node {
                return Err(EngineEditError::InvalidSiblingReference {
                    edit_index,
                    operation,
                    node,
                    sibling,
                });
            }
        }

        let effective_prev_sibling = match prev_sibling {
            Some(sibling) => {
                let sibling_parent = self
                    .nodes
                    .get(sibling)
                    .ok_or(EngineEditError::SiblingNotFound {
                        edit_index,
                        operation,
                        sibling,
                    })?
                    .node_data()
                    .parent;

                if sibling_parent != Some(parent) {
                    return Err(EngineEditError::InvalidSiblingParent {
                        edit_index,
                        operation,
                        parent,
                        sibling,
                        sibling_parent,
                    });
                }

                Some(sibling)
            }
            None => self.nodes.get(parent).and_then(|n| n.node_data().last_child),
        };

        if let Some(prev) = effective_prev_sibling {
            let next = self.nodes.get(prev).and_then(|n| n.node_data().next_sibling);

            {
                let node_data = self
                    .nodes
                    .get_mut(node)
                    .ok_or(EngineEditError::NodeNotFound {
                        edit_index,
                        operation,
                        node,
                    })?
                    .node_data_mut();
                node_data.parent = Some(parent);
                node_data.prev_sibling = Some(prev);
                node_data.next_sibling = next;
            }

            {
                let prev_data = self
                    .nodes
                    .get_mut(prev)
                    .ok_or(EngineEditError::SiblingNotFound {
                        edit_index,
                        operation,
                        sibling: prev,
                    })?
                    .node_data_mut();
                prev_data.next_sibling = Some(node);
            }

            if let Some(next) = next {
                let next_data = self
                    .nodes
                    .get_mut(next)
                    .ok_or(EngineEditError::NodeNotFound {
                        edit_index,
                        operation,
                        node: next,
                    })?
                    .node_data_mut();
                next_data.prev_sibling = Some(node);
            } else {
                let parent_data = self
                    .nodes
                    .get_mut(parent)
                    .ok_or(EngineEditError::ParentNotFound {
                        edit_index,
                        operation,
                        parent,
                    })?
                    .node_data_mut();
                parent_data.last_child = Some(node);
                if parent_data.first_child.is_none() {
                    parent_data.first_child = Some(node);
                }
            }
        } else {
            let old_first = self.nodes.get(parent).and_then(|n| n.node_data().first_child);

            {
                let node_data = self
                    .nodes
                    .get_mut(node)
                    .ok_or(EngineEditError::NodeNotFound {
                        edit_index,
                        operation,
                        node,
                    })?
                    .node_data_mut();
                node_data.parent = Some(parent);
                node_data.prev_sibling = None;
                node_data.next_sibling = old_first;
            }

            {
                let parent_data = self
                    .nodes
                    .get_mut(parent)
                    .ok_or(EngineEditError::ParentNotFound {
                        edit_index,
                        operation,
                        parent,
                    })?
                    .node_data_mut();
                parent_data.first_child = Some(node);
                if old_first.is_none() {
                    parent_data.last_child = Some(node);
                }
            }

            if let Some(first_child) = old_first {
                let first_data = self
                    .nodes
                    .get_mut(first_child)
                    .ok_or(EngineEditError::NodeNotFound {
                        edit_index,
                        operation,
                        node: first_child,
                    })?
                    .node_data_mut();
                first_data.prev_sibling = Some(node);
            }
        }

        Ok(())
    }

    fn detach_node(
        &mut self,
        edit_index: usize,
        operation: &'static str,
        node: NodeId,
    ) -> Result<NodeId, EngineEditError> {
        let (parent, prev_sibling, next_sibling) = {
            let data = self
                .nodes
                .get(node)
                .ok_or(EngineEditError::NodeNotFound {
                    edit_index,
                    operation,
                    node,
                })?
                .node_data();

            let Some(parent) = data.parent else {
                return Err(EngineEditError::CannotMutateRoot {
                    edit_index,
                    operation,
                    node,
                });
            };

            (parent, data.prev_sibling, data.next_sibling)
        };

        if let Some(prev) = prev_sibling {
            let prev_data = self
                .nodes
                .get_mut(prev)
                .ok_or(EngineEditError::NodeNotFound {
                    edit_index,
                    operation,
                    node: prev,
                })?
                .node_data_mut();
            prev_data.next_sibling = next_sibling;
        } else {
            let parent_data = self
                .nodes
                .get_mut(parent)
                .ok_or(EngineEditError::ParentNotFound {
                    edit_index,
                    operation,
                    parent,
                })?
                .node_data_mut();
            parent_data.first_child = next_sibling;
        }

        if let Some(next) = next_sibling {
            let next_data = self
                .nodes
                .get_mut(next)
                .ok_or(EngineEditError::NodeNotFound {
                    edit_index,
                    operation,
                    node: next,
                })?
                .node_data_mut();
            next_data.prev_sibling = prev_sibling;
        } else {
            let parent_data = self
                .nodes
                .get_mut(parent)
                .ok_or(EngineEditError::ParentNotFound {
                    edit_index,
                    operation,
                    parent,
                })?
                .node_data_mut();
            parent_data.last_child = prev_sibling;
        }

        {
            let node_data = self
                .nodes
                .get_mut(node)
                .ok_or(EngineEditError::NodeNotFound {
                    edit_index,
                    operation,
                    node,
                })?
                .node_data_mut();
            node_data.parent = None;
            node_data.prev_sibling = None;
            node_data.next_sibling = None;
        }

        Ok(parent)
    }

    fn collect_subtree(
        &self,
        edit_index: usize,
        operation: &'static str,
        root: NodeId,
    ) -> Result<Vec<NodeId>, EngineEditError> {
        if !self.nodes.contains(root) {
            return Err(EngineEditError::NodeNotFound {
                edit_index,
                operation,
                node: root,
            });
        }

        let mut out = Vec::new();
        let mut stack = vec![root];

        while let Some(node) = stack.pop() {
            out.push(node);

            let mut child = self
                .nodes
                .get(node)
                .ok_or(EngineEditError::NodeNotFound {
                    edit_index,
                    operation,
                    node,
                })?
                .node_data()
                .first_child;

            while let Some(child_id) = child {
                stack.push(child_id);
                child = self
                    .nodes
                    .get(child_id)
                    .ok_or(EngineEditError::NodeNotFound {
                        edit_index,
                        operation,
                        node: child_id,
                    })?
                    .node_data()
                    .next_sibling;
            }
        }

        Ok(out)
    }

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

    pub(crate) fn apply_add_node(
        &mut self,
        edit_index: usize,
        node: Box<dyn Node>,
        parent: NodeId,
        prev_sibling: Option<NodeId>,
    ) -> Result<(), EngineEditError> {
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

        self.emit_event(EventKind::NodeCreated { node: child_id });
        self.emit_event(EventKind::ChildAdded {
            parent,
            child: child_id,
        });

        Ok(())
    }

    pub(crate) fn apply_replace_node(
        &mut self,
        edit_index: usize,
        node: NodeId,
        new_node: Box<dyn Node>,
    ) -> Result<(), EngineEditError> {
        const OP: &str = "ReplaceNode";

        let old_data = self
            .nodes
            .get(node)
            .ok_or(EngineEditError::NodeNotFound {
                edit_index,
                operation: OP,
                node,
            })?
            .node_data()
            .clone();

        let Some(parent) = old_data.parent else {
            return Err(EngineEditError::CannotMutateRoot {
                edit_index,
                operation: OP,
                node,
            });
        };

        let mut replacement = self.coerce_node_for_engine(edit_index, OP, new_node)?;
        {
            let replacement_data = replacement.node_data_mut();
            replacement_data.parent = old_data.parent;
            replacement_data.first_child = old_data.first_child;
            replacement_data.last_child = old_data.last_child;
            replacement_data.prev_sibling = old_data.prev_sibling;
            replacement_data.next_sibling = old_data.next_sibling;
        }

        let new_id = self.nodes.insert(replacement);

        {
            let parent_data = self
                .nodes
                .get_mut(parent)
                .ok_or(EngineEditError::ParentNotFound {
                    edit_index,
                    operation: OP,
                    parent,
                })?
                .node_data_mut();
            if parent_data.first_child == Some(node) {
                parent_data.first_child = Some(new_id);
            }
            if parent_data.last_child == Some(node) {
                parent_data.last_child = Some(new_id);
            }
        }

        if let Some(prev) = old_data.prev_sibling {
            let prev_data = self
                .nodes
                .get_mut(prev)
                .ok_or(EngineEditError::NodeNotFound {
                    edit_index,
                    operation: OP,
                    node: prev,
                })?
                .node_data_mut();
            prev_data.next_sibling = Some(new_id);
        }

        if let Some(next) = old_data.next_sibling {
            let next_data = self
                .nodes
                .get_mut(next)
                .ok_or(EngineEditError::NodeNotFound {
                    edit_index,
                    operation: OP,
                    node: next,
                })?
                .node_data_mut();
            next_data.prev_sibling = Some(new_id);
        }

        let mut child = old_data.first_child;
        while let Some(child_id) = child {
            let next = self
                .nodes
                .get(child_id)
                .ok_or(EngineEditError::NodeNotFound {
                    edit_index,
                    operation: OP,
                    node: child_id,
                })?
                .node_data()
                .next_sibling;

            self.nodes
                .get_mut(child_id)
                .ok_or(EngineEditError::NodeNotFound {
                    edit_index,
                    operation: OP,
                    node: child_id,
                })?
                .node_data_mut()
                .parent = Some(new_id);

            child = next;
        }

        self.nodes
            .remove(node)
            .ok_or(EngineEditError::NodeNotFound {
                edit_index,
                operation: OP,
                node,
            })?;

        self.emit_event(EventKind::NodeCreated { node: new_id });
        self.emit_event(EventKind::ChildReplaced {
            parent,
            old: node,
            new: new_id,
        });
        self.emit_event(EventKind::NodeDeleted { node });

        Ok(())
    }

    pub(crate) fn apply_remove_node(
        &mut self,
        edit_index: usize,
        node: NodeId,
    ) -> Result<(), EngineEditError> {
        const OP: &str = "RemoveNode";

        if node == self.root {
            return Err(EngineEditError::CannotMutateRoot {
                edit_index,
                operation: OP,
                node,
            });
        }

        let subtree = self.collect_subtree(edit_index, OP, node)?;
        let parent = self.detach_node(edit_index, OP, node)?;

        for removed in subtree.into_iter().rev() {
            self.nodes
                .remove(removed)
                .ok_or(EngineEditError::NodeNotFound {
                    edit_index,
                    operation: OP,
                    node: removed,
                })?;
            self.emit_event(EventKind::NodeDeleted { node: removed });
        }

        self.emit_event(EventKind::ChildRemoved {
            parent,
            child: node,
        });

        Ok(())
    }

    pub(crate) fn apply_move_node(
        &mut self,
        edit_index: usize,
        node: NodeId,
        new_parent: NodeId,
        new_prev_sibling: Option<NodeId>,
    ) -> Result<(), EngineEditError> {
        const OP: &str = "MoveNode";

        if node == self.root {
            return Err(EngineEditError::CannotMutateRoot {
                edit_index,
                operation: OP,
                node,
            });
        }

        if !self.nodes.contains(node) {
            return Err(EngineEditError::NodeNotFound {
                edit_index,
                operation: OP,
                node,
            });
        }

        if !self.nodes.contains(new_parent) {
            return Err(EngineEditError::ParentNotFound {
                edit_index,
                operation: OP,
                parent: new_parent,
            });
        }

        if node == new_parent || self.is_descendant_of(new_parent, node) {
            return Err(EngineEditError::CycleDetected {
                edit_index,
                operation: OP,
                node,
                new_parent,
            });
        }

        if let Some(sibling) = new_prev_sibling {
            if sibling == node {
                return Err(EngineEditError::InvalidSiblingReference {
                    edit_index,
                    operation: OP,
                    node,
                    sibling,
                });
            }
        }

        let old_parent = self.detach_node(edit_index, OP, node)?;
        self.attach_node(edit_index, OP, node, new_parent, new_prev_sibling)?;

        if old_parent == new_parent {
            self.emit_event(EventKind::ChildReordered {
                parent: new_parent,
                child: node,
            });
        } else {
            self.emit_event(EventKind::ChildMoved {
                child: node,
                old_parent,
                new_parent,
            });
        }

        Ok(())
    }
}
