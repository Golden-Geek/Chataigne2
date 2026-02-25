use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

use mlua::{Function, Lua, LuaSerdeExt, RegistryKey, Table, Value};
use rquickjs::function::{Func as QuickJsFunc, MutFn as QuickJsMutFn};
use rquickjs::{
    Array as QuickJsArray, Context as QuickJsContext, Ctx as QuickJsCtx, Error as QuickJsError,
    Function as QuickJsFunction, IntoJs as _, Object as QuickJsObject,
    Runtime as QuickJsRuntimeHandle, Value as QuickJsValue,
};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use crate::engine::NodeExecutionRule;
use crate::events::{CustomEvent, Event, EventKind};
use crate::logger;
use crate::node::{DeclId, Node, NodeData, NodeId};
use crate::parameter::{
    FileConstraints, FileTypeGroup, ParamValue, ParameterConstraintPolicy, ParameterConstraints,
    ParameterEnumOption, ParameterUiHints, RangeConstraint,
};
use crate::process_ctx::ProcessCtx;

/// Script runtime capability flags.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ScriptCapability {
    /// Read one parameter value.
    ParamRead,
    /// Set one parameter value.
    ParamWrite,
    /// Read node metadata.
    NodeRead,
    /// Patch node metadata.
    NodePatchMeta,
    /// Add node under an allowed container.
    NodeAdd,
    /// Remove an allowed node.
    NodeRemove,
    /// Move an allowed node.
    NodeMove,
    /// Register event subscriptions.
    EventSubscribe,
    /// Emit custom events.
    EventEmit,
    /// Emit declarative UI contributions.
    UiContribute,
}

/// Ordered set wrapper for script capabilities.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptCapabilitySet {
    /// Enabled capabilities.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub values: Vec<ScriptCapability>,
}

impl ScriptCapabilitySet {
    /// Creates an empty capability set.
    pub fn none() -> Self {
        Self { values: Vec::new() }
    }

    /// Creates a capability set from an iterator.
    pub fn from_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = ScriptCapability>,
    {
        let mut ordered = BTreeSet::new();
        for value in iter {
            ordered.insert(value);
        }
        Self { values: ordered.into_iter().collect() }
    }

    /// Returns `true` when the set contains `capability`.
    pub fn contains(&self, capability: ScriptCapability) -> bool {
        self.values.binary_search(&capability).is_ok()
    }
}

/// Scope root used for script-relative path resolution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ScriptRootMode {
    /// Use engine root as script scope root.
    EngineRoot,
    /// Use script host node as scope root.
    HostNode,
    /// Use a decl-id child path under the host node.
    RelativeDeclPath(Vec<String>),
}

impl Default for ScriptRootMode {
    fn default() -> Self {
        Self::HostNode
    }
}

/// Script-host policy for one node type.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptHostPolicy {
    /// Whether script hosting is enabled for this node.
    pub enabled: bool,
    /// Maximum number of script child nodes allowed under this host.
    pub max_scripts: u16,
    /// Root mode used to resolve local selectors.
    #[serde(default)]
    pub script_root_mode: ScriptRootMode,
    /// Capabilities granted to scripts hosted by this node.
    #[serde(default)]
    pub capabilities: ScriptCapabilitySet,
    /// Whether graph structure edits are allowed from scripts.
    #[serde(default)]
    pub allow_structural_mutation: bool,
    /// Whether UI contributions are allowed from scripts.
    #[serde(default)]
    pub allow_ui_contributions: bool,
}

impl Default for ScriptHostPolicy {
    fn default() -> Self {
        Self {
            enabled: false,
            max_scripts: 0,
            script_root_mode: ScriptRootMode::HostNode,
            capabilities: ScriptCapabilitySet::none(),
            allow_structural_mutation: false,
            allow_ui_contributions: false,
        }
    }
}

impl ScriptHostPolicy {
    /// Default policy used by `#[node(scriptable)]` and `#[item(..., scriptable)]`.
    pub fn default_scriptable() -> Self {
        Self {
            enabled: true,
            max_scripts: 4,
            script_root_mode: ScriptRootMode::HostNode,
            capabilities: ScriptCapabilitySet::from_iter([
                ScriptCapability::ParamRead,
                ScriptCapability::ParamWrite,
                ScriptCapability::NodeRead,
                ScriptCapability::NodePatchMeta,
                ScriptCapability::EventSubscribe,
                ScriptCapability::EventEmit,
            ]),
            allow_structural_mutation: false,
            allow_ui_contributions: false,
        }
    }
}

/// Hard safety guardrails applied per script instance.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptBudgets {
    /// Maximum Lua instructions target for one callback.
    pub max_instructions_per_callback: u64,
    /// Maximum callback wall time in microseconds.
    pub max_wall_time_us_per_callback: u64,
    /// Maximum runtime memory target in bytes.
    pub max_memory_bytes: usize,
    /// Maximum host API calls per callback.
    pub max_host_calls_per_callback: u32,
    /// Maximum edits that may be emitted in one tick.
    pub max_emitted_edits_per_tick: u32,
    /// Maximum custom events that may be emitted in one tick.
    pub max_emitted_events_per_tick: u32,
    /// Maximum UI payload bytes per tick.
    pub max_ui_payload_bytes_per_tick: usize,
}

impl Default for ScriptBudgets {
    fn default() -> Self {
        Self {
            max_instructions_per_callback: 200_000,
            max_wall_time_us_per_callback: 5_000,
            max_memory_bytes: 16 * 1024 * 1024,
            max_host_calls_per_callback: 1_024,
            max_emitted_edits_per_tick: 512,
            max_emitted_events_per_tick: 512,
            max_ui_payload_bytes_per_tick: 64 * 1024,
        }
    }
}

/// Script source selection.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ScriptSource {
    /// Inline source text stored in node state.
    Inline(String),
    /// Project-relative file path.
    ProjectFile(String),
}

impl ScriptSource {
    /// Resolves this source to an on-disk path when file-backed.
    pub fn resolve_path(&self, project_root: Option<&Path>) -> Option<PathBuf> {
        match self {
            Self::Inline(_) => None,
            Self::ProjectFile(path) => {
                let mut resolved = PathBuf::new();
                if let Some(root) = project_root {
                    resolved.push(root);
                }
                resolved.push(path);
                Some(resolved)
            }
        }
    }

    /// Loads source text using `project_root` when needed.
    pub fn load_text(&self, project_root: Option<&Path>) -> Result<String, ScriptRuntimeError> {
        match self {
            Self::Inline(text) => Ok(text.clone()),
            Self::ProjectFile(path) => {
                let resolved = self.resolve_path(project_root).unwrap_or_else(|| PathBuf::from(path));
                std::fs::read_to_string(&resolved).map_err(|err| ScriptRuntimeError::Io(format!("failed to read script file '{}': {err}", resolved.display())))
            }
        }
    }
}

/// Runtime engine used by one script instance.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ScriptRuntimeKind {
    /// Luau VM backend (`.lua`, `.luau`).
    Luau,
    /// QuickJS VM backend (`.js`, `.mjs`, `.cjs`).
    QuickJs,
}

impl ScriptRuntimeKind {
    fn from_path(path: &Path) -> Option<Self> {
        let ext = path.extension()?.to_str()?.trim().to_ascii_lowercase();
        match ext.as_str() {
            "lua" | "luau" => Some(Self::Luau),
            "js" | "mjs" | "cjs" => Some(Self::QuickJs),
            _ => None,
        }
    }
}

/// Runtime script node configuration.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptNodeConfig {
    /// Script source.
    pub source: ScriptSource,
    /// Optional explicit runtime hint used when source extension does not select one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_hint: Option<ScriptRuntimeKind>,
    /// Whether runtime file changes should trigger reload.
    pub auto_reload: bool,
    /// Whether script execution is enabled.
    pub enabled: bool,
    /// Optional requested update rate override.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_update_rate_hz: Option<u32>,
    /// Optional project root used by `ScriptSource::ProjectFile`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_root: Option<PathBuf>,
}

impl ScriptNodeConfig {
    fn detect_runtime_kind(&self) -> Result<ScriptRuntimeKind, ScriptRuntimeError> {
        if let Some(path) = self.source.resolve_path(self.project_root.as_deref()) {
            if let Some(kind) = ScriptRuntimeKind::from_path(&path) {
                return Ok(kind);
            }
        }

        if let Some(kind) = self.runtime_hint {
            return Ok(kind);
        }

        match self.source {
            ScriptSource::Inline(_) => Ok(ScriptRuntimeKind::Luau),
            ScriptSource::ProjectFile(ref path) => Err(ScriptRuntimeError::InvalidManifest(format!(
                "unable to infer runtime kind from script file '{path}', set runtime_hint explicitly"
            ))),
        }
    }
}

impl Default for ScriptNodeConfig {
    fn default() -> Self {
        Self {
            source: ScriptSource::Inline("return { api_version = 1 }".to_string()),
            runtime_hint: Some(ScriptRuntimeKind::Luau),
            auto_reload: true,
            enabled: true,
            requested_update_rate_hz: Some(60),
            project_root: None,
        }
    }
}

/// UI payload for script source configuration.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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

impl From<&ScriptSource> for ScriptUiSource {
    fn from(value: &ScriptSource) -> Self {
        match value {
            ScriptSource::Inline(text) => Self::Inline { text: text.clone() },
            ScriptSource::ProjectFile(path) => Self::ProjectFile { path: path.clone() },
        }
    }
}

impl From<ScriptUiSource> for ScriptSource {
    fn from(value: ScriptUiSource) -> Self {
        match value {
            ScriptUiSource::Inline { text } => Self::Inline(text),
            ScriptUiSource::ProjectFile { path } => Self::ProjectFile(path),
        }
    }
}

/// UI payload for script runtime configuration.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptUiConfig {
    /// Script source selector.
    pub source: ScriptUiSource,
    /// Optional runtime hint used when source extension does not select one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_hint: Option<ScriptRuntimeKind>,
    /// Whether runtime file changes should trigger reload.
    pub auto_reload: bool,
    /// Whether script execution is enabled.
    pub enabled: bool,
    /// Optional requested update-rate override.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_update_rate_hz: Option<u32>,
    /// Optional project root used by file-backed sources.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_root: Option<String>,
}

impl From<&ScriptNodeConfig> for ScriptUiConfig {
    fn from(value: &ScriptNodeConfig) -> Self {
        Self {
            source: ScriptUiSource::from(&value.source),
            runtime_hint: value.runtime_hint,
            auto_reload: value.auto_reload,
            enabled: value.enabled,
            requested_update_rate_hz: value.requested_update_rate_hz,
            project_root: value.project_root.as_ref().map(|path| path.to_string_lossy().to_string()),
        }
    }
}

impl From<ScriptUiConfig> for ScriptNodeConfig {
    fn from(value: ScriptUiConfig) -> Self {
        Self {
            source: value.source.into(),
            runtime_hint: value.runtime_hint,
            auto_reload: value.auto_reload,
            enabled: value.enabled,
            requested_update_rate_hz: value.requested_update_rate_hz,
            project_root: value.project_root.map(PathBuf::from),
        }
    }
}

/// UI-facing script node runtime state payload.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScriptUiState {
    /// Current script configuration.
    pub config: ScriptUiConfig,
    /// Currently active runtime kind when loaded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_kind: Option<ScriptRuntimeKind>,
    /// Effective update-rate used by scheduler.
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
    fn from_manifest_label(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "trigger" => Some(Self::Trigger),
            "int" => Some(Self::Int),
            "float" => Some(Self::Float),
            "str" | "string" => Some(Self::Str),
            "file" | "path" => Some(Self::File),
            "enum" => Some(Self::Enum),
            "bool" | "boolean" => Some(Self::Bool),
            "vec2" => Some(Self::Vec2),
            "vec3" => Some(Self::Vec3),
            "color" => Some(Self::Color),
            "reference" => Some(Self::Reference),
            _ => None,
        }
    }
}

