use std::collections::HashSet;
use std::fmt;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

use rquickjs::function::{Args as QuickJsArgs, Func as QuickJsFunc, MutFn as QuickJsMutFn};
use rquickjs::context::EvalOptions as QuickJsEvalOptions;
use rquickjs::{
    Context as QuickJsContext, Ctx as QuickJsCtx, Error as QuickJsError,
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

/// Script-host policy for one node type.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptHostPolicy {
    /// Whether script hosting is enabled for this node.
    pub enabled: bool,
}

impl Default for ScriptHostPolicy {
    fn default() -> Self {
        Self { enabled: false }
    }
}

impl ScriptHostPolicy {
    /// Default policy used by `#[node(scriptable)]` and `#[item(..., scriptable)]`.
    pub fn default_scriptable() -> Self {
        Self { enabled: true }
    }
}

/// Hard safety guardrails applied per script instance.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptBudgets {
    /// Maximum VM instruction target for one callback.
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
}

impl Default for ScriptBudgets {
    fn default() -> Self {
        Self {
            max_instructions_per_callback: 200_000,
            max_wall_time_us_per_callback: 50_000,
            max_memory_bytes: 16 * 1024 * 1024,
            max_host_calls_per_callback: 1_024,
            max_emitted_edits_per_tick: 512,
            max_emitted_events_per_tick: 512,
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
    pub fn resolve_path(&self) -> Option<PathBuf> {
        match self {
            Self::Inline(_) => None,
            Self::ProjectFile(path) => Some(PathBuf::from(path)),
        }
    }

    /// Loads source text from configured source.
    pub fn load_text(&self) -> Result<String, ScriptRuntimeError> {
        match self {
            Self::Inline(text) => Ok(text.clone()),
            Self::ProjectFile(path) => {
                let resolved = self.resolve_path().unwrap_or_else(|| PathBuf::from(path));
                std::fs::read_to_string(&resolved).map_err(|err| ScriptRuntimeError::Io(format!("failed to read script file '{}': {err}", resolved.display())))
            }
        }
    }

    fn is_file_backed(&self) -> bool {
        matches!(self, Self::ProjectFile(_))
    }

    fn runtime_source_name(&self) -> String {
        match self {
            Self::Inline(_) => "inline_script.js".to_string(),
            Self::ProjectFile(path) if !path.trim().is_empty() => path.clone(),
            Self::ProjectFile(_) => "script_file.js".to_string(),
        }
    }
}

const SCRIPT_TEMPLATE_DIR: &str = "script/templates";
const SCRIPT_TEMPLATE_INCLUDE_PREFIX: &str = "{{include:";
const SCRIPT_TEMPLATE_INCLUDE_SUFFIX: &str = "}}";
const SCRIPT_TEMPLATE_EXTENSIONS: [&str; 3] = ["js", "mjs", "cjs"];
const SCRIPT_TEMPLATE_DEFAULT_SOURCE: &str = include_str!("templates/default.js");
const SCRIPT_BOOTSTRAP_UPDATE_RATE_HZ: u32 = 60;
const SCRIPT_FILE_RELOAD_POLL_HZ: u32 = 30;

struct ScriptTemplateResolved {
    source: String,
}

fn script_template_root_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(SCRIPT_TEMPLATE_DIR)
}

fn normalized_template_key(value: &str) -> Option<String> {
    let mut output = String::new();
    let mut previous_was_separator = false;
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            output.push(ch.to_ascii_lowercase());
            previous_was_separator = false;
            continue;
        }

        if !previous_was_separator {
            output.push('_');
            previous_was_separator = true;
        }
    }

    let output = output.trim_matches('_').to_string();
    if output.is_empty() { None } else { Some(output) }
}

fn template_candidate_basenames(host_node_type: &str) -> Vec<String> {
    let mut basenames = Vec::new();
    let raw = host_node_type.trim().to_ascii_lowercase();
    if !raw.is_empty() {
        basenames.push(raw);
    }
    if let Some(normalized) = normalized_template_key(host_node_type) {
        if !basenames.iter().any(|candidate| candidate == &normalized) {
            basenames.push(normalized);
        }
    }
    basenames.push("default".to_string());
    basenames
}

fn normalize_include_path(path: &str) -> Result<PathBuf, String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("template include path is empty".to_string());
    }

    let source = Path::new(trimmed);
    if source.is_absolute() {
        return Err(format!("template include path '{trimmed}' must be relative"));
    }

    let mut normalized = PathBuf::new();
    for component in source.components() {
        match component {
            Component::Normal(segment) => normalized.push(segment),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(format!("template include path '{trimmed}' escapes template directory"));
            }
        }
    }

    if normalized.as_os_str().is_empty() {
        return Err(format!("template include path '{trimmed}' resolved to empty path"));
    }

    Ok(normalized)
}

fn include_stack_contains(stack: &[String], key: &str) -> bool {
    stack.iter().any(|item| item == key)
}

fn expand_template_source(source: &str, root_dir: &Path, include_stack: &mut Vec<String>) -> Result<String, String> {
    let mut output = String::with_capacity(source.len());
    let mut cursor = source;

    while let Some(start_index) = cursor.find(SCRIPT_TEMPLATE_INCLUDE_PREFIX) {
        output.push_str(&cursor[..start_index]);
        let after_prefix = &cursor[start_index + SCRIPT_TEMPLATE_INCLUDE_PREFIX.len()..];
        let Some(end_index) = after_prefix.find(SCRIPT_TEMPLATE_INCLUDE_SUFFIX) else {
            return Err("template include directive is missing closing '}}'".to_string());
        };

        let include_path = &after_prefix[..end_index];
        let include_relative_path = normalize_include_path(include_path)?;
        let include_key = include_relative_path.to_string_lossy().replace('\\', "/").to_ascii_lowercase();
        if include_stack_contains(include_stack, &include_key) {
            let cycle = include_stack
                .iter()
                .cloned()
                .chain(std::iter::once(include_key.clone()))
                .collect::<Vec<_>>()
                .join(" -> ");
            return Err(format!("template include cycle detected: {cycle}"));
        }

        let include_path = root_dir.join(&include_relative_path);
        let include_source = std::fs::read_to_string(&include_path)
            .map_err(|err| format!("failed to read template include '{}': {err}", include_path.display()))?;
        include_stack.push(include_key);
        let expanded = expand_template_source(&include_source, root_dir, include_stack);
        include_stack.pop();
        output.push_str(&expanded?);
        cursor = &after_prefix[end_index + SCRIPT_TEMPLATE_INCLUDE_SUFFIX.len()..];
    }

    output.push_str(cursor);
    Ok(output)
}

