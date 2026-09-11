use std::fmt;
use std::sync::Arc;

use golden_model::NodeId;
use golden_parameters::{ParamValue, ParameterConstraints};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

pub use golden_script_contract::{
    ScriptExportSpec, ScriptFnSignature, ScriptManifest, ScriptNodeSelector, ScriptParameterSpec,
    ScriptSubscriptionSpec, ScriptUiConfig, ScriptUiSource, ScriptUiState, ScriptValueType,
};

/// Hard safety guardrails applied per script instance.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptBudgets {
    /// Maximum VM instruction target for one callback.
    pub max_instructions_per_callback: u64,
    /// Maximum VM instruction target while loading source and manifest state.
    #[serde(default = "default_script_load_instruction_budget")]
    pub max_instructions_per_load: u64,
    /// Maximum callback wall time in microseconds.
    pub max_wall_time_us_per_callback: u64,
    /// Maximum source-load wall time in microseconds.
    #[serde(default = "default_script_load_wall_time_us")]
    pub max_wall_time_us_per_load: u64,
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
            max_instructions_per_load: default_script_load_instruction_budget(),
            max_wall_time_us_per_callback: 50_000,
            max_wall_time_us_per_load: default_script_load_wall_time_us(),
            max_memory_bytes: 16 * 1024 * 1024,
            max_host_calls_per_callback: 1_024,
            max_emitted_edits_per_tick: 512,
            max_emitted_events_per_tick: 512,
        }
    }
}

const fn default_script_load_instruction_budget() -> u64 {
    1_000_000
}