/// Script selector for target nodes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
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
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptSubscriptionSpec {
    /// Target selector.
    pub node: ScriptNodeSelector,
    /// Maximum depth under target.
    pub max_depth: u32,
}

/// One exported function signature descriptor.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptFnSignature {
    /// Named argument labels for tooling.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    /// Optional return label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub returns: Option<String>,
}

/// One exported Rust-callable script function.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptExportSpec {
    /// Exported function name.
    pub name: String,
    /// Tooling signature metadata.
    #[serde(default)]
    pub signature: ScriptFnSignature,
}

/// Script-defined parameter descriptor.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScriptParameterSpec {
    /// Script-local stable name.
    pub name: String,
    /// Decl-id used when materialized as node.
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

/// Script menu contribution descriptor.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptMenuContribution {
    /// Stable menu entry id.
    pub id: String,
    /// Display label.
    pub label: String,
}

/// Script panel contribution descriptor.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptPanelContribution {
    /// Stable panel id.
    pub id: String,
    /// Display title.
    pub title: String,
}

/// Script draw contribution descriptor.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptDrawContribution {
    /// Stable drawing channel id.
    pub id: String,
}

/// Declarative UI contribution schema.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptUiSpec {
    /// Menu contributions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub menus: Vec<ScriptMenuContribution>,
    /// Panel contributions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub panels: Vec<ScriptPanelContribution>,
    /// Draw-channel contributions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub drawings: Vec<ScriptDrawContribution>,
}

/// Parsed script manifest.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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
    /// UI contribution descriptors.
    #[serde(default)]
    pub ui: ScriptUiSpec,
    /// Requested runtime capabilities.
    #[serde(default)]
    pub requested_capabilities: ScriptCapabilitySet,
}

impl Default for ScriptManifest {
    fn default() -> Self {
        Self {
            api_version: 1,
            update_rate_hz: None,
            parameters: Vec::new(),
            subscriptions: Vec::new(),
            exports: Vec::new(),
            ui: ScriptUiSpec::default(),
            requested_capabilities: ScriptCapabilitySet::none(),
        }
    }
}

/// Runtime value exchanged for script export calls.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ScriptValue {
    /// Nil-like value.
    Nil,
    /// Boolean value.
    Bool(bool),
    /// Signed integer value.
    Int(i64),
    /// Floating-point value.
    Float(f64),
    /// String value.
    Str(String),
    /// Raw JSON payload.
    Json(JsonValue),
}

/// Script log levels accepted from script callbacks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScriptLogLevel {
    /// Informational log.
    Info,
    /// Success log.
    Success,
    /// Warning log.
    Warning,
    /// Error log.
    Error,
}

impl ScriptLogLevel {
    fn from_manifest_label(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "info" => Some(Self::Info),
            "success" => Some(Self::Success),
            "warning" | "warn" => Some(Self::Warning),
            "error" => Some(Self::Error),
            _ => None,
        }
    }

    fn to_logger_level(self) -> logger::LogLevel {
        match self {
            Self::Info => logger::LogLevel::Info,
            Self::Success => logger::LogLevel::Success,
            Self::Warning => logger::LogLevel::Warning,
            Self::Error => logger::LogLevel::Error,
        }
    }
}

/// Event view passed to scripts.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScriptEvent {
    /// Stable event kind label.
    pub kind: String,
    /// Optional event origin node id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<NodeId>,
    /// Event payload.
    pub payload: JsonValue,
}

impl From<&Event> for ScriptEvent {
    fn from(event: &Event) -> Self {
        let (kind, origin) = match &event.kind {
            EventKind::ParamChanged { param, .. } => ("paramChanged".to_string(), Some(*param)),
            EventKind::ChildAdded { child, .. } => ("childAdded".to_string(), Some(*child)),
            EventKind::ChildRemoved { parent, .. } => ("childRemoved".to_string(), Some(*parent)),
            EventKind::ChildReplaced { new, .. } => ("childReplaced".to_string(), Some(*new)),
            EventKind::ChildMoved { child, .. } => ("childMoved".to_string(), Some(*child)),
            EventKind::ChildReordered { child, .. } => ("childReordered".to_string(), Some(*child)),
            EventKind::NodeCreated { node } => ("nodeCreated".to_string(), Some(*node)),
            EventKind::NodeDeleted { .. } => ("nodeDeleted".to_string(), None),
            EventKind::MetaChanged { node, .. } => ("metaChanged".to_string(), Some(*node)),
            EventKind::Custom(custom) => ("custom".to_string(), custom.origin),
        };
        let payload = serde_json::to_value(&event.kind).unwrap_or(JsonValue::Null);
        Self { kind, origin, payload }
    }
}

/// Host bridge consumed by script runtimes.
pub trait ScriptHostBridge {
    /// Owning node id when available.
    fn owner_node(&self) -> Option<NodeId> {
        None
    }

    /// Current script wall time in seconds.
    fn time_seconds(&self) -> f64 {
        0.0
    }

    /// Current callback delta in seconds.
    fn delta_seconds(&self) -> f64 {
        0.0
    }

    /// Emit one log record.
    fn log(&mut self, level: ScriptLogLevel, message: &str);

    /// Emit one custom engine event.
    fn emit_custom(&mut self, topic: &str, payload: JsonValue) -> Result<(), String>;
}

/// Default host bridge used when no engine context is available.
pub struct NoopScriptHostBridge;

impl ScriptHostBridge for NoopScriptHostBridge {
    fn log(&mut self, level: ScriptLogLevel, message: &str) {
        let _ = logger::log_message(level.to_logger_level(), "script".to_string(), None, message.to_string());
    }

    fn emit_custom(&mut self, _topic: &str, _payload: JsonValue) -> Result<(), String> {
        Ok(())
    }
}

/// Error type returned by scripting operations.
#[derive(Debug)]
pub enum ScriptRuntimeError {
    /// Source loading failure.
    Io(String),
    /// Lua runtime error.
    Lua(String),
    /// QuickJS runtime error.
    QuickJs(String),
    /// Invalid script manifest.
    InvalidManifest(String),
    /// Missing export function.
    MissingExport(String),
    /// Runtime callback budget violation.
    BudgetViolation(String),
    /// Host bridge call failure.
    Host(String),
}

impl fmt::Display for ScriptRuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(message) => write!(f, "{message}"),
            Self::Lua(message) => write!(f, "luau runtime error: {message}"),
            Self::QuickJs(message) => write!(f, "quickjs runtime error: {message}"),
            Self::InvalidManifest(message) => write!(f, "invalid script manifest: {message}"),
            Self::MissingExport(name) => write!(f, "missing script export '{name}'"),
            Self::BudgetViolation(message) => write!(f, "script budget violation: {message}"),
            Self::Host(message) => write!(f, "script host error: {message}"),
        }
    }
}

impl std::error::Error for ScriptRuntimeError {}

impl From<mlua::Error> for ScriptRuntimeError {
    fn from(value: mlua::Error) -> Self {
        Self::Lua(value.to_string())
    }
}

impl From<QuickJsError> for ScriptRuntimeError {
    fn from(value: QuickJsError) -> Self {
        Self::QuickJs(value.to_string())
    }
}

/// Runtime trait contract for embeddable scripting engines.
pub trait ScriptRuntime: Send {
    /// Loads script source and returns parsed manifest.
    fn load(&mut self, source: &str) -> Result<ScriptManifest, ScriptRuntimeError>;
    /// Reloads script source and returns parsed manifest.
    fn reload(&mut self, source: &str) -> Result<ScriptManifest, ScriptRuntimeError>;
    /// Returns current manifest when loaded.
    fn manifest(&self) -> Option<&ScriptManifest>;
    /// Returns exported function names.
    fn export_names(&self) -> Vec<String>;
    /// Calls one exported function.
    fn call_export(&mut self, export_name: &str, args: &[ScriptValue], host: &mut dyn ScriptHostBridge) -> Result<ScriptValue, ScriptRuntimeError>;
    /// Calls `on_init` if declared.
    fn call_on_init(&mut self, host: &mut dyn ScriptHostBridge) -> Result<(), ScriptRuntimeError>;
    /// Calls `on_update` if declared.
    fn call_on_update(&mut self, host: &mut dyn ScriptHostBridge) -> Result<(), ScriptRuntimeError>;
    /// Calls `on_event` if declared.
    fn call_on_event(&mut self, event: &ScriptEvent, host: &mut dyn ScriptHostBridge) -> Result<(), ScriptRuntimeError>;
    /// Calls `on_destroy` if declared.
    fn call_on_destroy(&mut self, host: &mut dyn ScriptHostBridge) -> Result<(), ScriptRuntimeError>;
}

#[derive(Default)]
struct LuauEntrypoints {
    on_init: Option<RegistryKey>,
    on_update: Option<RegistryKey>,
    on_event: Option<RegistryKey>,
    on_destroy: Option<RegistryKey>,
    exports: BTreeMap<String, RegistryKey>,
}

enum ScriptHostOp {
    Log { level: ScriptLogLevel, message: String },
    EmitCustom { topic: String, payload: JsonValue },
}

/// Luau-backed script runtime.
pub struct LuauRuntime {
    lua: Lua,
    budgets: ScriptBudgets,
    entrypoints: LuauEntrypoints,
    manifest: Option<ScriptManifest>,
    host_ops: Arc<Mutex<Vec<ScriptHostOp>>>,
    host_call_counter: Arc<AtomicU32>,
}

impl LuauRuntime {
    /// Creates a new Luau runtime with budget guardrails.
    pub fn new(budgets: ScriptBudgets) -> Result<Self, ScriptRuntimeError> {
        let lua = Lua::new();
        let host_ops = Arc::new(Mutex::new(Vec::new()));
        let host_call_counter = Arc::new(AtomicU32::new(0));

        let mut runtime = Self {
            lua,
            budgets,
            entrypoints: LuauEntrypoints::default(),
            manifest: None,
            host_ops,
            host_call_counter,
        };
        runtime.install_host_api()?;
        Ok(runtime)
    }

    fn install_host_api(&mut self) -> Result<(), ScriptRuntimeError> {
        let gc_table = self.lua.create_table()?;
        let max_host_calls = self.budgets.max_host_calls_per_callback.max(1);

        let host_ops = Arc::clone(&self.host_ops);
        let host_call_counter = Arc::clone(&self.host_call_counter);
        let log_fn = self.lua.create_function(move |_, (level_label, message): (String, String)| {
            let call_count = host_call_counter.fetch_add(1, Ordering::Relaxed) + 1;
            if call_count > max_host_calls {
                return Err(mlua::Error::runtime("script host-call budget exceeded in current callback"));
            }

            let level = ScriptLogLevel::from_manifest_label(&level_label).ok_or_else(|| mlua::Error::runtime(format!("invalid log level '{level_label}'")))?;
            let mut guard = host_ops.lock().map_err(|_| mlua::Error::runtime("script host-op queue lock poisoned"))?;
            guard.push(ScriptHostOp::Log { level, message });
            Ok(())
        })?;
        gc_table.set("log", log_fn)?;

        let host_ops = Arc::clone(&self.host_ops);
        let host_call_counter = Arc::clone(&self.host_call_counter);
        let emit_fn = self.lua.create_function(move |lua, (topic, payload): (String, Value)| {
            let call_count = host_call_counter.fetch_add(1, Ordering::Relaxed) + 1;
            if call_count > max_host_calls {
                return Err(mlua::Error::runtime("script host-call budget exceeded in current callback"));
            }

            let payload = lua.from_value::<JsonValue>(payload).unwrap_or(JsonValue::Null);
            let mut guard = host_ops.lock().map_err(|_| mlua::Error::runtime("script host-op queue lock poisoned"))?;
            guard.push(ScriptHostOp::EmitCustom { topic, payload });
            Ok(())
        })?;
        gc_table.set("emit", emit_fn)?;

        self.lua.globals().set("gc", gc_table)?;
        Ok(())
    }

