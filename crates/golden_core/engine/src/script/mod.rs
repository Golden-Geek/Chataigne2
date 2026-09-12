use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

use crate::edit::Edit;
use crate::engine::NodeExecutionRule;
use crate::node::{Node, NodeData, NodeId};
use crate::parameter::ParamValue;
use crate::process_ctx::{ProcessCtx, ProcessTreeNodeSnapshot};

pub use golden_script::{
    NoopScriptHostBridge, QuickJsRuntime, ScriptBudgets, ScriptCancellationHandle, ScriptEvent, ScriptHostBridge,
    ScriptLogLevel, ScriptRuntime, ScriptRuntimeError, ScriptTreeNodeView, ScriptTreeView, ScriptValue,
};
pub use golden_script_contract::{
    ScriptExportSpec, ScriptFnSignature, ScriptManifest, ScriptNodeSelector, ScriptParameterSpec,
    ScriptSubscriptionSpec, ScriptUiConfig, ScriptUiSource, ScriptUiState, ScriptValueType,
};

mod host;
mod template;

use host::NodeScriptHostBridge;
use template::{resolve_template_for_host, resolve_template_for_host_in_dir};

/// Script-host policy for one node type.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ScriptHostPolicy {
    /// Whether script hosting is enabled for this node.
    pub enabled: bool,
}

impl ScriptHostPolicy {
    /// Default policy used by `#[node(scriptable)]` and `#[item(..., scriptable)]`.
    pub fn default_scriptable() -> Self {
        Self { enabled: true }
    }
}

/// Script source selection.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ScriptSource {
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

impl ScriptSource {
    /// Resolves this source to an on-disk path when file-backed.
    pub fn resolve_path(&self) -> Option<PathBuf> {
        match self {
            Self::Inline { .. } => None,
            Self::ProjectFile { path } => Some(PathBuf::from(path)),
        }
    }

    /// Loads source text from configured source.
    pub fn load_text(&self) -> Result<String, ScriptRuntimeError> {
        match self {
            Self::Inline { text } => Ok(text.clone()),
            Self::ProjectFile { path } => {
                let resolved = self.resolve_path().unwrap_or_else(|| PathBuf::from(path));
                std::fs::read_to_string(&resolved).map_err(|err| {
                    ScriptRuntimeError::Io(format!("failed to read script file '{}': {err}", resolved.display()))
                })
            }
        }
    }

    fn is_file_backed(&self) -> bool {
        matches!(self, Self::ProjectFile { .. })
    }

    fn runtime_source_name(&self) -> String {
        match self {
            Self::Inline { .. } => "inline_script.js".to_string(),
            Self::ProjectFile { path } if !path.trim().is_empty() => path.clone(),
            Self::ProjectFile { .. } => "script_file.js".to_string(),
        }
    }
}

