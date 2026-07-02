use std::collections::HashMap;

use crate::blueprints::{BlueprintDecl, BlueprintId, BlueprintInstanceMeta, BlueprintRegistry};
use crate::edit::Edit;
use crate::node::{
    DeclId, FOLDER_NODE_TYPE, Node, NodeCreationContext, NodeId, USER_CONTEXT_NODE_TYPE, UserCreatableItem,
};
use crate::process_ctx::ProcessTreeSnapshot;

use super::history::AddNodeEffect;
use super::{Engine, EngineEditError};

impl<T: Node> Engine<T> {
    /// Registers or replaces one blueprint declaration in the unified catalog.
    pub fn register_blueprint(&mut self, blueprint: BlueprintDecl<T>) -> Option<BlueprintDecl<T>> {
        self.blueprints.register_blueprint(blueprint)
    }

    /// Removes one blueprint declaration from the unified catalog.
    pub fn unregister_blueprint(&mut self, blueprint_id: &BlueprintId) -> Option<BlueprintDecl<T>> {
        self.blueprints.unregister_blueprint(blueprint_id)
    }

    /// Returns one blueprint declaration from `blueprint_id`.
    pub fn blueprint_decl(&self, blueprint_id: &BlueprintId) -> Option<&BlueprintDecl<T>> {
        self.blueprints.blueprint(blueprint_id)
    }

    /// Returns runtime metadata for one blueprint instance root.
    pub fn blueprint_instance_meta(&self, root: NodeId) -> Option<&BlueprintInstanceMeta> {
        self.blueprints.instance_meta(root)
    }

    /// Returns all catalog-creatable items for `parent` (built-ins + blueprints).
    pub fn catalog_creatable_items(&self, parent: NodeId) -> Vec<UserCreatableItem> {
        let snapshot = self.build_process_tree_snapshot();
        self.catalog_creatable_items_with_snapshot(parent, snapshot.as_ref())
    }

    pub(crate) fn catalog_creatable_items_with_snapshot(
        &self,
        parent: NodeId,
        snapshot: &ProcessTreeSnapshot,
    ) -> Vec<UserCreatableItem> {
        let Some(factory_node_id) = self.catalog_factory_node(parent) else {
            return Vec::new();
        };
        let Some(factory_node) = self.nodes.get(factory_node_id) else {
            return Vec::new();
        };

        let child_catalog = |child_parent: NodeId| self.catalog_creatable_items_with_snapshot(child_parent, snapshot);
        let mut items = Vec::<UserCreatableItem>::new();
        for item in factory_node
            .user_creatable_items_with_context(snapshot, parent, &child_catalog)
            .into_iter()
        {
            if factory_node.user_container_accepts_item(&item.node_type, &item.item_kind) {
                if parent != factory_node_id && self.item_requires_direct_catalog_host(&item) {
                    continue;
                }
                items.push(item);
            }
        }

        for item in self.blueprints.creatable_items().into_iter() {
            if self.parent_accepts_catalog_item_kind(parent, item.item_kind.as_str()) {
                items.push(item);
            }
        }
        items
    }