    fn reset_host_callback_state(&self) -> Result<(), ScriptRuntimeError> {
        self.host_call_counter.store(0, Ordering::Relaxed);
        let mut guard = self.host_ops.lock().map_err(|_| ScriptRuntimeError::Host("script host-op queue lock poisoned".to_string()))?;
        guard.clear();
        Ok(())
    }

    fn flush_host_ops(&self, host: &mut dyn ScriptHostBridge) -> Result<(), ScriptRuntimeError> {
        let mut drained = Vec::new();
        {
            let mut guard = self.host_ops.lock().map_err(|_| ScriptRuntimeError::Host("script host-op queue lock poisoned".to_string()))?;
            std::mem::swap(&mut drained, &mut *guard);
        }

        for op in drained {
            match op {
                ScriptHostOp::Log { level, message } => {
                    host.log(level, &message);
                }
                ScriptHostOp::EmitCustom { topic, payload } => {
                    host.emit_custom(&topic, payload).map_err(ScriptRuntimeError::Host)?;
                }
            }
        }
        Ok(())
    }

    fn to_lua_value(&self, value: &ScriptValue) -> Result<Value, ScriptRuntimeError> {
        Ok(match value {
            ScriptValue::Nil => Value::Nil,
            ScriptValue::Bool(value) => Value::Boolean(*value),
            ScriptValue::Int(value) => Value::Integer(i32::try_from(*value).map_err(|_| ScriptRuntimeError::InvalidManifest(format!("integer value {value} is outside luau i32 range")))?),
            ScriptValue::Float(value) => Value::Number(*value),
            ScriptValue::Str(value) => Value::String(self.lua.create_string(value)?),
            ScriptValue::Json(value) => self.lua.to_value(value)?,
        })
    }

    fn from_lua_value(&self, value: Value) -> Result<ScriptValue, ScriptRuntimeError> {
        let script_value = match value {
            Value::Nil => ScriptValue::Nil,
            Value::Boolean(value) => ScriptValue::Bool(value),
            Value::Integer(value) => ScriptValue::Int(value.into()),
            Value::Number(value) => ScriptValue::Float(value),
            Value::String(value) => ScriptValue::Str(value.to_str()?.to_string()),
            other => {
                let json = self.lua.from_value::<JsonValue>(other)?;
                ScriptValue::Json(json)
            }
        };
        Ok(script_value)
    }

    fn build_callback_ctx(&self, host: &dyn ScriptHostBridge) -> Result<Table, ScriptRuntimeError> {
        let ctx = self.lua.create_table()?;
        let gc: Table = self.lua.globals().get("gc")?;
        let log_fn: Function = gc.get("log")?;
        let emit_fn: Function = gc.get("emit")?;
        ctx.set("log", log_fn)?;
        ctx.set("emit", emit_fn)?;
        ctx.set("time_seconds", host.time_seconds())?;
        ctx.set("delta_seconds", host.delta_seconds())?;
        if let Some(owner) = host.owner_node() {
            ctx.set("owner_node_id", owner.0 as i64)?;
        }
        Ok(ctx)
    }

    fn callback_timed<T, F>(&self, phase_label: &str, callback: F) -> Result<T, ScriptRuntimeError>
    where
        F: FnOnce() -> Result<T, ScriptRuntimeError>,
    {
        let started_at = Instant::now();
        let output = callback()?;
        let elapsed = started_at.elapsed();
        let elapsed_limit = Duration::from_micros(self.budgets.max_wall_time_us_per_callback.max(1));
        if elapsed > elapsed_limit {
            return Err(ScriptRuntimeError::BudgetViolation(format!("{phase_label} callback exceeded wall-time budget: {:?} > {:?}", elapsed, elapsed_limit)));
        }
        Ok(output)
    }

    fn callback_function(&self, key: &RegistryKey) -> Result<Function, ScriptRuntimeError> {
        let function = self.lua.registry_value::<Function>(key)?;
        Ok(function)
    }

    fn parse_manifest(&self, root: &Table, export_names: Vec<String>) -> Result<ScriptManifest, ScriptRuntimeError> {
        let api_version = root.get::<Option<u32>>("api_version")?.unwrap_or(1);
        if api_version == 0 {
            return Err(ScriptRuntimeError::InvalidManifest("api_version must be >= 1".to_string()));
        }

        let update_rate_hz = root.get::<Option<u32>>("update_rate_hz")?;
        let requested_capabilities = parse_capability_set(root.get::<Option<Table>>("capabilities")?)?;
        let parameters = parse_parameter_specs(root.get::<Option<Table>>("parameters")?)?;
        let subscriptions = parse_subscription_specs(root.get::<Option<Table>>("subscriptions")?)?;

        let exports = export_names.into_iter().map(|name| ScriptExportSpec { name, signature: ScriptFnSignature::default() }).collect();

        Ok(ScriptManifest {
            api_version,
            update_rate_hz,
            parameters,
            subscriptions,
            exports,
            ui: ScriptUiSpec::default(),
            requested_capabilities,
        })
    }
}

impl ScriptRuntime for LuauRuntime {
    fn load(&mut self, source: &str) -> Result<ScriptManifest, ScriptRuntimeError> {
        self.entrypoints = LuauEntrypoints::default();
        self.manifest = None;

        let root_value = self.lua.load(source).eval::<Value>()?;
        let root_table = match root_value {
            Value::Table(table) => table,
            _ => return Err(ScriptRuntimeError::InvalidManifest("script must return a manifest table".to_string())),
        };

        if let Some(callback) = root_table.get::<Option<Function>>("on_init")? {
            self.entrypoints.on_init = Some(self.lua.create_registry_value(callback)?);
        }
        if let Some(callback) = root_table.get::<Option<Function>>("on_update")? {
            self.entrypoints.on_update = Some(self.lua.create_registry_value(callback)?);
        }
        if let Some(callback) = root_table.get::<Option<Function>>("on_event")? {
            self.entrypoints.on_event = Some(self.lua.create_registry_value(callback)?);
        }
        if let Some(callback) = root_table.get::<Option<Function>>("on_destroy")? {
            self.entrypoints.on_destroy = Some(self.lua.create_registry_value(callback)?);
        }

        let mut export_names = Vec::new();
        if let Some(exports_table) = root_table.get::<Option<Table>>("exports")? {
            for pair in exports_table.pairs::<String, Function>() {
                let (name, callback) = pair?;
                let registry_key = self.lua.create_registry_value(callback)?;
                self.entrypoints.exports.insert(name.clone(), registry_key);
                export_names.push(name);
            }
        }
        export_names.sort();

        let manifest = self.parse_manifest(&root_table, export_names)?;
        self.manifest = Some(manifest.clone());
        Ok(manifest)
    }

    fn reload(&mut self, source: &str) -> Result<ScriptManifest, ScriptRuntimeError> {
        let budgets = self.budgets;
        *self = Self::new(budgets)?;
        self.load(source)
    }

    fn manifest(&self) -> Option<&ScriptManifest> {
        self.manifest.as_ref()
    }

    fn export_names(&self) -> Vec<String> {
        let mut names = self.entrypoints.exports.keys().cloned().collect::<Vec<_>>();
        names.sort();
        names
    }

    fn call_export(&mut self, export_name: &str, args: &[ScriptValue], host: &mut dyn ScriptHostBridge) -> Result<ScriptValue, ScriptRuntimeError> {
        let Some(key) = self.entrypoints.exports.get(export_name) else {
            return Err(ScriptRuntimeError::MissingExport(export_name.to_string()));
        };

        self.reset_host_callback_state()?;
        let result = self.callback_timed("export", || {
            let callback = self.callback_function(key)?;
            let ctx = self.build_callback_ctx(host)?;
            let args_table = self.lua.create_table()?;
            for (index, argument) in args.iter().enumerate() {
                args_table.set((index + 1) as i64, self.to_lua_value(argument)?)?;
            }
            let return_value = callback.call::<Value>((ctx, args_table))?;
            self.from_lua_value(return_value)
        });
        let flush_result = self.flush_host_ops(host);
        flush_result?;
        result
    }

    fn call_on_init(&mut self, host: &mut dyn ScriptHostBridge) -> Result<(), ScriptRuntimeError> {
        let Some(key) = &self.entrypoints.on_init else {
            return Ok(());
        };

        self.reset_host_callback_state()?;
        let result = self.callback_timed("on_init", || {
            let callback = self.callback_function(key)?;
            let ctx = self.build_callback_ctx(host)?;
            callback.call::<()>((ctx,))?;
            Ok(())
        });
        let flush_result = self.flush_host_ops(host);
        flush_result?;
        result
    }

    fn call_on_update(&mut self, host: &mut dyn ScriptHostBridge) -> Result<(), ScriptRuntimeError> {
        let Some(key) = &self.entrypoints.on_update else {
            return Ok(());
        };

        self.reset_host_callback_state()?;
        let delta_seconds = host.delta_seconds();
        let result = self.callback_timed("on_update", || {
            let callback = self.callback_function(key)?;
            let ctx = self.build_callback_ctx(host)?;
            callback.call::<()>((ctx, delta_seconds))?;
            Ok(())
        });
        let flush_result = self.flush_host_ops(host);
        flush_result?;
        result
    }

    fn call_on_event(&mut self, event: &ScriptEvent, host: &mut dyn ScriptHostBridge) -> Result<(), ScriptRuntimeError> {
        let Some(key) = &self.entrypoints.on_event else {
            return Ok(());
        };

        self.reset_host_callback_state()?;
        let result = self.callback_timed("on_event", || {
            let callback = self.callback_function(key)?;
            let ctx = self.build_callback_ctx(host)?;
            let event_payload = self.lua.to_value(event)?;
            callback.call::<()>((ctx, event_payload))?;
            Ok(())
        });
        let flush_result = self.flush_host_ops(host);
        flush_result?;
        result
    }

    fn call_on_destroy(&mut self, host: &mut dyn ScriptHostBridge) -> Result<(), ScriptRuntimeError> {
        let Some(key) = &self.entrypoints.on_destroy else {
            return Ok(());
        };

        self.reset_host_callback_state()?;
        let result = self.callback_timed("on_destroy", || {
            let callback = self.callback_function(key)?;
            let ctx = self.build_callback_ctx(host)?;
            callback.call::<()>((ctx,))?;
            Ok(())
        });
        let flush_result = self.flush_host_ops(host);
        flush_result?;
        result
    }
}

#[derive(Default)]
struct QuickJsEntrypoints {
    on_init: bool,
    on_update: bool,
    on_event: bool,
    on_destroy: bool,
    exports: Vec<String>,
}

/// QuickJS-backed script runtime.
pub struct QuickJsRuntime {
    _runtime: QuickJsRuntimeHandle,
    context: QuickJsContext,
    budgets: ScriptBudgets,
    entrypoints: QuickJsEntrypoints,
    manifest: Option<ScriptManifest>,
    host_ops: Arc<Mutex<Vec<ScriptHostOp>>>,
    host_call_counter: Arc<AtomicU32>,
}

