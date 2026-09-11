//! Engine-independent scripting declarations shared by runtimes, protocol, and tooling.

use golden_model::{DeclId, NodeId};
use golden_parameters::{ParamValue, ParameterConstraints, ParameterUiHints};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// UI payload for script source configuration.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ScriptUiSource {
    /// Inline source text stored in node state.
    Inline {
        /// Inline script text.
        text: String,
    },
    /// Project-relative file path.
    ProjectFile {
        /// Project-relative script path.
        path: String,
    },
}

/// UI payload for script source configuration.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct ScriptUiConfig {
    /// Script source selector.
    pub source: ScriptUiSource,
}

/// UI-facing script node runtime state payload.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct ScriptUiState {
    /// Current script configuration.
    pub config: ScriptUiConfig,
    /// Effective update rate used by the scheduler.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effective_update_rate_hz: Option<u32>,
    /// Export names currently available.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub export_names: Vec<String>,
    /// Last successfully parsed manifest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manifest: Option<ScriptManifest>,
}

/// Supported script parameter value families.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum ScriptValueType {
    /// Trigger pulse.
    Trigger,
    /// Signed integer.
    Int,
    /// Floating-point scalar.
    Float,
    /// UTF-8 string.
    Str,
    /// Project/user file path.
    File,
    /// Enum variant id string.
    Enum,
    /// Boolean.
    Bool,
    /// CSS scalar value.
    #[serde(rename = "css_value")]
    CssValue,
    /// 2D vector.
    Vec2,
    /// 3D vector.
    Vec3,
    /// RGBA color.
    Color,
    /// Node reference.
    Reference,
}

impl ScriptValueType {
    /// Parses a manifest value-family label.
    pub fn from_manifest_label(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "trigger" => Some(Self::Trigger),
            "int" => Some(Self::Int),
            "float" => Some(Self::Float),
            "str" | "string" => Some(Self::Str),
            "file" | "path" => Some(Self::File),
            "enum" => Some(Self::Enum),
            "bool" | "boolean" => Some(Self::Bool),
            "css_value" | "css-value" | "cssvalue" => Some(Self::CssValue),
            "vec2" => Some(Self::Vec2),
            "vec3" => Some(Self::Vec3),
            "color" => Some(Self::Color),
            "reference" => Some(Self::Reference),
            _ => None,
        }
    }
}

/// Script selector for target nodes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
pub enum ScriptNodeSelector {
    /// Select by runtime node id.
    NodeId(NodeId),
    /// Select by path under script root.
    Path(String),
    /// Select from host anchor.
    HostPath(String),
    /// Select from root anchor.
    RootPath(String),
}

/// One subscription entry declared by a script.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct ScriptSubscriptionSpec {
    /// Target selector.
    pub node: ScriptNodeSelector,
    /// Maximum depth under target.
    pub max_depth: u32,
}

/// One exported function signature descriptor.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct ScriptFnSignature {
    /// Named argument labels for tooling.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    /// Optional return label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub returns: Option<String>,
}

/// One exported Rust-callable script function.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct ScriptExportSpec {
    /// Exported function name.
    pub name: String,
    /// Tooling signature metadata.
    #[serde(default)]
    pub signature: ScriptFnSignature,
}

/// Script-defined parameter descriptor.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct ScriptParameterSpec {
    /// Script-local stable name.
    pub name: String,
    /// Declaration id used when materialized as a node.
    pub decl_id: DeclId,
    /// Optional user-facing label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Declared parameter value family.
    pub value_type: ScriptValueType,
    /// Default value.
    pub default_value: ParamValue,
    /// Read-only flag.
    #[serde(default)]
    pub read_only: bool,
    /// Parameter constraints.
    #[serde(default)]
    pub constraints: ParameterConstraints,
    /// UI hints.
    #[serde(default)]
    pub ui_hints: ParameterUiHints,
}

/// Parsed script manifest.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct ScriptManifest {
    /// Manifest schema version.
    pub api_version: u32,
    /// Optional runtime update rate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub update_rate_hz: Option<u32>,
    /// Script-defined parameters.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parameters: Vec<ScriptParameterSpec>,
    /// Event subscriptions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub subscriptions: Vec<ScriptSubscriptionSpec>,
    /// Exported functions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exports: Vec<ScriptExportSpec>,
}

impl Default for ScriptManifest {
    fn default() -> Self {
        Self {
            api_version: 1,
            update_rate_hz: None,
            parameters: Vec::new(),
            subscriptions: Vec::new(),
            exports: Vec::new(),
        }
    }
}