fn read_template_from_path(path: &Path, root_dir: &Path) -> Result<String, String> {
    let source = std::fs::read_to_string(path).map_err(|err| format!("failed to read script template '{}': {err}", path.display()))?;
    let mut stack = Vec::new();
    let relative = path
        .strip_prefix(root_dir)
        .map(|path| path.to_string_lossy().replace('\\', "/").to_ascii_lowercase())
        .unwrap_or_else(|_| path.to_string_lossy().replace('\\', "/").to_ascii_lowercase());
    stack.push(relative);
    expand_template_source(&source, root_dir, &mut stack)
}

fn default_embedded_template() -> String {
    let root_dir = script_template_root_dir();
    let mut stack = vec!["default.js".to_string()];
    expand_template_source(SCRIPT_TEMPLATE_DEFAULT_SOURCE, &root_dir, &mut stack).unwrap_or_else(|_| SCRIPT_TEMPLATE_DEFAULT_SOURCE.to_string())
}

fn resolve_template_for_host(host_node_type: &str) -> ScriptTemplateResolved {
    let root_dir = script_template_root_dir();
    for basename in template_candidate_basenames(host_node_type) {
        for extension in SCRIPT_TEMPLATE_EXTENSIONS {
            let path = root_dir.join(format!("{basename}.{extension}"));
            if !path.is_file() {
                continue;
            }

            match read_template_from_path(&path, &root_dir) {
                Ok(source) => {
                    return ScriptTemplateResolved { source };
                }
                Err(error) => {
                    let _ = logger::log_message(
                        logger::LogLevel::Warning,
                        "script".to_string(),
                        None,
                        format!("failed to load script template '{}': {error}", path.display()),
                    );
                }
            }
        }
    }

    ScriptTemplateResolved {
        source: default_embedded_template(),
    }
}

/// Runtime script node configuration.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptNodeConfig {
    /// Script source.
    pub source: ScriptSource,
}

impl ScriptNodeConfig {
    /// Creates default script config for a host node type using script templates.
    pub fn for_host_node_type(host_node_type: &str) -> Self {
        let template = resolve_template_for_host(host_node_type);
        Self { source: ScriptSource::Inline(template.source) }
    }

    fn validate_source_kind(&self) -> Result<(), ScriptRuntimeError> {
        let Some(path) = self.source.resolve_path() else {
            return Ok(());
        };

        let ext = path
            .extension()
            .and_then(|raw| raw.to_str())
            .map(|raw| raw.trim().to_ascii_lowercase());
        if matches!(ext.as_deref(), Some("js" | "mjs" | "cjs")) {
            return Ok(());
        }

        let ScriptSource::ProjectFile(raw_path) = &self.source else {
            return Ok(());
        };
        Err(ScriptRuntimeError::InvalidManifest(format!(
            "unsupported script file '{raw_path}', expected one of: .js, .mjs, .cjs"
        )))
    }
}

impl Default for ScriptNodeConfig {
    fn default() -> Self {
        Self::for_host_node_type("default")
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

/// UI payload for script source configuration.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptUiConfig {
    /// Script source selector.
    pub source: ScriptUiSource,
}

impl From<&ScriptNodeConfig> for ScriptUiConfig {
    fn from(value: &ScriptNodeConfig) -> Self {
        Self { source: ScriptUiSource::from(&value.source) }
    }
}

impl From<ScriptUiConfig> for ScriptNodeConfig {
    fn from(value: ScriptUiConfig) -> Self {
        Self { source: value.source.into() }
    }
}

/// UI-facing script node runtime state payload.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScriptUiState {
    /// Current script configuration.
    pub config: ScriptUiConfig,
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
            Self::QuickJs(message) => write!(f, "quickjs runtime error: {message}"),
            Self::InvalidManifest(message) => write!(f, "invalid script manifest: {message}"),
            Self::MissingExport(name) => write!(f, "missing script export '{name}'"),
            Self::BudgetViolation(message) => write!(f, "script budget violation: {message}"),
            Self::Host(message) => write!(f, "script host error: {message}"),
        }
    }
}

impl std::error::Error for ScriptRuntimeError {}

impl From<QuickJsError> for ScriptRuntimeError {
    fn from(value: QuickJsError) -> Self {
        Self::QuickJs(value.to_string())
    }
}

/// Runtime trait contract for embeddable scripting engines.
pub trait ScriptRuntime: Send {
    /// Loads script source and returns parsed manifest.
    fn load(&mut self, source: &str, source_name: &str) -> Result<ScriptManifest, ScriptRuntimeError>;
    /// Reloads script source and returns parsed manifest.
    fn reload(&mut self, source: &str, source_name: &str) -> Result<ScriptManifest, ScriptRuntimeError>;
    /// Returns current manifest when loaded.
    fn manifest(&self) -> Option<&ScriptManifest>;
    /// Returns exported function names.
    fn export_names(&self) -> Vec<String>;
    /// Calls one exported function.
    fn call_export(&mut self, export_name: &str, args: &[ScriptValue], host: &mut dyn ScriptHostBridge) -> Result<ScriptValue, ScriptRuntimeError>;
    /// Calls `init` if declared.
    fn call_on_init(&mut self, host: &mut dyn ScriptHostBridge) -> Result<(), ScriptRuntimeError>;
    /// Calls `update` if declared.
    fn call_on_update(&mut self, host: &mut dyn ScriptHostBridge) -> Result<(), ScriptRuntimeError>;
    /// Calls `event`/`paramChanged` if declared.
    fn call_on_event(&mut self, event: &ScriptEvent, host: &mut dyn ScriptHostBridge) -> Result<(), ScriptRuntimeError>;
    /// Calls `destroy` if declared.
    fn call_on_destroy(&mut self, host: &mut dyn ScriptHostBridge) -> Result<(), ScriptRuntimeError>;
    /// Returns `true` when an update hook is declared by the script.
    fn has_on_update(&self) -> bool;
}

enum ScriptHostOp {
    Log { level: ScriptLogLevel, message: String },
    EmitCustom { topic: String, payload: JsonValue },
}

#[derive(Default)]
struct QuickJsEntrypoints {
    init: Option<String>,
    update: Option<String>,
    event: Option<String>,
    param_changed: Option<String>,
    destroy: Option<String>,
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
                r#"
globalThis.gc.emit = (topic, payload) => globalThis.gc.__emit_raw(
  topic,
  JSON.stringify(payload === undefined ? null : payload)
);
globalThis.log = (message) => globalThis.gc.log("info", String(message ?? ""));
globalThis.success = (message) => globalThis.gc.log("success", String(message ?? ""));
globalThis.warn = (message) => globalThis.gc.log("warning", String(message ?? ""));
globalThis.error = (message) => globalThis.gc.log("error", String(message ?? ""));
globalThis.emit = (topic, payload) => globalThis.gc.emit(topic, payload);
"#,
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