impl QuickJsRuntime {
    /// Creates a new QuickJS runtime with budget guardrails.
    pub fn new(budgets: ScriptBudgets) -> Result<Self, ScriptRuntimeError> {
        let runtime = QuickJsRuntimeHandle::new()?;
        runtime.set_memory_limit(budgets.max_memory_bytes);
        let context = QuickJsContext::full(&runtime)?;
        let host_ops = Arc::new(Mutex::new(Vec::new()));
        let host_call_counter = Arc::new(AtomicU32::new(0));

        let mut runtime = Self {
            _runtime: runtime,
            context,
            budgets,
            entrypoints: QuickJsEntrypoints::default(),
            manifest: None,
            host_ops,
            host_call_counter,
        };
        runtime.install_host_api()?;
        Ok(runtime)
    }

    fn install_host_api(&mut self) -> Result<(), ScriptRuntimeError> {
        let max_host_calls = self.budgets.max_host_calls_per_callback.max(1);
        let shared_host_ops = Arc::clone(&self.host_ops);
        let shared_host_call_counter = Arc::clone(&self.host_call_counter);
        self.context.with(|ctx| -> Result<(), QuickJsError> {
            let gc_table = QuickJsObject::new(ctx.clone())?;

            let log_host_ops = Arc::clone(&shared_host_ops);
            let log_host_call_counter = Arc::clone(&shared_host_call_counter);
            let log_fn = QuickJsFunc::from(QuickJsMutFn::from(
                move |level_label: String, message: String| -> Result<(), QuickJsError> {
                    let call_count = log_host_call_counter.fetch_add(1, Ordering::Relaxed) + 1;
                    if call_count > max_host_calls {
                        return Err(QuickJsError::new_from_js_message(
                            "script",
                            "host",
                            "script host-call budget exceeded in current callback",
                        ));
                    }

                    let level = ScriptLogLevel::from_manifest_label(&level_label).ok_or_else(|| {
                        QuickJsError::new_from_js_message(
                            "string",
                            "scriptLogLevel",
                            format!("invalid log level '{level_label}'"),
                        )
                    })?;
                    let mut guard = log_host_ops.lock().map_err(|_| {
                        QuickJsError::new_from_js_message("script", "host", "script host-op queue lock poisoned")
                    })?;
                    guard.push(ScriptHostOp::Log { level, message });
                    Ok(())
                },
            ));
            gc_table.set("log", log_fn)?;

            let emit_host_ops = Arc::clone(&shared_host_ops);
            let emit_host_call_counter = Arc::clone(&shared_host_call_counter);
            let emit_raw_fn = QuickJsFunc::from(QuickJsMutFn::from(
                move |topic: String, payload_json: Option<String>| -> Result<(), QuickJsError> {
                    let call_count = emit_host_call_counter.fetch_add(1, Ordering::Relaxed) + 1;
                    if call_count > max_host_calls {
                        return Err(QuickJsError::new_from_js_message(
                            "script",
                            "host",
                            "script host-call budget exceeded in current callback",
                        ));
                    }

                    let payload_json = serde_json::from_str::<JsonValue>(payload_json.as_deref().unwrap_or("null")).unwrap_or(JsonValue::Null);

                    let mut guard = emit_host_ops.lock().map_err(|_| {
                        QuickJsError::new_from_js_message("script", "host", "script host-op queue lock poisoned")
                    })?;
                    guard.push(ScriptHostOp::EmitCustom { topic, payload: payload_json });
                    Ok(())
                },
            ));
            gc_table.set("__emit_raw", emit_raw_fn)?;

            ctx.globals().set("gc", gc_table)?;
            ctx.eval::<(), _>(
                "globalThis.gc.emit = (topic, payload) => globalThis.gc.__emit_raw(topic, JSON.stringify(payload === undefined ? null : payload));",
            )?;
            Ok(())
        })?;
        Ok(())
    }

    fn reset_host_callback_state(&self) -> Result<(), ScriptRuntimeError> {
        self.host_call_counter.store(0, Ordering::Relaxed);
        let mut guard = self.host_ops.lock().map_err(|_| ScriptRuntimeError::Host("script host-op queue lock poisoned".to_string()))?;
        guard.clear();
        Ok(())
    }

    fn flush_host_ops(&self, host: &mut dyn ScriptHostBridge) -> Result<(), ScriptRuntimeError> {
        let mut drained = Vec::new();
        {
            let mut guard = self.host_ops.lock().map_err(|_| ScriptRuntimeError::Host("script host-op queue lock poisoned".to_string()))?;
            std::mem::swap(&mut drained, &mut *guard);
        }

        for op in drained {
            match op {
                ScriptHostOp::Log { level, message } => {
                    host.log(level, &message);
                }
                ScriptHostOp::EmitCustom { topic, payload } => {
                    host.emit_custom(&topic, payload).map_err(ScriptRuntimeError::Host)?;
                }
            }
        }
        Ok(())
    }

    fn callback_timed<T, F>(&self, phase_label: &str, callback: F) -> Result<T, ScriptRuntimeError>
    where
        F: FnOnce() -> Result<T, ScriptRuntimeError>,
    {
        let started_at = Instant::now();
        let output = callback()?;
        let elapsed = started_at.elapsed();
        let elapsed_limit = Duration::from_micros(self.budgets.max_wall_time_us_per_callback.max(1));
        if elapsed > elapsed_limit {
            return Err(ScriptRuntimeError::BudgetViolation(format!(
                "{phase_label} callback exceeded wall-time budget: {:?} > {:?}",
                elapsed, elapsed_limit
            )));
        }
        Ok(output)
    }

    fn build_callback_ctx<'js>(&self, ctx: &QuickJsCtx<'js>, host: &dyn ScriptHostBridge) -> Result<QuickJsObject<'js>, ScriptRuntimeError> {
        let callback_ctx = QuickJsObject::new(ctx.clone())?;
        let gc_table: QuickJsObject = ctx.globals().get("gc")?;
        let log_fn: QuickJsFunction = gc_table.get("log")?;
        let emit_fn: QuickJsFunction = gc_table.get("emit")?;
        callback_ctx.set("log", log_fn)?;
        callback_ctx.set("emit", emit_fn)?;
        callback_ctx.set("time_seconds", host.time_seconds())?;
        callback_ctx.set("delta_seconds", host.delta_seconds())?;
        if let Some(owner) = host.owner_node() {
            callback_ctx.set("owner_node_id", owner.0 as i64)?;
        }
        Ok(callback_ctx)
    }

    fn to_quickjs_value<'js>(&self, ctx: &QuickJsCtx<'js>, value: &ScriptValue) -> Result<QuickJsValue<'js>, ScriptRuntimeError> {
        let js_value = match value {
            ScriptValue::Nil => QuickJsValue::new_null(ctx.clone()),
            ScriptValue::Bool(value) => value.into_js(ctx)?,
            ScriptValue::Int(value) => {
                if let Ok(small) = i32::try_from(*value) {
                    small.into_js(ctx)?
                } else {
                    (*value as f64).into_js(ctx)?
                }
            }
            ScriptValue::Float(value) => value.into_js(ctx)?,
            ScriptValue::Str(value) => value.as_str().into_js(ctx)?,
            ScriptValue::Json(value) => {
                let json = serde_json::to_string(value).map_err(|err| ScriptRuntimeError::InvalidManifest(format!("failed to serialize JSON argument: {err}")))?;
                ctx.json_parse(json)?
            }
        };
        Ok(js_value)
    }

    fn from_quickjs_value<'js>(&self, ctx: &QuickJsCtx<'js>, value: QuickJsValue<'js>) -> Result<ScriptValue, ScriptRuntimeError> {
        if value.is_null() || value.is_undefined() {
            return Ok(ScriptValue::Nil);
        }
        if let Some(value) = value.as_bool() {
            return Ok(ScriptValue::Bool(value));
        }
        if let Some(value) = value.as_int() {
            return Ok(ScriptValue::Int(value as i64));
        }
        if let Some(value) = value.as_float() {
            return Ok(ScriptValue::Float(value));
        }
        if value.is_string() {
            let text: String = value.get()?;
            return Ok(ScriptValue::Str(text));
        }
        if value.is_big_int() {
            let int_value: i64 = value.get()?;
            return Ok(ScriptValue::Int(int_value));
        }

        let Some(payload_text) = ctx.json_stringify(&value)?.and_then(|raw| raw.to_string().ok()) else {
            return Ok(ScriptValue::Nil);
        };
        let payload = serde_json::from_str::<JsonValue>(&payload_text)
            .map_err(|err| ScriptRuntimeError::InvalidManifest(format!("failed to parse JSON return value: {err}")))?;
        Ok(ScriptValue::Json(payload))
    }
}

impl ScriptRuntime for QuickJsRuntime {
    fn load(&mut self, source: &str) -> Result<ScriptManifest, ScriptRuntimeError> {
        self.entrypoints = QuickJsEntrypoints::default();
        self.manifest = None;

        let wrapped = format!("globalThis.__gc_script_manifest = (() => {{\n{source}\n}})();");
        let (entrypoints, manifest_json) = self.context.with(|ctx| -> Result<(QuickJsEntrypoints, String), QuickJsError> {
            ctx.eval::<(), _>(wrapped.as_str())?;

            let globals = ctx.globals();
            let root_value: QuickJsValue = globals.get("__gc_script_manifest")?;
            if root_value.is_null() || root_value.is_undefined() || !root_value.is_object() {
                return Err(QuickJsError::new_from_js_message(
                    "value",
                    "object",
                    "script must return a manifest object",
                ));
            }
            let root = root_value.into_object().ok_or_else(|| {
                QuickJsError::new_from_js_message("value", "object", "script must return a manifest object")
            })?;

            let on_init = root.get::<_, Option<QuickJsFunction>>("on_init")?.is_some();
            let on_update = root.get::<_, Option<QuickJsFunction>>("on_update")?.is_some();
            let on_event = root.get::<_, Option<QuickJsFunction>>("on_event")?.is_some();
            let on_destroy = root.get::<_, Option<QuickJsFunction>>("on_destroy")?.is_some();

            let mut exports = Vec::new();
            if let Some(export_table) = root.get::<_, Option<QuickJsObject>>("exports")? {
                for key in export_table.keys::<String>() {
                    let key = key?;
                    if export_table.get::<_, Option<QuickJsFunction>>(key.as_str())?.is_some() {
                        exports.push(key);
                    }
                }
            }
            exports.sort();

            let manifest_json = ctx
                .eval::<Option<String>, _>(
                    "JSON.stringify(globalThis.__gc_script_manifest, (key, value) => typeof value === 'function' ? undefined : value)",
                )?
                .ok_or_else(|| {
                    QuickJsError::new_from_js_message(
                        "object",
                        "string",
                        "failed to stringify script manifest",
                    )
                })?;

            Ok((
                QuickJsEntrypoints {
                    on_init,
                    on_update,
                    on_event,
                    on_destroy,
                    exports,
                },
                manifest_json,
            ))
        })?;

        let manifest_payload = serde_json::from_str::<JsonValue>(&manifest_json)
            .map_err(|err| ScriptRuntimeError::InvalidManifest(format!("failed to parse manifest JSON: {err}")))?;
        let manifest = parse_manifest_from_json(&manifest_payload, entrypoints.exports.clone())?;

        self.entrypoints = entrypoints;
        self.manifest = Some(manifest.clone());
        Ok(manifest)
    }

    fn reload(&mut self, source: &str) -> Result<ScriptManifest, ScriptRuntimeError> {
        let budgets = self.budgets;
        *self = Self::new(budgets)?;
        self.load(source)
    }

    fn manifest(&self) -> Option<&ScriptManifest> {
        self.manifest.as_ref()
    }

    fn export_names(&self) -> Vec<String> {
        self.entrypoints.exports.clone()
    }

