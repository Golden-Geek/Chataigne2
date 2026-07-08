use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use crate::color::Color;
use crate::parameter::ParamValue;

use super::{DeclId, NodeId, NodeUserPermissions, NodeUuid, UserNodeRole};

/// Semantic hints used for tooling, UX, and interpretation.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
pub struct SemanticsHint {
    /// Optional high-level intent of the node.
    pub intent: Option<String>,
    /// Optional unit for value-oriented nodes.
    pub unit: Option<String>,
}

/// Warning message shown in UI for a node.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct NodeWarning {
    /// Warning identifier. Empty string is the default id.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub id: String,
    /// Main warning message.
    pub message: String,
    /// Optional warning details.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl NodeWarning {
    /// Creates a warning with the default empty id.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            id: String::new(),
            message: message.into(),
            detail: None,
        }
    }

    /// Sets or replaces the warning id.
    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = id.into();
        self
    }

    /// Sets warning detail text.
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

impl Default for NodeWarning {
    fn default() -> Self {
        Self::new("")
    }
}

fn is_zero_u32(value: &u32) -> bool {
    *value == 0
}

fn default_nested_inspector_visibility() -> bool {
    true
}

fn is_true(value: &bool) -> bool {
    *value
}

fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
/// Node-level presentation hints persisted in metadata.
pub struct PresentationHint {
    /// User-selected UI color override.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,
    /// Backend-provided default UI color for this node kind or declaration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_color: Option<Color>,
    /// Preferred UI icon, as a data URI (e.g. `data:image/svg+xml;base64,...`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// Whether UI tree/inspector containers should start collapsed until a user chooses otherwise.
    #[serde(default, skip_serializing_if = "is_false")]
    pub collapsed: bool,
    /// Warnings attached to this node, keyed by warning id.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<NodeWarning>,
    /// If greater than zero, this node surfaces descendant warnings up to this depth.
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub show_child_warnings_max_depth: u32,
    /// Whether this node stays visible when rendered as a nested inspector child.
    ///
    /// Nodes default to visible at any nested inspector level. Set this to `false`
    /// only for item roots that should stay hidden unless selected directly.
    #[serde(default = "default_nested_inspector_visibility", skip_serializing_if = "is_true")]
    pub show_in_nested_inspector: bool,
    /// Whether this node is rendered in its parent's inspector content area.
    ///
    /// Nodes default to visible in inspector content. Set this to `false` for controls
    /// that are rendered by a custom inspector location such as a header action.
    #[serde(default = "default_nested_inspector_visibility", skip_serializing_if = "is_true")]
    pub show_in_inspector_content: bool,
}

impl Default for PresentationHint {
    fn default() -> Self {
        Self {
            color: None,
            default_color: None,
            icon: None,
            collapsed: false,
            warnings: Vec::new(),
            show_child_warnings_max_depth: 0,
            show_in_nested_inspector: default_nested_inspector_visibility(),
            show_in_inspector_content: default_nested_inspector_visibility(),
        }
    }
}

impl PresentationHint {
    /// Sets or replaces a warning by id.
    pub fn set_warning(&mut self, mut warning: NodeWarning) {
        if warning.id.is_empty() {
            warning.id = String::new();
        }

        if let Some(existing) = self.warnings.iter_mut().find(|existing| existing.id == warning.id) {
            *existing = warning;
            return;
        }

        self.warnings.push(warning);
    }

    /// Convenience wrapper for setting/replacing a warning by `warning_id`.
    ///
    /// `None` uses the default empty warning id.
    pub fn set_warning_message(
        &mut self,
        warning_id: Option<&str>,
        message: impl Into<String>,
        detail: Option<String>,
    ) {
        let warning = NodeWarning {
            id: warning_id.unwrap_or_default().to_string(),
            message: message.into(),
            detail,
        };
        self.set_warning(warning);
    }

    /// Clears one warning by id.
    ///
    /// `None` clears the warning using the default empty id.
    /// Returns `true` when a warning was removed.
    pub fn clear_warning(&mut self, warning_id: Option<&str>) -> bool {
        let warning_id = warning_id.unwrap_or_default();
        let Some(index) = self.warnings.iter().position(|warning| warning.id == warning_id) else {
            return false;
        };

        self.warnings.remove(index);
        true
    }

    /// Clears all warnings attached to this node.
    ///
    /// Returns `true` when at least one warning was removed.
    pub fn clear_warnings(&mut self) -> bool {
        if self.warnings.is_empty() {
            return false;
        }

        self.warnings.clear();
        true
    }

    /// Returns `true` when this node has at least one warning.
    pub fn has_warnings(&self) -> bool {
        !self.warnings.is_empty()
    }