    /// Queues creation of one catalog item for `parent`.
    ///
    /// Supports both built-in container-created types and blueprint dynamic types.
    pub fn queue_catalog_create(
        &mut self,
        parent: NodeId,
        node_type: impl Into<String>,
        label: Option<String>,
        prev_sibling: Option<NodeId>,
    ) -> Result<(), EngineEditError> {
        let node_type = node_type.into();
        if !self.nodes.contains(parent) {
            return Err(EngineEditError::ParentNotFound {
                edit_index: 0,
                operation: "CreateCatalogItem",
                parent,
            });
        }
        let Some(factory_node_id) = self.catalog_factory_node(parent) else {
            return Err(EngineEditError::UserItemTypeUnavailable {
                edit_index: 0,
                operation: "CreateCatalogItem",
                parent,
                node_type,
            });
        };
        let Some(factory_node) = self.nodes.get(factory_node_id) else {
            return Err(EngineEditError::UserItemTypeUnavailable {
                edit_index: 0,
                operation: "CreateCatalogItem",
                parent,
                node_type,
            });
        };

        let catalog_item = self
            .catalog_creatable_items(parent)
            .into_iter()
            .find(|candidate| candidate.node_type == node_type)
            .ok_or_else(|| EngineEditError::UserItemTypeUnavailable {
                edit_index: 0,
                operation: "CreateCatalogItem",
                parent,
                node_type: node_type.clone(),
            })?;

        if let Some(blueprint_id) = BlueprintRegistry::<T>::parse_type_id(node_type.as_str()) {
            if self.blueprints.blueprint(&blueprint_id).is_none() {
                return Err(EngineEditError::UserItemTypeUnavailable {
                    edit_index: 0,
                    operation: "CreateCatalogItem",
                    parent,
                    node_type,
                });
            }

            self.edits.push(Edit::CreateBlueprintInstance {
                blueprint_id: blueprint_id.to_string(),
                parent,
                prev_sibling,
                label,
            });
            return Ok(());
        }

        let resolved_label = label.unwrap_or(catalog_item.label);

        let Some(mut tree) = factory_node.create_user_item_tree(node_type.as_str()) else {
            return Err(EngineEditError::UserItemTypeUnavailable {
                edit_index: 0,
                operation: "CreateCatalogItem",
                parent,
                node_type,
            });
        };
        tree.node.node_data_mut().meta.label = resolved_label;

        self.edits.push(Edit::AddUserItemTree {
            parent,
            prev_sibling,
            tree,
        });
        Ok(())
    }

    pub(crate) fn apply_create_blueprint_instance(
        &mut self,
        edit_index: usize,
        blueprint_id: String,
        parent: NodeId,
        prev_sibling: Option<NodeId>,
        label: Option<String>,
        creation_context: Option<NodeCreationContext>,
    ) -> Result<AddNodeEffect, EngineEditError> {
        const OPERATION: &str = "CreateBlueprintInstance";

        let blueprint_id = BlueprintId::new(blueprint_id);
        let Some(decl) = self.blueprints.blueprint(&blueprint_id) else {
            return Err(EngineEditError::UserItemTypeUnavailable {
                edit_index,
                operation: OPERATION,
                parent,
                node_type: blueprint_id.to_type_id(),
            });
        };

        let blueprint_version = decl.version;
        let resolved_label = label.unwrap_or_else(|| decl.label.clone());
        let mut node = decl.instantiate();
        node.node_data_mut().meta.label = resolved_label;
        let tag = format!("blueprint:{}", blueprint_id.as_str());
        if !node.node_data().meta.tags.iter().any(|existing| existing == &tag) {
            node.node_data_mut().meta.tags.push(tag);
        }

        let effect = self.apply_add_user_item(edit_index, Box::new(node), parent, prev_sibling, creation_context)?;
        let decl_index = self.collect_blueprint_decl_index(effect.node, edit_index, OPERATION)?;
        self.blueprints.register_instance(
            effect.node,
            BlueprintInstanceMeta::new(blueprint_id, blueprint_version, decl_index),
        );
        Ok(effect)
    }

    fn collect_blueprint_decl_index(
        &self,
        root: NodeId,
        edit_index: usize,
        operation: &'static str,
    ) -> Result<HashMap<DeclId, NodeId>, EngineEditError> {
        let subtree = self.collect_subtree(edit_index, operation, root)?;
        let mut decl_index = HashMap::with_capacity(subtree.len());
        for node_id in subtree {
            let Some(node) = self.nodes.get(node_id) else {
                continue;
            };
            decl_index.insert(node.node_data().meta.decl_id.clone(), node_id);
        }
        Ok(decl_index)
    }

    fn parent_accepts_catalog_item_kind(&self, parent: NodeId, item_kind: &str) -> bool {
        let Some(container_id) = self.catalog_factory_node(parent) else {
            return false;
        };
        let Some(container_node) = self.nodes.get(container_id) else {
            return false;
        };

        container_node
            .user_container_rules()
            .is_some_and(|rules| rules.accepts(item_kind))
            || container_node
                .user_creatable_items()
                .into_iter()
                .any(|item| item.item_kind == item_kind)
    }

    fn catalog_factory_node(&self, parent: NodeId) -> Option<NodeId> {
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

    fn item_requires_direct_catalog_host(&self, item: &UserCreatableItem) -> bool {
        item.node_type == "script" || item.node_type == USER_CONTEXT_NODE_TYPE
    }
}