    fn call_export(&mut self, export_name: &str, args: &[ScriptValue], host: &mut dyn ScriptHostBridge) -> Result<ScriptValue, ScriptRuntimeError> {
        if !self.entrypoints.exports.iter().any(|name| name == export_name) {
            return Err(ScriptRuntimeError::MissingExport(export_name.to_string()));
        }

        self.reset_host_callback_state()?;
        let result = self.callback_timed("export", || {
            self.context.with(|ctx| -> Result<ScriptValue, ScriptRuntimeError> {
                let callback_ctx = self.build_callback_ctx(&ctx, host)?;

                let globals = ctx.globals();
                let root: QuickJsObject = globals.get("__gc_script_manifest")?;
                let exports = root
                    .get::<_, Option<QuickJsObject>>("exports")?
                    .ok_or_else(|| ScriptRuntimeError::MissingExport(export_name.to_string()))?;
                let callback = exports
                    .get::<_, Option<QuickJsFunction>>(export_name)?
                    .ok_or_else(|| ScriptRuntimeError::MissingExport(export_name.to_string()))?;

                let args_table = QuickJsArray::new(ctx.clone())?;
                for (index, argument) in args.iter().enumerate() {
                    args_table.set(index, self.to_quickjs_value(&ctx, argument)?)?;
                }
                let return_value = callback.call::<_, QuickJsValue>((callback_ctx, args_table))?;
                self.from_quickjs_value(&ctx, return_value)
            })
        });
        let flush_result = self.flush_host_ops(host);
        flush_result?;
        result
    }

    fn call_on_init(&mut self, host: &mut dyn ScriptHostBridge) -> Result<(), ScriptRuntimeError> {
        if !self.entrypoints.on_init {
            return Ok(());
        }

        self.reset_host_callback_state()?;
        let result = self.callback_timed("on_init", || {
            self.context.with(|ctx| -> Result<(), ScriptRuntimeError> {
                let callback_ctx = self.build_callback_ctx(&ctx, host)?;
                let root: QuickJsObject = ctx.globals().get("__gc_script_manifest")?;
                if let Some(callback) = root.get::<_, Option<QuickJsFunction>>("on_init")? {
                    callback.call::<_, ()>((callback_ctx,))?;
                }
                Ok(())
            })
        });
        let flush_result = self.flush_host_ops(host);
        flush_result?;
        result
    }

    fn call_on_update(&mut self, host: &mut dyn ScriptHostBridge) -> Result<(), ScriptRuntimeError> {
        if !self.entrypoints.on_update {
            return Ok(());
        }

        self.reset_host_callback_state()?;
        let delta_seconds = host.delta_seconds();
        let result = self.callback_timed("on_update", || {
            self.context.with(|ctx| -> Result<(), ScriptRuntimeError> {
                let callback_ctx = self.build_callback_ctx(&ctx, host)?;
                let root: QuickJsObject = ctx.globals().get("__gc_script_manifest")?;
                if let Some(callback) = root.get::<_, Option<QuickJsFunction>>("on_update")? {
                    callback.call::<_, ()>((callback_ctx, delta_seconds))?;
                }
                Ok(())
            })
        });
        let flush_result = self.flush_host_ops(host);
        flush_result?;
        result
    }

    fn call_on_event(&mut self, event: &ScriptEvent, host: &mut dyn ScriptHostBridge) -> Result<(), ScriptRuntimeError> {
        if !self.entrypoints.on_event {
            return Ok(());
        }

        self.reset_host_callback_state()?;
        let event_payload = serde_json::to_string(event)
            .map_err(|err| ScriptRuntimeError::InvalidManifest(format!("failed to encode event payload: {err}")))?;
        let result = self.callback_timed("on_event", || {
            self.context.with(|ctx| -> Result<(), ScriptRuntimeError> {
                let callback_ctx = self.build_callback_ctx(&ctx, host)?;
                let event_value = ctx.json_parse(event_payload.as_str())?;
                let root: QuickJsObject = ctx.globals().get("__gc_script_manifest")?;
                if let Some(callback) = root.get::<_, Option<QuickJsFunction>>("on_event")? {
                    callback.call::<_, ()>((callback_ctx, event_value))?;
                }
                Ok(())
            })
        });
        let flush_result = self.flush_host_ops(host);
        flush_result?;
        result
    }

    fn call_on_destroy(&mut self, host: &mut dyn ScriptHostBridge) -> Result<(), ScriptRuntimeError> {
        if !self.entrypoints.on_destroy {
            return Ok(());
        }

        self.reset_host_callback_state()?;
        let result = self.callback_timed("on_destroy", || {
            self.context.with(|ctx| -> Result<(), ScriptRuntimeError> {
                let callback_ctx = self.build_callback_ctx(&ctx, host)?;
                let root: QuickJsObject = ctx.globals().get("__gc_script_manifest")?;
                if let Some(callback) = root.get::<_, Option<QuickJsFunction>>("on_destroy")? {
                    callback.call::<_, ()>((callback_ctx,))?;
                }
                Ok(())
            })
        });
        let flush_result = self.flush_host_ops(host);
        flush_result?;
        result
    }
}

fn parse_capability_set(table: Option<Table>) -> Result<ScriptCapabilitySet, ScriptRuntimeError> {
    let Some(table) = table else {
        return Ok(ScriptCapabilitySet::none());
    };

    let mut capabilities = Vec::new();
    for item in table.sequence_values::<String>() {
        let name = item?;
        let capability = match name.trim().to_ascii_lowercase().as_str() {
            "paramread" | "param_read" | "param-read" => ScriptCapability::ParamRead,
            "paramwrite" | "param_write" | "param-write" => ScriptCapability::ParamWrite,
            "noderead" | "node_read" | "node-read" => ScriptCapability::NodeRead,
            "nodepatchmeta" | "node_patch_meta" | "node-patch-meta" => ScriptCapability::NodePatchMeta,
            "nodeadd" | "node_add" | "node-add" => ScriptCapability::NodeAdd,
            "noderemove" | "node_remove" | "node-remove" => ScriptCapability::NodeRemove,
            "nodemove" | "node_move" | "node-move" => ScriptCapability::NodeMove,
            "eventsubscribe" | "event_subscribe" | "event-subscribe" => ScriptCapability::EventSubscribe,
            "eventemit" | "event_emit" | "event-emit" => ScriptCapability::EventEmit,
            "uicontribute" | "ui_contribute" | "ui-contribute" => ScriptCapability::UiContribute,
            other => return Err(ScriptRuntimeError::InvalidManifest(format!("unknown capability '{other}'"))),
        };
        capabilities.push(capability);
    }
    Ok(ScriptCapabilitySet::from_iter(capabilities))
}

fn parse_subscription_specs(table: Option<Table>) -> Result<Vec<ScriptSubscriptionSpec>, ScriptRuntimeError> {
    let Some(table) = table else {
        return Ok(Vec::new());
    };

    let mut specs = Vec::new();
    for entry in table.sequence_values::<Table>() {
        let entry = entry?;
        let selector_raw = entry.get::<String>("node")?;
        let max_depth = entry.get::<Option<u32>>("max_depth")?.unwrap_or(0);
        let selector = if selector_raw == "@host" {
            ScriptNodeSelector::HostPath(String::new())
        } else if selector_raw == "@root" {
            ScriptNodeSelector::RootPath(String::new())
        } else if let Some(path) = selector_raw.strip_prefix("@host/") {
            ScriptNodeSelector::HostPath(path.to_string())
        } else if let Some(path) = selector_raw.strip_prefix("@root/") {
            ScriptNodeSelector::RootPath(path.to_string())
        } else {
            ScriptNodeSelector::Path(selector_raw)
        };
        specs.push(ScriptSubscriptionSpec { node: selector, max_depth });
    }

    Ok(specs)
}

fn parse_parameter_specs(table: Option<Table>) -> Result<Vec<ScriptParameterSpec>, ScriptRuntimeError> {
    let Some(table) = table else {
        return Ok(Vec::new());
    };

    let mut specs = Vec::new();
    for pair in table.pairs::<String, Table>() {
        let (name, entry) = pair?;
        let value_type_label = entry.get::<Option<String>>("type")?.unwrap_or_else(|| "float".to_string());
        let value_type = ScriptValueType::from_manifest_label(&value_type_label).ok_or_else(|| ScriptRuntimeError::InvalidManifest(format!("parameter '{name}' has unsupported type '{value_type_label}'")))?;

        let default_value = match entry.get::<Option<Value>>("default")? {
            Some(raw) => parameter_default_from_lua_value(value_type, raw)?,
            None => default_param_value(value_type),
        };

        let mut constraints = ParameterConstraints::default();
        if let Some(step) = entry.get::<Option<f64>>("step")? {
            constraints.step = Some(step);
        }
        if let Some(step_base) = entry.get::<Option<f64>>("step_base")? {
            constraints.step_base = Some(step_base);
        }
        if let Some(policy_label) = entry.get::<Option<String>>("policy")? {
            constraints.policy = match policy_label.trim().to_ascii_lowercase().as_str() {
                "clampadapt" | "clamp_adapt" | "clamp-adapt" => ParameterConstraintPolicy::ClampAdapt,
                "reject" => ParameterConstraintPolicy::Reject,
                _ => return Err(ScriptRuntimeError::InvalidManifest(format!("parameter '{name}' has unsupported policy '{}'", policy_label))),
            };
        }

        let min = entry.get::<Option<Value>>("min")?;
        let max = entry.get::<Option<Value>>("max")?;
        constraints.range = parse_range_constraint(value_type, min, max)?;

        if let Some(enum_options) = entry.get::<Option<Table>>("enum_options")? {
            let mut options = Vec::new();
            for option in enum_options.sequence_values::<String>() {
                let variant_id = option?;
                options.push(ParameterEnumOption {
                    variant_id: variant_id.clone(),
                    value: ParamValue::Enum(variant_id.clone()),
                    label: variant_id,
                    tags: Vec::new(),
                    ordering: None,
                });
            }
            constraints.enum_options = options;
        }
        constraints.file = parse_file_constraints_lua(&entry, &name)?;

        let ui_hints = ParameterUiHints {
            widget: entry.get::<Option<String>>("widget")?,
            unit: entry.get::<Option<String>>("unit")?,
        };

        let decl_id = entry.get::<Option<String>>("decl_id")?.unwrap_or_else(|| name.clone());

        specs.push(ScriptParameterSpec {
            name: name.clone(),
            decl_id: DeclId(decl_id),
            label: entry.get::<Option<String>>("label")?,
            value_type,
            default_value,
            read_only: entry.get::<Option<bool>>("read_only")?.unwrap_or(false),
            constraints,
            ui_hints,
        });
    }

    specs.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(specs)
}

fn parse_file_constraints_lua(entry: &Table, param_name: &str) -> Result<FileConstraints, ScriptRuntimeError> {
    let mut constraints = FileConstraints::default();

    if let Some(allowed_types) = entry.get::<Option<Table>>("allowed_types")? {
        let mut parsed = Vec::new();
        for value in allowed_types.sequence_values::<String>() {
            let value = value?;
            let Some(group) = FileTypeGroup::from_label(&value) else {
                return Err(ScriptRuntimeError::InvalidManifest(format!(
                    "parameter '{param_name}' has unknown file group '{value}' in allowed_types"
                )));
            };
            parsed.push(group);
        }
        constraints.allowed_types = parsed;
    }

    if let Some(allowed_extensions) = entry.get::<Option<Table>>("allowed_extensions")? {
        let mut parsed = Vec::new();
        for value in allowed_extensions.sequence_values::<String>() {
            let value = value?;
            let Some(ext) = FileConstraints::normalize_extension_label(&value) else {
                return Err(ScriptRuntimeError::InvalidManifest(format!(
                    "parameter '{param_name}' has invalid extension '{value}' in allowed_extensions"
                )));
            };
            parsed.push(ext);
        }
        constraints.allowed_extensions = parsed;
    }

    Ok(constraints)
}