    fn is_js_identifier_start(ch: char) -> bool {
        ch == '_' || ch == '$' || ch.is_ascii_alphabetic()
    }

    fn is_js_identifier_continue(ch: char) -> bool {
        Self::is_js_identifier_start(ch) || ch.is_ascii_digit()
    }

    fn parse_exported_function_name(declaration: &str) -> Option<&str> {
        let trimmed = declaration.trim_start();
        let mut chars = trimmed.char_indices();
        let (_, first) = chars.next()?;
        if !Self::is_js_identifier_start(first) {
            return None;
        }

        let mut end = first.len_utf8();
        for (index, ch) in chars {
            if Self::is_js_identifier_continue(ch) {
                end = index + ch.len_utf8();
            } else {
                break;
            }
        }
        Some(&trimmed[..end])
    }

    fn preprocess_source_for_exported_functions(source: &str) -> String {
        let mut transformed = String::new();
        let mut exported_names: Vec<String> = Vec::new();

        for segment in source.split_inclusive('\n') {
            let (line, line_break) = if let Some(stripped) = segment.strip_suffix('\n') {
                (stripped, "\n")
            } else {
                (segment, "")
            };

            let trimmed = line.trim_start();
            let indent_len = line.len().saturating_sub(trimmed.len());
            let indent = &line[..indent_len];

            if let Some(rest) = trimmed.strip_prefix("export function ") {
                if let Some(name) = Self::parse_exported_function_name(rest) {
                    if !exported_names.iter().any(|existing| existing == name) {
                        exported_names.push(name.to_string());
                    }
                    transformed.push_str(indent);
                    transformed.push_str("function ");
                    transformed.push_str(rest);
                    transformed.push_str(line_break);
                    continue;
                }
            }

            if let Some(rest) = trimmed.strip_prefix("export async function ") {
                if let Some(name) = Self::parse_exported_function_name(rest) {
                    if !exported_names.iter().any(|existing| existing == name) {
                        exported_names.push(name.to_string());
                    }
                    transformed.push_str(indent);
                    transformed.push_str("async function ");
                    transformed.push_str(rest);
                    transformed.push_str(line_break);
                    continue;
                }
            }

            transformed.push_str(segment);
        }

        if !exported_names.is_empty() {
            transformed.push_str("\n");
            transformed.push_str("// Auto-registered exported functions.\n");
            for name in exported_names {
                transformed.push_str("globalThis.__gc_script_exports[\"");
                transformed.push_str(&name);
                transformed.push_str("\"] = ");
                transformed.push_str(&name);
                transformed.push_str(";\n");
            }
        }

        transformed
    }

    fn first_function_name<'js>(object: &QuickJsObject<'js>, candidates: &[&str]) -> Result<Option<String>, QuickJsError> {
        for candidate in candidates {
            if object.get::<_, Option<QuickJsFunction>>(*candidate)?.is_some() {
                return Ok(Some((*candidate).to_string()));
            }
        }
        Ok(None)
    }

    fn collect_export_names<'js>(object: &QuickJsObject<'js>) -> Result<Vec<String>, QuickJsError> {
        let mut exports = Vec::new();
        for key in object.keys::<String>() {
            let key = key?;
            if object.get::<_, Option<QuickJsFunction>>(key.as_str())?.is_some() {
                exports.push(key);
            }
        }
        exports.sort();
        exports.dedup();
        Ok(exports)
    }

    fn lookup_export_callback<'js>(globals: &QuickJsObject<'js>, export_name: &str) -> Result<Option<QuickJsFunction<'js>>, QuickJsError> {
        if let Some(export_table) = globals.get::<_, Option<QuickJsObject>>("__gc_script_exports")? {
            if let Some(callback) = export_table.get::<_, Option<QuickJsFunction>>(export_name)? {
                return Ok(Some(callback));
            }
        }
        Ok(None)
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

    fn is_exception_placeholder(message: &str) -> bool {
        message.trim().eq_ignore_ascii_case("exception generated by quickjs")
    }

    fn js_value_to_text<'js>(ctx: &QuickJsCtx<'js>, value: &QuickJsValue<'js>) -> Option<String> {
        if value.is_string() {
            if let Ok(text) = value.get::<String>() {
                return Some(text);
            }
        }

        if let Ok(Some(raw)) = ctx.json_stringify(value) {
            if let Ok(text) = raw.to_string() {
                return Some(text);
            }
        }

        None
    }

    fn describe_quickjs_exception<'js>(ctx: &QuickJsCtx<'js>) -> String {
        let exception = ctx.catch();
        if exception.is_null() || exception.is_undefined() {
            return "exception generated by QuickJS with no error value".to_string();
        }

        if let Some(error_object) = exception.as_object() {
            let name = error_object.get::<_, Option<String>>("name").ok().flatten();
            let message = error_object.get::<_, Option<String>>("message").ok().flatten();
            let stack = error_object.get::<_, Option<String>>("stack").ok().flatten();
            let file_name = error_object.get::<_, Option<String>>("fileName").ok().flatten();
            let line_number = error_object.get::<_, Option<i32>>("lineNumber").ok().flatten();
            let column_number = error_object.get::<_, Option<i32>>("columnNumber").ok().flatten();

            let mut summary = match (name.as_deref(), message.as_deref()) {
                (Some(name), Some(message)) if !name.trim().is_empty() && !message.trim().is_empty() => {
                    format!("{name}: {message}")
                }
                (_, Some(message)) if !message.trim().is_empty() => message.to_string(),
                (Some(name), _) if !name.trim().is_empty() => name.to_string(),
                _ => "JavaScript exception".to_string(),
            };

            if let Some(stack) = stack.as_deref().map(str::trim).filter(|value| !value.is_empty()) {
                summary.push('\n');
                summary.push_str(stack);
                return summary;
            }

            if let Some(file_name) = file_name {
                let mut location = file_name;
                if let Some(line_number) = line_number {
                    location.push(':');
                    location.push_str(&line_number.to_string());
                    if let Some(column_number) = column_number {
                        location.push(':');
                        location.push_str(&column_number.to_string());
                    }
                }
                summary.push_str(" (");
                summary.push_str(&location);
                summary.push(')');
            }

            return summary;
        }

        if let Some(text) = Self::js_value_to_text(ctx, &exception) {
            return format!("JavaScript exception: {text}");
        }

        "JavaScript exception (unable to stringify thrown value)".to_string()
    }

    fn quickjs_error_with_context<'js>(ctx: &QuickJsCtx<'js>, phase: &str, error: QuickJsError) -> ScriptRuntimeError {
        if error.is_exception() {
            return ScriptRuntimeError::QuickJs(format!("{phase}: {}", Self::describe_quickjs_exception(ctx)));
        }

        ScriptRuntimeError::QuickJs(format!("{phase}: {error}"))
    }

    fn enrich_runtime_error_with_context<'js>(ctx: &QuickJsCtx<'js>, phase: &str, error: ScriptRuntimeError) -> ScriptRuntimeError {
        match error {
            ScriptRuntimeError::QuickJs(message) => {
                if Self::is_exception_placeholder(&message) {
                    ScriptRuntimeError::QuickJs(format!("{phase}: {}", Self::describe_quickjs_exception(ctx)))
                } else {
                    ScriptRuntimeError::QuickJs(format!("{phase}: {message}"))
                }
            }
            other => other,
        }
    }
}

