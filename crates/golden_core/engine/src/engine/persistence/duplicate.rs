use std::collections::HashMap;

use crate::blueprints::{BlueprintInstanceMeta, BlueprintRegistry};
use crate::edit::NodeTree;
use crate::node::{
    DeclId, FOLDER_NODE_TYPE, Node, NodeCreationContext, NodeId, NodeMeta, NodeReference, NodeUserPermissions,
    NodeUuid, UserNodeRole,
};
use crate::parameter::ParamValue;

use super::super::history::{AddNodeEffect, HistoryTransaction};
use super::*;

// Codec and initializer work is fallible, so persisted subtrees are decoded
// completely outside the live NodeStore before the prevalidated insertion phase.
pub(super) struct DecodedProjectTree<T> {
    pub(super) node: T,
    pub(super) children: Vec<DecodedProjectTree<T>>,
}

/// Request-validated detached subtree ready for live-graph insertion.
///
/// Codec, destination, catalog, initializer, value-constraint, and dependent
/// source-lookup failures are exhausted here. App-owned live lifecycle callbacks
/// still run after insertion because their runtime resources and parent context
/// cannot be staged safely.
pub(crate) struct PreparedProjectSubtree<T> {
    parent: NodeId,
    prev_sibling: Option<NodeId>,
    tree: DecodedProjectTree<T>,
    blueprint_meta: Option<BlueprintInstanceMeta>,
}

pub(crate) struct CommittedProjectSubtree {
    pub(crate) root: NodeId,
    pub(crate) node_ids: Vec<NodeId>,
    effect: AddNodeEffect,
}

impl<T: Node> PreparedProjectSubtree<T> {
    pub(crate) fn root_uuid(&self) -> NodeUuid {
        self.tree.node.node_data().meta.uuid
    }

    pub(crate) fn root_label(&self) -> &str {
        self.tree.node.node_data().meta.label.as_str()
    }

    pub(crate) fn set_root_label(&mut self, label: String) {
        let short_name = generate_short_name(&label);
        let meta = &mut self.tree.node.node_data_mut().meta;
        meta.label = label;
        meta.short_name = short_name.clone();
        meta.decl_id = DeclId(short_name);
    }
}

impl<T: Node> Engine<T> {
    /// Duplicates a persisted subtree by round-tripping through the project codec hooks.
    pub fn duplicate_subtree_with<Encode, Decode>(
        &mut self,
        source: NodeId,
        new_parent: NodeId,
        new_prev_sibling: Option<NodeId>,
        label_override: Option<String>,
        encode_data: Encode,
        decode_node: Decode,
    ) -> Result<NodeId, ProjectPersistenceError>
    where
        Encode: FnMut(&T) -> Result<serde_json::Value, String>,
        Decode: FnMut(&str, &serde_json::Value, &NodeMeta) -> Result<T, String>,
    {
        let mut encode_data = encode_data;
        let mut decode_node = decode_node;
        let prepared = self.prepare_duplicate_subtree_with(
            source,
            new_parent,
            new_prev_sibling,
            label_override,
            &mut encode_data,
            &mut decode_node,
        )?;
        let committed =
            self.commit_prepared_project_subtree(prepared, NodeCreationContext::Duplicate, true, "DuplicateNode")?;
        let duplicated_root = committed.root;
        self.finalize_committed_project_subtrees(vec![committed])?;
        Ok(duplicated_root)
    }

    pub(crate) fn duplicate_subtree_with_initial_params<Encode, Decode>(
        &mut self,
        source: NodeId,
        new_parent: NodeId,
        new_prev_sibling: Option<NodeId>,
        initial_params: Vec<(DeclId, ParamValue)>,
        mut encode_data: Encode,
        mut decode_node: Decode,
    ) -> Result<NodeId, ProjectPersistenceError>
    where
        Encode: FnMut(&T) -> Result<serde_json::Value, String>,
        Decode: FnMut(&str, &serde_json::Value, &NodeMeta) -> Result<T, String>,
    {
        let prepared = self.prepare_duplicate_subtree_with(
            source,
            new_parent,
            new_prev_sibling,
            None,
            &mut encode_data,
            &mut decode_node,
        )?;
        let prepared = self.prepare_project_subtree_initial_params(prepared, initial_params, "DuplicateNode")?;
        let committed =
            self.commit_prepared_project_subtree(prepared, NodeCreationContext::Duplicate, true, "DuplicateNode")?;
        let duplicated_root = committed.root;
        self.finalize_committed_project_subtrees(vec![committed])?;
        Ok(duplicated_root)
    }