fn parse_range_constraint(value_type: ScriptValueType, min: Option<Value>, max: Option<Value>) -> Result<Option<RangeConstraint>, ScriptRuntimeError> {
    match value_type {
        ScriptValueType::Int
        | ScriptValueType::Float
        | ScriptValueType::Enum
        | ScriptValueType::Bool
        | ScriptValueType::Str
        | ScriptValueType::File
        | ScriptValueType::Trigger
        | ScriptValueType::Reference => {
            let min = min.as_ref().and_then(value_as_f64);
            let max = max.as_ref().and_then(value_as_f64);
            Ok(RangeConstraint::uniform(min, max))
        }
        ScriptValueType::Vec2 | ScriptValueType::Vec3 | ScriptValueType::Color => {
            let min = min.as_ref().and_then(value_as_f64_vec);
            let max = max.as_ref().and_then(value_as_f64_vec);
            Ok(RangeConstraint::components(min, max))
        }
    }
}

fn parse_manifest_from_json(payload: &JsonValue, export_names: Vec<String>) -> Result<ScriptManifest, ScriptRuntimeError> {
    let Some(root) = payload.as_object() else {
        return Err(ScriptRuntimeError::InvalidManifest("manifest JSON root must be an object".to_string()));
    };

    let api_version = root.get("api_version").and_then(JsonValue::as_u64).unwrap_or(1) as u32;
    if api_version == 0 {
        return Err(ScriptRuntimeError::InvalidManifest("api_version must be >= 1".to_string()));
    }

    let update_rate_hz = root.get("update_rate_hz").and_then(JsonValue::as_u64).map(|value| value as u32);
    let requested_capabilities = parse_capability_set_json(root.get("capabilities"))?;
    let parameters = parse_parameter_specs_json(root.get("parameters"))?;
    let subscriptions = parse_subscription_specs_json(root.get("subscriptions"))?;
    let exports = export_names.into_iter().map(|name| ScriptExportSpec { name, signature: ScriptFnSignature::default() }).collect();

    Ok(ScriptManifest {
        api_version,
        update_rate_hz,
        parameters,
        subscriptions,
        exports,
        ui: ScriptUiSpec::default(),
        requested_capabilities,
    })
}

fn parse_capability_set_json(value: Option<&JsonValue>) -> Result<ScriptCapabilitySet, ScriptRuntimeError> {
    let Some(value) = value else {
        return Ok(ScriptCapabilitySet::none());
    };

    let Some(items) = value.as_array() else {
        return Err(ScriptRuntimeError::InvalidManifest("capabilities must be an array of strings".to_string()));
    };

    let mut capabilities = Vec::new();
    for item in items {
        let Some(name) = item.as_str() else {
            return Err(ScriptRuntimeError::InvalidManifest("capabilities entries must be strings".to_string()));
        };
        let capability = match name.trim().to_ascii_lowercase().as_str() {
            "paramread" | "param_read" | "param-read" => ScriptCapability::ParamRead,
            "paramwrite" | "param_write" | "param-write" => ScriptCapability::ParamWrite,
            "noderead" | "node_read" | "node-read" => ScriptCapability::NodeRead,
            "nodepatchmeta" | "node_patch_meta" | "node-patch-meta" => ScriptCapability::NodePatchMeta,
            "nodeadd" | "node_add" | "node-add" => ScriptCapability::NodeAdd,
            "noderemove" | "node_remove" | "node-remove" => ScriptCapability::NodeRemove,
            "nodemove" | "node_move" | "node-move" => ScriptCapability::NodeMove,
            "eventsubscribe" | "event_subscribe" | "event-subscribe" => ScriptCapability::EventSubscribe,
            "eventemit" | "event_emit" | "event-emit" => ScriptCapability::EventEmit,
            "uicontribute" | "ui_contribute" | "ui-contribute" => ScriptCapability::UiContribute,
            other => return Err(ScriptRuntimeError::InvalidManifest(format!("unknown capability '{other}'"))),
        };
        capabilities.push(capability);
    }

    Ok(ScriptCapabilitySet::from_iter(capabilities))
}

fn parse_subscription_specs_json(value: Option<&JsonValue>) -> Result<Vec<ScriptSubscriptionSpec>, ScriptRuntimeError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };

    let Some(items) = value.as_array() else {
        return Err(ScriptRuntimeError::InvalidManifest("subscriptions must be an array".to_string()));
    };

    let mut specs = Vec::new();
    for item in items {
        let Some(entry) = item.as_object() else {
            return Err(ScriptRuntimeError::InvalidManifest("subscription entry must be an object".to_string()));
        };

        let selector_raw = entry
            .get("node")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| ScriptRuntimeError::InvalidManifest("subscription entry must define string field 'node'".to_string()))?;
        let max_depth = entry.get("max_depth").and_then(JsonValue::as_u64).unwrap_or(0) as u32;
        let selector = if selector_raw == "@host" {
            ScriptNodeSelector::HostPath(String::new())
        } else if selector_raw == "@root" {
            ScriptNodeSelector::RootPath(String::new())
        } else if let Some(path) = selector_raw.strip_prefix("@host/") {
            ScriptNodeSelector::HostPath(path.to_string())
        } else if let Some(path) = selector_raw.strip_prefix("@root/") {
            ScriptNodeSelector::RootPath(path.to_string())
        } else {
            ScriptNodeSelector::Path(selector_raw.to_string())
        };

        specs.push(ScriptSubscriptionSpec { node: selector, max_depth });
    }

    Ok(specs)
}

fn parse_parameter_specs_json(value: Option<&JsonValue>) -> Result<Vec<ScriptParameterSpec>, ScriptRuntimeError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };

    let Some(parameters) = value.as_object() else {
        return Err(ScriptRuntimeError::InvalidManifest("parameters must be an object map".to_string()));
    };

    let mut specs = Vec::new();
    for (name, raw_entry) in parameters {
        let Some(entry) = raw_entry.as_object() else {
            return Err(ScriptRuntimeError::InvalidManifest(format!("parameter '{name}' must be an object")));
        };

        let value_type_label = entry.get("type").and_then(JsonValue::as_str).unwrap_or("float");
        let value_type = ScriptValueType::from_manifest_label(value_type_label)
            .ok_or_else(|| ScriptRuntimeError::InvalidManifest(format!("parameter '{name}' has unsupported type '{value_type_label}'")))?;

        let default_value = match entry.get("default") {
            Some(raw) => parameter_default_from_json_value(value_type, raw)?,
            None => default_param_value(value_type),
        };

        let mut constraints = ParameterConstraints::default();
        constraints.step = entry.get("step").and_then(JsonValue::as_f64);
        constraints.step_base = entry.get("step_base").and_then(JsonValue::as_f64);
        if let Some(policy_label) = entry.get("policy").and_then(JsonValue::as_str) {
            constraints.policy = match policy_label.trim().to_ascii_lowercase().as_str() {
                "clampadapt" | "clamp_adapt" | "clamp-adapt" => ParameterConstraintPolicy::ClampAdapt,
                "reject" => ParameterConstraintPolicy::Reject,
                _ => return Err(ScriptRuntimeError::InvalidManifest(format!("parameter '{name}' has unsupported policy '{policy_label}'"))),
            };
        }

        constraints.range = parse_range_constraint_json(value_type, entry.get("min"), entry.get("max"))?;

        if let Some(enum_options) = entry.get("enum_options") {
            let Some(options) = enum_options.as_array() else {
                return Err(ScriptRuntimeError::InvalidManifest(format!("parameter '{name}' enum_options must be an array")));
            };

            let mut mapped = Vec::new();
            for option in options {
                let Some(variant_id) = option.as_str() else {
                    return Err(ScriptRuntimeError::InvalidManifest(format!(
                        "parameter '{name}' enum_options entries must be strings"
                    )));
                };
                mapped.push(ParameterEnumOption {
                    variant_id: variant_id.to_string(),
                    value: ParamValue::Enum(variant_id.to_string()),
                    label: variant_id.to_string(),
                    tags: Vec::new(),
                    ordering: None,
                });
            }
            constraints.enum_options = mapped;
        }
        constraints.file = parse_file_constraints_json(entry, name)?;

        let ui_hints = ParameterUiHints {
            widget: entry.get("widget").and_then(JsonValue::as_str).map(ToString::to_string),
            unit: entry.get("unit").and_then(JsonValue::as_str).map(ToString::to_string),
        };

        let decl_id = entry.get("decl_id").and_then(JsonValue::as_str).unwrap_or(name);
        let label = entry.get("label").and_then(JsonValue::as_str).map(ToString::to_string);
        let read_only = entry.get("read_only").and_then(JsonValue::as_bool).unwrap_or(false);

        specs.push(ScriptParameterSpec {
            name: name.to_string(),
            decl_id: DeclId(decl_id.to_string()),
            label,
            value_type,
            default_value,
            read_only,
            constraints,
            ui_hints,
        });
    }

    specs.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(specs)
}

fn parse_file_constraints_json(
    entry: &serde_json::Map<String, JsonValue>,
    param_name: &str,
) -> Result<FileConstraints, ScriptRuntimeError> {
    let mut constraints = FileConstraints::default();

    if let Some(allowed_types) = entry.get("allowed_types") {
        let Some(values) = allowed_types.as_array() else {
            return Err(ScriptRuntimeError::InvalidManifest(format!(
                "parameter '{param_name}' allowed_types must be an array"
            )));
        };
        let mut parsed = Vec::new();
        for value in values {
            let Some(value) = value.as_str() else {
                return Err(ScriptRuntimeError::InvalidManifest(format!(
                    "parameter '{param_name}' allowed_types entries must be strings"
                )));
            };
            let Some(group) = FileTypeGroup::from_label(value) else {
                return Err(ScriptRuntimeError::InvalidManifest(format!(
                    "parameter '{param_name}' has unknown file group '{value}' in allowed_types"
                )));
            };
            parsed.push(group);
        }
        constraints.allowed_types = parsed;
    }

    if let Some(allowed_extensions) = entry.get("allowed_extensions") {
        let Some(values) = allowed_extensions.as_array() else {
            return Err(ScriptRuntimeError::InvalidManifest(format!(
                "parameter '{param_name}' allowed_extensions must be an array"
            )));
        };
        let mut parsed = Vec::new();
        for value in values {
            let Some(value) = value.as_str() else {
                return Err(ScriptRuntimeError::InvalidManifest(format!(
                    "parameter '{param_name}' allowed_extensions entries must be strings"
                )));
            };
            let Some(extension) = FileConstraints::normalize_extension_label(value) else {
                return Err(ScriptRuntimeError::InvalidManifest(format!(
                    "parameter '{param_name}' has invalid extension '{value}' in allowed_extensions"
                )));
            };
            parsed.push(extension);
        }
        constraints.allowed_extensions = parsed;
    }

    Ok(constraints)
}

fn parse_range_constraint_json(
    value_type: ScriptValueType,
    min: Option<&JsonValue>,
    max: Option<&JsonValue>,
) -> Result<Option<RangeConstraint>, ScriptRuntimeError> {
    match value_type {
        ScriptValueType::Int
        | ScriptValueType::Float
        | ScriptValueType::Enum
        | ScriptValueType::Bool
        | ScriptValueType::Str
        | ScriptValueType::File
        | ScriptValueType::Trigger
        | ScriptValueType::Reference => {
            let min = min.and_then(json_as_f64);
            let max = max.and_then(json_as_f64);
            Ok(RangeConstraint::uniform(min, max))
        }
        ScriptValueType::Vec2 | ScriptValueType::Vec3 | ScriptValueType::Color => {
            let min = min.and_then(json_as_f64_vec);
            let max = max.and_then(json_as_f64_vec);
            Ok(RangeConstraint::components(min, max))
        }
    }
}

