use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::parameter::ParamValue;

use super::{DeclId, Node};

fn is_false(value: &bool) -> bool {
    !*value
}

/// User-edit permissions for UI tooling and editor workflows.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct NodeUserPermissions {
    /// Whether the node label can be edited by users.
    #[serde(default, skip_serializing_if = "is_false")]
    pub can_edit_name: bool,
    /// Whether the node can be removed or duplicated.
    #[serde(default, skip_serializing_if = "is_false")]
    pub can_remove_and_duplicate: bool,
    /// Whether parameter constraints can be edited.
    #[serde(default, skip_serializing_if = "is_false")]
    pub can_edit_constraints: bool,
    /// Whether metadata tags can be edited.
    #[serde(default, skip_serializing_if = "is_false")]
    pub can_edit_tags: bool,
    /// Whether presentation color can be edited.
    #[serde(default, skip_serializing_if = "is_false")]
    pub can_edit_color: bool,
}

impl NodeUserPermissions {
    /// Returns a permission set with every capability disabled.
    pub const fn none() -> Self {
        Self {
            can_edit_name: false,
            can_remove_and_duplicate: false,
            can_edit_constraints: false,
            can_edit_tags: false,
            can_edit_color: false,
        }
    }

    /// Returns a permission set with every capability enabled.
    pub const fn all() -> Self {
        Self {
            can_edit_name: true,
            can_remove_and_duplicate: true,
            can_edit_constraints: true,
            can_edit_tags: true,
            can_edit_color: true,
        }
    }
}

/// Classification for user-managed structure inside the runtime tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default, TS)]
#[serde(rename_all = "camelCase")]
pub enum UserNodeRole {
    /// Regular runtime node (internal/generated or non-curated).
    #[default]
    Regular,
    /// User-curated item root inside a container.
    ItemRoot,
}

/// Declarative container admission rules for user-curated items.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UserContainerRules {
    /// Supported item kinds accepted by this container.
    pub accepts_item_kinds: &'static [&'static str],
}

impl UserContainerRules {
    /// Creates a new static container rule set.
    pub const fn new(accepts_item_kinds: &'static [&'static str]) -> Self {
        Self { accepts_item_kinds }
    }

    /// Returns `true` when `item_kind` is accepted by this container.
    ///
    /// The wildcard `"*"` accepts any item kind.
    pub fn accepts(&self, item_kind: &str) -> bool {
        self.accepts_item_kinds
            .iter()
            .any(|kind| *kind == "*" || *kind == item_kind)
    }
}

/// UI-facing descriptor for a user-creatable item node type.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct UserCreatableItemInitialParam {
    /// Direct child decl id on the newly-created root node.
    pub decl_id: DeclId,
    /// Initial value to assign.
    pub value: ParamValue,
}

impl UserCreatableItemInitialParam {
    /// Creates a direct parameter initializer for a newly-created catalog item.
    pub fn new(decl_id: impl Into<String>, value: ParamValue) -> Self {
        Self {
            decl_id: DeclId(decl_id.into()),
            value,
        }
    }
}

/// UI-facing descriptor for a user-creatable item node type.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct UserCreatableItem {
    /// Runtime node type identifier.
    pub node_type: String,
    /// Logical user-item kind used for container admission.
    pub item_kind: String,
    /// Suggested default label for newly created items.
    pub label: String,
    /// Optional Add menu submenu path, excluding the item label.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub menu_path: Vec<String>,
    /// Optional direct parameter values applied immediately after creation.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub initial_params: Vec<UserCreatableItemInitialParam>,
    /// Whether UI creation flows should auto-select the created item.
    pub select_when_created: bool,
}

impl UserCreatableItem {
    /// Creates a new user-creatable item descriptor.
    pub fn new(node_type: impl Into<String>, item_kind: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            node_type: node_type.into(),
            item_kind: item_kind.into(),
            label: label.into(),
            menu_path: Vec::new(),
            initial_params: Vec::new(),
            select_when_created: true,
        }
    }

    /// Places the item under a nested Add menu path.
    pub fn with_menu_path<I, S>(mut self, menu_path: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.menu_path = menu_path
            .into_iter()
            .map(|segment| segment.as_ref().trim().to_string())
            .filter(|segment| !segment.is_empty())
            .collect();
        self
    }

    /// Adds one direct parameter initializer applied after the item is created.
    pub fn with_initial_param(mut self, decl_id: impl Into<String>, value: ParamValue) -> Self {
        self.initial_params
            .push(UserCreatableItemInitialParam::new(decl_id, value));
        self
    }

    /// Replaces direct parameter initializers applied after the item is created.
    pub fn with_initial_params<I>(mut self, initial_params: I) -> Self
    where
        I: IntoIterator<Item = UserCreatableItemInitialParam>,
    {
        self.initial_params = initial_params.into_iter().collect();
        self
    }

    /// Overrides whether UI creation flows should auto-select the created item.
    pub fn with_select_when_created(mut self, select_when_created: bool) -> Self {
        self.select_when_created = select_when_created;
        self
    }
}

/// Compile-time metadata and construction hook for declared user-item node types.
pub trait DeclaredUserItemNode: Node + Sized {
    /// Runtime node type identifier exposed by the item declaration.
    const ITEM_NODE_TYPE: &'static str;
    /// Logical user-item kind used for container admission.
    const ITEM_KIND: &'static str;

    /// Default label shown for newly created instances of this item.
    fn item_default_label() -> String;

    /// Optional Add menu submenu path, excluding the item label.
    fn item_menu_path() -> Vec<String> {
        Vec::new()
    }

    /// Creates a new item instance using the item's own constructor policy.
    fn create_item() -> Self;
}
