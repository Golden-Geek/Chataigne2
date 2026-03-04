use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::node::{DeclId, Node, NodeId, UserCreatableItem};
use crate::parameter::ParamValue;

/// Type id prefix used by the unified catalog for blueprint-backed dynamic node types.
pub const BLUEPRINT_TYPE_PREFIX: &str = "blueprint::";

/// Stable identifier for a blueprint declaration.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BlueprintId(pub String);

impl BlueprintId {
    /// Creates a new id.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Returns the inner id as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns the unified-catalog node type id for this blueprint.
    pub fn to_type_id(&self) -> String {
        format!("{BLUEPRINT_TYPE_PREFIX}{}", self.0)
    }
}

impl fmt::Display for BlueprintId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0.as_str())
    }
}

/// Field aspect used by blueprint override tracking.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BlueprintFieldAspect {
    /// Value default ownership.
    Value,
    /// Control-mode ownership.
    Control,
}

/// Stable key for one blueprint field aspect.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BlueprintFieldKey {
    /// Field declaration id.
    pub decl_id: DeclId,
    /// Aspect type.
    pub aspect: BlueprintFieldAspect,
}

/// Runtime metadata stored for one instantiated blueprint root.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlueprintInstanceMeta {
    /// Source blueprint declaration id.
    pub blueprint_id: BlueprintId,
    /// Blueprint version used for this instance.
    pub blueprint_version: u32,
    /// Override ownership keyed by field + aspect.
    #[serde(default, skip_serializing_if = "HashSet::is_empty")]
    pub overrides: HashSet<BlueprintFieldKey>,
    /// Runtime decl-id lookup table for this instance subtree.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub decl_index: HashMap<DeclId, NodeId>,
}

impl BlueprintInstanceMeta {
    /// Creates instance metadata initialized with no overrides.
    pub fn new(blueprint_id: BlueprintId, blueprint_version: u32, decl_index: HashMap<DeclId, NodeId>) -> Self {
        Self {
            blueprint_id,
            blueprint_version,
            overrides: HashSet::new(),
            decl_index,
        }
    }
}

type InstantiateBlueprintFn<T> = dyn Fn(String) -> T + Send + Sync;

/// Blueprint declaration exposed as one dynamic node type in the unified catalog.
#[derive(Clone)]
pub struct BlueprintDecl<T: Node> {
    /// Stable declaration id.
    pub id: BlueprintId,
    /// Default display label used when no explicit label is provided at creation.
    pub label: String,
    /// Logical item kind used by container admission.
    pub item_kind: String,
    /// Declaration version.
    pub version: u32,
    /// Optional default values keyed by decl id.
    pub defaults: HashMap<DeclId, ParamValue>,
    instantiate: Arc<InstantiateBlueprintFn<T>>,
}

impl<T: Node> fmt::Debug for BlueprintDecl<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BlueprintDecl")
            .field("id", &self.id)
            .field("label", &self.label)
            .field("item_kind", &self.item_kind)
            .field("version", &self.version)
            .field("defaults_len", &self.defaults.len())
            .finish()
    }
}

impl<T: Node> BlueprintDecl<T> {
    /// Creates a new blueprint declaration.
    pub fn new(
        id: BlueprintId,
        label: impl Into<String>,
        item_kind: impl Into<String>,
        instantiate: impl Fn(String) -> T + Send + Sync + 'static,
    ) -> Self {
        Self {
            id,
            label: label.into(),
            item_kind: item_kind.into(),
            version: 1,
            defaults: HashMap::new(),
            instantiate: Arc::new(instantiate),
        }
    }

    /// Sets declaration version.
    pub fn with_version(mut self, version: u32) -> Self {
        self.version = version;
        self
    }

    /// Sets default values.
    pub fn with_defaults(mut self, defaults: HashMap<DeclId, ParamValue>) -> Self {
        self.defaults = defaults;
        self
    }

    /// Returns unified-catalog node type id (`blueprint::<id>`).
    pub fn type_id(&self) -> String {
        self.id.to_type_id()
    }

    /// Instantiates one runtime root node.
    pub fn instantiate(&self, label: String) -> T {
        (self.instantiate)(label)
    }
}