fn parameter_default_from_json_value(value_type: ScriptValueType, value: &JsonValue) -> Result<ParamValue, ScriptRuntimeError> {
    let parsed = match value_type {
        ScriptValueType::Trigger => ParamValue::Trigger(),
        ScriptValueType::Int => {
            let Some(raw) = value.as_i64().or_else(|| value.as_f64().map(|value| value as i64)) else {
                return Err(ScriptRuntimeError::InvalidManifest("expected numeric default for int parameter".to_string()));
            };
            ParamValue::Int(i32::try_from(raw).map_err(|_| ScriptRuntimeError::InvalidManifest(format!("int default {raw} is outside i32 range")))?)
        }
        ScriptValueType::Float => {
            let Some(raw) = value.as_f64().or_else(|| value.as_i64().map(|value| value as f64)) else {
                return Err(ScriptRuntimeError::InvalidManifest("expected numeric default for float parameter".to_string()));
            };
            ParamValue::Float(raw)
        }
        ScriptValueType::Str => {
            let Some(raw) = value.as_str() else {
                return Err(ScriptRuntimeError::InvalidManifest("expected string default for str parameter".to_string()));
            };
            ParamValue::Str(raw.to_string())
        }
        ScriptValueType::File => {
            let Some(raw) = value.as_str() else {
                return Err(ScriptRuntimeError::InvalidManifest("expected string default for file parameter".to_string()));
            };
            ParamValue::File(raw.to_string())
        }
        ScriptValueType::Enum => {
            let Some(raw) = value.as_str() else {
                return Err(ScriptRuntimeError::InvalidManifest("expected string default for enum parameter".to_string()));
            };
            ParamValue::Enum(raw.to_string())
        }
        ScriptValueType::Bool => {
            let Some(raw) = value.as_bool() else {
                return Err(ScriptRuntimeError::InvalidManifest("expected boolean default for bool parameter".to_string()));
            };
            ParamValue::Bool(raw)
        }
        ScriptValueType::Vec2 => {
            let Some(raw) = value.as_array() else {
                return Err(ScriptRuntimeError::InvalidManifest("expected [x,y] array default for vec2 parameter".to_string()));
            };
            if raw.len() != 2 {
                return Err(ScriptRuntimeError::InvalidManifest("vec2 default must have exactly 2 components".to_string()));
            }
            ParamValue::Vec2(json_as_f64(&raw[0]).ok_or_else(|| ScriptRuntimeError::InvalidManifest("vec2[0] must be numeric".to_string()))?, json_as_f64(&raw[1]).ok_or_else(|| ScriptRuntimeError::InvalidManifest("vec2[1] must be numeric".to_string()))?)
        }
        ScriptValueType::Vec3 => {
            let Some(raw) = value.as_array() else {
                return Err(ScriptRuntimeError::InvalidManifest("expected [x,y,z] array default for vec3 parameter".to_string()));
            };
            if raw.len() != 3 {
                return Err(ScriptRuntimeError::InvalidManifest("vec3 default must have exactly 3 components".to_string()));
            }
            ParamValue::Vec3(
                json_as_f64(&raw[0]).ok_or_else(|| ScriptRuntimeError::InvalidManifest("vec3[0] must be numeric".to_string()))?,
                json_as_f64(&raw[1]).ok_or_else(|| ScriptRuntimeError::InvalidManifest("vec3[1] must be numeric".to_string()))?,
                json_as_f64(&raw[2]).ok_or_else(|| ScriptRuntimeError::InvalidManifest("vec3[2] must be numeric".to_string()))?,
            )
        }
        ScriptValueType::Color => {
            let Some(raw) = value.as_array() else {
                return Err(ScriptRuntimeError::InvalidManifest("expected [r,g,b,a] array default for color parameter".to_string()));
            };
            if raw.len() < 3 || raw.len() > 4 {
                return Err(ScriptRuntimeError::InvalidManifest(
                    "color default must have 3 or 4 components".to_string(),
                ));
            }
            let r = json_as_f64(&raw[0]).ok_or_else(|| ScriptRuntimeError::InvalidManifest("color[0] must be numeric".to_string()))?;
            let g = json_as_f64(&raw[1]).ok_or_else(|| ScriptRuntimeError::InvalidManifest("color[1] must be numeric".to_string()))?;
            let b = json_as_f64(&raw[2]).ok_or_else(|| ScriptRuntimeError::InvalidManifest("color[2] must be numeric".to_string()))?;
            let a = raw.get(3).and_then(json_as_f64).unwrap_or(1.0);
            ParamValue::Color(r, g, b, a)
        }
        ScriptValueType::Reference => ParamValue::Reference(crate::node::NodeReference::empty()),
    };
    Ok(parsed)
}

fn json_as_f64(value: &JsonValue) -> Option<f64> {
    value.as_f64().or_else(|| value.as_i64().map(|value| value as f64))
}

fn json_as_f64_vec(value: &JsonValue) -> Option<Vec<f64>> {
    let values = value.as_array()?;
    let mut out = Vec::with_capacity(values.len());
    for item in values {
        out.push(json_as_f64(item)?);
    }
    Some(out)
}

fn value_as_f64(value: &Value) -> Option<f64> {
    match value {
        Value::Integer(value) => Some(*value as f64),
        Value::Number(value) => Some(*value),
        _ => None,
    }
}

fn value_as_f64_vec(value: &Value) -> Option<Vec<f64>> {
    let Value::Table(table) = value else {
        return None;
    };
    let mut out = Vec::new();
    for item in table.sequence_values::<Value>() {
        let item = item.ok()?;
        out.push(value_as_f64(&item)?);
    }
    Some(out)
}

fn default_param_value(value_type: ScriptValueType) -> ParamValue {
    match value_type {
        ScriptValueType::Trigger => ParamValue::Trigger(),
        ScriptValueType::Int => ParamValue::Int(0),
        ScriptValueType::Float => ParamValue::Float(0.0),
        ScriptValueType::Str => ParamValue::Str(String::new()),
        ScriptValueType::File => ParamValue::File(String::new()),
        ScriptValueType::Enum => ParamValue::Enum(String::new()),
        ScriptValueType::Bool => ParamValue::Bool(false),
        ScriptValueType::Vec2 => ParamValue::Vec2(0.0, 0.0),
        ScriptValueType::Vec3 => ParamValue::Vec3(0.0, 0.0, 0.0),
        ScriptValueType::Color => ParamValue::Color(0.0, 0.0, 0.0, 1.0),
        ScriptValueType::Reference => ParamValue::Reference(crate::node::NodeReference::empty()),
    }
}

fn parameter_default_from_lua_value(value_type: ScriptValueType, value: Value) -> Result<ParamValue, ScriptRuntimeError> {
    let parsed = match value_type {
        ScriptValueType::Trigger => ParamValue::Trigger(),
        ScriptValueType::Int => match value {
            Value::Integer(value) => ParamValue::Int(value as i32),
            Value::Number(value) => ParamValue::Int(value as i32),
            _ => return Err(ScriptRuntimeError::InvalidManifest("expected numeric default for int parameter".to_string())),
        },
        ScriptValueType::Float => match value {
            Value::Integer(value) => ParamValue::Float(value as f64),
            Value::Number(value) => ParamValue::Float(value),
            _ => return Err(ScriptRuntimeError::InvalidManifest("expected numeric default for float parameter".to_string())),
        },
        ScriptValueType::Str => match value {
            Value::String(value) => ParamValue::Str(value.to_str()?.to_string()),
            _ => return Err(ScriptRuntimeError::InvalidManifest("expected string default for str parameter".to_string())),
        },
        ScriptValueType::File => match value {
            Value::String(value) => ParamValue::File(value.to_str()?.to_string()),
            _ => return Err(ScriptRuntimeError::InvalidManifest("expected string default for file parameter".to_string())),
        },
        ScriptValueType::Enum => match value {
            Value::String(value) => ParamValue::Enum(value.to_str()?.to_string()),
            _ => return Err(ScriptRuntimeError::InvalidManifest("expected string default for enum parameter".to_string())),
        },
        ScriptValueType::Bool => match value {
            Value::Boolean(value) => ParamValue::Bool(value),
            _ => return Err(ScriptRuntimeError::InvalidManifest("expected boolean default for bool parameter".to_string())),
        },
        ScriptValueType::Vec2 => {
            let Value::Table(values) = value else {
                return Err(ScriptRuntimeError::InvalidManifest("expected [x,y] table default for vec2 parameter".to_string()));
            };
            let x = values.get::<f64>(1)?;
            let y = values.get::<f64>(2)?;
            ParamValue::Vec2(x, y)
        }
        ScriptValueType::Vec3 => {
            let Value::Table(values) = value else {
                return Err(ScriptRuntimeError::InvalidManifest("expected [x,y,z] table default for vec3 parameter".to_string()));
            };
            let x = values.get::<f64>(1)?;
            let y = values.get::<f64>(2)?;
            let z = values.get::<f64>(3)?;
            ParamValue::Vec3(x, y, z)
        }
        ScriptValueType::Color => {
            let Value::Table(values) = value else {
                return Err(ScriptRuntimeError::InvalidManifest("expected [r,g,b,a] table default for color parameter".to_string()));
            };
            let r = values.get::<f64>(1)?;
            let g = values.get::<f64>(2)?;
            let b = values.get::<f64>(3)?;
            let a = values.get::<Option<f64>>(4)?.unwrap_or(1.0);
            ParamValue::Color(r, g, b, a)
        }
        ScriptValueType::Reference => ParamValue::Reference(crate::node::NodeReference::empty()),
    };
    Ok(parsed)
}

struct NodeScriptHostBridge<'a> {
    owner: NodeId,
    ctx: &'a mut ProcessCtx,
}

impl<'a> NodeScriptHostBridge<'a> {
    fn new(owner: NodeId, ctx: &'a mut ProcessCtx) -> Self {
        Self { owner, ctx }
    }
}

impl ScriptHostBridge for NodeScriptHostBridge<'_> {
    fn owner_node(&self) -> Option<NodeId> {
        Some(self.owner)
    }

    fn time_seconds(&self) -> f64 {
        self.ctx.time.tick as f64
    }

    fn delta_seconds(&self) -> f64 {
        self.ctx.delta_time.as_secs_f64()
    }

    fn log(&mut self, level: ScriptLogLevel, message: &str) {
        let _ = logger::log_message(level.to_logger_level(), "script".to_string(), Some(self.owner), message.to_string());
    }

    fn emit_custom(&mut self, topic: &str, payload: JsonValue) -> Result<(), String> {
        self.ctx.emit_custom_event(CustomEvent::new(topic, Some(self.owner), payload));
        Ok(())
    }
}