impl ScriptRuntime for QuickJsRuntime {
    fn load(&mut self, source: &str, source_name: &str) -> Result<ScriptManifest, ScriptRuntimeError> {
        self.entrypoints = QuickJsEntrypoints::default();
        self.manifest = None;

        let bootstrap = r#"
globalThis.__gc_script_exports = {};
globalThis.__gc_script_manifest = {
  apiVersion: 1,
  updateRateHz: null,
  parameters: {},
  subscriptions: [],
  exports: globalThis.__gc_script_exports,
};
globalThis.script = {
  setApiVersion(value) {
    const numeric = Number(value);
    if (Number.isFinite(numeric) && numeric >= 1) {
      globalThis.__gc_script_manifest.apiVersion = Math.floor(numeric);
    }
    return globalThis.__gc_script_manifest.apiVersion;
  },
  setUpdateRateHz(value) {
    if (value === null || value === undefined) {
      globalThis.__gc_script_manifest.updateRateHz = null;
      return null;
    }
    const numeric = Number(value);
    if (!Number.isFinite(numeric) || numeric <= 0) {
      globalThis.__gc_script_manifest.updateRateHz = null;
      return null;
    }
    const rounded = Math.floor(numeric);
    globalThis.__gc_script_manifest.updateRateHz = rounded > 0 ? rounded : null;
    return globalThis.__gc_script_manifest.updateRateHz;
  },
  listen(node, maxDepth = 0) {
    const selector = typeof node === "string" ? node : String(node ?? "@host");
    const depth = Number.isFinite(Number(maxDepth)) && Number(maxDepth) >= 0 ? Math.floor(Number(maxDepth)) : 0;
    globalThis.__gc_script_manifest.subscriptions.push({ node: selector, maxDepth: depth });
    return { node: selector, maxDepth: depth };
  },
  unlisten(node, maxDepth = 0) {
    const selector = typeof node === "string" ? node : String(node ?? "@host");
    const depth = Number.isFinite(Number(maxDepth)) && Number(maxDepth) >= 0 ? Math.floor(Number(maxDepth)) : 0;
    const list = globalThis.__gc_script_manifest.subscriptions;
    for (let index = list.length - 1; index >= 0; index -= 1) {
      const item = list[index];
      if (item && item.node === selector && Number(item.maxDepth ?? item.max_depth ?? 0) === depth) {
        list.splice(index, 1);
        return true;
      }
    }
    return false;
  },
  clearListeners() {
    globalThis.__gc_script_manifest.subscriptions = [];
  },
  addParameter(name, spec = {}) {
    const key = String(name ?? "").trim();
    if (key.length === 0) {
      return false;
    }
    globalThis.__gc_script_manifest.parameters[key] = spec ?? {};
    return true;
  },
  removeParameter(name) {
    const key = String(name ?? "").trim();
    if (key.length === 0) {
      return false;
    }
    return delete globalThis.__gc_script_manifest.parameters[key];
  },
};
if (globalThis.gc && typeof globalThis.gc === "object") {
  globalThis.gc.script = globalThis.script;
}
"#;

        let source_name = source_name.to_string();
        let preprocessed_source = Self::preprocess_source_for_exported_functions(source);
        let (entrypoints, manifest_json) = self.context.with(|ctx| -> Result<(QuickJsEntrypoints, String), ScriptRuntimeError> {
            let result = (|| -> Result<(QuickJsEntrypoints, String), QuickJsError> {
                let mut bootstrap_options = QuickJsEvalOptions::default();
                bootstrap_options.filename = Some(format!("{source_name}#bootstrap"));
                ctx.eval_with_options::<(), _>(bootstrap, bootstrap_options)?;

                let mut eval_options = QuickJsEvalOptions::default();
                eval_options.filename = Some(source_name.clone());
                ctx.eval_with_options::<(), _>(preprocessed_source.as_str(), eval_options)?;

                let globals = ctx.globals();
                let root_value: QuickJsValue = globals.get("__gc_script_manifest")?;
                if root_value.is_null() || root_value.is_undefined() || !root_value.is_object() {
                    return Err(QuickJsError::new_from_js_message(
                        "value",
                        "object",
                        "script manifest state must be an object",
                    ));
                }
                let _root = root_value.into_object().ok_or_else(|| {
                    QuickJsError::new_from_js_message("value", "object", "script manifest state must be an object")
                })?;

                let init = Self::first_function_name(&globals, &["init"])?;
                let update = Self::first_function_name(&globals, &["update"])?;
                let event = Self::first_function_name(&globals, &["event"])?;
                let param_changed = Self::first_function_name(&globals, &["paramChanged"])?;
                let destroy = Self::first_function_name(&globals, &["destroy"])?;

                let mut exports = Vec::new();
                if let Some(export_table) = globals.get::<_, Option<QuickJsObject>>("__gc_script_exports")? {
                    exports.extend(Self::collect_export_names(&export_table)?);
                }
                exports.sort();
                exports.dedup();

                let manifest_json = ctx
                    .eval::<Option<String>, _>(
                        "JSON.stringify(globalThis.__gc_script_manifest ?? {}, (key, value) => typeof value === 'function' ? undefined : value)",
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
                        init,
                        update,
                        event,
                        param_changed,
                        destroy,
                        exports,
                    },
                    manifest_json,
                ))
            })();
            result.map_err(|error| Self::quickjs_error_with_context(&ctx, "script load", error))
        })?;

        let manifest_payload = serde_json::from_str::<JsonValue>(&manifest_json)
            .map_err(|err| ScriptRuntimeError::InvalidManifest(format!("failed to parse manifest JSON: {err}")))?;
        let manifest = parse_manifest_from_json(&manifest_payload, entrypoints.exports.clone())?;

        self.entrypoints = entrypoints;
        self.manifest = Some(manifest.clone());
        Ok(manifest)
    }

    fn reload(&mut self, source: &str, source_name: &str) -> Result<ScriptManifest, ScriptRuntimeError> {
        let budgets = self.budgets;
        *self = Self::new(budgets)?;
        self.load(source, source_name)
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
                let result = (|| -> Result<ScriptValue, ScriptRuntimeError> {
                    let globals = ctx.globals();
                    let callback = Self::lookup_export_callback(&globals, export_name)?
                        .ok_or_else(|| ScriptRuntimeError::MissingExport(export_name.to_string()))?;

                    let mut call_args = QuickJsArgs::new(ctx.clone(), args.len());
                    for argument in args {
                        call_args.push_arg(self.to_quickjs_value(&ctx, argument)?)?;
                    }
                    let return_value = callback.call_arg::<QuickJsValue>(call_args)?;
                    self.from_quickjs_value(&ctx, return_value)
                })();
                result.map_err(|error| Self::enrich_runtime_error_with_context(&ctx, "export callback", error))
            })
        });
        let flush_result = self.flush_host_ops(host);
        flush_result?;
        result
    }

    fn call_on_init(&mut self, host: &mut dyn ScriptHostBridge) -> Result<(), ScriptRuntimeError> {
        let Some(callback_name) = self.entrypoints.init.clone() else {
            return Ok(());
        };

        self.reset_host_callback_state()?;
        let result = self.callback_timed("on_init", || {
            self.context.with(|ctx| -> Result<(), ScriptRuntimeError> {
                let result = (|| -> Result<(), ScriptRuntimeError> {
                    let globals = ctx.globals();
                    if let Some(callback) = globals.get::<_, Option<QuickJsFunction>>(callback_name.as_str())? {
                        callback.call::<_, ()>(())?;
                    }
                    Ok(())
                })();
                result.map_err(|error| Self::enrich_runtime_error_with_context(&ctx, "on_init callback", error))
            })
        });
        let flush_result = self.flush_host_ops(host);
        flush_result?;
        result
    }

    fn call_on_update(&mut self, host: &mut dyn ScriptHostBridge) -> Result<(), ScriptRuntimeError> {
        let Some(callback_name) = self.entrypoints.update.clone() else {
            return Ok(());
        };

        self.reset_host_callback_state()?;
        let delta_seconds = host.delta_seconds();
        let result = self.callback_timed("on_update", || {
            self.context.with(|ctx| -> Result<(), ScriptRuntimeError> {
                let result = (|| -> Result<(), ScriptRuntimeError> {
                    let globals = ctx.globals();
                    if let Some(callback) = globals.get::<_, Option<QuickJsFunction>>(callback_name.as_str())? {
                        callback.call::<_, ()>((delta_seconds,))?;
                    }
                    Ok(())
                })();
                result.map_err(|error| Self::enrich_runtime_error_with_context(&ctx, "on_update callback", error))
            })
        });
        let flush_result = self.flush_host_ops(host);
        flush_result?;
        result
    }

    fn call_on_event(&mut self, event: &ScriptEvent, host: &mut dyn ScriptHostBridge) -> Result<(), ScriptRuntimeError> {
        let event_callback_name = self.entrypoints.event.clone();
        let param_changed_callback_name = if event.kind == "paramChanged" {
            self.entrypoints.param_changed.clone()
        } else {
            None
        };
        if event_callback_name.is_none() && param_changed_callback_name.is_none() {
            return Ok(());
        }

        self.reset_host_callback_state()?;
        let event_payload = serde_json::to_string(event)
            .map_err(|err| ScriptRuntimeError::InvalidManifest(format!("failed to encode event payload: {err}")))?;
        let result = self.callback_timed("on_event", || {
            self.context.with(|ctx| -> Result<(), ScriptRuntimeError> {
                let result = (|| -> Result<(), ScriptRuntimeError> {
                    let globals = ctx.globals();

                    if let Some(callback_name) = param_changed_callback_name.as_deref() {
                        if let Some(callback) = globals.get::<_, Option<QuickJsFunction>>(callback_name)? {
                            let event_value = ctx.json_parse(event_payload.as_str())?;
                            callback.call::<_, ()>((event_value,))?;
                        }
                    }

                    if let Some(callback_name) = event_callback_name.as_deref() {
                        if let Some(callback) = globals.get::<_, Option<QuickJsFunction>>(callback_name)? {
                            let event_value = ctx.json_parse(event_payload.as_str())?;
                            callback.call::<_, ()>((event_value,))?;
                        }
                    }
                    Ok(())
                })();
                result.map_err(|error| Self::enrich_runtime_error_with_context(&ctx, "on_event callback", error))
            })
        });
        let flush_result = self.flush_host_ops(host);
        flush_result?;
        result
    }

    fn call_on_destroy(&mut self, host: &mut dyn ScriptHostBridge) -> Result<(), ScriptRuntimeError> {
        let Some(callback_name) = self.entrypoints.destroy.clone() else {
            return Ok(());
        };

        self.reset_host_callback_state()?;
        let result = self.callback_timed("on_destroy", || {
            self.context.with(|ctx| -> Result<(), ScriptRuntimeError> {
                let result = (|| -> Result<(), ScriptRuntimeError> {
                    let globals = ctx.globals();
                    if let Some(callback) = globals.get::<_, Option<QuickJsFunction>>(callback_name.as_str())? {
                        callback.call::<_, ()>(())?;
                    }
                    Ok(())
                })();
                result.map_err(|error| Self::enrich_runtime_error_with_context(&ctx, "on_destroy callback", error))
            })
        });
        let flush_result = self.flush_host_ops(host);
        flush_result?;
        result
    }

    fn has_on_update(&self) -> bool {
        self.entrypoints.update.is_some()
    }
}