    pub(crate) fn prepare_duplicate_subtree_with<Encode, Decode>(
        &self,
        source: NodeId,
        new_parent: NodeId,
        new_prev_sibling: Option<NodeId>,
        label_override: Option<String>,
        mut encode_data: Encode,
        mut decode_node: Decode,
    ) -> Result<PreparedProjectSubtree<T>, ProjectPersistenceError>
    where
        Encode: FnMut(&T) -> Result<serde_json::Value, String>,
        Decode: FnMut(&str, &serde_json::Value, &NodeMeta) -> Result<T, String>,
    {
        self.validate_persisted_subtree_destination(new_parent, new_prev_sibling, "DuplicateNode")?;

        let mut record = self.encode_node_record_with(source, &mut encode_data)?;
        let source_label = record
            .meta
            .label
            .as_deref()
            .map(str::trim)
            .filter(|label| !label.is_empty())
            .unwrap_or(record.node_type.as_str());
        let preferred_label = label_override
            .as_deref()
            .map(str::trim)
            .filter(|label| !label.is_empty())
            .unwrap_or(source_label);
        let label = self.next_unique_child_label(new_parent, preferred_label);
        let short_name = generate_short_name(&label);
        record.meta.label = Some(label);
        record.meta.short_name = Some(short_name.clone());
        record.meta.decl_id = Some(DeclId(short_name));

        let mut uuid_map = HashMap::<NodeUuid, NodeUuid>::new();
        remap_record_uuids(&mut record, &mut uuid_map);

        let decoded_tree = {
            let parent_node = self
                .nodes
                .get(new_parent)
                .ok_or(ProjectPersistenceError::MissingNode(new_parent))?;
            Self::decode_project_record_tree_with(parent_node, &record, &uuid_map, &mut decode_node)?
        };

        Ok(PreparedProjectSubtree {
            parent: new_parent,
            prev_sibling: new_prev_sibling,
            tree: decoded_tree,
            blueprint_meta: self.blueprints.instance_meta(source).cloned(),
        })
    }

    pub(crate) fn prepare_project_subtree_initial_params(
        &self,
        mut prepared: PreparedProjectSubtree<T>,
        initial_params: Vec<(DeclId, ParamValue)>,
        operation: &'static str,
    ) -> Result<PreparedProjectSubtree<T>, ProjectPersistenceError> {
        if initial_params.is_empty() {
            return Ok(prepared);
        }

        prepared.tree = Self::stage_tree_with_initial_params(prepared.tree, initial_params, operation)?;
        Ok(prepared)
    }

    pub(crate) fn prepare_prevalidated_catalog_item_subtree(
        &self,
        parent: NodeId,
        prev_sibling: Option<NodeId>,
        node_type: &str,
        resolved_label: String,
        initial_params: Vec<(DeclId, ParamValue)>,
        operation: &'static str,
    ) -> Result<PreparedProjectSubtree<T>, ProjectPersistenceError> {
        self.validate_persisted_subtree_destination(parent, prev_sibling, operation)?;

        if let Some(blueprint_id) = BlueprintRegistry::<T>::parse_type_id(node_type) {
            let declaration = self.blueprints.blueprint(&blueprint_id).ok_or_else(|| {
                ProjectPersistenceError::Engine(EngineEditError::UserItemTypeUnavailable {
                    edit_index: 0,
                    operation,
                    parent,
                    node_type: node_type.to_string(),
                })
            })?;
            let blueprint_version = declaration.version;
            let mut node = declaration.instantiate();
            node.node_data_mut().meta.label = resolved_label;
            let tag = format!("blueprint:{}", blueprint_id.as_str());
            if !node.node_data().meta.tags.iter().any(|existing| existing == &tag) {
                node.node_data_mut().meta.tags.push(tag);
            }
            self.validate_prepared_user_item(parent, &node, operation)?;
            Self::prepare_detached_user_item_root(&mut node);
            let tree = Self::stage_tree_with_initial_params(
                DecodedProjectTree {
                    node,
                    children: Vec::new(),
                },
                initial_params,
                operation,
            )?;
            return Ok(PreparedProjectSubtree {
                parent,
                prev_sibling,
                tree,
                blueprint_meta: Some(BlueprintInstanceMeta::new(
                    blueprint_id,
                    blueprint_version,
                    HashMap::new(),
                )),
            });
        }

        let factory_node_id = self.catalog_factory_node_for_preflight(parent).ok_or_else(|| {
            ProjectPersistenceError::Engine(EngineEditError::UserItemTypeUnavailable {
                edit_index: 0,
                operation,
                parent,
                node_type: node_type.to_string(),
            })
        })?;
        let factory_node = self
            .nodes
            .get(factory_node_id)
            .ok_or(ProjectPersistenceError::MissingNode(factory_node_id))?;
        let tree = factory_node.create_user_item_tree(node_type).ok_or_else(|| {
            ProjectPersistenceError::Engine(EngineEditError::UserItemTypeUnavailable {
                edit_index: 0,
                operation,
                parent,
                node_type: node_type.to_string(),
            })
        })?;
        self.prepare_user_item_subtree(parent, prev_sibling, tree, resolved_label, initial_params, operation)
    }

