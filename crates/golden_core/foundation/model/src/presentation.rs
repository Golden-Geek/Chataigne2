use golden_values::ColorValue as Color;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

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

/// Node-level presentation hints persisted in metadata.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct PresentationHint {
    /// User-selected UI color override.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,
    /// Backend-provided default UI color for this node kind or declaration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_color: Option<Color>,
    /// Preferred UI icon, as a data URI.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// Whether UI containers should start collapsed.
    #[serde(default, skip_serializing_if = "is_false")]
    pub collapsed: bool,
    /// Warnings attached to this node, keyed by warning id.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<NodeWarning>,
    /// If greater than zero, surface descendant warnings up to this depth.
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub show_child_warnings_max_depth: u32,
    /// Whether this node stays visible when rendered as a nested inspector child.
    #[serde(default = "default_nested_inspector_visibility", skip_serializing_if = "is_true")]
    pub show_in_nested_inspector: bool,
    /// Whether this node is rendered in its parent's inspector content area.
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
        } else {
            self.warnings.push(warning);
        }
    }

    /// Sets or replaces a warning message by id.
    pub fn set_warning_message(
        &mut self,
        warning_id: Option<&str>,
        message: impl Into<String>,
        detail: Option<String>,
    ) {
        self.set_warning(NodeWarning {
            id: warning_id.unwrap_or_default().to_string(),
            message: message.into(),
            detail,
        });
    }

    /// Clears one warning by id.
    pub fn clear_warning(&mut self, warning_id: Option<&str>) -> bool {
        let warning_id = warning_id.unwrap_or_default();
        let Some(index) = self.warnings.iter().position(|warning| warning.id == warning_id) else {
            return false;
        };
        self.warnings.remove(index);
        true
    }

    /// Clears all warnings.
    pub fn clear_warnings(&mut self) -> bool {
        if self.warnings.is_empty() {
            return false;
        }
        self.warnings.clear();
        true
    }

    /// Returns whether at least one warning is attached.
    pub fn has_warnings(&self) -> bool {
        !self.warnings.is_empty()
    }

    /// Returns a warning by id.
    pub fn warning(&self, warning_id: Option<&str>) -> Option<&NodeWarning> {
        let warning_id = warning_id.unwrap_or_default();
        self.warnings.iter().find(|warning| warning.id == warning_id)
    }

    /// Sets the descendant warning visibility depth.
    pub fn set_child_warning_depth(&mut self, max_depth: u32) -> bool {
        if self.show_child_warnings_max_depth == max_depth {
            return false;
        }
        self.show_child_warnings_max_depth = max_depth;
        true
    }
}