fn parse_manifest_from_json(payload: &JsonValue, export_names: Vec<String>) -> Result<ScriptManifest, ScriptRuntimeError> {
    let Some(root) = payload.as_object() else {
        return Err(ScriptRuntimeError::InvalidManifest("manifest JSON root must be an object".to_string()));
    };

    let api_version = json_object_get(root, &["apiVersion"])
        .and_then(JsonValue::as_u64)
        .unwrap_or(1) as u32;
    if api_version == 0 {
        return Err(ScriptRuntimeError::InvalidManifest(
            "apiVersion must be >= 1".to_string(),
        ));
    }

    let update_rate_hz = json_object_get(root, &["updateRateHz"])
        .and_then(JsonValue::as_u64)
        .map(|value| value as u32);
    let parameters = parse_parameter_specs_json(json_object_get(root, &["parameters"]))?;
    let subscriptions = parse_subscription_specs_json(json_object_get(root, &["subscriptions"]))?;
    let exports = export_names.into_iter().map(|name| ScriptExportSpec { name, signature: ScriptFnSignature::default() }).collect();

    Ok(ScriptManifest {
        api_version,
        update_rate_hz,
        parameters,
        subscriptions,
        exports,
    })
}

fn json_object_get<'a>(object: &'a serde_json::Map<String, JsonValue>, keys: &[&str]) -> Option<&'a JsonValue> {
    for key in keys {
        if let Some(value) = object.get(*key) {
            return Some(value);
        }
    }
    None
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
        let max_depth = json_object_get(entry, &["maxDepth"])
            .and_then(JsonValue::as_u64)
            .unwrap_or(0) as u32;
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
        constraints.step = json_object_get(entry, &["step"]).and_then(JsonValue::as_f64);
        constraints.step_base = json_object_get(entry, &["stepBase"]).and_then(JsonValue::as_f64);
        if let Some(policy_label) = entry.get("policy").and_then(JsonValue::as_str) {
            constraints.policy = match policy_label.trim().to_ascii_lowercase().as_str() {
                "clampadapt" | "clamp_adapt" | "clamp-adapt" => ParameterConstraintPolicy::ClampAdapt,
                "reject" => ParameterConstraintPolicy::Reject,
                _ => return Err(ScriptRuntimeError::InvalidManifest(format!("parameter '{name}' has unsupported policy '{policy_label}'"))),
            };
        }

        constraints.range = parse_range_constraint_json(value_type, entry.get("min"), entry.get("max"))?;

        if let Some(enum_options) = json_object_get(entry, &["enumOptions"]) {
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

        let decl_id = json_object_get(entry, &["declId"]).and_then(JsonValue::as_str).unwrap_or(name);
        let label = entry.get("label").and_then(JsonValue::as_str).map(ToString::to_string);
        let read_only = json_object_get(entry, &["readOnly"])
            .and_then(JsonValue::as_bool)
            .unwrap_or(false);

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

    if let Some(allowed_types) = json_object_get(entry, &["allowedTypes"]) {
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

    if let Some(allowed_extensions) = json_object_get(entry, &["allowedExtensions"]) {
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
    runtime: Box<dyn ScriptRuntime>,
}

fn create_runtime(budgets: ScriptBudgets) -> Result<Box<dyn ScriptRuntime>, ScriptRuntimeError> {
    Ok(Box::new(QuickJsRuntime::new(budgets)?))
}

/// Built-in QuickJS script node.
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
    runtime_subscriptions: Vec<crate::node::EventSubscription>,
    reload_requested: bool,
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
            runtime_subscriptions: Vec::new(),
            reload_requested: false,
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

    /// Returns UI-facing script state.
    pub fn ui_state(&self) -> ScriptUiState {
        ScriptUiState {
            config: ScriptUiConfig::from(&self.config),
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

        if force_reload {
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
        self.reload_requested = true;
        self.source_stamp = None;
        self.effective_update_rate_hz = None;
    }

    fn clear_runtime_subscriptions(&mut self, ctx: &mut ProcessCtx) {
        let owner = self.id();
        for subscription in self.runtime_subscriptions.drain(..) {
            ctx.remove_event_listener_subtree(owner, subscription.node, subscription.max_depth);
        }
    }

    fn resolve_subscription_target(&self, selector: &ScriptNodeSelector) -> Result<NodeId, String> {
        match selector {
            ScriptNodeSelector::NodeId(node) => Ok(*node),
            ScriptNodeSelector::HostPath(path)
            | ScriptNodeSelector::RootPath(path)
            | ScriptNodeSelector::Path(path) => {
                if path.trim().is_empty() || path.trim() == "." {
                    self.node_data.parent.ok_or_else(|| "script node is detached and has no host parent".to_string())
                } else {
                    Err(format!(
                        "selector path '{}' is not resolved yet for runtime subscriptions; use '@host' for now",
                        path
                    ))
                }
            }
        }
    }

    fn desired_runtime_subscriptions(&self, manifest: &ScriptManifest) -> Vec<crate::node::EventSubscription> {
        let mut subscriptions = HashSet::new();
        for spec in &manifest.subscriptions {
            match self.resolve_subscription_target(&spec.node) {
                Ok(target) => {
                    subscriptions.insert(crate::node::EventSubscription::subtree(target, spec.max_depth));
                }
                Err(reason) => {
                    let _ = logger::log_message(
                        logger::LogLevel::Warning,
                        "script".to_string(),
                        Some(self.id()),
                        format!("ignored script subscription: {reason}"),
                    );
                }
            }
        }
        subscriptions.into_iter().collect()
    }

    fn sync_runtime_subscriptions(&mut self, ctx: &mut ProcessCtx, manifest: &ScriptManifest) {
        let owner = self.id();
        let desired = self.desired_runtime_subscriptions(manifest);
        let desired_set = desired.iter().copied().collect::<HashSet<_>>();
        let current_set = self.runtime_subscriptions.iter().copied().collect::<HashSet<_>>();

        for removed in current_set.difference(&desired_set) {
            ctx.remove_event_listener_subtree(owner, removed.node, removed.max_depth);
        }
        for added in desired_set.difference(&current_set) {
            ctx.add_event_listener_subtree(owner, added.node, added.max_depth);
        }

        self.runtime_subscriptions = desired;
    }

    fn teardown_runtime(&mut self, ctx: &mut ProcessCtx) {
        self.clear_runtime_subscriptions(ctx);

        let owner = self.id();
        if let Some(mut active) = self.runtime.take() {
            let mut host = NodeScriptHostBridge::new(owner, ctx);
            if let Err(error) = active.runtime.call_on_destroy(&mut host) {
                self.handle_runtime_error(ctx, &error);
            }
        }
    }

    fn source_file_modified(&self) -> Option<SystemTime> {
        let path = self.config.source.resolve_path()?;
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

                let script_source = self.config.source.load_text()?;
                Ok(hash_source_text(&script_source) != last_stamp.source_hash)
            }
        }
    }

    fn load_or_reload_internal(&mut self, ctx: &mut ProcessCtx, force_reload: bool) -> Result<(), ScriptRuntimeError> {
        if !self.node_data.meta.enabled {
            self.teardown_runtime(ctx);
            self.reload_requested = false;
            self.manifest = None;
            self.source_stamp = None;
            self.effective_update_rate_hz = None;
            return Ok(());
        }

        if self.runtime.is_some() && !force_reload && !self.reload_requested {
            return Ok(());
        }

        let script_source = self.config.source.load_text()?;
        self.config.validate_source_kind()?;
        let source_stamp = self.source_stamp_from_text(&script_source);
        self.teardown_runtime(ctx);

        let mut runtime = create_runtime(self.budgets)?;
        let source_name = self.config.source.runtime_source_name();
        let manifest = runtime.load(&script_source, &source_name)?;
        self.sync_runtime_subscriptions(ctx, &manifest);

        let mut host = NodeScriptHostBridge::new(self.id(), ctx);
        runtime.call_on_init(&mut host)?;

        self.effective_update_rate_hz = manifest.update_rate_hz;
        self.manifest = Some(manifest);
        self.runtime = Some(ActiveRuntime { runtime });
        self.source_stamp = Some(source_stamp);
        self.reload_requested = false;
        ctx.reevaluate_graph();
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
        if !self.node_data.meta.enabled {
            self.teardown_runtime(ctx);
            return;
        }

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
        if !self.node_data.meta.enabled {
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
        self.teardown_runtime(ctx);
        self.manifest = None;
        self.source_stamp = None;
        self.effective_update_rate_hz = None;
        self.reload_requested = false;
    }

    fn execution_rule(&self) -> NodeExecutionRule {
        if !self.node_data.meta.enabled {
            return NodeExecutionRule::passive();
        }

        if self.reload_requested || self.runtime.is_none() {
            return NodeExecutionRule::periodic(SCRIPT_BOOTSTRAP_UPDATE_RATE_HZ);
        }

        let has_on_update = self.runtime.as_ref().is_some_and(|active| active.runtime.has_on_update());
        if !has_on_update {
            if self.config.source.is_file_backed() {
                return NodeExecutionRule::periodic(SCRIPT_FILE_RELOAD_POLL_HZ);
            }
            return NodeExecutionRule::passive();
        }

        match self.effective_update_rate_hz {
            Some(rate_hz) if rate_hz > 0 => NodeExecutionRule::periodic(rate_hz),
            None => NodeExecutionRule::periodic(SCRIPT_BOOTSTRAP_UPDATE_RATE_HZ),
            _ => NodeExecutionRule::passive(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
    use std::sync::Arc;

    use crate::edit::Edit;
    use crate::engine::EngineTime;
    use crate::process_ctx::ExecutionPhase;

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

    struct MockRuntime {
        has_on_update: bool,
        destroy_counter: Arc<AtomicUsize>,
    }

    impl MockRuntime {
        fn new(has_on_update: bool, destroy_counter: Arc<AtomicUsize>) -> Self {
            Self {
                has_on_update,
                destroy_counter,
            }
        }
    }

    impl ScriptRuntime for MockRuntime {
        fn load(&mut self, _source: &str, _source_name: &str) -> Result<ScriptManifest, ScriptRuntimeError> {
            Ok(ScriptManifest::default())
        }

        fn reload(&mut self, _source: &str, _source_name: &str) -> Result<ScriptManifest, ScriptRuntimeError> {
            Ok(ScriptManifest::default())
        }

        fn manifest(&self) -> Option<&ScriptManifest> {
            None
        }

        fn export_names(&self) -> Vec<String> {
            Vec::new()
        }

        fn call_export(&mut self, _export_name: &str, _args: &[ScriptValue], _host: &mut dyn ScriptHostBridge) -> Result<ScriptValue, ScriptRuntimeError> {
            Ok(ScriptValue::Nil)
        }

        fn call_on_init(&mut self, _host: &mut dyn ScriptHostBridge) -> Result<(), ScriptRuntimeError> {
            Ok(())
        }

        fn call_on_update(&mut self, _host: &mut dyn ScriptHostBridge) -> Result<(), ScriptRuntimeError> {
            Ok(())
        }

        fn call_on_event(&mut self, _event: &ScriptEvent, _host: &mut dyn ScriptHostBridge) -> Result<(), ScriptRuntimeError> {
            Ok(())
        }

        fn call_on_destroy(&mut self, _host: &mut dyn ScriptHostBridge) -> Result<(), ScriptRuntimeError> {
            self.destroy_counter.fetch_add(1, AtomicOrdering::SeqCst);
            Ok(())
        }

        fn has_on_update(&self) -> bool {
            self.has_on_update
        }
    }

    #[test]
    fn quickjs_runtime_loads_manifest_and_exports() {
        let source = r#"
script.setApiVersion(1);
script.setUpdateRateHz(30);
export function ping(value) {
  log("ping called");
  emit("script.test", { value });
  return value;
}
"#;

        let mut runtime = QuickJsRuntime::new(ScriptBudgets::default()).expect("runtime should initialize");
        let manifest = runtime.load(source, "quickjs_runtime_loads_manifest_and_exports.js").expect("manifest should parse");
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

    #[test]
    fn quickjs_runtime_accepts_empty_script() {
        let mut runtime = QuickJsRuntime::new(ScriptBudgets::default()).expect("runtime should initialize");
        let manifest = runtime.load("", "empty_script.js").expect("empty script should parse");
        assert_eq!(manifest.api_version, 1);
        assert_eq!(manifest.update_rate_hz, None);
        assert!(manifest.parameters.is_empty());
        assert!(manifest.subscriptions.is_empty());
        assert!(runtime.export_names().is_empty());
    }

    #[test]
    fn quickjs_runtime_script_methods_build_manifest() {
        let source = r#"
script.setApiVersion(2);
script.setUpdateRateHz(24);
script.addParameter("gain", { type: "float", default: 0.5, readOnly: true });
script.listen("@host", 2);
"#;

        let mut runtime = QuickJsRuntime::new(ScriptBudgets::default()).expect("runtime should initialize");
        let manifest = runtime.load(source, "manifest_methods_test.js").expect("manifest should parse");
        assert_eq!(manifest.api_version, 2);
        assert_eq!(manifest.update_rate_hz, Some(24));
        assert_eq!(manifest.parameters.len(), 1);
        assert_eq!(manifest.parameters[0].name, "gain");
        assert!(manifest.parameters[0].read_only);
        assert_eq!(manifest.subscriptions.len(), 1);
        assert_eq!(manifest.subscriptions[0], ScriptSubscriptionSpec { node: ScriptNodeSelector::HostPath(String::new()), max_depth: 2 });
    }

    #[test]
    fn quickjs_runtime_invokes_param_changed_hook() {
        let source = r#"
function paramChanged(event) {
  log(String(event.kind));
}
"#;

        let mut runtime = QuickJsRuntime::new(ScriptBudgets::default()).expect("runtime should initialize");
        runtime
            .load(source, "param_changed_callback_test.js")
            .expect("script should parse");

        let event = ScriptEvent {
            kind: "paramChanged".to_string(),
            origin: None,
            payload: JsonValue::Null,
        };
        let mut host = TestHostBridge::new();
        runtime
            .call_on_event(&event, &mut host)
            .expect("paramChanged callback should execute");
        assert_eq!(host.logs.len(), 1);
    }

    #[test]
    fn quickjs_runtime_reports_parse_error_with_source_location() {
        let source = r#"
function update(delta) {
  void delta;
  const broken = ;
}
"#;

        let mut runtime = QuickJsRuntime::new(ScriptBudgets::default()).expect("runtime should initialize");
        let error = runtime
            .load(source, "parse_error_test.js")
            .expect_err("invalid script should fail to parse");
        let ScriptRuntimeError::QuickJs(message) = error else {
            panic!("expected quickjs error");
        };

        assert!(message.contains("parse_error_test.js"), "error: {message}");
        assert!(message.to_ascii_lowercase().contains("syntax"), "error: {message}");
    }

    #[test]
    fn quickjs_runtime_reports_callback_error_with_stack() {
        let source = r#"
script.setUpdateRateHz(60);
function update(delta) {
  void delta;
  throw new Error("boom");
}
"#;

        let mut runtime = QuickJsRuntime::new(ScriptBudgets::default()).expect("runtime should initialize");
        runtime
            .load(source, "callback_error_test.js")
            .expect("manifest should parse");

        let mut host = TestHostBridge::new();
        let error = runtime
            .call_on_update(&mut host)
            .expect_err("on_update should surface thrown exception");
        let ScriptRuntimeError::QuickJs(message) = error else {
            panic!("expected quickjs error");
        };

        assert!(message.contains("boom"), "error: {message}");
        assert!(message.contains("callback_error_test.js"), "error: {message}");
    }

    #[test]
    fn default_template_includes_snippets() {
        let config = ScriptNodeConfig::for_host_node_type("default");
        let source = match config.source {
            ScriptSource::Inline(source) => source,
            ScriptSource::ProjectFile(path) => panic!("expected inline source, got project file: {path}"),
        };
        assert!(source.contains("script.setApiVersion(1);"), "template source:\n{source}");
        assert!(source.contains("script.addParameter(\"gain\""), "template source:\n{source}");
        assert!(source.contains("function update(delta)"), "template source:\n{source}");
    }

    #[test]
    fn host_specific_template_is_selected() {
        let config = ScriptNodeConfig::for_host_node_type("module");
        let source = match config.source {
            ScriptSource::Inline(source) => source,
            ScriptSource::ProjectFile(path) => panic!("expected inline source, got project file: {path}"),
        };
        assert!(source.contains("module-scoped script initialized"));
    }

    #[test]
    fn script_node_selector_serializes_with_tagged_content_shape() {
        let selector = ScriptNodeSelector::HostPath("child/path".to_string());
        let encoded = serde_json::to_string(&selector).expect("selector should serialize");
        assert_eq!(encoded, r#"{"kind":"hostPath","value":"child/path"}"#);
    }

    #[test]
    fn host_subscription_resolves_to_parent_listener() {
        let mut script = ScriptNode::new("Script", ScriptNodeConfig::default());
        script.node_data.parent = Some(NodeId(42));
        let mut manifest = ScriptManifest::default();
        manifest.subscriptions = vec![ScriptSubscriptionSpec {
            node: ScriptNodeSelector::HostPath(String::new()),
            max_depth: 2,
        }];

        let subscriptions = script.desired_runtime_subscriptions(&manifest);
        assert_eq!(subscriptions.len(), 1);
        assert_eq!(subscriptions[0], crate::node::EventSubscription::subtree(NodeId(42), 2));
    }

    #[test]
    fn sync_runtime_subscriptions_queues_engine_listener_edits() {
        let mut script = ScriptNode::new("Script", ScriptNodeConfig::default());
        script.node_data.id = NodeId(99);
        script.node_data.parent = Some(NodeId(42));
        let mut manifest = ScriptManifest::default();
        manifest.subscriptions = vec![ScriptSubscriptionSpec {
            node: ScriptNodeSelector::HostPath(String::new()),
            max_depth: 2,
        }];

        let mut ctx = ProcessCtx::new(
            ExecutionPhase::EngineTick,
            EngineTime {
                tick: 0,
                micro: 0,
                seq: 0,
            },
        );
        script.sync_runtime_subscriptions(&mut ctx, &manifest);

        assert!(ctx.edits.pending.iter().any(|request| matches!(
            &request.edit,
            crate::edit::Edit::AddEventListener { subscriber, subscription }
                if *subscriber == NodeId(99) && *subscription == crate::node::EventSubscription::subtree(NodeId(42), 2)
        )));
    }

    #[test]
    fn inline_script_without_on_update_is_passive_after_load() {
        let destroy_counter = Arc::new(AtomicUsize::new(0));
        let mut script = ScriptNode::new("Script", ScriptNodeConfig::default());
        script.runtime = Some(ActiveRuntime {
            runtime: Box::new(MockRuntime::new(false, destroy_counter)),
        });
        script.reload_requested = false;
        script.effective_update_rate_hz = Some(60);
        script.config.source = ScriptSource::Inline(String::new());
        script.node_data.meta.enabled = true;

        let rule = script.execution_rule();
        assert_eq!(rule.update_rate, None);
    }

    #[test]
    fn file_backed_script_without_on_update_still_polls_for_reload() {
        let destroy_counter = Arc::new(AtomicUsize::new(0));
        let mut script = ScriptNode::new("Script", ScriptNodeConfig::default());
        script.runtime = Some(ActiveRuntime {
            runtime: Box::new(MockRuntime::new(false, destroy_counter)),
        });
        script.reload_requested = false;
        script.effective_update_rate_hz = Some(60);
        script.config.source = ScriptSource::ProjectFile("scripts/example.js".to_string());
        script.node_data.meta.enabled = true;

        let rule = script.execution_rule();
        assert_eq!(rule.update_rate, Some(SCRIPT_FILE_RELOAD_POLL_HZ));
    }

    #[test]
    fn request_reload_keeps_runtime_until_teardown() {
        let destroy_counter = Arc::new(AtomicUsize::new(0));
        let mut script = ScriptNode::new("Script", ScriptNodeConfig::default());
        script.runtime = Some(ActiveRuntime {
            runtime: Box::new(MockRuntime::new(true, Arc::clone(&destroy_counter))),
        });
        script.request_reload();
        assert!(script.runtime.is_some(), "runtime should stay alive until teardown");

        let mut ctx = ProcessCtx::new(
            ExecutionPhase::EngineTick,
            EngineTime {
                tick: 0,
                micro: 0,
                seq: 0,
            },
        );
        script.destroy(&mut ctx);
        assert_eq!(destroy_counter.load(AtomicOrdering::SeqCst), 1);
    }

    #[test]
    fn manifest_update_rate_applies_to_execution_rule_and_queues_reeval() {
        let source = r#"
script.setUpdateRateHz(12);
function update(delta) {
  void delta;
}
"#;
        let mut script = ScriptNode::new(
            "Script",
            ScriptNodeConfig {
                source: ScriptSource::Inline(source.to_string()),
            },
        );

        let mut ctx = ProcessCtx::new(
            ExecutionPhase::EngineTick,
            EngineTime {
                tick: 0,
                micro: 0,
                seq: 0,
            },
        );

        script.init(&mut ctx);
        let rule = script.execution_rule();
        assert_eq!(rule.update_rate, Some(12));
        assert!(ctx.edits.pending.iter().any(|request| matches!(request.edit, Edit::ReevaluateGraph)));
    }
}