    fn prepare_user_item_subtree(
        &self,
        parent: NodeId,
        prev_sibling: Option<NodeId>,
        tree: NodeTree,
        resolved_label: String,
        initial_params: Vec<(DeclId, ParamValue)>,
        operation: &'static str,
    ) -> Result<PreparedProjectSubtree<T>, ProjectPersistenceError> {
        let mut tree = self.coerce_detached_node_tree(tree, operation)?;
        tree.node.node_data_mut().meta.label = resolved_label;
        self.validate_prepared_user_item(parent, &tree.node, operation)?;
        Self::prepare_detached_user_item_root(&mut tree.node);
        let tree = Self::stage_tree_with_initial_params(tree, initial_params, operation)?;

        Ok(PreparedProjectSubtree {
            parent,
            prev_sibling,
            tree,
            blueprint_meta: None,
        })
    }

    fn coerce_detached_node_tree(
        &self,
        tree: NodeTree,
        operation: &'static str,
    ) -> Result<DecodedProjectTree<T>, ProjectPersistenceError> {
        let node = self
            .coerce_node_for_engine(0, operation, tree.node)
            .map_err(ProjectPersistenceError::Engine)?;
        let mut children = Vec::with_capacity(tree.children.len());
        for child in tree.children {
            children.push(self.coerce_detached_node_tree(child, operation)?);
        }
        Ok(DecodedProjectTree { node, children })
    }

    fn validate_prepared_user_item(
        &self,
        parent: NodeId,
        node: &T,
        operation: &'static str,
    ) -> Result<(), ProjectPersistenceError> {
        let parent_node = self.nodes.get(parent).ok_or_else(|| {
            ProjectPersistenceError::Engine(EngineEditError::ParentNotFound {
                edit_index: 0,
                operation,
                parent,
            })
        })?;
        if parent_node.user_container_accepts_item(node.get_type(), node.user_item_kind()) {
            return Ok(());
        }

        let has_user_item_capability =
            parent_node.user_container_rules().is_some() || !parent_node.user_creatable_items().is_empty();
        if !has_user_item_capability {
            return Err(ProjectPersistenceError::Engine(
                EngineEditError::UserItemContainerRequired {
                    edit_index: 0,
                    operation,
                    parent,
                },
            ));
        }

        Err(ProjectPersistenceError::Engine(EngineEditError::UserItemKindRejected {
            edit_index: 0,
            operation,
            container: parent,
            container_type: parent_node.get_type().to_string(),
            item_type: node.get_type().to_string(),
            item_kind: node.user_item_kind().to_string(),
        }))
    }

    fn prepare_detached_user_item_root(node: &mut T) {
        let node_data = node.node_data_mut();
        node_data.user_role = UserNodeRole::ItemRoot;
        if node_data.meta.user_permissions == NodeUserPermissions::default() {
            node_data.meta.user_permissions = NodeUserPermissions::all();
        }
    }

    fn catalog_factory_node_for_preflight(&self, parent: NodeId) -> Option<NodeId> {
        let mut cursor = Some(parent);
        while let Some(node_id) = cursor {
            let node = self.nodes.get(node_id)?;
            if node.get_type() != FOLDER_NODE_TYPE {
                return Some(node_id);
            }
            cursor = node.node_data().parent;
        }
        None
    }