    /// Returns the warning associated with `warning_id`.
    ///
    /// `None` addresses the default empty warning id.
    pub fn warning(&self, warning_id: Option<&str>) -> Option<&NodeWarning> {
        let warning_id = warning_id.unwrap_or_default();
        self.warnings.iter().find(|warning| warning.id == warning_id)
    }

    /// Sets the descendant warning visibility depth.
    ///
    /// Returns `true` when the value changed.
    pub fn set_child_warning_depth(&mut self, max_depth: u32) -> bool {
        if self.show_child_warnings_max_depth == max_depth {
            return false;
        }

        self.show_child_warnings_max_depth = max_depth;
        true
    }
}

/// Runtime node links and metadata.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NodeData {
    /// Stable node id assigned by the store.
    pub id: NodeId,
    /// Parent node id (`None` for root or detached nodes).
    pub parent: Option<NodeId>,
    /// First child in sibling chain.
    pub first_child: Option<NodeId>,
    /// Last child in sibling chain.
    pub last_child: Option<NodeId>,
    /// Previous sibling in parent chain.
    pub prev_sibling: Option<NodeId>,
    /// Next sibling in parent chain.
    pub next_sibling: Option<NodeId>,
    /// User-facing role for curation and container logic.
    pub user_role: UserNodeRole,
    /// User-facing metadata.
    pub meta: NodeMeta,
    /// Effective enabled state accounting for all ancestors.
    /// Written by the engine before each `on_effective_enabled_changed` call;
    /// nodes can read this instead of querying the tree snapshot.
    #[serde(skip)]
    pub effective_enabled: bool,
}

impl NodeData {
    /// Creates detached node data initialized with default metadata.
    pub fn new(label: String) -> Self {
        let meta = NodeMeta::new(label);

        Self {
            id: NodeId(0),
            parent: None,
            first_child: None,
            last_child: None,
            prev_sibling: None,
            next_sibling: None,
            user_role: UserNodeRole::Regular,
            meta,
            effective_enabled: true,
        }
    }
}

/// Metadata associated with a node.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NodeMeta {
    /// Persistent unique id for this node instance.
    pub uuid: NodeUuid,
    /// Declaration id used by schema and tools.
    pub decl_id: DeclId,
    /// Generated short name.
    pub short_name: String,
    /// Whether the node is currently enabled.
    pub enabled: bool,
    /// Whether the enabled flag may be toggled.
    pub can_be_disabled: bool,
    /// User-visible label.
    pub label: String,
    /// Optional free-form description.
    pub description: Option<String>,
    /// Canonical declaration-description key shared by repeated declared nodes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declared_description_key: Option<String>,
    /// Canonical declaration description before any instance-level override.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declared_description: Option<String>,
    /// Arbitrary classification tags.
    pub tags: Vec<String>,
    /// User-edit permissions for tooling and editor workflows.
    pub user_permissions: NodeUserPermissions,
    /// Semantic hints.
    pub semantics: SemanticsHint,
    /// Presentation hints.
    pub presentation: PresentationHint,
}

impl NodeMeta {
    /// Creates default metadata from a label.
    pub fn new(label: String) -> Self {
        let short_name = Self::generate_short_name(&label);

        Self {
            uuid: NodeUuid(Uuid::new_v4()),
            decl_id: DeclId(short_name.clone()),
            short_name,
            enabled: true,
            can_be_disabled: true,
            label,
            description: None,
            declared_description_key: None,
            declared_description: None,
            tags: vec![],
            user_permissions: NodeUserPermissions::default(),
            semantics: SemanticsHint::default(),
            presentation: PresentationHint::default(),
        }
    }

