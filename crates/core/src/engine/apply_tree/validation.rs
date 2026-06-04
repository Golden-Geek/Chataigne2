use super::*;

impl<T: Node> Engine<T> {
    /// Returns `true` when `node` is equal to or under `ancestor` in the parent chain.
    pub(super) fn is_descendant_of(&self, node: NodeId, ancestor: NodeId) -> bool {
        let mut cursor = Some(node);
        while let Some(current) = cursor {
            if current == ancestor {
                return true;
            }
            cursor = self.nodes.get(current).and_then(|n| n.node_data().parent);
        }
        false
    }

    pub(super) fn nearest_container_ancestor(&self, start: NodeId) -> Option<NodeId> {
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

    pub(super) fn has_item_root_ancestor(&self, start: NodeId) -> bool {
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

    pub(super) fn default_user_permissions_for_new_node(
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

    pub(super) fn prepare_node_for_insert(&self, node: &mut T, parent: NodeId, user_role: UserNodeRole) {
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

    pub(super) fn ensure_item_kind_allowed(
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

    pub(super) fn validate_item_roots_for_move(
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

    pub(super) fn coerce_pending_node_tree(
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
}