fn hash_source_text(source: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    source.hash(&mut hasher);
    hasher.finish()
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ScriptSourceStamp {
    source_hash: u64,
    file_modified: Option<SystemTime>,
}

struct ActiveRuntime {
    kind: ScriptRuntimeKind,
    runtime: Box<dyn ScriptRuntime>,
}

fn create_runtime(kind: ScriptRuntimeKind, budgets: ScriptBudgets) -> Result<Box<dyn ScriptRuntime>, ScriptRuntimeError> {
    match kind {
        ScriptRuntimeKind::Luau => Ok(Box::new(LuauRuntime::new(budgets)?)),
        ScriptRuntimeKind::QuickJs => Ok(Box::new(QuickJsRuntime::new(budgets)?)),
    }
}

/// Built-in runtime-agnostic script node.
pub struct ScriptNode {
    node_data: NodeData,
    /// Script runtime configuration.
    pub config: ScriptNodeConfig,
    /// Script runtime safety budgets.
    pub budgets: ScriptBudgets,
    runtime: Option<ActiveRuntime>,
    manifest: Option<ScriptManifest>,
    source_stamp: Option<ScriptSourceStamp>,
    effective_update_rate_hz: Option<u32>,
}

impl ScriptNode {
    /// Creates a new script node.
    pub fn new(label: impl Into<String>, config: ScriptNodeConfig) -> Self {
        let node_data = NodeData::new(label.into());
        Self {
            node_data,
            config,
            budgets: ScriptBudgets::default(),
            runtime: None,
            manifest: None,
            source_stamp: None,
            effective_update_rate_hz: None,
        }
    }

    /// Returns the last successfully parsed manifest.
    pub fn manifest(&self) -> Option<&ScriptManifest> {
        self.manifest.as_ref()
    }

    /// Returns currently detected script export names.
    pub fn export_names(&self) -> Vec<String> {
        self.runtime.as_ref().map(|runtime| runtime.runtime.export_names()).unwrap_or_default()
    }

    /// Returns active runtime kind when loaded.
    pub fn runtime_kind(&self) -> Option<ScriptRuntimeKind> {
        self.runtime.as_ref().map(|runtime| runtime.kind)
    }

    /// Returns UI-facing script state.
    pub fn ui_state(&self) -> ScriptUiState {
        ScriptUiState {
            config: ScriptUiConfig::from(&self.config),
            runtime_kind: self.runtime_kind(),
            effective_update_rate_hz: self.effective_update_rate_hz,
            export_names: self.export_names(),
            manifest: self.manifest.clone(),
        }
    }

    /// Replaces script runtime configuration and invalidates loaded runtime state.
    pub fn set_config(&mut self, config: ScriptNodeConfig, force_reload: bool) {
        let config_changed = self.config != config;
        if config_changed {
            self.config = config;
        }

        if config_changed || force_reload {
            self.invalidate_runtime_state();
        }
    }

    /// Marks the runtime as dirty so next update reloads source and callbacks.
    pub fn request_reload(&mut self) {
        self.invalidate_runtime_state();
    }

    /// Reloads source and updates runtime state.
    pub fn reload(&mut self, ctx: &mut ProcessCtx) -> Result<(), ScriptRuntimeError> {
        self.load_or_reload_internal(ctx, true)
    }

    fn invalidate_runtime_state(&mut self) {
        self.runtime = None;
        self.manifest = None;
        self.source_stamp = None;
        self.effective_update_rate_hz = None;
    }

    fn source_file_modified(&self) -> Option<SystemTime> {
        let path = self.config.source.resolve_path(self.config.project_root.as_deref())?;
        std::fs::metadata(path).ok().and_then(|metadata| metadata.modified().ok())
    }

    fn source_stamp_from_text(&self, source_text: &str) -> ScriptSourceStamp {
        ScriptSourceStamp {
            source_hash: hash_source_text(source_text),
            file_modified: self.source_file_modified(),
        }
    }

    fn has_source_changed(&self) -> Result<bool, ScriptRuntimeError> {
        let Some(last_stamp) = &self.source_stamp else {
            return Ok(true);
        };

        match &self.config.source {
            ScriptSource::Inline(source) => Ok(hash_source_text(source) != last_stamp.source_hash),
            ScriptSource::ProjectFile(_) => {
                let current_modified = self.source_file_modified();
                if current_modified.is_some() && current_modified == last_stamp.file_modified {
                    return Ok(false);
                }

                let script_source = self.config.source.load_text(self.config.project_root.as_deref())?;
                Ok(hash_source_text(&script_source) != last_stamp.source_hash)
            }
        }
    }

    fn load_or_reload_internal(&mut self, ctx: &mut ProcessCtx, force_reload: bool) -> Result<(), ScriptRuntimeError> {
        if !self.config.enabled {
            return Ok(());
        }

        if self.runtime.is_some() && !force_reload {
            return Ok(());
        }

        let script_source = self.config.source.load_text(self.config.project_root.as_deref())?;
        let runtime_kind = self.config.detect_runtime_kind()?;
        let source_stamp = self.source_stamp_from_text(&script_source);

        let has_matching_runtime = self.runtime.as_ref().is_some_and(|active| active.kind == runtime_kind);
        let mut runtime = if has_matching_runtime {
            self.runtime.take().expect("runtime presence just checked").runtime
        } else {
            create_runtime(runtime_kind, self.budgets)?
        };

        let manifest = if has_matching_runtime { runtime.reload(&script_source)? } else { runtime.load(&script_source)? };

        let mut host = NodeScriptHostBridge::new(self.id(), ctx);
        runtime.call_on_init(&mut host)?;

        self.effective_update_rate_hz = manifest.update_rate_hz.or(self.config.requested_update_rate_hz);
        self.manifest = Some(manifest);
        self.runtime = Some(ActiveRuntime { kind: runtime_kind, runtime });
        self.source_stamp = Some(source_stamp);
        ctx.clear_node_warning(self.id(), Some("script"));
        Ok(())
    }

    fn handle_runtime_error(&self, ctx: &mut ProcessCtx, error: &ScriptRuntimeError) {
        ctx.set_node_warning_with(self.id(), Some("script"), format!("Script runtime error: {error}"), None);
    }
}

impl Node for ScriptNode {
    fn node_data(&self) -> &NodeData {
        &self.node_data
    }

    fn node_data_mut(&mut self) -> &mut NodeData {
        &mut self.node_data
    }

    fn get_type(&self) -> &str {
        "script"
    }

    fn engine_script_state(&self) -> Option<ScriptUiState> {
        Some(self.ui_state())
    }

    fn engine_set_script_config(&mut self, config: ScriptNodeConfig, force_reload: bool) -> Result<(), String> {
        self.set_config(config, force_reload);
        Ok(())
    }

    fn engine_request_script_reload(&mut self) -> Result<(), String> {
        self.request_reload();
        Ok(())
    }

    fn init(&mut self, ctx: &mut ProcessCtx) {
        if let Err(error) = self.load_or_reload_internal(ctx, false) {
            self.handle_runtime_error(ctx, &error);
        }
    }

    fn update(&mut self, ctx: &mut ProcessCtx) {
        if !self.config.enabled {
            return;
        }

        if self.config.auto_reload {
            match self.has_source_changed() {
                Ok(true) => {
                    if let Err(error) = self.load_or_reload_internal(ctx, true) {
                        self.handle_runtime_error(ctx, &error);
                        return;
                    }
                }
                Ok(false) => {}
                Err(error) => {
                    self.handle_runtime_error(ctx, &error);
                    return;
                }
            }
        }

        if self.runtime.is_none() {
            if let Err(error) = self.load_or_reload_internal(ctx, false) {
                self.handle_runtime_error(ctx, &error);
                return;
            }
        }

        let owner = self.id();
        let mut runtime_error = None;
        if let Some(runtime) = self.runtime.as_mut() {
            let mut host = NodeScriptHostBridge::new(owner, ctx);
            if let Err(error) = runtime.runtime.call_on_update(&mut host) {
                runtime_error = Some(error);
            }
        }
        if let Some(error) = runtime_error {
            self.handle_runtime_error(ctx, &error);
        }
    }

    fn on_inbox(&mut self, ctx: &mut ProcessCtx) {
        if !self.config.enabled {
            return;
        }
        let events = ctx.events.clone();
        let owner = self.id();
        let mut runtime_error = None;

        let Some(runtime) = self.runtime.as_mut() else {
            return;
        };

        for event in &events {
            let script_event = ScriptEvent::from(event);
            let mut host = NodeScriptHostBridge::new(owner, ctx);
            if let Err(error) = runtime.runtime.call_on_event(&script_event, &mut host) {
                runtime_error = Some(error);
                break;
            }
        }
        if let Some(error) = runtime_error {
            self.handle_runtime_error(ctx, &error);
        }
    }

    fn destroy(&mut self, ctx: &mut ProcessCtx) {
        let owner = self.id();
        let mut runtime_error = None;
        let Some(runtime) = self.runtime.as_mut() else {
            return;
        };
        let mut host = NodeScriptHostBridge::new(owner, ctx);
        if let Err(error) = runtime.runtime.call_on_destroy(&mut host) {
            runtime_error = Some(error);
        }
        if let Some(error) = runtime_error {
            self.handle_runtime_error(ctx, &error);
        }
    }

    fn execution_rule(&self) -> NodeExecutionRule {
        if !self.config.enabled {
            return NodeExecutionRule::passive();
        }

        match self.effective_update_rate_hz.or(self.config.requested_update_rate_hz) {
            Some(rate_hz) if rate_hz > 0 => NodeExecutionRule::periodic(rate_hz),
            _ => NodeExecutionRule::passive(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestHostBridge {
        logs: Vec<(ScriptLogLevel, String)>,
        emitted: Vec<(String, JsonValue)>,
    }

    impl TestHostBridge {
        fn new() -> Self {
            Self {
                logs: Vec::new(),
                emitted: Vec::new(),
            }
        }
    }

    impl ScriptHostBridge for TestHostBridge {
        fn time_seconds(&self) -> f64 {
            12.0
        }

        fn delta_seconds(&self) -> f64 {
            0.016
        }

        fn log(&mut self, level: ScriptLogLevel, message: &str) {
            self.logs.push((level, message.to_string()));
        }

        fn emit_custom(&mut self, topic: &str, payload: JsonValue) -> Result<(), String> {
            self.emitted.push((topic.to_string(), payload));
            Ok(())
        }
    }

    #[test]
    fn luau_runtime_loads_manifest_and_exports() {
        let source = r#"
return {
  api_version = 1,
  update_rate_hz = 60,
  exports = {
    ping = function(ctx, args)
      ctx.log("info", "ping called")
      ctx.emit("script.test", { value = args[1] })
      return args[1]
    end
  }
}
"#;

        let mut runtime = LuauRuntime::new(ScriptBudgets::default()).expect("runtime should initialize");
        let manifest = runtime.load(source).expect("manifest should parse");
        assert_eq!(manifest.api_version, 1);
        assert_eq!(manifest.update_rate_hz, Some(60));
        assert_eq!(runtime.export_names(), vec!["ping".to_string()]);

        let mut host = TestHostBridge::new();
        let output = runtime
            .call_export("ping", &[ScriptValue::Int(7)], &mut host)
            .expect("export should run");
        assert_eq!(output, ScriptValue::Int(7));
        assert_eq!(host.logs.len(), 1);
        assert_eq!(host.logs[0].0, ScriptLogLevel::Info);
        assert_eq!(host.emitted.len(), 1);
        assert_eq!(host.emitted[0].0, "script.test");
    }

    #[test]
    fn quickjs_runtime_loads_manifest_and_exports() {
        let source = r#"
return {
  api_version: 1,
  update_rate_hz: 30,
  exports: {
    ping: function(ctx, args) {
      ctx.log("info", "ping called");
      ctx.emit("script.test", { value: args[0] });
      return args[0];
    }
  }
};
"#;

        let mut runtime = QuickJsRuntime::new(ScriptBudgets::default()).expect("runtime should initialize");
        let manifest = runtime.load(source).expect("manifest should parse");
        assert_eq!(manifest.api_version, 1);
        assert_eq!(manifest.update_rate_hz, Some(30));
        assert_eq!(runtime.export_names(), vec!["ping".to_string()]);

        let mut host = TestHostBridge::new();
        let output = runtime
            .call_export("ping", &[ScriptValue::Int(7)], &mut host)
            .expect("export should run");
        assert_eq!(output, ScriptValue::Int(7));
        assert_eq!(host.logs.len(), 1);
        assert_eq!(host.logs[0].0, ScriptLogLevel::Info);
        assert_eq!(host.emitted.len(), 1);
        assert_eq!(host.emitted[0].0, "script.test");
    }
}