    fn stage_tree_with_initial_params(
        mut tree: DecodedProjectTree<T>,
        initial_params: Vec<(DeclId, ParamValue)>,
        operation: &'static str,
    ) -> Result<DecodedProjectTree<T>, ProjectPersistenceError> {
        Self::reset_detached_tree_links(&mut tree);
        let DecodedProjectTree { node, children } = tree;
        let mut staging = Engine::new(node);
        let staging_root = staging.root;
        let mut prev_sibling = None;
        for child in children {
            let child_id = staging.insert_decoded_project_tree(staging_root, prev_sibling, child, operation)?;
            prev_sibling = Some(child_id);
        }

        let staged_node_ids = staging.collect_loaded_subtree_node_ids(staging_root)?;
        staging
            .run_node_attached_for_batch(staged_node_ids.as_slice(), None)
            .map_err(ProjectPersistenceError::Engine)?;

        for (decl_id, value) in initial_params {
            let param = staging
                .find_staged_parameter(staging_root, decl_id.0.as_str())
                .ok_or_else(|| {
                    let node_type = staging
                        .nodes
                        .get(staging_root)
                        .map(|node| node.get_type().to_string())
                        .unwrap_or_else(|| "unknown".to_string());
                    ProjectPersistenceError::Engine(EngineEditError::NodeMutationRejected {
                        edit_index: 0,
                        operation,
                        node: staging_root,
                        node_type,
                        message: format!("parameter '{}' is unavailable on the prepared item", decl_id.0),
                    })
                })?;
            staging
                .apply_set_param(0, param, value)
                .map_err(ProjectPersistenceError::Engine)?;
        }

        staging.take_staged_tree(staging_root, operation)
    }

    fn find_staged_parameter(&self, root: NodeId, decl_id: &str) -> Option<NodeId> {
        if decl_id.contains('/') {
            let mut current = root;
            for segment in decl_id.split('/').filter(|segment| !segment.is_empty()) {
                let mut child = self.nodes.get(current)?.node_data().first_child;
                let mut matched = None;
                while let Some(child_id) = child {
                    let child_node = self.nodes.get(child_id)?;
                    if child_node.node_data().meta.decl_id.0 == segment {
                        matched = Some(child_id);
                        break;
                    }
                    child = child_node.node_data().next_sibling;
                }
                current = matched?;
            }
            if self
                .nodes
                .get(current)
                .is_some_and(|node| node.engine_param_snapshot().is_some())
            {
                return Some(current);
            }
        }

        let mut stack = Vec::new();
        let mut child = self.nodes.get(root)?.node_data().first_child;
        while let Some(child_id) = child {
            let child_node = self.nodes.get(child_id)?;
            stack.push(child_id);
            child = child_node.node_data().next_sibling;
        }

        while let Some(node_id) = stack.pop() {
            let node = self.nodes.get(node_id)?;
            if node.engine_param_snapshot().is_some()
                && (node.node_data().meta.decl_id.0 == decl_id
                    || node.node_data().meta.decl_id.0.rsplit('/').next() == Some(decl_id))
            {
                return Some(node_id);
            }

            let mut child = node.node_data().first_child;
            while let Some(child_id) = child {
                let child_node = self.nodes.get(child_id)?;
                stack.push(child_id);
                child = child_node.node_data().next_sibling;
            }
        }

        None
    }

    fn take_staged_tree(
        &mut self,
        root: NodeId,
        operation: &'static str,
    ) -> Result<DecodedProjectTree<T>, ProjectPersistenceError> {
        let mut child_ids = Vec::new();
        let mut child = self
            .nodes
            .get(root)
            .ok_or(ProjectPersistenceError::MissingNode(root))?
            .node_data()
            .first_child;
        while let Some(child_id) = child {
            child_ids.push(child_id);
            child = self
                .nodes
                .get(child_id)
                .ok_or(ProjectPersistenceError::MissingNode(child_id))?
                .node_data()
                .next_sibling;
        }

        let mut children = Vec::with_capacity(child_ids.len());
        for child_id in child_ids {
            children.push(self.take_staged_tree(child_id, operation)?);
        }

        let mut node = self.nodes.remove(root).ok_or_else(|| {
            ProjectPersistenceError::Engine(EngineEditError::NodeNotFound {
                edit_index: 0,
                operation,
                node: root,
            })
        })?;
        node.engine_visit_references_mut(&mut |reference: &mut NodeReference| {
            reference.clear_cached_id();
        });
        let node_data = node.node_data_mut();
        node_data.parent = None;
        node_data.first_child = None;
        node_data.last_child = None;
        node_data.prev_sibling = None;
        node_data.next_sibling = None;

        Ok(DecodedProjectTree { node, children })
    }