const fn default_script_load_wall_time_us() -> u64 {
    250_000
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
    pub(crate) fn from_manifest_label(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "info" => Some(Self::Info),
            "success" => Some(Self::Success),
            "warning" | "warn" => Some(Self::Warning),
            "error" => Some(Self::Error),
            _ => None,
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
    /// Previous parameter value for `paramChanged` events.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub old_value: Option<ParamValue>,
    /// Event payload.
    pub payload: JsonValue,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ScriptCallbackInvocation {
    pub(crate) name: String,
    pub(crate) args: Vec<JsonValue>,
}

impl ScriptEvent {
    fn custom_payload(&self) -> Option<&JsonValue> {
        if self.kind != "custom" {
            return None;
        }

        self.payload
            .get("Custom")
            .and_then(|custom| custom.get("payload"))
            .or_else(|| self.payload.get("payload"))
    }

    pub(crate) fn custom_callback_invocation(&self) -> Option<ScriptCallbackInvocation> {
        let payload = self.custom_payload()?;
        let object = payload.as_object()?;
        let name = object
            .get("callback")
            .or_else(|| object.get("callbackName"))
            .and_then(JsonValue::as_str)?
            .trim();
        if name.is_empty() {
            return None;
        }

        let args = object
            .get("args")
            .and_then(JsonValue::as_array)
            .cloned()
            .unwrap_or_default();
        Some(ScriptCallbackInvocation {
            name: name.to_string(),
            args,
        })
    }
}

/// Read-only node data exposed to a script VM.
#[derive(Clone, Debug, PartialEq)]
pub struct ScriptTreeNodeView {
    /// Runtime node id.
    pub id: NodeId,
    /// Runtime node type.
    pub node_type: String,
    /// Declaration id used by path selectors.
    pub decl_id: String,
    /// Generated short name.
    pub short_name: String,
    /// User-visible label.
    pub label: String,
    /// Effective enabled state.
    pub enabled: bool,
    /// Number of direct children.
    pub child_count: usize,
    /// Current parameter value when this is a parameter.
    pub param_value: Option<ParamValue>,
    /// Current parameter constraints when this is a parameter.
    pub param_constraints: Option<ParameterConstraints>,
}

impl ScriptTreeNodeView {
    /// Returns whether this view represents a parameter.
    pub fn is_parameter(&self) -> bool {
        self.param_value.is_some()
    }
}

/// Immutable tree interface supplied by a script host.
///
/// Implementations may retain their native snapshot representation; the VM only
/// observes this object-safe read contract.
pub trait ScriptTreeView: Send + Sync {
    /// Returns the root node id.
    fn root(&self) -> NodeId;
    /// Returns one node view.
    fn node(&self, node: NodeId) -> Option<ScriptTreeNodeView>;
    /// Finds one direct child by declaration id, short name, or label.
    fn find_child(&self, parent: NodeId, key: &str) -> Option<NodeId>;
    /// Returns direct children in stable sibling order.
    fn child_ids(&self, parent: NodeId) -> Vec<NodeId>;
    /// Returns one direct child by stable sibling index.
    fn child_at(&self, parent: NodeId, index: usize) -> Option<NodeId>;
    /// Returns one script-facing property value.
    fn script_property(&self, node: NodeId, key: &str) -> Option<ParamValue>;
    /// Returns all additional script-facing properties.
    fn script_properties(&self, node: NodeId) -> Vec<(String, ParamValue)>;
    /// Returns whether a method is script-callable on the node.
    fn has_script_method(&self, node: NodeId, method: &str) -> bool;
    /// Returns the user-facing node description used by toString.
    fn describe_node(&self, node: NodeId) -> Option<String>;
}

/// Host bridge consumed by script runtimes.
pub trait ScriptHostBridge {
    /// Owning node id when available.
    fn owner_node(&self) -> Option<NodeId> {
        None
    }

    /// Script node id when available.
    ///
    /// Defaults to [`Self::owner_node`] for host implementations that do not distinguish
    /// between script container and local host target.
    fn script_node(&self) -> Option<NodeId> {
        self.owner_node()
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

    /// Returns a read-only tree snapshot for the current callback.
    fn tree_snapshot(&self) -> Option<Arc<dyn ScriptTreeView>> {
        None
    }

    /// Queues one script-exposed property write on `node`.
    fn set_node_script_property(&mut self, _node: NodeId, _property: String, _value: ParamValue) -> Result<(), String> {
        Err("node script-property mutation is unavailable for this script host".to_string())
    }

    /// Queues one script-exposed method call on `node`.
    fn call_node_script_method(
        &mut self,
        _node: NodeId,
        _method: String,
        _args: Vec<ParamValue>,
    ) -> Result<(), String> {
        Err("node script-method invocation is unavailable for this script host".to_string())
    }

    /// Sets or updates one runtime listener configuration for this script.
    fn set_event_listener(&mut self, _target: NodeId, _level: u32) -> Result<(), String> {
        Err("runtime event listeners are unavailable for this script host".to_string())
    }

    /// Removes one runtime listener configuration for this script.
    fn remove_event_listener(&mut self, _target: NodeId) -> Result<(), String> {
        Err("runtime event listeners are unavailable for this script host".to_string())
    }

    /// Removes all runtime listener configurations for this script.
    fn clear_event_listeners(&mut self) -> Result<(), String> {
        Err("runtime event listeners are unavailable for this script host".to_string())
    }
}

/// Default host bridge used when no engine context is available.
pub struct NoopScriptHostBridge;

impl ScriptHostBridge for NoopScriptHostBridge {
    fn log(&mut self, _level: ScriptLogLevel, _message: &str) {}

    fn emit_custom(&mut self, _topic: &str, _payload: JsonValue) -> Result<(), String> {
        Ok(())
    }

    fn set_event_listener(&mut self, _target: NodeId, _level: u32) -> Result<(), String> {
        Ok(())
    }

    fn remove_event_listener(&mut self, _target: NodeId) -> Result<(), String> {
        Ok(())
    }

    fn clear_event_listeners(&mut self) -> Result<(), String> {
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

/// Runtime trait contract for embeddable scripting engines.
pub trait ScriptRuntime: Send {
    /// Loads script source and returns parsed manifest.
    fn load(
        &mut self,
        source: &str,
        source_name: &str,
        host: Option<&mut dyn ScriptHostBridge>,
    ) -> Result<ScriptManifest, ScriptRuntimeError>;
    /// Reloads script source and returns parsed manifest.
    fn reload(
        &mut self,
        source: &str,
        source_name: &str,
        host: Option<&mut dyn ScriptHostBridge>,
    ) -> Result<ScriptManifest, ScriptRuntimeError>;
    /// Returns current manifest when loaded.
    fn manifest(&self) -> Option<&ScriptManifest>;
    /// Returns exported function names.
    fn export_names(&self) -> Vec<String>;
    /// Calls one exported function.
    fn call_export(
        &mut self,
        export_name: &str,
        args: &[ScriptValue],
        host: &mut dyn ScriptHostBridge,
    ) -> Result<ScriptValue, ScriptRuntimeError>;
    /// Calls `init` if declared.
    fn call_on_init(&mut self, host: &mut dyn ScriptHostBridge) -> Result<(), ScriptRuntimeError>;
    /// Calls `update` if declared.
    fn call_on_update(&mut self, host: &mut dyn ScriptHostBridge) -> Result<(), ScriptRuntimeError>;
    /// Calls `event`/`paramChanged` if declared.
    fn call_on_event(&mut self, event: &ScriptEvent, host: &mut dyn ScriptHostBridge)
    -> Result<(), ScriptRuntimeError>;
    /// Calls `destroy` if declared.
    fn call_on_destroy(&mut self, host: &mut dyn ScriptHostBridge) -> Result<(), ScriptRuntimeError>;
    /// Returns `true` when an update hook is declared by the script.
    fn has_on_update(&self) -> bool;
}
