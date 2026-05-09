use std::collections::HashSet;
use std::sync::Arc;

use crate::edit::NodeTree;
use crate::events::EventKind;
use crate::node::{DeclId, Node, NodeCreationContext, NodeId, NodeUserPermissions, UserNodeRole};
use crate::process_ctx::{ExecutionPhase, ProcessCtx};
use crate::ui_sync::UiGraphOp;

use super::history::{AddNodeEffect, MoveNodeEffect, RemoveNodeEffect, ReplaceNodeEffect};
use super::{Engine, EngineEditError};

struct PendingNodeTree<T: Node> {
    node: T,
    children: Vec<PendingNodeTree<T>>,
}

struct InsertedNode {
    id: NodeId,
    parent: NodeId,
    decl_id: DeclId,
}

impl<T: Node> Engine<T> {
    /// Attaches `node` under `parent`, resolving insertion position from `prev_sibling`.
    ///
    /// When `prev_sibling` is `None`, insertion defaults to the end of the parent's child list.
    pub(crate) fn attach_node(
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

        let effective_prev_sibling = match prev_sibling {
            Some(sibling) => {
                if sibling == node {
                    return Err(EngineEditError::InvalidSiblingReference {
                        edit_index,
                        operation,
                        node,
                        sibling,
                    });
                }

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

        let effective_next_sibling = match effective_prev_sibling {
            Some(prev) => self.nodes.get(prev).and_then(|n| n.node_data().next_sibling),
            None => self.nodes.get(parent).and_then(|n| n.node_data().first_child),
        };

        self.attach_node_between(
            edit_index,
            operation,
            node,
            parent,
            effective_prev_sibling,
            effective_next_sibling,
        )
    }

    /// Attaches `node` under `parent` between explicitly provided sibling boundaries.
    ///
    /// Both `prev_sibling` and `next_sibling` are validated when present.
    pub(crate) fn attach_node_between(
        &mut self,
        edit_index: usize,
        operation: &'static str,
        node: NodeId,
        parent: NodeId,
        prev_sibling: Option<NodeId>,
        next_sibling: Option<NodeId>,
    ) -> Result<(), EngineEditError> {
        if !self.nodes.contains(parent) {
            return Err(EngineEditError::ParentNotFound {
                edit_index,
                operation,
                parent,
            });
        }

        if let Some(prev) = prev_sibling {
            if prev == node {
                return Err(EngineEditError::InvalidSiblingReference {
                    edit_index,
                    operation,
                    node,
                    sibling: prev,
                });
            }

            let prev_parent = self
                .nodes
                .get(prev)
                .ok_or(EngineEditError::SiblingNotFound {
                    edit_index,
                    operation,
                    sibling: prev,
                })?
                .node_data()
                .parent;
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
                return Err(EngineEditError::InvalidSiblingReference {
                    edit_index,
                    operation,
                    node,
                    sibling: next,
                });
            }

            let next_parent = self
                .nodes
                .get(next)
                .ok_or(EngineEditError::SiblingNotFound {
                    edit_index,
                    operation,
                    sibling: next,
                })?
                .node_data()
                .parent;
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
            node_data.prev_sibling = prev_sibling;
            node_data.next_sibling = next_sibling;
        }

        if let Some(prev) = prev_sibling {
            self.nodes
                .get_mut(prev)
                .ok_or(EngineEditError::SiblingNotFound {
                    edit_index,
                    operation,
                    sibling: prev,
                })?
                .node_data_mut()
                .next_sibling = Some(node);
        } else {
            self.nodes
                .get_mut(parent)
                .ok_or(EngineEditError::ParentNotFound {
                    edit_index,
                    operation,
                    parent,
                })?
                .node_data_mut()
                .first_child = Some(node);
        }

        if let Some(next) = next_sibling {
            self.nodes
                .get_mut(next)
                .ok_or(EngineEditError::SiblingNotFound {
                    edit_index,
                    operation,
                    sibling: next,
                })?
                .node_data_mut()
                .prev_sibling = Some(node);
        } else {
            self.nodes
                .get_mut(parent)
                .ok_or(EngineEditError::ParentNotFound {
                    edit_index,
                    operation,
                    parent,
                })?
                .node_data_mut()
                .last_child = Some(node);
        }

        self.mark_schedule_dirty();
        Ok(())
    }

    /// Detaches `node` from its parent sibling chain and returns the previous parent id.
    pub(crate) fn detach_node(
        &mut self,
        edit_index: usize,
        operation: &'static str,
        node: NodeId,
    ) -> Result<NodeId, EngineEditError> {
        let (parent, prev_sibling, next_sibling) = self.node_position(edit_index, operation, node)?;

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

        self.mark_schedule_dirty();
        Ok(parent)
    }

    /// Returns structural position information for `node` as `(parent, prev_sibling, next_sibling)`.
    ///
    /// Fails when the node is root or missing.
    pub(crate) fn node_position(
        &self,
        edit_index: usize,
        operation: &'static str,
        node: NodeId,
    ) -> Result<(NodeId, Option<NodeId>, Option<NodeId>), EngineEditError> {
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

        Ok((parent, data.prev_sibling, data.next_sibling))
    }

    /// Collects all node ids in the subtree rooted at `root` using depth-first traversal.
    pub(crate) fn collect_subtree(
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

    /// Rewrites `parent` for every node in a sibling chain starting at `first_child`.
    pub(crate) fn reparent_child_chain(
        &mut self,
        edit_index: usize,
        operation: &'static str,
        first_child: Option<NodeId>,
        parent: NodeId,
    ) -> Result<(), EngineEditError> {
        let mut child = first_child;
        while let Some(child_id) = child {
            let next = self
                .nodes
                .get(child_id)
                .ok_or(EngineEditError::NodeNotFound {
                    edit_index,
                    operation,
                    node: child_id,
                })?
                .node_data()
                .next_sibling;

            self.nodes
                .get_mut(child_id)
                .ok_or(EngineEditError::NodeNotFound {
                    edit_index,
                    operation,
                    node: child_id,
                })?
                .node_data_mut()
                .parent = Some(parent);

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

    fn nearest_container_ancestor(&self, start: NodeId) -> Option<NodeId> {
        let mut cursor = Some(start);
        while let Some(node_id) = cursor {
            let node = self.nodes.get(node_id)?;
            if node.user_container_rules().is_some()
                || node.script_host_policy().is_some_and(|policy| policy.enabled)
                || node.user_context_host_policy().is_some_and(|policy| policy.enabled)
            {
                return Some(node_id);
            }
            cursor = node.node_data().parent;
        }
        None
    }

    fn has_item_root_ancestor(&self, start: NodeId) -> bool {
        let mut cursor = Some(start);
        while let Some(node_id) = cursor {
            let Some(node) = self.nodes.get(node_id) else {
                return false;
            };

            if node.node_data().user_role == UserNodeRole::ItemRoot {
                return true;
            }

            cursor = node.node_data().parent;
        }

        false
    }

    fn default_user_permissions_for_new_node(
        &self,
        node: &T,
        parent: NodeId,
        user_role: UserNodeRole,
    ) -> NodeUserPermissions {
        let mut permissions = if node
            .node_data()
            .meta
            .tags
            .iter()
            .any(|tag| tag == "is_user_made" || tag == "name_changeable")
        {
            NodeUserPermissions::all()
        } else {
            let is_managed_item = user_role == UserNodeRole::ItemRoot;
            let is_folder_under_manager = node.get_type() == "folder"
                && self.nearest_container_ancestor(parent).is_some()
                && !self.has_item_root_ancestor(parent);

            if is_managed_item || is_folder_under_manager {
                NodeUserPermissions::all()
            } else {
                NodeUserPermissions::default()
            }
        };

        if node.is_declared_user_item() {
            permissions.can_remove_and_duplicate = true;
            permissions.can_edit_tags = true;
            permissions.can_edit_color = true;
        }

        permissions
    }

    fn prepare_node_for_insert(&self, node: &mut T, parent: NodeId, user_role: UserNodeRole) {
        let inferred_permissions = self.default_user_permissions_for_new_node(node, parent, user_role);
        let node_data = node.node_data_mut();
        node_data.parent = None;
        node_data.first_child = None;
        node_data.last_child = None;
        node_data.prev_sibling = None;
        node_data.next_sibling = None;
        node_data.user_role = user_role;
        if node_data.meta.user_permissions == NodeUserPermissions::default() {
            node_data.meta.user_permissions = inferred_permissions;
        }
    }

    fn ensure_item_kind_allowed(
        &self,
        edit_index: usize,
        operation: &'static str,
        container: NodeId,
        item_type: &str,
        item_kind: &str,
    ) -> Result<(), EngineEditError> {
        let container_node = self.nodes.get(container).ok_or(EngineEditError::NodeNotFound {
            edit_index,
            operation,
            node: container,
        })?;

        let container_type = container_node.get_type().to_string();
        if container_node.user_container_accepts_item(item_type, item_kind) {
            return Ok(());
        }

        let has_user_item_capability =
            container_node.user_container_rules().is_some() || !container_node.user_creatable_items().is_empty();
        if !has_user_item_capability {
            return Err(EngineEditError::UserItemContainerRequired {
                edit_index,
                operation,
                parent: container,
            });
        }

        Err(EngineEditError::UserItemKindRejected {
            edit_index,
            operation,
            container,
            container_type,
            item_type: item_type.to_string(),
            item_kind: item_kind.to_string(),
        })
    }

    fn validate_item_roots_for_move(
        &self,
        edit_index: usize,
        operation: &'static str,
        node: NodeId,
        new_parent: NodeId,
    ) -> Result<(), EngineEditError> {
        let subtree = self.collect_subtree(edit_index, operation, node)?;
        let subtree_set: HashSet<NodeId> = subtree.iter().copied().collect();
        let target_container = self.nearest_container_ancestor(new_parent);

        for moved in subtree {
            let moved_node = self.nodes.get(moved).ok_or(EngineEditError::NodeNotFound {
                edit_index,
                operation,
                node: moved,
            })?;

            if moved_node.node_data().user_role != UserNodeRole::ItemRoot {
                continue;
            }

            let old_container = self.nearest_container_ancestor(moved);
            let container_changes = match old_container {
                Some(container) => !subtree_set.contains(&container),
                None => target_container.is_some(),
            };

            if !container_changes {
                continue;
            }

            let Some(target_container) = target_container else {
                return Err(EngineEditError::UserItemContainerRequired {
                    edit_index,
                    operation,
                    parent: new_parent,
                });
            };

            self.ensure_item_kind_allowed(
                edit_index,
                operation,
                target_container,
                moved_node.get_type(),
                moved_node.user_item_kind(),
            )?;
        }

        Ok(())
    }

    fn coerce_pending_node_tree(
        &self,
        edit_index: usize,
        operation: &'static str,
        tree: NodeTree,
    ) -> Result<PendingNodeTree<T>, EngineEditError> {
        let node = self.coerce_node_for_engine(edit_index, operation, tree.node)?;
        let mut children = Vec::with_capacity(tree.children.len());
        for child in tree.children {
            children.push(self.coerce_pending_node_tree(edit_index, operation, child)?);
        }
        Ok(PendingNodeTree { node, children })
    }

    fn insert_pending_node_tree(
        &mut self,
        edit_index: usize,
        operation: &'static str,
        mut tree: PendingNodeTree<T>,
        parent: NodeId,
        prev_sibling: Option<NodeId>,
        user_role: UserNodeRole,
        inserted: &mut Vec<InsertedNode>,
    ) -> Result<NodeId, EngineEditError> {
        self.prepare_node_for_insert(&mut tree.node, parent, user_role);
        let node_id = self.nodes.insert(tree.node);
        self.attach_node(edit_index, operation, node_id, parent, prev_sibling)?;
        self.populate_param_cache_entry(node_id);
        let decl_id = self
            .nodes
            .get(node_id)
            .ok_or(EngineEditError::NodeNotFound {
                edit_index,
                operation,
                node: node_id,
            })?
            .node_data()
            .meta
            .decl_id
            .clone();
        inserted.push(InsertedNode {
            id: node_id,
            parent,
            decl_id,
        });

        let mut child_prev_sibling = None;
        for child in tree.children {
            let child_id = self.insert_pending_node_tree(
                edit_index,
                operation,
                child,
                node_id,
                child_prev_sibling,
                UserNodeRole::Regular,
                inserted,
            )?;
            child_prev_sibling = Some(child_id);
        }

        Ok(node_id)
    }

    pub(crate) fn run_node_attached_for_batch(
        &mut self,
        node_ids: &[NodeId],
        creation_context: Option<NodeCreationContext>,
    ) -> Result<(), EngineEditError> {
        if node_ids.is_empty() {
            return Ok(());
        }

        let event_cursor = self.inbox.events.len();
        let tree_snapshot = self.build_process_tree_snapshot();
        for node_id in node_ids.iter().copied() {
            let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, self.time);
            ctx.runtime_elapsed = self.runtime_elapsed;
            ctx.set_tree_snapshot(Arc::clone(&tree_snapshot));

            if let Some(node) = self.nodes.get_mut(node_id) {
                crate::logger::with_node_origin(node_id, || {
                    node.engine_on_attached(&mut ctx);
                });
            }
            self.absorb_edits(&mut ctx)?;
        }
        if self.stabilization_scope_depth == 0 {
            self.stabilize_added_node_structure(event_cursor, creation_context)?;
        }

        Ok(())
    }

    pub(crate) fn run_node_init_for_batch(
        &mut self,
        node_ids: &[NodeId],
        creation_context: Option<NodeCreationContext>,
    ) -> Result<(), EngineEditError> {
        if node_ids.is_empty() {
            return Ok(());
        }

        let event_cursor = self.inbox.events.len();
        let tree_snapshot = self.build_process_tree_snapshot();
        for node_id in node_ids.iter().copied() {
            let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, self.time);
            ctx.runtime_elapsed = self.runtime_elapsed;
            ctx.set_tree_snapshot(Arc::clone(&tree_snapshot));

            if let Some(node) = self.nodes.get_mut(node_id) {
                crate::logger::with_node_origin(node_id, || {
                    node.init(&mut ctx);
                });
            }
            self.absorb_edits(&mut ctx)?;
        }
        if self.stabilization_scope_depth == 0 {
            self.stabilize_added_node_structure(event_cursor, creation_context)?;
        }

        Ok(())
    }

    pub(crate) fn run_node_ready_for_batch(
        &mut self,
        node_ids: &[NodeId],
        creation_context: NodeCreationContext,
    ) -> Result<(), EngineEditError> {
        if node_ids.is_empty() {
            return Ok(());
        }

        let event_cursor = self.inbox.events.len();
        let tree_snapshot = self.build_process_tree_snapshot();
        for node_id in node_ids.iter().copied() {
            let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, self.time);
            ctx.runtime_elapsed = self.runtime_elapsed;
            ctx.set_tree_snapshot(Arc::clone(&tree_snapshot));

            if let Some(node) = self.nodes.get_mut(node_id) {
                crate::logger::with_node_origin(node_id, || {
                    node.on_node_ready(&mut ctx, creation_context);
                });
            }
            self.absorb_edits(&mut ctx)?;
        }
        if self.stabilization_scope_depth == 0 {
            self.stabilize_added_node_structure(event_cursor, Some(creation_context))?;
        }

        Ok(())
    }

    /// Applies queued structural side effects and preprocesses newly emitted events
    /// until the add/bootstrap pipeline reaches a fixed point.
    pub(crate) fn stabilize_added_node_structure(
        &mut self,
        mut event_cursor: usize,
        creation_context: Option<NodeCreationContext>,
    ) -> Result<(), EngineEditError> {
        self.stabilization_scope_depth = self.stabilization_scope_depth.saturating_add(1);
        let result = (|| -> Result<(), EngineEditError> {
            loop {
                while !self.edits.pending.is_empty() {
                    self.apply_edits_internal(false, creation_context)?;
                }

                let precomputed = self.precompute_inbox_dispatch_since(event_cursor);
                event_cursor = self.inbox.events.len();

                if precomputed.is_empty() {
                    if self.edits.pending.is_empty() {
                        break;
                    }
                    continue;
                }

                self.preprocess_precomputed_inbox(ExecutionPhase::EngineTick, precomputed)?;
            }

            Ok(())
        })();
        self.stabilization_scope_depth = self.stabilization_scope_depth.saturating_sub(1);
        result
    }

    fn apply_add_node_with_role(
        &mut self,
        edit_index: usize,
        operation: &'static str,
        node: Box<dyn Node>,
        parent: NodeId,
        prev_sibling: Option<NodeId>,
        user_role: UserNodeRole,
        validate_as_user_item: bool,
        creation_context: Option<NodeCreationContext>,
    ) -> Result<AddNodeEffect, EngineEditError> {
        if !self.nodes.contains(parent) {
            return Err(EngineEditError::ParentNotFound {
                edit_index,
                operation,
                parent,
            });
        }

        let mut node = self.coerce_node_for_engine(edit_index, operation, node)?;

        if validate_as_user_item {
            let container =
                self.nearest_container_ancestor(parent)
                    .ok_or(EngineEditError::UserItemContainerRequired {
                        edit_index,
                        operation,
                        parent,
                    })?;
            self.ensure_item_kind_allowed(edit_index, operation, container, node.get_type(), node.user_item_kind())?;
        }

        self.prepare_node_for_insert(&mut node, parent, user_role);
        let child_id = self.nodes.insert(node);
        self.attach_node(edit_index, operation, child_id, parent, prev_sibling)?;

        // Initialize effective_enabled now that the parent link is established.
        // Uses the parent-chain walk (init path only — not a hot path).
        {
            let enabled = self.is_effectively_enabled(child_id);
            if let Some(n) = self.nodes.get_mut(child_id) {
                n.node_data_mut().effective_enabled = enabled;
            }
        }
        self.populate_param_cache_entry(child_id);

        let (attached_prev_sibling, attached_next_sibling) = {
            let attached_data = self
                .nodes
                .get(child_id)
                .ok_or(EngineEditError::NodeNotFound {
                    edit_index,
                    operation,
                    node: child_id,
                })?
                .node_data();
            (attached_data.prev_sibling, attached_data.next_sibling)
        };

        let decl_id = self
            .nodes
            .get(child_id)
            .ok_or(EngineEditError::NodeNotFound {
                edit_index,
                operation,
                node: child_id,
            })?
            .node_data()
            .meta
            .decl_id
            .clone();

        self.emit_inbox_event(EventKind::NodeCreated { node: child_id });
        self.emit_inbox_event(EventKind::ChildAdded {
            parent,
            child: child_id,
            decl_id,
        });
        let child_tree_snapshot = Some(self.build_process_tree_snapshot());

        // Allow newly attached nodes to request deterministic follow-up structure before app init.
        let mut attach_ctx = ProcessCtx::new(ExecutionPhase::EngineTick, self.time);
        attach_ctx.runtime_elapsed = self.runtime_elapsed;
        if let Some(child_tree_snapshot) = &child_tree_snapshot {
            attach_ctx.set_tree_snapshot(Arc::clone(child_tree_snapshot));
        }
        if let Some(node) = self.nodes.get_mut(child_id) {
            crate::logger::with_node_origin(child_id, || {
                node.engine_on_attached(&mut attach_ctx);
            });
        }
        self.absorb_edits(&mut attach_ctx)?;
        if self.stabilization_scope_depth == 0 {
            self.stabilize_added_node_structure(self.inbox.events.len(), creation_context)?;
        }

        // Run app init after declared/generated children are materialized and handles are bound.
        self.run_node_init(child_id, creation_context)?;
        if let Some(context) = creation_context {
            self.run_node_ready(child_id, context)?;
        }

        self.push_added_subtree_ui_events(child_id, parent);

        Ok(AddNodeEffect {
            node: child_id,
            parent,
            prev_sibling: attached_prev_sibling,
            next_sibling: attached_next_sibling,
        })
    }

    /// Applies an add-node edit and returns history data required for undo/redo.
    pub(crate) fn apply_add_node(
        &mut self,
        edit_index: usize,
        node: Box<dyn Node>,
        parent: NodeId,
        prev_sibling: Option<NodeId>,
        creation_context: Option<NodeCreationContext>,
    ) -> Result<AddNodeEffect, EngineEditError> {
        self.apply_add_node_with_role(
            edit_index,
            "AddNode",
            node,
            parent,
            prev_sibling,
            UserNodeRole::Regular,
            false,
            creation_context,
        )
    }

    /// Applies an add-node-tree edit and returns history data for the inserted root.
    pub(crate) fn apply_add_node_tree(
        &mut self,
        edit_index: usize,
        tree: NodeTree,
        parent: NodeId,
        prev_sibling: Option<NodeId>,
        creation_context: Option<NodeCreationContext>,
    ) -> Result<AddNodeEffect, EngineEditError> {
        const OP: &str = "AddNodeTree";

        if !self.nodes.contains(parent) {
            return Err(EngineEditError::ParentNotFound {
                edit_index,
                operation: OP,
                parent,
            });
        }

        let tree = self.coerce_pending_node_tree(edit_index, OP, tree)?;
        let mut inserted = Vec::new();
        let root_id = self.insert_pending_node_tree(
            edit_index,
            OP,
            tree,
            parent,
            prev_sibling,
            UserNodeRole::Regular,
            &mut inserted,
        )?;

        let (attached_prev_sibling, attached_next_sibling) = {
            let attached_data = self
                .nodes
                .get(root_id)
                .ok_or(EngineEditError::NodeNotFound {
                    edit_index,
                    operation: OP,
                    node: root_id,
                })?
                .node_data();
            (attached_data.prev_sibling, attached_data.next_sibling)
        };

        for inserted_node in &inserted {
            self.emit_inbox_event(EventKind::NodeCreated { node: inserted_node.id });
            self.emit_inbox_event(EventKind::ChildAdded {
                parent: inserted_node.parent,
                child: inserted_node.id,
                decl_id: inserted_node.decl_id.clone(),
            });
        }

        let inserted_ids = inserted.iter().map(|node| node.id).collect::<Vec<_>>();

        // Initialize effective_enabled before any node callbacks fire.
        for node_id in &inserted_ids {
            let enabled = self.is_effectively_enabled(*node_id);
            if let Some(node) = self.nodes.get_mut(*node_id) {
                node.node_data_mut().effective_enabled = enabled;
            }
        }

        self.run_node_attached_for_batch(inserted_ids.as_slice(), creation_context)?;
        self.run_node_init_for_batch(inserted_ids.as_slice(), creation_context)?;
        if let Some(context) = creation_context {
            self.run_node_ready_for_batch(inserted_ids.as_slice(), context)?;
        }

        self.push_added_subtree_ui_events(root_id, parent);

        Ok(AddNodeEffect {
            node: root_id,
            parent,
            prev_sibling: attached_prev_sibling,
            next_sibling: attached_next_sibling,
        })
    }

    /// Applies an add-user-item edit and returns history data required for undo/redo.
    pub(crate) fn apply_add_user_item(
        &mut self,
        edit_index: usize,
        node: Box<dyn Node>,
        parent: NodeId,
        prev_sibling: Option<NodeId>,
        creation_context: Option<NodeCreationContext>,
    ) -> Result<AddNodeEffect, EngineEditError> {
        self.apply_add_node_with_role(
            edit_index,
            "AddUserItem",
            node,
            parent,
            prev_sibling,
            UserNodeRole::ItemRoot,
            true,
            creation_context,
        )
    }

    pub(crate) fn run_node_init(
        &mut self,
        node_id: NodeId,
        creation_context: Option<NodeCreationContext>,
    ) -> Result<(), EngineEditError> {
        let init_tree_snapshot = Some(self.build_process_tree_snapshot());
        let mut init_ctx = ProcessCtx::new(ExecutionPhase::EngineTick, self.time);
        init_ctx.runtime_elapsed = self.runtime_elapsed;
        if let Some(init_tree_snapshot) = &init_tree_snapshot {
            init_ctx.set_tree_snapshot(Arc::clone(init_tree_snapshot));
        }
        if let Some(node) = self.nodes.get_mut(node_id) {
            crate::logger::with_node_origin(node_id, || {
                node.init(&mut init_ctx);
            });
        }
        self.absorb_edits(&mut init_ctx)?;
        if self.stabilization_scope_depth == 0 {
            self.stabilize_added_node_structure(self.inbox.events.len(), creation_context)?;
        }

        Ok(())
    }

    pub(crate) fn run_node_ready(
        &mut self,
        node_id: NodeId,
        creation_context: NodeCreationContext,
    ) -> Result<(), EngineEditError> {
        let created_tree_snapshot = Some(self.build_process_tree_snapshot());
        let mut created_ctx = ProcessCtx::new(ExecutionPhase::EngineTick, self.time);
        created_ctx.runtime_elapsed = self.runtime_elapsed;
        if let Some(created_tree_snapshot) = &created_tree_snapshot {
            created_ctx.set_tree_snapshot(Arc::clone(created_tree_snapshot));
        }
        if let Some(node) = self.nodes.get_mut(node_id) {
            crate::logger::with_node_origin(node_id, || {
                node.on_node_ready(&mut created_ctx, creation_context);
            });
        }
        self.absorb_edits(&mut created_ctx)?;
        if self.stabilization_scope_depth == 0 {
            self.stabilize_added_node_structure(self.inbox.events.len(), Some(creation_context))?;
        }

        Ok(())
    }

    pub(crate) fn run_node_destroy(
        &mut self,
        node_id: NodeId,
        tree_snapshot: &Arc<crate::process_ctx::ProcessTreeSnapshot>,
    ) {
        let mut destroy_ctx = ProcessCtx::new(ExecutionPhase::EngineTick, self.time);
        destroy_ctx.runtime_elapsed = self.runtime_elapsed;
        destroy_ctx.set_tree_snapshot(Arc::clone(tree_snapshot));

        if let Some(node) = self.nodes.get_mut(node_id) {
            crate::logger::with_node_origin(node_id, || {
                node.destroy(&mut destroy_ctx);
            });
        }
    }

    pub(crate) fn run_destroy_for_subtree(&mut self, node_ids: &[NodeId]) {
        if node_ids.is_empty() {
            return;
        }

        let tree_snapshot = self.build_process_tree_snapshot();
        for node_id in node_ids.iter().rev().copied() {
            self.run_node_destroy(node_id, &tree_snapshot);
        }
    }

    pub(crate) fn run_node_ready_for_subtree(
        &mut self,
        node_ids: &[NodeId],
        creation_context: NodeCreationContext,
    ) -> Result<(), EngineEditError> {
        let ready_ids = node_ids
            .iter()
            .copied()
            .filter(|node_id| self.nodes.contains(*node_id))
            .collect::<Vec<_>>();
        self.run_node_ready_for_batch(ready_ids.as_slice(), creation_context)
    }

    pub(crate) fn queue_node_ready(&mut self, node_id: NodeId, creation_context: NodeCreationContext) {
        self.pending_node_ready.push((node_id, creation_context));
    }

    pub(crate) fn run_pending_node_ready_callbacks(&mut self) -> Result<(), EngineEditError> {
        let pending = std::mem::take(&mut self.pending_node_ready);
        let mut current_context = None::<NodeCreationContext>;
        let mut ready_ids = Vec::<NodeId>::new();

        for (node_id, creation_context) in pending {
            if !self.nodes.contains(node_id) {
                continue;
            }

            if current_context.is_some_and(|context| context != creation_context) {
                let context = current_context.expect("current_context should exist for a non-empty batch");
                self.run_node_ready_for_batch(ready_ids.as_slice(), context)?;
                ready_ids.clear();
            }

            current_context = Some(creation_context);
            ready_ids.push(node_id);
        }

        if let Some(context) = current_context {
            self.run_node_ready_for_batch(ready_ids.as_slice(), context)?;
        }

        Ok(())
    }

    /// Applies a replace-node edit and returns history data required for undo/redo.
    pub(crate) fn apply_replace_node(
        &mut self,
        edit_index: usize,
        node: NodeId,
        new_node: Box<dyn Node>,
    ) -> Result<ReplaceNodeEffect<T>, EngineEditError> {
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
            replacement_data.id = node;
            replacement_data.parent = old_data.parent;
            replacement_data.first_child = old_data.first_child;
            replacement_data.last_child = old_data.last_child;
            replacement_data.prev_sibling = old_data.prev_sibling;
            replacement_data.next_sibling = old_data.next_sibling;
            replacement_data.meta.decl_id = old_data.meta.decl_id.clone();
        }

        let old_node = self.nodes.detach(node).ok_or(EngineEditError::NodeNotFound {
            edit_index,
            operation: OP,
            node,
        })?;
        self.purge_param_cache_entry(node);
        self.nodes.reattach(node, replacement);
        self.populate_param_cache_entry(node);
        self.mark_schedule_dirty();
        self.blueprints.unregister_instance(node);

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
    pub(crate) fn apply_remove_node(
        &mut self,
        edit_index: usize,
        node: NodeId,
    ) -> Result<RemoveNodeEffect<T>, EngineEditError> {
        const OP: &str = "RemoveNode";

        if node == self.root {
            return Err(EngineEditError::CannotMutateRoot {
                edit_index,
                operation: OP,
                node,
            });
        }

        let (parent, prev_sibling, next_sibling) = self.node_position(edit_index, OP, node)?;
        let subtree = self.collect_subtree(edit_index, OP, node)?;
        let removed_ids = subtree.clone();
        self.run_destroy_for_subtree(subtree.as_slice());
        self.detach_node(edit_index, OP, node)?;

        let mut detached_nodes = Vec::with_capacity(subtree.len());
        for removed in subtree.into_iter().rev() {
            let detached_node = self.nodes.detach(removed).ok_or(EngineEditError::NodeNotFound {
                edit_index,
                operation: OP,
                node: removed,
            })?;
            detached_nodes.push((removed, detached_node));
            self.purge_param_cache_entry(removed);
            self.blueprints.unregister_instance(removed);
            self.emit_inbox_event(EventKind::NodeDeleted { node: removed });
        }

        self.emit_inbox_event(EventKind::ChildRemoved { parent, child: node });
        self.push_ui_graph_transaction(vec![UiGraphOp::SubtreeRemoved {
            root: node,
            removed_ids,
            parent_after: self.ui_children_order_patch(parent),
        }]);

        Ok(RemoveNodeEffect {
            node,
            parent,
            prev_sibling,
            next_sibling,
            detached_nodes,
        })
    }

    /// Applies a move-node edit and returns history data required for undo/redo.
    pub(crate) fn apply_move_node(
        &mut self,
        edit_index: usize,
        node: NodeId,
        new_parent: NodeId,
        new_prev_sibling: Option<NodeId>,
    ) -> Result<MoveNodeEffect, EngineEditError> {
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

        self.validate_item_roots_for_move(edit_index, OP, node, new_parent)?;

        let (old_parent, old_prev_sibling, old_next_sibling) = self.node_position(edit_index, OP, node)?;

        let detached_parent = self.detach_node(edit_index, OP, node)?;
        if let Some(new_prev_sibling) = new_prev_sibling {
            self.attach_node(edit_index, OP, node, new_parent, Some(new_prev_sibling))?;
        } else {
            // MoveNode uses previous-sibling semantics: None means "become first child".
            let new_next_sibling = self
                .nodes
                .get(new_parent)
                .and_then(|entry| entry.node_data().first_child);
            self.attach_node_between(edit_index, OP, node, new_parent, None, new_next_sibling)?;
        }

        let new_node_data = self
            .nodes
            .get(node)
            .ok_or(EngineEditError::NodeNotFound {
                edit_index,
                operation: OP,
                node,
            })?
            .node_data();
        let new_prev_sibling = new_node_data.prev_sibling;
        let new_next_sibling = new_node_data.next_sibling;

        if detached_parent == new_parent {
            self.emit_inbox_event(EventKind::ChildReordered {
                parent: new_parent,
                child: node,
            });
            if let Some(children) = self.ui_direct_children(new_parent) {
                self.push_ui_graph_transaction(vec![UiGraphOp::ChildrenReordered {
                    parent: new_parent,
                    children,
                }]);
            }
        } else {
            self.emit_inbox_event(EventKind::ChildMoved {
                child: node,
                old_parent: detached_parent,
                new_parent,
            });
            self.push_ui_graph_transaction(vec![UiGraphOp::NodeMoved {
                node,
                old_parent: Some(detached_parent),
                new_parent: Some(new_parent),
                old_parent_after: self.ui_children_order_patch(detached_parent),
                new_parent_after: self.ui_children_order_patch(new_parent),
            }]);
        }

        // When the parent changed the effective_enabled of the whole subtree may have changed.
        if detached_parent != new_parent {
            let changes = self.subtree_effective_enabled_changes(node);
            self.queue_effective_enabled_callbacks(&changes)?;
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

    /// Computes which nodes in the subtree rooted at `root` need their `effective_enabled`
    /// updated based on the current parent chain and `meta.enabled` flags.
    ///
    /// Returns `(node_id, new_effective_enabled)` only for nodes whose cached value differs.
    fn subtree_effective_enabled_changes(&self, root: NodeId) -> Vec<(NodeId, bool)> {
        let parent_effective = self
            .nodes
            .get(root)
            .and_then(|n| n.node_data().parent)
            .map(|p| self.nodes.get(p).map(|n| n.node_data().effective_enabled).unwrap_or(false))
            .unwrap_or(true);

        let mut changes = Vec::new();
        let mut stack = vec![(root, parent_effective)];
        while let Some((node_id, parent_enabled)) = stack.pop() {
            let Some(node) = self.nodes.get(node_id) else {
                continue;
            };
            let new_effective = parent_enabled && node.node_data().meta.enabled;
            if new_effective != node.node_data().effective_enabled {
                changes.push((node_id, new_effective));
            }
            let mut child = node.node_data().first_child;
            while let Some(child_id) = child {
                let next = self.nodes.get(child_id).and_then(|n| n.node_data().next_sibling);
                stack.push((child_id, new_effective));
                child = next;
            }
        }
        changes
    }

    /// Emits a `GraphTransaction` covering all nodes in the subtree rooted at `root`
    /// so that the UI can insert them without a full resync.
    ///
    /// Must be called AFTER all lifecycle hooks (attached / init / ready) so that
    /// any declared children added during init are also included in the transaction.
    fn push_added_subtree_ui_events(&mut self, root: NodeId, root_parent: NodeId) {
        let node_ids = self.collect_subtree_node_ids(root);

        let mut ops = Vec::with_capacity(node_ids.len() + 1);
        for node_id in &node_ids {
            let parent = self.nodes.get(*node_id).and_then(|n| n.node_data().parent);
            let Some(snapshot) = self.ui_node_dto_for_event(*node_id) else {
                continue;
            };
            let index = parent.and_then(|p| self.ui_child_index(p, *node_id));
            ops.push(UiGraphOp::NodeCreated { snapshot, parent, index });
        }

        // Append the final child order for the insertion parent so the UI knows where `root` sits.
        if let Some(children) = self.ui_direct_children(root_parent) {
            ops.push(UiGraphOp::ChildrenReordered { parent: root_parent, children });
        }

        self.push_ui_graph_transaction(ops);
    }
}