    /// Sets a description.
    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }

    /// Sets the canonical declaration-backed description for this node.
    pub fn with_declared_description(mut self, key: impl Into<String>, description: impl Into<String>) -> Self {
        self.set_declared_description(key, description);
        self
    }

    /// Replaces the canonical declaration-backed description for this node.
    pub fn set_declared_description(&mut self, key: impl Into<String>, description: impl Into<String>) {
        let description = description.into();
        self.declared_description_key = Some(key.into());
        self.declared_description = Some(description.clone());
        self.description = Some(description);
    }

    /// Replaces tags.
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    /// Replaces user-edit permissions.
    pub fn with_user_permissions(mut self, user_permissions: NodeUserPermissions) -> Self {
        self.user_permissions = user_permissions;
        self
    }

    /// Replaces semantic hints.
    pub fn with_semantics(mut self, semantics: SemanticsHint) -> Self {
        self.semantics = semantics;
        self
    }

    /// Replaces presentation hints.
    pub fn with_presentation(mut self, presentation: PresentationHint) -> Self {
        self.presentation = presentation;
        self
    }

    /// Sets enablement metadata.
    pub fn with_enabled(mut self, enabled: bool, can_be_disabled: bool) -> Self {
        self.enabled = enabled;
        self.can_be_disabled = can_be_disabled;
        self
    }

    /// Sets or replaces one warning by id.
    ///
    /// `warning_id = None` uses the default empty warning id.
    pub fn set_warning(&mut self, warning_id: Option<&str>, message: impl Into<String>, detail: Option<&str>) {
        self.presentation
            .set_warning_message(warning_id, message, detail.map(str::to_string));
    }

    /// Clears one warning by id.
    ///
    /// `warning_id = None` clears the default empty warning id.
    pub fn clear_warning(&mut self, warning_id: Option<&str>) -> bool {
        self.presentation.clear_warning(warning_id)
    }

    /// Clears all node warnings.
    pub fn clear_warnings(&mut self) -> bool {
        self.presentation.clear_warnings()
    }

    /// Sets how many child levels should be included when surfacing warnings.
    ///
    /// Returns `true` when the value changed.
    pub fn set_child_warning_depth(&mut self, max_depth: u32) -> bool {
        self.presentation.set_child_warning_depth(max_depth)
    }

    fn generate_short_name(label: &String) -> String {
        let mut short_name = String::new();
        let mut capitalize_next = false;
        for c in label.chars() {
            if c.is_alphanumeric() {
                if capitalize_next {
                    short_name.push(c.to_ascii_uppercase());
                    capitalize_next = false;
                } else {
                    short_name.push(c.to_ascii_lowercase());
                }
            } else if c == '+' {
                short_name.push_str(if capitalize_next { "Plus" } else { "plus" });
                capitalize_next = false;
            } else if c == '-' {
                short_name.push_str(if capitalize_next { "Minus" } else { "minus" });
                capitalize_next = false;
            } else {
                capitalize_next = true;
            }
        }
        short_name
    }
}

/// Patch payload for metadata updates.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
pub struct NodeMetaPatch {
    /// Optional short name replacement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub short_name: Option<String>,
    /// Optional enabled-state replacement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// Optional disablement capability replacement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub can_be_disabled: Option<bool>,
    /// Optional label replacement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Optional description replacement (`Some(None)` clears the description).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<Option<String>>,
    /// Optional tags replacement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    /// Optional user-edit permissions replacement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_permissions: Option<NodeUserPermissions>,
    /// Optional semantic hints replacement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantics: Option<SemanticsHint>,
    /// Optional presentation hints replacement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presentation: Option<PresentationHint>,
}

impl NodeMetaPatch {
    /// Applies this patch to runtime metadata.
    pub fn apply_to(&self, meta: &mut NodeMeta) {
        if let Some(short_name) = &self.short_name {
            meta.short_name = short_name.clone();
        }
        if let Some(enabled) = self.enabled {
            meta.enabled = enabled;
        }
        if let Some(can_be_disabled) = self.can_be_disabled {
            meta.can_be_disabled = can_be_disabled;
        }
        if let Some(label) = &self.label {
            meta.label = label.clone();
        }
        if let Some(description) = &self.description {
            meta.description = description.clone();
        }
        if let Some(tags) = &self.tags {
            meta.tags = tags.clone();
        }
        if let Some(user_permissions) = &self.user_permissions {
            meta.user_permissions = user_permissions.clone();
        }
        if let Some(semantics) = &self.semantics {
            meta.semantics = semantics.clone();
        }
        if let Some(presentation) = &self.presentation {
            meta.presentation = presentation.clone();
        }
    }
}

/// Script-facing property/method metadata provided by one node instance.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct NodeScriptDescriptor {
    /// Script-readable properties exposed on the node object.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub properties: HashMap<String, ParamValue>,
    /// Script-callable method names exposed on the node object.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub methods: Vec<String>,
}

impl NodeScriptDescriptor {
    /// Creates the standard script descriptor shared by all node proxies.
    pub fn for_node(node_data: &NodeData, node_type: &str) -> Self {
        let _ = (node_data, node_type);
        Self::default()
    }

    /// Adds one script-callable method if it is not already present.
    pub fn add_method(&mut self, method: impl AsRef<str>) {
        let method = method.as_ref();
        if self.methods.iter().any(|candidate| candidate == method) {
            return;
        }
        self.methods.push(method.to_string());
    }

    /// Adds script-callable methods if they are not already present.
    pub fn add_methods<'a>(&mut self, methods: impl IntoIterator<Item = &'a str>) {
        for method in methods {
            self.add_method(method);
        }
    }
}

#[cfg(test)]
mod metadata_tests;