/// Runtime blueprint registry plus per-instance tracking.
pub struct BlueprintRegistry<T: Node> {
    blueprints: HashMap<BlueprintId, BlueprintDecl<T>>,
    instances_by_blueprint: HashMap<BlueprintId, Vec<NodeId>>,
    instance_meta_by_root: HashMap<NodeId, BlueprintInstanceMeta>,
}

impl<T: Node> Default for BlueprintRegistry<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Node> BlueprintRegistry<T> {
    /// Creates an empty blueprint registry.
    pub fn new() -> Self {
        Self {
            blueprints: HashMap::new(),
            instances_by_blueprint: HashMap::new(),
            instance_meta_by_root: HashMap::new(),
        }
    }

    /// Parses a catalog node type id as a blueprint id.
    pub fn parse_type_id(node_type: &str) -> Option<BlueprintId> {
        let id = node_type.trim().strip_prefix(BLUEPRINT_TYPE_PREFIX)?;
        if id.is_empty() {
            return None;
        }
        Some(BlueprintId::new(id))
    }

    /// Registers or replaces one blueprint declaration.
    pub fn register_blueprint(&mut self, blueprint: BlueprintDecl<T>) -> Option<BlueprintDecl<T>> {
        self.blueprints.insert(blueprint.id.clone(), blueprint)
    }

    /// Removes one blueprint declaration.
    pub fn unregister_blueprint(&mut self, blueprint_id: &BlueprintId) -> Option<BlueprintDecl<T>> {
        self.blueprints.remove(blueprint_id)
    }

    /// Returns one blueprint declaration by id.
    pub fn blueprint(&self, blueprint_id: &BlueprintId) -> Option<&BlueprintDecl<T>> {
        self.blueprints.get(blueprint_id)
    }

    /// Returns one blueprint declaration from a catalog node type id.
    pub fn blueprint_by_type_id(&self, node_type: &str) -> Option<&BlueprintDecl<T>> {
        let id = Self::parse_type_id(node_type)?;
        self.blueprint(&id)
    }

    /// Returns all declarations.
    pub fn blueprints(&self) -> impl Iterator<Item = &BlueprintDecl<T>> {
        self.blueprints.values()
    }

    /// Returns declarations exposed as user-creatable catalog items.
    pub fn creatable_items(&self) -> Vec<UserCreatableItem> {
        let mut items = self
            .blueprints
            .values()
            .map(|decl| UserCreatableItem::new(decl.type_id(), decl.item_kind.clone(), decl.label.clone()))
            .collect::<Vec<_>>();
        items.sort_by(|left, right| left.node_type.cmp(&right.node_type));
        items
    }

    /// Registers one runtime instance root.
    pub fn register_instance(&mut self, root: NodeId, meta: BlueprintInstanceMeta) {
        if let Some(previous) = self.instance_meta_by_root.remove(&root) {
            if let Some(roots) = self.instances_by_blueprint.get_mut(&previous.blueprint_id) {
                roots.retain(|candidate| *candidate != root);
                if roots.is_empty() {
                    self.instances_by_blueprint.remove(&previous.blueprint_id);
                }
            }
        }

        self.instances_by_blueprint.entry(meta.blueprint_id.clone()).or_default().push(root);
        self.instance_meta_by_root.insert(root, meta);
    }

    /// Removes one runtime instance root.
    pub fn unregister_instance(&mut self, root: NodeId) -> Option<BlueprintInstanceMeta> {
        let removed = self.instance_meta_by_root.remove(&root)?;
        if let Some(roots) = self.instances_by_blueprint.get_mut(&removed.blueprint_id) {
            roots.retain(|candidate| *candidate != root);
            if roots.is_empty() {
                self.instances_by_blueprint.remove(&removed.blueprint_id);
            }
        }
        Some(removed)
    }

    /// Returns instance metadata for one root.
    pub fn instance_meta(&self, root: NodeId) -> Option<&BlueprintInstanceMeta> {
        self.instance_meta_by_root.get(&root)
    }

    /// Returns mutable instance metadata for one root.
    pub fn instance_meta_mut(&mut self, root: NodeId) -> Option<&mut BlueprintInstanceMeta> {
        self.instance_meta_by_root.get_mut(&root)
    }

    /// Returns known instance roots for one blueprint id.
    pub fn instance_roots_for(&self, blueprint_id: &BlueprintId) -> &[NodeId] {
        self.instances_by_blueprint.get(blueprint_id).map(Vec::as_slice).unwrap_or(&[])
    }
}