const SCRIPT_BOOTSTRAP_UPDATE_RATE_HZ: u32 = 60;
const SCRIPT_FILE_RELOAD_POLL_HZ: u32 = 30;

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
        Self {
            source: ScriptSource::Inline { text: template.source },
        }
    }

    /// Tries to create default script config from a caller-provided template directory.
    pub fn try_for_host_node_type_in_template_dir(
        host_node_type: &str,
        template_dir: impl AsRef<Path>,
    ) -> Option<Self> {
        let template = resolve_template_for_host_in_dir(host_node_type, template_dir.as_ref())?;
        Some(Self {
            source: ScriptSource::Inline { text: template.source },
        })
    }

    /// Creates default script config for a host node type using a caller-provided template directory.
    pub fn for_host_node_type_in_template_dir(host_node_type: &str, template_dir: impl AsRef<Path>) -> Self {
        Self::try_for_host_node_type_in_template_dir(host_node_type, template_dir)
            .unwrap_or_else(|| Self::for_host_node_type(host_node_type))
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

        let ScriptSource::ProjectFile { path: raw_path } = &self.source else {
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

impl From<&ScriptSource> for ScriptUiSource {
    fn from(value: &ScriptSource) -> Self {
        match value {
            ScriptSource::Inline { text } => Self::Inline { text: text.clone() },
            ScriptSource::ProjectFile { path } => Self::ProjectFile { path: path.clone() },
        }
    }
}

impl From<ScriptUiSource> for ScriptSource {
    fn from(value: ScriptUiSource) -> Self {
        match value {
            ScriptUiSource::Inline { text } => Self::Inline { text },
            ScriptUiSource::ProjectFile { path } => Self::ProjectFile { path },
        }
    }
}

impl From<&ScriptNodeConfig> for ScriptUiConfig {
    fn from(value: &ScriptNodeConfig) -> Self {
        Self {
            source: ScriptUiSource::from(&value.source),
        }
    }
}

impl From<ScriptUiConfig> for ScriptNodeConfig {
    fn from(value: ScriptUiConfig) -> Self {
        Self {
            source: value.source.into(),
        }
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

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct ManagedLoadChild {
    parent: NodeId,
    key: String,
}

fn managed_child_key_matches(snapshot: &ProcessTreeNodeSnapshot, key: &str) -> bool {
    let key = key.trim();
    if key.is_empty() {
        return false;
    }

    snapshot.decl_id.eq_ignore_ascii_case(key)
        || snapshot.short_name.eq_ignore_ascii_case(key)
        || snapshot.label.eq_ignore_ascii_case(key)
}

fn managed_child_from_script_call(parent: NodeId, method: &str, args: &[ParamValue]) -> Option<ManagedLoadChild> {
    let key = match method {
        "addParameter" => args
            .first()
            .and_then(ParamValue::as_str)
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "parameter".to_string()),
        "addFolder" => args
            .first()
            .and_then(ParamValue::as_str)
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "Folder".to_string()),
        "addNode" => {
            let node_type = args
                .first()
                .and_then(ParamValue::as_str)
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "folder".to_string());
            let normalized_node_type = node_type.trim().to_ascii_lowercase();
            let default_label = match normalized_node_type.as_str() {
                "parameter" | "param" => "parameter".to_string(),
                "folder" | "" => "Folder".to_string(),
                _ => node_type.clone(),
            };
            args.get(1)
                .and_then(ParamValue::as_str)
                .filter(|value| !value.trim().is_empty())
                .unwrap_or(default_label)
        }
        _ => return None,
    };

    Some(ManagedLoadChild { parent, key })
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
    managed_load_children: HashSet<ManagedLoadChild>,
    pending_manifest_apply: Option<Vec<ManagedLoadChild>>,
    reload_requested: bool,
    runtime_started_elapsed: Duration,
}

impl ScriptNode {
    /// Creates a new script node.
    pub fn new(label: impl Into<String>, config: ScriptNodeConfig) -> Self {
        Self {
            node_data: NodeData::new(label.into()),
            config,
            budgets: ScriptBudgets::default(),
            runtime: None,
            manifest: None,
            source_stamp: None,
            effective_update_rate_hz: None,
            runtime_subscriptions: Vec::new(),
            managed_load_children: HashSet::new(),
            pending_manifest_apply: None,
            reload_requested: false,
            runtime_started_elapsed: Duration::ZERO,
        }
    }

    /// Returns the last successfully parsed manifest.
    pub fn manifest(&self) -> Option<&ScriptManifest> {
        self.manifest.as_ref()
    }

    /// Returns currently detected script export names.
    pub fn export_names(&self) -> Vec<String> {
        self.runtime
            .as_ref()
            .map(|runtime| runtime.runtime.export_names())
            .unwrap_or_default()
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

    fn reconcile_load_declared_children(&mut self, ctx: &mut ProcessCtx, declared: &HashSet<ManagedLoadChild>) {
        let stale_entries = self
            .managed_load_children
            .difference(declared)
            .cloned()
            .collect::<Vec<_>>();
        if stale_entries.is_empty() {
            self.managed_load_children = declared.clone();
            return;
        }

        if let Some(snapshot) = ctx.tree_snapshot() {
            let mut stale_child_nodes = HashSet::new();
            for stale in &stale_entries {
                let mut child = snapshot.node(stale.parent).and_then(|node| node.first_child);
                while let Some(child_id) = child {
                    let Some(child_snapshot) = snapshot.node(child_id) else {
                        break;
                    };

                    if managed_child_key_matches(child_snapshot, stale.key.as_str()) {
                        stale_child_nodes.insert(child_id);
                    }

                    child = child_snapshot.next_sibling;
                }
            }

            for child_id in stale_child_nodes {
                ctx.edits.push(Edit::RemoveNode { node: child_id });
            }
        }

        self.managed_load_children = declared.clone();
    }

    fn teardown_runtime(&mut self, ctx: &mut ProcessCtx) {
        let script_node = self.id();
        let host_node = self.node_data.parent;
        if let Some(mut active) = self.runtime.take() {
            let mut host = NodeScriptHostBridge::new(
                script_node,
                host_node,
                self.runtime_started_elapsed,
                &mut self.runtime_subscriptions,
                None,
                ctx,
            );
            if let Err(error) = active.runtime.call_on_destroy(&mut host) {
                self.handle_runtime_error(ctx, &error);
            }
        }

        self.clear_runtime_subscriptions(ctx);
        self.runtime_started_elapsed = ctx.runtime_elapsed;
    }

    fn source_file_modified(&self) -> Option<SystemTime> {
        let path = self.config.source.resolve_path()?;
        std::fs::metadata(path)
            .ok()
            .and_then(|metadata| metadata.modified().ok())
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
            ScriptSource::Inline { text } => Ok(hash_source_text(text) != last_stamp.source_hash),
            ScriptSource::ProjectFile { .. } => {
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
        self.runtime_started_elapsed = ctx.runtime_elapsed;

        let mut runtime = create_runtime(self.budgets)?;
        let source_name = self.config.source.runtime_source_name();
        let script_node = self.id();
        let host_node = self.node_data.parent;
        let mut declared_load_children = Vec::new();
        let manifest = {
            let mut host = NodeScriptHostBridge::new(
                script_node,
                host_node,
                self.runtime_started_elapsed,
                &mut self.runtime_subscriptions,
                Some(&mut declared_load_children),
                ctx,
            );
            runtime.load(&script_source, &source_name, Some(&mut host))?
        };
        {
            let mut host = NodeScriptHostBridge::new(
                script_node,
                host_node,
                self.runtime_started_elapsed,
                &mut self.runtime_subscriptions,
                Some(&mut declared_load_children),
                ctx,
            );
            runtime.call_on_init(&mut host)?;
        }
        let declared_set: HashSet<_> = declared_load_children.iter().cloned().collect();
        self.reconcile_load_declared_children(ctx, &declared_set);

        self.pending_manifest_apply = Some(declared_load_children);
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
        ctx.set_node_warning_with(
            self.id(),
            Some("script"),
            format!("Script runtime error: {error}"),
            None,
        );
    }

    fn quarantine_runtime_after_error(&mut self) {
        self.runtime = None;
        self.source_stamp = None;
        self.reload_requested = true;
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ScriptProjectData {
    config: ScriptNodeConfig,
    #[serde(default)]
    budgets: ScriptBudgets,
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

    fn type_description(&self) -> Option<&str> {
        Some("Built-in QuickJS script node.")
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn project_encode_data(&self) -> Result<serde_json::Value, String> {
        serde_json::to_value(ScriptProjectData {
            config: self.config.clone(),
            budgets: self.budgets,
        })
        .map_err(|err| format!("failed to encode script node data: {err}"))
    }

    fn project_decode_data(&mut self, data: &serde_json::Value) -> Result<(), String> {
        let parsed = if data.is_null() {
            ScriptProjectData {
                config: ScriptNodeConfig::default(),
                budgets: ScriptBudgets::default(),
            }
        } else {
            serde_json::from_value::<ScriptProjectData>(data.clone())
                .map_err(|err| format!("invalid script payload: {err}"))?
        };

        self.config = parsed.config;
        self.budgets = parsed.budgets;
        self.runtime = None;
        self.manifest = None;
        self.source_stamp = None;
        self.effective_update_rate_hz = None;
        self.runtime_subscriptions.clear();
        self.managed_load_children.clear();
        self.pending_manifest_apply = None;
        self.reload_requested = false;
        self.runtime_started_elapsed = Duration::ZERO;
        Ok(())
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == "script").then(|| Self::new("Script", ScriptNodeConfig::default()))
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

    fn engine_validate_project_candidate(&self) -> Result<(), String> {
        if !self.node_data.meta.enabled || self.runtime.is_some() {
            return Ok(());
        }
        let detail = self
            .node_data
            .meta
            .presentation
            .warning(Some("script"))
            .map(|warning| warning.message.clone())
            .unwrap_or_else(|| "script runtime did not initialize".to_string());
        Err(format!("script '{}': {detail}", self.node_data.meta.label))
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

        if self.runtime.is_none()
            && let Err(error) = self.load_or_reload_internal(ctx, false)
        {
            self.handle_runtime_error(ctx, &error);
            return;
        }

        if let Some(declared) = self.pending_manifest_apply.take()
            && let Some(snapshot) = ctx.tree_snapshot_arc()
        {
            let mut prev_sibling = None;
            for declared_child in declared {
                let mut found_id = None;
                let mut child = snapshot.node(declared_child.parent).and_then(|node| node.first_child);
                while let Some(child_id) = child {
                    let Some(child_snapshot) = snapshot.node(child_id) else {
                        break;
                    };
                    if managed_child_key_matches(child_snapshot, declared_child.key.as_str()) {
                        found_id = Some(child_id);
                        break;
                    }
                    child = child_snapshot.next_sibling;
                }

                if let Some(child_id) = found_id {
                    ctx.edits.push(crate::edit::Edit::MoveNode {
                        node: child_id,
                        new_parent: declared_child.parent,
                        new_prev_sibling: prev_sibling,
                    });
                    prev_sibling = Some(child_id);

                    if let Some(manifest) = &self.manifest {
                        for spec in &manifest.parameters {
                            if spec.decl_id.0 == declared_child.key || spec.name == declared_child.key {
                                let mut meta_patch = crate::node::NodeMetaPatch::default();
                                let mut needs_patch = false;

                                if let Some(child_snapshot) = snapshot.node(child_id) {
                                    if let Some(label) = &spec.label
                                        && label != &child_snapshot.label
                                    {
                                        meta_patch.label = Some(label.clone());
                                        needs_patch = true;
                                    }

                                    if child_snapshot.param_constraints.as_ref() != Some(&spec.constraints) {
                                        ctx.edits.push(crate::edit::Edit::SetParamConstraints {
                                            node: child_id,
                                            constraints: spec.constraints.clone(),
                                        });
                                    }
                                }

                                if needs_patch {
                                    ctx.patch_node_meta(child_id, meta_patch);
                                }

                                let hints = spec.ui_hints.clone();
                                let read_only = spec.read_only;
                                let new_default = spec.default_value.clone();
                                ctx.edits.push(crate::edit::Edit::CallNodeMutation {
                                    node: child_id,
                                    // Leaf param-spec sync: the callback only mutates the
                                    // parameter and queues follow-up edits.
                                    needs_tree_snapshot: false,
                                    callback: Box::new(move |node_dyn, ctx| {
                                        if let Some(param) =
                                            node_dyn.as_any_mut().downcast_mut::<crate::parameter::Parameter>()
                                        {
                                            param.ui_hints = hints;
                                            param.read_only = read_only;

                                            if param.default_value != new_default {
                                                let is_at_default = param.value == param.default_value;
                                                param.default_value = new_default.clone();

                                                if is_at_default && param.value != new_default {
                                                    ctx.edits.push(crate::edit::Edit::SetParam {
                                                        node: child_id,
                                                        value: new_default,
                                                        behaviour: crate::parameter::ParameterEventBehaviour::Coalesce,
                                                    });
                                                }
                                            }
                                        }
                                        Ok(())
                                    }),
                                });
                            }
                        }
                    }
                }
            }
        }

        let script_node = self.id();
        let host_node = self.node_data.parent;
        let mut runtime_error = None;
        if let Some(runtime) = self.runtime.as_mut() {
            let mut host = NodeScriptHostBridge::new(
                script_node,
                host_node,
                self.runtime_started_elapsed,
                &mut self.runtime_subscriptions,
                None,
                ctx,
            );
            if let Err(error) = runtime.runtime.call_on_update(&mut host) {
                runtime_error = Some(error);
            }
        }
        if let Some(error) = runtime_error {
            self.quarantine_runtime_after_error();
            self.handle_runtime_error(ctx, &error);
        }
    }

    fn on_inbox(&mut self, ctx: &mut ProcessCtx) {
        if !self.node_data.meta.enabled {
            return;
        }
        let events = ctx.events.clone();
        let script_node = self.id();
        let host_node = self.node_data.parent;
        let mut runtime_error = None;

        let Some(runtime) = self.runtime.as_mut() else {
            return;
        };

        for event in &events {
            let script_event = ScriptEvent::from(event.as_ref());
            let mut host = NodeScriptHostBridge::new(
                script_node,
                host_node,
                self.runtime_started_elapsed,
                &mut self.runtime_subscriptions,
                None,
                ctx,
            );
            if let Err(error) = runtime.runtime.call_on_event(&script_event, &mut host) {
                runtime_error = Some(error);
                break;
            }
        }
        if let Some(error) = runtime_error {
            self.quarantine_runtime_after_error();
            self.handle_runtime_error(ctx, &error);
        }
    }

    fn destroy(&mut self, ctx: &mut ProcessCtx) {
        self.teardown_runtime(ctx);
        self.manifest = None;
        self.source_stamp = None;
        self.effective_update_rate_hz = None;
        self.reload_requested = false;
        self.runtime_started_elapsed = Duration::ZERO;
    }

    fn update_requires_tree_snapshot(&self) -> bool {
        true
    }

    fn execution_rule(&self) -> NodeExecutionRule {
        if !self.node_data.meta.enabled {
            return NodeExecutionRule::passive();
        }

        if self.reload_requested || self.runtime.is_none() {
            return NodeExecutionRule::periodic(SCRIPT_BOOTSTRAP_UPDATE_RATE_HZ)
                .with_compiled_kernel("golden.runtime.script");
        }

        let has_on_update = self
            .runtime
            .as_ref()
            .is_some_and(|active| active.runtime.has_on_update());
        if !has_on_update {
            if self.config.source.is_file_backed() {
                return NodeExecutionRule::periodic(SCRIPT_FILE_RELOAD_POLL_HZ)
                    .with_compiled_kernel("golden.runtime.script");
            }
            return NodeExecutionRule::passive();
        }

        match self.effective_update_rate_hz {
            Some(rate_hz) if rate_hz > 0 => {
                NodeExecutionRule::periodic(rate_hz).with_compiled_kernel("golden.runtime.script")
            }
            None => NodeExecutionRule::periodic(SCRIPT_BOOTSTRAP_UPDATE_RATE_HZ)
                .with_compiled_kernel("golden.runtime.script"),
            _ => NodeExecutionRule::passive(),
        }
    }
}

#[cfg(test)]
mod tests;
