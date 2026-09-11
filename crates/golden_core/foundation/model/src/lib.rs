//! Stable identities shared by Golden model, runtime, protocol, and UI layers.

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

mod curve;
mod event;
mod log;
mod presentation;

pub use curve::{CurveBezierFitOptions, CurveFitPoint};
pub use event::CustomEventRetention;
pub use log::{LogLevel, LogRecord};
pub use presentation::{NodeWarning, PresentationHint};

/// Process-local identifier for a materialized model entity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
pub struct NodeId(pub u64);

/// Persistent UUID assigned to a model entity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
pub struct NodeUuid(pub Uuid);

impl NodeUuid {
    /// Returns the nil UUID value.
    #[must_use]
    pub fn nil() -> Self {
        Self(Uuid::nil())
    }

    /// Returns `true` when this UUID is nil.
    #[must_use]
    pub fn is_nil(&self) -> bool {
        self.0.is_nil()
    }
}

impl Default for NodeUuid {
    fn default() -> Self {
        Self::nil()
    }
}

/// Declaration identifier used to refer to model definitions.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
pub struct DeclId(pub String);

/// Logical time shared by the engine, protocol, and host layers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
pub struct EngineTime {
    /// Monotonic engine tick counter. Increments only on an engine tick.
    pub tick: u64,

    /// Micro-step index within the same tick.
    ///
    /// Zero is the main tick pass; later values identify stabilization or
    /// immediate-flush rounds within that tick.
    pub micro: u32,

    /// Total ordering within the same `(tick, micro)` pair.
    pub seq: u32,
}

fn is_false(value: &bool) -> bool {
    !*value
}

/// User-edit permissions shared by model, persistence, protocol, and editor layers.
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

/// App-provided project file metadata consumed by hosts and UIs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectFileSpec {
    /// Human-readable name for one project document, such as `Noisette`.
    pub display_name: &'static str,
    /// Preferred filename extension without a leading dot.
    pub extension: &'static str,
}

impl ProjectFileSpec {
    /// Creates one project-file descriptor.
    pub const fn new(display_name: &'static str, extension: &'static str) -> Self {
        Self {
            display_name,
            extension,
        }
    }

    /// Returns the normalized extension used by hosts and transports.
    pub fn normalized_extension(&self) -> String {
        let normalized = self.extension.trim().trim_start_matches('.').to_ascii_lowercase();
        if normalized.is_empty() {
            "json".to_string()
        } else {
            normalized
        }
    }

    /// Returns the human-readable label, falling back to a generic default.
    pub fn normalized_display_name(&self) -> String {
        let normalized = self.display_name.trim();
        if normalized.is_empty() {
            "Project".to_string()
        } else {
            normalized.to_string()
        }
    }
}

impl Default for ProjectFileSpec {
    fn default() -> Self {
        Self::new("Project", "json")
    }
}

#[cfg(test)]
mod tests;