    fn reset_detached_tree_links(tree: &mut DecodedProjectTree<T>) {
        let node_data = tree.node.node_data_mut();
        node_data.parent = None;
        node_data.first_child = None;
        node_data.last_child = None;
        node_data.prev_sibling = None;
        node_data.next_sibling = None;
        for child in &mut tree.children {
            Self::reset_detached_tree_links(child);
        }
    }

    pub(crate) fn commit_prepared_project_subtree(
        &mut self,
        prepared: PreparedProjectSubtree<T>,
        creation_context: NodeCreationContext,
        dispatch_structure_events: bool,
        operation: &'static str,
    ) -> Result<CommittedProjectSubtree, ProjectPersistenceError> {
        let PreparedProjectSubtree {
            parent,
            prev_sibling,
            tree,
            blueprint_meta,
        } = prepared;
        let root = self.insert_decoded_project_tree(parent, prev_sibling, tree, operation)?;
        self.replay_loaded_subtree_lifecycle(root, creation_context, LoadedReadyMode::Immediate)?;
        let node_ids = self.collect_loaded_subtree_node_ids(root)?;

        if let Some(mut blueprint_meta) = blueprint_meta {
            blueprint_meta.decl_index = node_ids
                .iter()
                .filter_map(|node_id| {
                    self.nodes
                        .get(*node_id)
                        .map(|node| (node.node_data().meta.decl_id.clone(), *node_id))
                })
                .collect();
            self.blueprints.register_instance(root, blueprint_meta);
        }
        if dispatch_structure_events {
            self.queue_loaded_subtree_structure_events(&[root])?;
        }

        let effect = AddNodeEffect {
            node: root,
            parent,
            prev_sibling: self.nodes.get(root).and_then(|node| node.node_data().prev_sibling),
            next_sibling: self.nodes.get(root).and_then(|node| node.node_data().next_sibling),
        };

        Ok(CommittedProjectSubtree { root, node_ids, effect })
    }

    pub(crate) fn finalize_committed_project_subtrees(
        &mut self,
        committed: Vec<CommittedProjectSubtree>,
    ) -> Result<(), ProjectPersistenceError> {
        if committed.is_empty() {
            return Ok(());
        }

        let inserted_node_count = committed.iter().map(|subtree| subtree.node_ids.len()).sum();
        let mut inserted_node_ids = Vec::with_capacity(inserted_node_count);
        for subtree in &committed {
            inserted_node_ids.extend_from_slice(subtree.node_ids.as_slice());
        }

        self.sync_missing_reference_warnings_for_nodes_silent(inserted_node_ids.as_slice());
        if inserted_node_ids
            .iter()
            .copied()
            .any(|node| self.node_within_user_context_scope(node))
        {
            self.rebuild_user_context_registry_from_nodes();
            self.mark_user_context_graph_changed();
        }

        let catalog_snapshot = inserted_node_ids
            .iter()
            .copied()
            .any(|node| self.catalog_creatable_items_require_tree_snapshot(node))
            .then(|| self.build_process_tree_snapshot());
        let mut ui_ops = Vec::new();
        for subtree in &committed {
            ui_ops.extend(self.loaded_subtree_ui_ops(subtree.node_ids.as_slice(), catalog_snapshot.as_deref())?);
        }
        self.push_ui_graph_transaction(ui_ops);

        let capture_active_session = self.has_active_edit_session();
        let mut transaction = HistoryTransaction::new();
        for subtree in committed {
            if capture_active_session {
                self.record_single_history_step(subtree.effect.into());
            } else {
                transaction.push(subtree.effect.into());
            }
        }
        if !capture_active_session {
            self.clear_redo_history();
            self.push_undo_transaction(transaction);
        }

        Ok(())
    }
}
