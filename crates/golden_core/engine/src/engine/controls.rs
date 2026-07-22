use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use crate::contexts::{
    UserContextEntryKind, UserContextLookup, UserContextValueType, parse_multiplex_context_link_symbol,
    parse_multiplex_template_token,
};
use crate::edit::Edit;
use crate::events::EventKind;
use crate::logger::{self, LogLevel};
use crate::node::{
    CONTEXT_LINK_LANE_DEFERRED_TAG, DeclId, EventSubscription, Node, NodeId, NodeReference,
    PARAMETER_ANIMATION_CONTROL_NODE_TYPE, PARAMETER_CONTROL_REFERENCE_DECL_ID, PARAMETER_EXPRESSION_SOURCE_DECL_ID,
};
use crate::parameter::{
    ParamValue, ParamValueProjection, Parameter, ParameterAnimationControlNode, ParameterChangeCheck,
    ParameterControlDiagnostic, ParameterControlMode, ParameterControlSpec, ParameterControlState,
    ParameterEventBehaviour, ParameterSnapshot, ReferenceTargetKind, available_control_modes_for_parameter,
    coerce_param_value_for_target, coerce_param_value_for_target_reverse,
};
use crate::process_ctx::ProcessTreeSnapshot;
use crate::script::{QuickJsRuntime, ScriptBudgets, ScriptHostBridge, ScriptLogLevel, ScriptRuntime, ScriptValue};
use serde_json::{Map as JsonMap, Value as JsonValue};

use super::history::SetParamControlStateEffect;
use super::{Engine, EngineEditError, ExpressionControlRuntime};

#[derive(Clone, Debug, PartialEq)]
enum TemplateSegment {
    Literal(String),
    Token(String),
}

struct BindingEvaluation {
    pending_writes: Vec<(NodeId, ParamValue)>,
    diagnostics_by_param: HashMap<NodeId, Vec<ParameterControlDiagnostic>>,
}

#[derive(Clone, Copy, Debug)]
struct BindingTargetConfig {
    target: NodeId,
    projection: Option<ParamValueProjection>,
}

#[derive(Clone, Debug)]
struct ReferenceControlConfig {
    target: NodeReference,
    projection: Option<ParamValueProjection>,
}

#[derive(Clone, Debug)]
struct ExpressionControlConfig {
    source_param: NodeId,
    expression: String,
}

const EXPRESSION_EXPORT_NAME: &str = "__gc_eval_expression";
const EXPRESSION_RUNTIME_SOURCE_NAME: &str = "expression_control.js";
const CONTROL_DIAGNOSTIC_WARNING_ID_PREFIX: &str = "control-diagnostic:";
const EXPRESSION_ERROR_WRAPPER_PREFIXES: [&str; 6] = [
    "failed to compile expression:",
    "failed to create quickjs runtime:",
    "expression evaluation failed:",
    "quickjs runtime error:",
    "script load:",
    "export callback:",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ExpressionErrorStage {
    Setup,
    Evaluation,
}

impl ExpressionErrorStage {
    fn label(self) -> &'static str {
        match self {
            Self::Setup => "setup",
            Self::Evaluation => "evaluation",
        }
    }

    fn fallback_summary(self) -> &'static str {
        match self {
            Self::Setup => "setup failed",
            Self::Evaluation => "evaluation failed",
        }
    }
}

fn control_mode_id(mode: ParameterControlMode) -> &'static str {
    match mode {
        ParameterControlMode::Manual => "manual",
        ParameterControlMode::ContextLink => "context-link",
        ParameterControlMode::TemplateText => "template-text",
        ParameterControlMode::Expression => "expression",
        ParameterControlMode::Proxy => "proxy",
        ParameterControlMode::Binding => "binding",
        ParameterControlMode::Animation => "animation",
    }
}

fn control_mode_label(mode: ParameterControlMode) -> &'static str {
    match mode {
        ParameterControlMode::Manual => "Manual",
        ParameterControlMode::ContextLink => "Context Link",
        ParameterControlMode::TemplateText => "Template",
        ParameterControlMode::Expression => "Expression",
        ParameterControlMode::Proxy => "Proxy",
        ParameterControlMode::Binding => "Binding",
        ParameterControlMode::Animation => "Animation",
    }
}

fn parameter_control_is_active(state: &ParameterControlState) -> bool {
    state.mode != ParameterControlMode::Manual || !state.diagnostics.is_empty()
}

fn reference_target_is_unset(reference: &NodeReference) -> bool {
    reference.uuid().is_nil() && reference.relative_path_from_root().is_empty()
}

fn strip_prefix_case_insensitive<'a>(value: &'a str, prefix: &str) -> Option<&'a str> {
    let candidate = value.get(..prefix.len())?;
    candidate
        .eq_ignore_ascii_case(prefix)
        .then(|| value.get(prefix.len()..))
        .flatten()
}

fn compact_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn peel_expression_error_wrappers(value: &str) -> String {
    let mut current = value.trim().to_string();

    loop {
        let trimmed = current.trim_start();
        let mut next = None;

        for prefix in EXPRESSION_ERROR_WRAPPER_PREFIXES {
            if let Some(rest) = strip_prefix_case_insensitive(trimmed, prefix) {
                next = Some(rest.trim_start().to_string());
                break;
            }
        }

        if let Some(next_value) = next {
            current = next_value;
        } else {
            return current;
        }
    }
}

fn looks_like_location(value: &str) -> bool {
    let candidate = value.trim();
    if candidate.is_empty() || candidate.contains('(') || candidate.contains(')') {
        return false;
    }

    let mut segments = candidate.rsplit(':');
    let Some(column) = segments.next() else {
        return false;
    };
    let Some(line) = segments.next() else {
        return false;
    };
    !column.is_empty()
        && !line.is_empty()
        && column.chars().all(|ch| ch.is_ascii_digit())
        && line.chars().all(|ch| ch.is_ascii_digit())
}

fn split_expression_error_payload(value: &str) -> (String, Option<String>, Vec<String>) {
    let compact = compact_whitespace(value);
    if compact.is_empty() {
        return (String::new(), None, Vec::new());
    }

    let Some((head, tail)) = compact.split_once(" at ") else {
        return (compact, None, Vec::new());
    };

    let mut stack = tail
        .split(" at ")
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .map(|segment| format!("at {segment}"))
        .collect::<Vec<_>>();

    let mut location = None;
    if let Some(first) = stack.first() {
        let candidate = first.trim_start_matches("at ").trim();
        if looks_like_location(candidate) {
            location = Some(candidate.to_string());
            stack.remove(0);
        }
    }

    (head.trim().to_string(), location, stack)
}

fn format_expression_error_detail(
    stage: ExpressionErrorStage,
    summary: &str,
    location: Option<&str>,
    stack: &[String],
    raw_detail: &str,
) -> String {
    let mut lines = vec![format!("error: {summary}"), format!("stage: {}", stage.label())];
    if let Some(location) = location {
        lines.push(format!("location: {location}"));
    }
    if !stack.is_empty() {
        lines.push("stack:".to_string());
        lines.extend(stack.iter().map(|entry| format!("  {entry}")));
    }

    let raw_lines = raw_detail
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    if !raw_lines.is_empty() {
        lines.push("raw:".to_string());
        lines.extend(raw_lines.into_iter().map(|line| format!("  {line}")));
    }

    lines.join("\n")
}

fn expression_error_diagnostic(stage: ExpressionErrorStage, detail: String) -> ParameterControlDiagnostic {
    let raw_detail = detail.trim();
    let peeled = peel_expression_error_wrappers(raw_detail);
    let (parsed_summary, location, stack) = split_expression_error_payload(peeled.as_str());
    let summary = if parsed_summary.is_empty() {
        stage.fallback_summary().to_string()
    } else {
        parsed_summary
    };

    let diagnostic = ParameterControlDiagnostic::new("expression_error", format!("Expression error: {summary}"));
    let formatted_detail = format_expression_error_detail(
        stage,
        summary.as_str(),
        location.as_deref(),
        stack.as_slice(),
        raw_detail,
    );
    if formatted_detail.trim().is_empty() {
        diagnostic
    } else {
        diagnostic.with_detail(formatted_detail)
    }
}

struct ExpressionScriptHostBridge {
    consumer: NodeId,
    local_node: Option<NodeId>,
    tree_snapshot: Arc<ProcessTreeSnapshot>,
    time_seconds: f64,
    delta_seconds: f64,
}

impl ExpressionScriptHostBridge {
    fn new(
        consumer: NodeId,
        local_node: Option<NodeId>,
        tree_snapshot: Arc<ProcessTreeSnapshot>,
        time_seconds: f64,
        delta_seconds: f64,
    ) -> Self {
        Self {
            consumer,
            local_node,
            tree_snapshot,
            time_seconds,
            delta_seconds,
        }
    }
}

impl ScriptHostBridge for ExpressionScriptHostBridge {
    fn owner_node(&self) -> Option<NodeId> {
        self.local_node
    }

    fn script_node(&self) -> Option<NodeId> {
        Some(self.consumer)
    }

    fn time_seconds(&self) -> f64 {
        self.time_seconds
    }

    fn delta_seconds(&self) -> f64 {
        self.delta_seconds
    }

    fn log(&mut self, level: ScriptLogLevel, message: &str) {
        let level = match level {
            ScriptLogLevel::Info => LogLevel::Info,
            ScriptLogLevel::Success => LogLevel::Success,
            ScriptLogLevel::Warning => LogLevel::Warning,
            ScriptLogLevel::Error => LogLevel::Error,
        };
        let _ = logger::log_message(
            level,
            "expression".to_string(),
            Some(self.consumer),
            message.to_string(),
        );
    }

    fn emit_custom(&mut self, _topic: &str, _payload: JsonValue) -> Result<(), String> {
        Err("expression mode cannot emit custom events".to_string())
    }

    fn tree_snapshot(&self) -> Option<Arc<ProcessTreeSnapshot>> {
        Some(Arc::clone(&self.tree_snapshot))
    }

    fn set_node_script_property(&mut self, _node: NodeId, _property: String, _value: ParamValue) -> Result<(), String> {
        Err("expression mode cannot mutate node script properties".to_string())
    }

    fn call_node_script_method(
        &mut self,
        _node: NodeId,
        _method: String,
        _args: Vec<ParamValue>,
    ) -> Result<(), String> {
        Err("expression mode cannot call node script methods".to_string())
    }

    fn set_event_listener(&mut self, _target: NodeId, _level: u32) -> Result<(), String> {
        Err("expression mode cannot register script listeners".to_string())
    }

    fn remove_event_listener(&mut self, _target: NodeId) -> Result<(), String> {
        Err("expression mode cannot remove script listeners".to_string())
    }

    fn clear_event_listeners(&mut self) -> Result<(), String> {
        Err("expression mode cannot clear script listeners".to_string())
    }
}

#[derive(Default)]
struct ControlChangeSet {
    changed_params: HashSet<NodeId>,
    changed_controls: HashSet<NodeId>,
    structural: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct BindingWriteScore {
    source_counter: u64,
    source: NodeId,
}

impl<T: Node> Engine<T> {
    pub(crate) fn text_param_input_uses_template_mode(&self, value: &str) -> bool {
        text_has_template_token(value)
    }

    pub(crate) fn mark_param_control_index_dirty(&mut self) {
        self.control_index_dirty = true;
    }

    fn control_param_snapshot(&self, param: NodeId) -> Option<ParameterSnapshot> {
        self.nodes.get(param).and_then(|node| node.engine_param_snapshot())
    }

    fn rebuild_param_control_index_if_dirty(&mut self) -> bool {
        if !self.control_index_dirty {
            return false;
        }

        self.active_control_params.clear();
        self.active_control_param_set.clear();
        self.control_source_dependents.clear();

        for (node_id, node) in self.nodes.iter() {
            let Some(snapshot) = node.engine_param_snapshot() else {
                continue;
            };
            if parameter_control_is_active(&snapshot.control) {
                self.active_control_params.push(node_id);
                self.active_control_param_set.insert(node_id);
            }
        }
        self.active_control_params.sort_by_key(|node| node.0);

        let active_params = self.active_control_params.clone();
        for param in active_params {
            let Some(snapshot) = self.control_param_snapshot(param) else {
                continue;
            };
            let mut sources = HashSet::<NodeId>::new();
            self.collect_control_sources(param, &snapshot, &mut sources);
            for source in sources {
                self.record_control_source_dependency(source, param);
            }
        }

        for dependents in self.control_source_dependents.values_mut() {
            dependents.sort_by_key(|node| node.0);
            dependents.dedup();
        }

        self.control_index_dirty = false;
        true
    }

    fn record_control_source_dependency(&mut self, source: NodeId, dependent: NodeId) {
        self.control_source_dependents
            .entry(source)
            .or_default()
            .push(dependent);
    }

    fn collect_control_sources(&mut self, param: NodeId, snapshot: &ParameterSnapshot, sources: &mut HashSet<NodeId>) {
        match snapshot.control.mode {
            ParameterControlMode::ContextLink => {
                if let ParameterControlSpec::ContextLink { symbol, .. } = &snapshot.control.spec {
                    self.insert_context_symbol_source(param, symbol, sources);
                }
            }
            ParameterControlMode::TemplateText => {
                if let ParameterControlSpec::TemplateText { template } = &snapshot.control.spec {
                    for segment in parse_template_segments(template) {
                        let TemplateSegment::Token(token) = segment else {
                            continue;
                        };
                        self.insert_template_token_source(param, token.as_str(), sources);
                    }
                }
            }
            ParameterControlMode::Expression => {
                let mut diagnostics = Vec::new();
                if let Some(config) = self.read_expression_control_config(param, &mut diagnostics) {
                    sources.insert(config.source_param);
                    if let Some(runtime) = self.expression_runtime.get(&param) {
                        sources.extend(runtime.subscriptions.iter().copied());
                    } else {
                        self.insert_expression_context_sources(param, config.expression.as_str(), sources);
                    }
                }
            }
            ParameterControlMode::Proxy | ParameterControlMode::Binding => {
                let mode_code = match snapshot.control.mode {
                    ParameterControlMode::Proxy => "proxy",
                    ParameterControlMode::Binding => "binding",
                    _ => unreachable!(),
                };
                if let Some(reference_param) =
                    self.find_direct_child_by_decl_id(param, PARAMETER_CONTROL_REFERENCE_DECL_ID)
                {
                    sources.insert(reference_param);
                }
                let mut diagnostics = Vec::new();
                if let Some(config) = self.read_reference_control_config(param, mode_code, &mut diagnostics)
                    && !reference_target_is_unset(&config.target)
                    && let Some(target_param) = self.resolve_reference_target_node(&config.target)
                    && self.control_param_snapshot(target_param).is_some()
                {
                    sources.insert(target_param);
                }
            }
            ParameterControlMode::Manual | ParameterControlMode::Animation => {}
        }
    }

    fn insert_context_symbol_source(&self, consumer: NodeId, symbol: &str, sources: &mut HashSet<NodeId>) {
        let symbol = symbol.trim();
        if symbol.is_empty() {
            return;
        }
        let candidates = self.user_contexts.collect_candidates(consumer, None, |node| {
            self.nodes.get(node).and_then(|entry| entry.node_data().parent)
        });
        for candidate in candidates {
            if candidate.shadowed || candidate.symbol != symbol {
                continue;
            }
            sources.insert(candidate.entry_param);
            break;
        }
    }

    fn insert_template_token_source(&self, consumer: NodeId, token: &str, sources: &mut HashSet<NodeId>) {
        let token = token.trim();
        if token.is_empty() || token.starts_with('$') {
            return;
        }
        let symbol = token.split_once(".$").map(|(symbol, _)| symbol).unwrap_or(token);
        self.insert_context_symbol_source(consumer, symbol, sources);
    }

    fn insert_expression_context_sources(&self, consumer: NodeId, expression: &str, sources: &mut HashSet<NodeId>) {
        let mut symbols = HashSet::<String>::new();
        let candidates = self.user_contexts.collect_candidates(consumer, None, |node| {
            self.nodes.get(node).and_then(|entry| entry.node_data().parent)
        });
        for candidate in candidates {
            if candidate.shadowed || !symbols.insert(candidate.symbol.clone()) {
                continue;
            }
            if expression_mentions_symbol(expression, candidate.symbol.as_str()) {
                sources.insert(candidate.entry_param);
            }
        }
    }

    fn prune_expression_runtimes_to_active_controls(&mut self) {
        let stale_consumers = self
            .expression_runtime
            .keys()
            .copied()
            .filter(|consumer| !self.active_control_param_set.contains(consumer))
            .collect::<Vec<_>>();
        for consumer in stale_consumers {
            self.clear_expression_runtime(consumer);
        }
    }

    fn select_control_evaluation_candidates(&self, change_set: &ControlChangeSet, full_pass: bool) -> Vec<NodeId> {
        if full_pass {
            return self.active_control_params.clone();
        }

        let mut candidates = Vec::<NodeId>::new();
        for param in &change_set.changed_controls {
            if self.active_control_param_set.contains(param) {
                candidates.push(*param);
            }
        }
        for param in &change_set.changed_params {
            if self.active_control_param_set.contains(param) {
                candidates.push(*param);
            }
            if let Some(dependents) = self.control_source_dependents.get(param) {
                candidates.extend(dependents.iter().copied());
            }
        }
        for (consumer, runtime) in &self.expression_runtime {
            if runtime.continuous
                && self.active_control_param_set.contains(consumer)
                && self.runtime_elapsed > runtime.last_eval_elapsed
            {
                candidates.push(*consumer);
            }
        }

        candidates.sort_by_key(|node| node.0);
        candidates.dedup();
        candidates
    }

    fn collect_control_param_snapshots(&mut self, candidates: &[NodeId]) -> HashMap<NodeId, ParameterSnapshot> {
        let mut wanted = HashSet::<NodeId>::new();
        for candidate in candidates {
            wanted.insert(*candidate);
            let Some(snapshot) = self.control_param_snapshot(*candidate) else {
                continue;
            };
            self.collect_control_sources(*candidate, &snapshot, &mut wanted);
        }

        let mut params = wanted.into_iter().collect::<Vec<_>>();
        params.sort_by_key(|node| node.0);
        params
            .into_iter()
            .filter_map(|param| self.control_param_snapshot(param).map(|snapshot| (param, snapshot)))
            .collect()
    }

    fn control_dependency_sources_changed(&self, change_set: &ControlChangeSet) -> bool {
        change_set.changed_params.iter().any(|param| {
            self.control_source_dependents.contains_key(param)
                && self.nodes.get(*param).is_some_and(|node| {
                    let decl_id = node.node_data().meta.decl_id.0.as_str();
                    decl_id == PARAMETER_CONTROL_REFERENCE_DECL_ID || decl_id == PARAMETER_EXPRESSION_SOURCE_DECL_ID
                })
        })
    }

    /// Evaluates all parameter-control states and queues resulting parameter writes.
    ///
    /// Returns `true` when it changed queued edits or updated control diagnostics.
    pub(crate) fn evaluate_parameter_controls(&mut self) -> bool {
        let change_set = self.collect_control_change_set();
        if change_set.structural || !change_set.changed_controls.is_empty() {
            self.mark_param_control_index_dirty();
        }
        let index_rebuilt = self.rebuild_param_control_index_if_dirty();
        if self.active_control_params.is_empty() {
            if !self.expression_runtime.is_empty() {
                let consumers = self.expression_runtime.keys().copied().collect::<Vec<_>>();
                for consumer in consumers {
                    self.clear_expression_runtime(consumer);
                }
            }
            return false;
        }
        self.prune_expression_runtimes_to_active_controls();

        let candidates = self.select_control_evaluation_candidates(
            &change_set,
            index_rebuilt || change_set.structural || !change_set.changed_controls.is_empty(),
        );
        if candidates.is_empty() {
            return false;
        }
        let dependency_sources_changed = self.control_dependency_sources_changed(&change_set);
        self.tick_scratch.stats.controls_params_scanned += candidates.len();

        let param_snapshots = self.collect_control_param_snapshots(candidates.as_slice());
        if param_snapshots.is_empty() {
            if !self.expression_runtime.is_empty() {
                let consumers = self.expression_runtime.keys().copied().collect::<Vec<_>>();
                for consumer in consumers {
                    self.clear_expression_runtime(consumer);
                }
            }
            return false;
        }

        let expression_tree_snapshot = candidates
            .iter()
            .any(|param| {
                param_snapshots.get(param).is_some_and(|snapshot| {
                    matches!(
                        (&snapshot.control.mode, &snapshot.control.spec),
                        (ParameterControlMode::Expression, ParameterControlSpec::Expression)
                    )
                })
            })
            .then(|| self.build_process_tree_snapshot());

        let mut diagnostics_by_param = HashMap::<NodeId, Vec<ParameterControlDiagnostic>>::new();
        let mut binding_targets = HashMap::<NodeId, BindingTargetConfig>::new();
        let mut queued_writes = Vec::<(NodeId, ParamValue)>::new();

        for param in &candidates {
            let Some(snapshot) = param_snapshots.get(param) else {
                continue;
            };
            let control = &snapshot.control;
            let mut diagnostics = Vec::<ParameterControlDiagnostic>::new();

            let maybe_value = match control.mode {
                ParameterControlMode::Manual => None,
                ParameterControlMode::ContextLink => match &control.spec {
                    ParameterControlSpec::ContextLink { symbol, projection } => self.evaluate_context_link(
                        *param,
                        snapshot,
                        symbol.as_str(),
                        *projection,
                        &param_snapshots,
                        &mut diagnostics,
                    ),
                    _ => {
                        diagnostics.push(ParameterControlDiagnostic::new(
                            "invalid_control_spec",
                            "control mode/context-link mismatch",
                        ));
                        None
                    }
                },
                ParameterControlMode::TemplateText => match &control.spec {
                    ParameterControlSpec::TemplateText { template } => self.evaluate_template_text(
                        *param,
                        snapshot,
                        template.as_str(),
                        &param_snapshots,
                        &mut diagnostics,
                    ),
                    _ => {
                        diagnostics.push(ParameterControlDiagnostic::new(
                            "invalid_control_spec",
                            "control mode/template-text mismatch",
                        ));
                        None
                    }
                },
                ParameterControlMode::Expression => match &control.spec {
                    ParameterControlSpec::Expression => self.evaluate_expression(
                        *param,
                        snapshot,
                        &param_snapshots,
                        expression_tree_snapshot.as_ref(),
                        &change_set,
                        &mut diagnostics,
                    ),
                    _ => {
                        diagnostics.push(ParameterControlDiagnostic::new(
                            "invalid_control_spec",
                            "control mode/expression mismatch",
                        ));
                        None
                    }
                },
                ParameterControlMode::Proxy => match &control.spec {
                    ParameterControlSpec::Proxy => {
                        if let Some(config) = self.read_reference_control_config(*param, "proxy", &mut diagnostics) {
                            if reference_target_is_unset(&config.target) {
                                None
                            } else if let Some(target_param) =
                                self.resolve_control_target_param(&config.target, &param_snapshots)
                            {
                                self.evaluate_proxy_one_way(
                                    *param,
                                    snapshot,
                                    target_param,
                                    config.projection,
                                    &param_snapshots,
                                    &mut diagnostics,
                                )
                            } else {
                                diagnostics.push(ParameterControlDiagnostic::new(
                                    "proxy_target_missing",
                                    "proxy target parameter could not be resolved",
                                ));
                                None
                            }
                        } else {
                            None
                        }
                    }
                    _ => {
                        diagnostics.push(ParameterControlDiagnostic::new(
                            "invalid_control_spec",
                            "control mode/proxy mismatch",
                        ));
                        None
                    }
                },
                ParameterControlMode::Binding => match &control.spec {
                    ParameterControlSpec::Binding => {
                        if let Some(config) = self.read_reference_control_config(*param, "binding", &mut diagnostics) {
                            if reference_target_is_unset(&config.target) {
                                None
                            } else if let Some(target_param) =
                                self.resolve_control_target_param(&config.target, &param_snapshots)
                            {
                                binding_targets.insert(
                                    *param,
                                    BindingTargetConfig {
                                        target: target_param,
                                        projection: config.projection,
                                    },
                                );
                                None
                            } else {
                                diagnostics.push(ParameterControlDiagnostic::new(
                                    "binding_target_missing",
                                    "binding target parameter could not be resolved",
                                ));
                                None
                            }
                        } else {
                            None
                        }
                    }
                    _ => {
                        diagnostics.push(ParameterControlDiagnostic::new(
                            "invalid_control_spec",
                            "control mode/binding mismatch",
                        ));
                        None
                    }
                },
                ParameterControlMode::Animation => match &control.spec {
                    ParameterControlSpec::Animation => None,
                    _ => {
                        diagnostics.push(ParameterControlDiagnostic::new(
                            "invalid_control_spec",
                            "control mode/animation mismatch",
                        ));
                        None
                    }
                },
            };

            if let Some(value) = maybe_value
                && snapshot.value != value
            {
                queued_writes.push((*param, value));
            }

            diagnostics_by_param.insert(*param, diagnostics);
        }

        let binding_result = self.evaluate_bindings(&binding_targets, &param_snapshots);
        for (node, value) in binding_result.pending_writes {
            if param_snapshots
                .get(&node)
                .is_some_and(|snapshot| snapshot.value != value)
            {
                queued_writes.push((node, value));
            }
        }
        for (param, diagnostics) in binding_result.diagnostics_by_param {
            diagnostics_by_param.insert(param, diagnostics);
        }

        let mut queued_any = false;
        for (node, value) in queued_writes {
            self.edits.push(Edit::SetParam {
                node,
                value,
                behaviour: ParameterEventBehaviour::Coalesce,
            });
            queued_any = true;
        }

        let diagnostics_changed = self.apply_parameter_control_diagnostics(&param_snapshots, diagnostics_by_param);
        if diagnostics_changed || dependency_sources_changed || change_set.structural {
            self.mark_param_control_index_dirty();
        }
        let warning_sync_changed = self.sync_parameter_control_warnings(&param_snapshots);
        queued_any || diagnostics_changed || warning_sync_changed
    }

    fn set_param_control_state_impl(
        &mut self,
        param: NodeId,
        mut state: ParameterControlState,
    ) -> Result<bool, String> {
        if !control_spec_matches_mode(state.mode, &state.spec) {
            return Err("parameter control state has a mode/spec mismatch".to_string());
        }

        let Some(node) = self.nodes.get(param) else {
            return Err(format!("parameter node {} was not found", param.0));
        };
        let Some(snapshot) = node.engine_param_snapshot() else {
            return Err(format!("node {} is not a parameter node", param.0));
        };
        if !available_control_modes_for_parameter(&snapshot.value, snapshot.control_modes_enabled).contains(&state.mode)
        {
            return Err(format!(
                "control mode '{:?}' is not supported for parameter type '{}'",
                state.mode,
                node.get_type()
            ));
        }
        if matches!(state.mode, ParameterControlMode::TemplateText)
            && self.ui_context_candidates_for_param(param).candidates.is_empty()
        {
            return Err("control mode 'templateText' requires at least one visible context entry".to_string());
        }

        let Some(current_state) = node.engine_param_control_state() else {
            return Err(format!("node {} does not expose parameter control state", param.0));
        };
        state.diagnostics.clear();
        self.sync_parameter_control_nodes(param, state.mode)?;
        if current_state == state {
            return Ok(false);
        }

        let next_state = state.clone();
        let Some(node) = self.nodes.get_mut(param) else {
            return Err(format!("parameter node {} was not found", param.0));
        };
        node.engine_set_param_control_state(state)?;
        self.mark_param_control_index_dirty();
        self.emit_event(EventKind::ParamControlChanged {
            param,
            old_state: current_state,
            new_state: next_state,
        });
        Ok(true)
    }

    /// Sets control state on one parameter node.
    ///
    /// Returns `true` when the state changed.
    pub fn set_param_control_state(&mut self, param: NodeId, state: ParameterControlState) -> Result<bool, String> {
        self.set_param_control_state_impl(param, state)
    }

    fn param_control_rejected_error(
        &self,
        edit_index: usize,
        operation: &'static str,
        node: NodeId,
        message: String,
    ) -> EngineEditError {
        let node_type = self
            .nodes
            .get(node)
            .map(|entry| entry.get_type().to_string())
            .unwrap_or_else(|| "unknown".to_string());
        EngineEditError::ParamControlStateRejected {
            edit_index,
            operation,
            node,
            node_type,
            message,
        }
    }

    fn read_param_control_state_for_edit(
        &self,
        edit_index: usize,
        operation: &'static str,
        node: NodeId,
    ) -> Result<ParameterControlState, EngineEditError> {
        let Some(target) = self.nodes.get(node) else {
            return Err(EngineEditError::NodeNotFound {
                edit_index,
                operation,
                node,
            });
        };
        target.engine_param_control_state().ok_or_else(|| {
            self.param_control_rejected_error(
                edit_index,
                operation,
                node,
                "target node does not expose parameter control state".to_string(),
            )
        })
    }

    fn apply_set_param_control_state_internal(
        &mut self,
        edit_index: usize,
        operation: &'static str,
        node: NodeId,
        state: ParameterControlState,
    ) -> Result<Option<SetParamControlStateEffect>, EngineEditError> {
        let old_state = self.read_param_control_state_for_edit(edit_index, operation, node)?;
        let changed = self
            .set_param_control_state_impl(node, state)
            .map_err(|message| self.param_control_rejected_error(edit_index, operation, node, message))?;
        if !changed {
            return Ok(None);
        }

        let new_state = self.read_param_control_state_for_edit(edit_index, operation, node)?;
        let _ = self.evaluate_parameter_controls();
        if !self.edits.pending.is_empty() {
            self.apply_edits_without_history()?;
        }

        Ok(Some(SetParamControlStateEffect {
            node,
            old_state,
            new_state,
        }))
    }

    pub(crate) fn apply_set_param_control_state(
        &mut self,
        edit_index: usize,
        node: NodeId,
        state: ParameterControlState,
    ) -> Result<Option<SetParamControlStateEffect>, EngineEditError> {
        self.apply_set_param_control_state_internal(edit_index, "SetParamControlState", node, state)
    }

    pub(crate) fn apply_set_param_control_state_for_history(
        &mut self,
        operation: &'static str,
        node: NodeId,
        state: ParameterControlState,
    ) -> Result<(), EngineEditError> {
        let _ = self.apply_set_param_control_state_internal(0, operation, node, state)?;
        Ok(())
    }

    fn make_expression_source_parameter(initial_expression: String) -> Parameter {
        let mut source = Parameter::new(
            "Expression",
            ParamValue::Str(initial_expression),
            ParameterChangeCheck::ValueChange,
        );
        source.node_data_mut().meta.decl_id = DeclId(PARAMETER_EXPRESSION_SOURCE_DECL_ID.to_string());
        source.node_data_mut().meta.can_be_disabled = false;
        source.control_modes_enabled = false;
        source
    }

    fn make_control_reference_parameter() -> Parameter {
        let mut reference = Parameter::new(
            "Reference",
            ParamValue::Reference(NodeReference::default()),
            ParameterChangeCheck::ValueChange,
        );
        reference.node_data_mut().meta.decl_id = DeclId(PARAMETER_CONTROL_REFERENCE_DECL_ID.to_string());
        reference.node_data_mut().meta.can_be_disabled = false;
        reference.control_modes_enabled = false;
        reference.constraints.reference.target_kind = ReferenceTargetKind::ParameterOnly;
        reference
    }

    fn sync_parameter_control_nodes(&mut self, param: NodeId, mode: ParameterControlMode) -> Result<(), String> {
        let children = self.direct_children(param);
        let mut control_reference_params = Vec::<NodeId>::new();
        let mut animation_nodes = Vec::<NodeId>::new();
        let mut expression_source_params = Vec::<NodeId>::new();

        for child in children {
            let Some(child_node) = self.nodes.get(child) else {
                continue;
            };

            if child_node.get_type() == PARAMETER_ANIMATION_CONTROL_NODE_TYPE {
                animation_nodes.push(child);
                continue;
            }
            if child_node.node_data().meta.decl_id.0 == PARAMETER_EXPRESSION_SOURCE_DECL_ID
                && child_node.engine_param_snapshot().is_some()
            {
                expression_source_params.push(child);
                continue;
            }
            if child_node.node_data().meta.decl_id.0 == PARAMETER_CONTROL_REFERENCE_DECL_ID
                && child_node.engine_param_snapshot().is_some()
            {
                control_reference_params.push(child);
            }
        }

        let mut changed = false;
        let mut to_remove = Vec::<NodeId>::new();

        match mode {
            ParameterControlMode::Proxy | ParameterControlMode::Binding => {
                if control_reference_params.is_empty() {
                    self.edits.push(Edit::AddNode {
                        parent: param,
                        node: Box::new(Self::make_control_reference_parameter()),
                        prev_sibling: None,
                    });
                    changed = true;
                } else {
                    to_remove.extend(control_reference_params.into_iter().skip(1));
                }
                to_remove.extend(expression_source_params);
                to_remove.extend(animation_nodes);
            }
            ParameterControlMode::Expression => {
                if expression_source_params.is_empty() {
                    self.edits.push(Edit::AddNode {
                        parent: param,
                        node: Box::new(Self::make_expression_source_parameter(String::new())),
                        prev_sibling: None,
                    });
                    changed = true;
                } else {
                    to_remove.extend(expression_source_params.into_iter().skip(1));
                }
                to_remove.extend(control_reference_params);
                to_remove.extend(animation_nodes);
            }
            ParameterControlMode::Animation => {
                if animation_nodes.is_empty() {
                    self.edits.push(Edit::AddNode {
                        parent: param,
                        node: Box::new(ParameterAnimationControlNode::new("Animation")),
                        prev_sibling: None,
                    });
                    changed = true;
                } else {
                    to_remove.extend(animation_nodes.into_iter().skip(1));
                }
                to_remove.extend(control_reference_params);
                to_remove.extend(expression_source_params);
            }
            _ => {
                to_remove.extend(control_reference_params);
                to_remove.extend(animation_nodes);
                to_remove.extend(expression_source_params);
            }
        }

        for node in to_remove {
            self.edits.push(Edit::RemoveNode { node });
            changed = true;
        }

        if changed {
            self.apply_edits_without_history()
                .map_err(|err| format!("failed to sync control nodes for parameter {}: {err}", param.0))?;
        }

        Ok(())
    }

    fn direct_children(&self, parent: NodeId) -> Vec<NodeId> {
        let mut out = Vec::<NodeId>::new();
        let mut child = self.nodes.get(parent).and_then(|node| node.node_data().first_child);
        while let Some(child_id) = child {
            out.push(child_id);
            child = self.nodes.get(child_id).and_then(|node| node.node_data().next_sibling);
        }
        out
    }

    fn find_direct_child_by_decl_id(&self, parent: NodeId, decl_id: &str) -> Option<NodeId> {
        self.direct_children(parent).into_iter().find(|child| {
            self.nodes
                .get(*child)
                .is_some_and(|node| node.node_data().meta.decl_id.0 == decl_id)
        })
    }

    fn read_parameter_value(&self, param: NodeId) -> Option<ParamValue> {
        self.nodes
            .get(param)
            .and_then(|node| node.engine_param_snapshot())
            .map(|snapshot| snapshot.value)
    }

    fn read_reference_control_config(
        &self,
        param: NodeId,
        mode_code: &str,
        diagnostics: &mut Vec<ParameterControlDiagnostic>,
    ) -> Option<ReferenceControlConfig> {
        let Some(reference_param) = self.find_direct_child_by_decl_id(param, PARAMETER_CONTROL_REFERENCE_DECL_ID)
        else {
            diagnostics.push(ParameterControlDiagnostic::new(
                format!("{mode_code}_reference_missing"),
                format!("{mode_code} reference parameter is missing"),
            ));
            return None;
        };

        let Some(reference_value) = self.read_parameter_value(reference_param) else {
            diagnostics.push(ParameterControlDiagnostic::new(
                format!("{mode_code}_reference_invalid"),
                format!("{mode_code} reference node is not a parameter"),
            ));
            return None;
        };

        let ParamValue::Reference(target) = reference_value else {
            diagnostics.push(ParameterControlDiagnostic::new(
                format!("{mode_code}_reference_type_mismatch"),
                format!("{mode_code} reference parameter must be a reference parameter"),
            ));
            return None;
        };

        Some(ReferenceControlConfig {
            projection: target.projection(),
            target,
        })
    }

    fn read_expression_control_config(
        &self,
        param: NodeId,
        diagnostics: &mut Vec<ParameterControlDiagnostic>,
    ) -> Option<ExpressionControlConfig> {
        let Some(source_param) = self.find_direct_child_by_decl_id(param, PARAMETER_EXPRESSION_SOURCE_DECL_ID) else {
            diagnostics.push(ParameterControlDiagnostic::new(
                "expression_source_missing",
                "expression source parameter is missing",
            ));
            return None;
        };

        let Some(source_value) = self.read_parameter_value(source_param) else {
            diagnostics.push(ParameterControlDiagnostic::new(
                "expression_source_invalid",
                "expression source node is not a parameter",
            ));
            return None;
        };

        let Some(expression) = source_value.as_str() else {
            diagnostics.push(ParameterControlDiagnostic::new(
                "expression_source_type_mismatch",
                "expression source parameter must be string-compatible",
            ));
            return None;
        };

        Some(ExpressionControlConfig {
            source_param,
            expression,
        })
    }

    fn evaluate_context_link(
        &mut self,
        consumer: NodeId,
        target_snapshot: &ParameterSnapshot,
        symbol: &str,
        projection: Option<ParamValueProjection>,
        param_snapshots: &HashMap<NodeId, ParameterSnapshot>,
        diagnostics: &mut Vec<ParameterControlDiagnostic>,
    ) -> Option<ParamValue> {
        let symbol = symbol.trim();
        if symbol.is_empty() {
            return None;
        }
        let source = self.resolve_context_symbol_value(consumer, symbol, None, param_snapshots, diagnostics)?;
        self.convert_for_target(&source, target_snapshot, "context_link", projection, diagnostics)
    }

    fn evaluate_template_text(
        &mut self,
        consumer: NodeId,
        target_snapshot: &ParameterSnapshot,
        template: &str,
        param_snapshots: &HashMap<NodeId, ParameterSnapshot>,
        diagnostics: &mut Vec<ParameterControlDiagnostic>,
    ) -> Option<ParamValue> {
        let mut output = String::new();
        for segment in parse_template_segments(template) {
            match segment {
                TemplateSegment::Literal(text) => output.push_str(text.as_str()),
                TemplateSegment::Token(token) => {
                    if let Some(resolved) =
                        self.resolve_template_token(consumer, token.as_str(), param_snapshots, diagnostics)
                    {
                        output.push_str(resolved.as_str());
                    }
                }
            }
        }

        self.convert_for_target(
            &ParamValue::Str(output),
            target_snapshot,
            "template_text",
            None,
            diagnostics,
        )
    }

    fn evaluate_expression(
        &mut self,
        consumer: NodeId,
        target_snapshot: &ParameterSnapshot,
        param_snapshots: &HashMap<NodeId, ParameterSnapshot>,
        tree_snapshot: Option<&Arc<ProcessTreeSnapshot>>,
        change_set: &ControlChangeSet,
        diagnostics: &mut Vec<ParameterControlDiagnostic>,
    ) -> Option<ParamValue> {
        let Some(config) = self.read_expression_control_config(consumer, diagnostics) else {
            self.clear_expression_runtime(consumer);
            return None;
        };
        if config.expression.trim().is_empty() {
            self.clear_expression_runtime(consumer);
            return None;
        }
        let Some(tree_snapshot) = tree_snapshot.cloned() else {
            diagnostics.push(ParameterControlDiagnostic::new(
                "expression_runtime_error",
                "expression runtime tree snapshot is unavailable",
            ));
            return None;
        };

        let previous_runtime = self.expression_runtime.get(&consumer).cloned().unwrap_or_default();
        if !self.expression_should_evaluate(consumer, &config, &previous_runtime, change_set) {
            diagnostics.extend(target_snapshot.control.diagnostics.iter().cloned());
            return None;
        }

        let time_seconds = self.runtime_elapsed.as_secs_f64();
        let delta_seconds = if previous_runtime.source_param.is_some() {
            self.runtime_elapsed
                .saturating_sub(previous_runtime.last_eval_elapsed)
                .as_secs_f64()
        } else {
            0.0
        };
        let local_node = self.nodes.get(consumer).and_then(|node| node.node_data().parent);
        let (symbols, dependencies) =
            self.expression_symbols_and_dependencies(consumer, config.expression.as_str(), param_snapshots);
        let continuous = expression_should_run_continuously(config.expression.as_str());

        let script_runtime = match self.ensure_expression_script_runtime(
            consumer,
            config.expression.as_str(),
            previous_runtime.script_runtime.clone(),
            previous_runtime.source_expression.as_str(),
            local_node,
            tree_snapshot.clone(),
            time_seconds,
            delta_seconds,
        ) {
            Ok(runtime) => runtime,
            Err(message) => {
                diagnostics.push(expression_error_diagnostic(ExpressionErrorStage::Setup, message));
                return None;
            }
        };

        let value = match self.evaluate_expression_script(
            consumer,
            local_node,
            tree_snapshot,
            script_runtime.clone(),
            symbols,
            time_seconds,
            delta_seconds,
        ) {
            Ok(value) => value,
            Err(message) => {
                diagnostics.push(expression_error_diagnostic(ExpressionErrorStage::Evaluation, message));
                return None;
            }
        };

        let mut subscriptions = dependencies.clone();
        subscriptions.insert(config.source_param);
        let next_runtime = ExpressionControlRuntime {
            source_param: Some(config.source_param),
            dependencies: dependencies.clone(),
            subscriptions,
            continuous,
            last_eval_elapsed: self.runtime_elapsed,
            source_expression: config.expression.clone(),
            script_runtime: Some(script_runtime),
        };
        self.reconcile_expression_runtime(consumer, &previous_runtime, &next_runtime);
        self.expression_runtime.insert(consumer, next_runtime);

        self.convert_for_target(&value, target_snapshot, "expression", None, diagnostics)
    }

    fn evaluate_proxy_one_way(
        &mut self,
        param: NodeId,
        target_snapshot: &ParameterSnapshot,
        target_param: NodeId,
        projection: Option<ParamValueProjection>,
        param_snapshots: &HashMap<NodeId, ParameterSnapshot>,
        diagnostics: &mut Vec<ParameterControlDiagnostic>,
    ) -> Option<ParamValue> {
        if target_param == param {
            diagnostics.push(ParameterControlDiagnostic::new(
                "proxy_cycle",
                "proxy target cannot reference the same parameter",
            ));
            return None;
        }

        if self.proxy_chain_contains(param, target_param, param_snapshots) {
            diagnostics.push(ParameterControlDiagnostic::new(
                "proxy_cycle",
                "proxy chain contains a cycle",
            ));
            return None;
        }

        let source = match param_snapshots.get(&target_param) {
            Some(snapshot) => snapshot.value.clone(),
            None => {
                diagnostics.push(ParameterControlDiagnostic::new(
                    "proxy_target_not_parameter",
                    "proxy target is not a parameter node",
                ));
                return None;
            }
        };

        self.convert_for_target(&source, target_snapshot, "proxy", projection, diagnostics)
    }

    fn evaluate_bindings(
        &self,
        binding_targets: &HashMap<NodeId, BindingTargetConfig>,
        param_snapshots: &HashMap<NodeId, ParameterSnapshot>,
    ) -> BindingEvaluation {
        let mut diagnostics_by_param = HashMap::<NodeId, Vec<ParameterControlDiagnostic>>::new();
        if binding_targets.is_empty() {
            return BindingEvaluation {
                pending_writes: Vec::new(),
                diagnostics_by_param,
            };
        }

        let mut pairs = HashSet::<(NodeId, NodeId)>::new();

        for (param, config) in binding_targets {
            let param = *param;
            let target_param = config.target;
            let mut diagnostics = Vec::<ParameterControlDiagnostic>::new();
            let Some(_) = param_snapshots.get(&param) else {
                diagnostics.push(ParameterControlDiagnostic::new(
                    "binding_param_missing",
                    "binding parameter snapshot is missing",
                ));
                diagnostics_by_param.insert(param, diagnostics);
                continue;
            };

            if !param_snapshots.contains_key(&target_param) {
                diagnostics.push(ParameterControlDiagnostic::new(
                    "binding_target_missing",
                    "binding target parameter could not be resolved",
                ));
                diagnostics_by_param.insert(param, diagnostics);
                continue;
            }

            if target_param == param {
                diagnostics.push(ParameterControlDiagnostic::new(
                    "binding_cycle",
                    "binding target cannot reference the same parameter",
                ));
                diagnostics_by_param.insert(param, diagnostics);
                continue;
            }

            let key = if param.0 <= target_param.0 {
                (param, target_param)
            } else {
                (target_param, param)
            };
            pairs.insert(key);
            diagnostics_by_param.insert(param, diagnostics);
        }

        let mut best_write_by_target = HashMap::<NodeId, (BindingWriteScore, ParamValue)>::new();
        let mut sorted_pairs = pairs.into_iter().collect::<Vec<_>>();
        sorted_pairs.sort_by_key(|(a, b)| (a.0, b.0));

        for (a, b) in sorted_pairs {
            let (Some(a_snapshot), Some(b_snapshot)) = (param_snapshots.get(&a), param_snapshots.get(&b)) else {
                continue;
            };

            let a_counter = self.param_last_change_counter.get(&a).copied().unwrap_or(0);
            let b_counter = self.param_last_change_counter.get(&b).copied().unwrap_or(0);

            let (source, source_snapshot, source_counter, target, target_snapshot) =
                if a_counter > b_counter || (a_counter == b_counter && a.0 <= b.0) {
                    (a, a_snapshot, a_counter, b, b_snapshot)
                } else {
                    (b, b_snapshot, b_counter, a, a_snapshot)
                };

            let mut target_diagnostics = Vec::<ParameterControlDiagnostic>::new();
            let Some(converted) = self.convert_binding_for_target(
                source,
                &source_snapshot.value,
                target,
                target_snapshot,
                binding_targets,
                &mut target_diagnostics,
            ) else {
                if !target_diagnostics.is_empty() {
                    diagnostics_by_param
                        .entry(target)
                        .or_default()
                        .extend(target_diagnostics);
                }
                continue;
            };

            if converted == target_snapshot.value {
                continue;
            }

            let score = BindingWriteScore { source_counter, source };
            let replace = match best_write_by_target.get(&target) {
                Some((existing_score, _)) => {
                    score.source_counter > existing_score.source_counter
                        || (score.source_counter == existing_score.source_counter
                            && score.source.0 < existing_score.source.0)
                }
                None => true,
            };
            if replace {
                best_write_by_target.insert(target, (score, converted));
            }
        }

        let mut pending_writes = best_write_by_target
            .into_iter()
            .map(|(target, (_, value))| (target, value))
            .collect::<Vec<_>>();
        pending_writes.sort_by_key(|(node, _)| node.0);

        BindingEvaluation {
            pending_writes,
            diagnostics_by_param,
        }
    }

    fn convert_binding_for_target(
        &self,
        source: NodeId,
        source_value: &ParamValue,
        target: NodeId,
        target_snapshot: &ParameterSnapshot,
        binding_targets: &HashMap<NodeId, BindingTargetConfig>,
        diagnostics: &mut Vec<ParameterControlDiagnostic>,
    ) -> Option<ParamValue> {
        if let Some(config) = binding_targets.get(&target).filter(|config| config.target == source) {
            return self.convert_for_target(source_value, target_snapshot, "binding", config.projection, diagnostics);
        }

        if let Some(config) = binding_targets.get(&source).filter(|config| config.target == target) {
            return self.convert_for_target_reverse(
                source_value,
                target_snapshot,
                "binding",
                config.projection,
                diagnostics,
            );
        }

        self.convert_for_target(source_value, target_snapshot, "binding", None, diagnostics)
    }

    fn expression_symbols_and_dependencies(
        &self,
        consumer: NodeId,
        expression: &str,
        param_snapshots: &HashMap<NodeId, ParameterSnapshot>,
    ) -> (JsonMap<String, JsonValue>, HashSet<NodeId>) {
        let mut symbols = JsonMap::<String, JsonValue>::new();
        let mut dependencies = HashSet::<NodeId>::new();
        let candidates = self.user_contexts.collect_candidates(consumer, None, |node| {
            self.nodes.get(node).and_then(|entry| entry.node_data().parent)
        });

        for candidate in candidates {
            if candidate.shadowed || symbols.contains_key(candidate.symbol.as_str()) {
                continue;
            }
            if !expression_mentions_symbol(expression, candidate.symbol.as_str()) {
                continue;
            }

            let Some(snapshot) = param_snapshots.get(&candidate.entry_param) else {
                continue;
            };

            symbols.insert(candidate.symbol, snapshot.value.to_script_json());
            dependencies.insert(candidate.entry_param);
        }

        (symbols, dependencies)
    }

    fn ensure_expression_script_runtime(
        &self,
        consumer: NodeId,
        expression: &str,
        existing_runtime: Option<Arc<Mutex<Box<dyn ScriptRuntime>>>>,
        previous_expression: &str,
        local_node: Option<NodeId>,
        tree_snapshot: Arc<ProcessTreeSnapshot>,
        time_seconds: f64,
        delta_seconds: f64,
    ) -> Result<Arc<Mutex<Box<dyn ScriptRuntime>>>, String> {
        if let Some(runtime) = existing_runtime {
            if expression == previous_expression {
                return Ok(runtime);
            }
        }

        let mut runtime: Box<dyn ScriptRuntime> = Box::new(
            QuickJsRuntime::new(ScriptBudgets::default())
                .map_err(|error| format!("failed to create QuickJS runtime: {error}"))?,
        );
        let source = build_expression_runtime_source(expression);
        let source_name = format!("{EXPRESSION_RUNTIME_SOURCE_NAME}#{}", consumer.0);
        let mut host =
            ExpressionScriptHostBridge::new(consumer, local_node, tree_snapshot, time_seconds, delta_seconds);
        runtime
            .load(source.as_str(), source_name.as_str(), Some(&mut host))
            .map_err(|error| format!("failed to compile expression: {error}"))?;

        Ok(Arc::new(Mutex::new(runtime)))
    }

    fn evaluate_expression_script(
        &self,
        consumer: NodeId,
        local_node: Option<NodeId>,
        tree_snapshot: Arc<ProcessTreeSnapshot>,
        runtime: Arc<Mutex<Box<dyn ScriptRuntime>>>,
        symbols: JsonMap<String, JsonValue>,
        time_seconds: f64,
        delta_seconds: f64,
    ) -> Result<ParamValue, String> {
        let mut host =
            ExpressionScriptHostBridge::new(consumer, local_node, tree_snapshot, time_seconds, delta_seconds);
        let mut runtime_guard = runtime
            .lock()
            .map_err(|_| "expression runtime lock poisoned".to_string())?;
        let result = runtime_guard
            .call_export(
                EXPRESSION_EXPORT_NAME,
                &[ScriptValue::Json(JsonValue::Object(symbols))],
                &mut host,
            )
            .map_err(|error| format!("expression evaluation failed: {error}"))?;
        script_value_to_param_value(result)
    }

    fn resolve_template_token(
        &mut self,
        consumer: NodeId,
        token: &str,
        param_snapshots: &HashMap<NodeId, ParameterSnapshot>,
        diagnostics: &mut Vec<ParameterControlDiagnostic>,
    ) -> Option<String> {
        let token = token.trim();
        if token.is_empty() {
            diagnostics.push(ParameterControlDiagnostic::new(
                "template_token_empty",
                "template token cannot be empty",
            ));
            return None;
        }

        if parse_multiplex_template_token(token).is_some()
            && self.node_or_ancestor_has_tag(consumer, CONTEXT_LINK_LANE_DEFERRED_TAG)
        {
            return Some(format!("{{{token}}}"));
        }

        if let Some(stripped) = token.strip_prefix('$') {
            return self.node_metadata_value(consumer, stripped).or_else(|| {
                diagnostics.push(ParameterControlDiagnostic::new(
                    "template_meta_field_unknown",
                    format!("unknown metadata field '${stripped}'"),
                ));
                None
            });
        }

        if let Some((symbol, field)) = token.split_once(".$") {
            let source = self.resolve_context_symbol_value(consumer, symbol, None, param_snapshots, diagnostics)?;
            let ParamValue::Reference(reference) = source else {
                diagnostics.push(ParameterControlDiagnostic::new(
                    "template_meta_requires_reference",
                    format!("symbol '{}' does not resolve to a reference parameter", symbol.trim()),
                ));
                return None;
            };
            let Some(target_node) = self.resolve_reference_target_node(&reference) else {
                diagnostics.push(ParameterControlDiagnostic::new(
                    "template_meta_target_missing",
                    format!("symbol '{}' references a missing node", symbol.trim()),
                ));
                return None;
            };

            return self.node_metadata_value(target_node, field).or_else(|| {
                diagnostics.push(ParameterControlDiagnostic::new(
                    "template_meta_field_unknown",
                    format!("unknown metadata field '${field}'"),
                ));
                None
            });
        }

        let source = self.resolve_context_symbol_value(consumer, token, None, param_snapshots, diagnostics)?;
        source.as_str().or_else(|| Some(source.to_string()))
    }

    fn resolve_context_symbol_value(
        &mut self,
        consumer: NodeId,
        symbol: &str,
        expected: Option<UserContextValueType>,
        param_snapshots: &HashMap<NodeId, ParameterSnapshot>,
        diagnostics: &mut Vec<ParameterControlDiagnostic>,
    ) -> Option<ParamValue> {
        if parse_multiplex_context_link_symbol(symbol).is_some()
            && self.node_or_ancestor_has_tag(consumer, CONTEXT_LINK_LANE_DEFERRED_TAG)
        {
            return None;
        }
        let lookup = self.resolve_user_context_symbol(consumer, symbol, expected);
        match lookup {
            UserContextLookup::Resolved(resolution) => {
                if resolution.kind == UserContextEntryKind::MultiplexList {
                    if self.node_or_ancestor_has_tag(consumer, CONTEXT_LINK_LANE_DEFERRED_TAG) {
                        return None;
                    }
                    diagnostics.push(ParameterControlDiagnostic::new(
                        "context_multiplex_requires_lane",
                        format!(
                            "symbol '{}' resolves to a multiplex list and can only be evaluated in a processor lane",
                            resolution.symbol
                        ),
                    ));
                    return None;
                }
                param_snapshots
                    .get(&resolution.entry_param)
                    .map(|snapshot| snapshot.value.clone())
                    .or_else(|| {
                        diagnostics.push(ParameterControlDiagnostic::new(
                            "context_target_missing",
                            format!(
                                "symbol '{}' resolved to missing parameter node {}",
                                resolution.symbol, resolution.entry_param.0
                            ),
                        ));
                        None
                    })
            }
            UserContextLookup::TypeMismatch(mismatch) => {
                diagnostics.push(
                    ParameterControlDiagnostic::new(
                        "context_type_mismatch",
                        format!(
                            "symbol '{}' resolved to {:?} but expected {:?}",
                            mismatch.symbol, mismatch.found, mismatch.expected
                        ),
                    )
                    .with_detail(format!(
                        "scope_owner={}, lexical_depth={}",
                        mismatch.scope_owner.0, mismatch.lexical_depth
                    )),
                );
                None
            }
            UserContextLookup::Missing { symbol } => {
                diagnostics.push(ParameterControlDiagnostic::new(
                    "context_symbol_missing",
                    format!("context symbol '{symbol}' was not found"),
                ));
                None
            }
        }
    }

    fn node_or_ancestor_has_tag(&self, node: NodeId, tag: &str) -> bool {
        let mut current = Some(node);
        while let Some(node_id) = current {
            let Some(node) = self.nodes.get(node_id) else {
                return false;
            };
            let data = node.node_data();
            if data.meta.tags.iter().any(|candidate| candidate == tag) {
                return true;
            }
            current = data.parent;
        }
        false
    }

    fn resolve_reference_target_node(&self, reference: &NodeReference) -> Option<NodeId> {
        reference
            .cached_id()
            .filter(|node_id| self.nodes.contains(*node_id))
            .or_else(|| {
                (!reference.uuid().is_nil())
                    .then(|| self.node_id_by_uuid(reference.uuid()))
                    .flatten()
            })
    }

    fn node_metadata_value(&self, node: NodeId, field: &str) -> Option<String> {
        let field = field.trim();
        let entry = self.nodes.get(node)?;
        match field {
            "name" => Some(entry.node_data().meta.label.clone()),
            "type" => Some(entry.get_type().to_string()),
            "id" => Some(node.0.to_string()),
            "uuid" => Some(entry.node_data().meta.uuid.0.to_string()),
            _ => None,
        }
    }

    fn resolve_control_target_param(
        &self,
        target: &NodeReference,
        param_snapshots: &HashMap<NodeId, ParameterSnapshot>,
    ) -> Option<NodeId> {
        let target_node = self.resolve_reference_target_node(target)?;
        param_snapshots.contains_key(&target_node).then_some(target_node)
    }

    fn proxy_chain_contains(
        &self,
        needle: NodeId,
        mut start: NodeId,
        param_snapshots: &HashMap<NodeId, ParameterSnapshot>,
    ) -> bool {
        let mut visited = HashSet::<NodeId>::new();

        loop {
            if start == needle {
                return true;
            }
            if !visited.insert(start) {
                return false;
            }

            let Some(snapshot) = param_snapshots.get(&start) else {
                return false;
            };
            if snapshot.control.mode != ParameterControlMode::Proxy {
                return false;
            }

            let ParameterControlSpec::Proxy = &snapshot.control.spec else {
                return false;
            };

            let mut diagnostics = Vec::<ParameterControlDiagnostic>::new();
            let Some(config) = self.read_reference_control_config(start, "proxy", &mut diagnostics) else {
                return false;
            };
            let Some(next) = self.resolve_control_target_param(&config.target, param_snapshots) else {
                return false;
            };
            start = next;
        }
    }

    fn convert_for_target(
        &self,
        source: &ParamValue,
        target_snapshot: &ParameterSnapshot,
        mode: &str,
        projection: Option<ParamValueProjection>,
        diagnostics: &mut Vec<ParameterControlDiagnostic>,
    ) -> Option<ParamValue> {
        self.convert_for_target_with_direction(source, target_snapshot, mode, projection, false, diagnostics)
    }

    fn convert_for_target_reverse(
        &self,
        source: &ParamValue,
        target_snapshot: &ParameterSnapshot,
        mode: &str,
        projection: Option<ParamValueProjection>,
        diagnostics: &mut Vec<ParameterControlDiagnostic>,
    ) -> Option<ParamValue> {
        self.convert_for_target_with_direction(source, target_snapshot, mode, projection, true, diagnostics)
    }

    fn convert_for_target_with_direction(
        &self,
        source: &ParamValue,
        target_snapshot: &ParameterSnapshot,
        mode: &str,
        projection: Option<ParamValueProjection>,
        reverse_projection: bool,
        diagnostics: &mut Vec<ParameterControlDiagnostic>,
    ) -> Option<ParamValue> {
        let converted = if reverse_projection {
            coerce_param_value_for_target_reverse(source, &target_snapshot.value, projection)
        } else {
            coerce_param_value_for_target(source, &target_snapshot.value, projection)
        };

        let Some(converted) = converted else {
            let projection_label = projection
                .map(|projection| {
                    if reverse_projection {
                        format!(" with reverse projection '{}'", projection.variant_id())
                    } else {
                        format!(" with projection '{}'", projection.variant_id())
                    }
                })
                .unwrap_or_default();
            diagnostics.push(ParameterControlDiagnostic::new(
                "control_type_incompatible",
                format!(
                    "control mode '{}' cannot convert value {:?}{} for target {:?}",
                    mode, source, projection_label, target_snapshot.value
                ),
            ));
            return None;
        };

        match target_snapshot.constraints.normalize(converted) {
            Ok(normalized) => Some(normalized),
            Err(message) => {
                diagnostics.push(ParameterControlDiagnostic::new(
                    "control_constraints_violation",
                    format!("control mode '{}' produced an invalid value: {message}", mode),
                ));
                None
            }
        }
    }

    fn collect_control_change_set(&self) -> ControlChangeSet {
        let mut changed_params = HashSet::<NodeId>::new();
        let mut changed_controls = HashSet::<NodeId>::new();
        let mut structural = false;

        for event in &self.inbox.events {
            match &event.kind {
                EventKind::ParamChanged { param, .. } => {
                    changed_params.insert(*param);
                }
                EventKind::ParamControlChanged { param, .. } => {
                    changed_controls.insert(*param);
                    structural = true;
                }
                _ => {
                    structural = true;
                }
            }
        }

        ControlChangeSet {
            changed_params,
            changed_controls,
            structural,
        }
    }

    fn expression_should_evaluate(
        &self,
        consumer: NodeId,
        config: &ExpressionControlConfig,
        previous: &ExpressionControlRuntime,
        change_set: &ControlChangeSet,
    ) -> bool {
        let continuous_tick_advanced = previous.continuous && self.runtime_elapsed > previous.last_eval_elapsed;
        continuous_tick_advanced
            || previous.source_param != Some(config.source_param)
            || change_set.changed_controls.contains(&consumer)
            || change_set.changed_params.contains(&config.source_param)
            || !change_set.changed_params.is_disjoint(&previous.dependencies)
            || change_set.structural
    }

    fn reconcile_expression_runtime(
        &mut self,
        consumer: NodeId,
        previous: &ExpressionControlRuntime,
        next: &ExpressionControlRuntime,
    ) {
        let mut removed = previous
            .subscriptions
            .difference(&next.subscriptions)
            .copied()
            .collect::<Vec<_>>();
        removed.sort_by_key(|node| node.0);
        for target in removed {
            self.edits.push(Edit::RemoveEventListener {
                subscriber: consumer,
                subscription: EventSubscription::node(target),
            });
        }

        let mut added = next
            .subscriptions
            .difference(&previous.subscriptions)
            .copied()
            .collect::<Vec<_>>();
        added.sort_by_key(|node| node.0);
        for target in added {
            self.edits.push(Edit::AddEventListener {
                subscriber: consumer,
                subscription: EventSubscription::node(target),
            });
        }
    }

    fn clear_expression_runtime(&mut self, consumer: NodeId) {
        let Some(runtime) = self.expression_runtime.remove(&consumer) else {
            return;
        };

        let mut subscriptions = runtime.subscriptions.into_iter().collect::<Vec<_>>();
        subscriptions.sort_by_key(|node| node.0);
        for target in subscriptions {
            self.edits.push(Edit::RemoveEventListener {
                subscriber: consumer,
                subscription: EventSubscription::node(target),
            });
        }
    }

    fn apply_parameter_control_diagnostics(
        &mut self,
        param_snapshots: &HashMap<NodeId, ParameterSnapshot>,
        mut diagnostics_by_param: HashMap<NodeId, Vec<ParameterControlDiagnostic>>,
    ) -> bool {
        let mut updates = Vec::<(NodeId, ParameterControlState, ParameterControlState)>::new();

        let mut params = param_snapshots.keys().copied().collect::<Vec<_>>();
        params.sort_by_key(|node| node.0);

        for param in params {
            let Some(node) = self.nodes.get(param) else {
                continue;
            };
            let Some(current_state) = node.engine_param_control_state() else {
                continue;
            };

            let diagnostics = diagnostics_by_param.remove(&param).unwrap_or_default();
            if current_state.diagnostics == diagnostics {
                continue;
            }

            let mut next_state = current_state.clone();
            next_state.diagnostics = diagnostics;
            updates.push((param, current_state, next_state));
        }

        let mut changed = false;
        for (param, old_state, new_state) in updates {
            let Some(node) = self.nodes.get_mut(param) else {
                continue;
            };
            if node.engine_set_param_control_state(new_state.clone()).is_ok() {
                self.emit_event(EventKind::ParamControlChanged {
                    param,
                    old_state,
                    new_state,
                });
                changed = true;
            }
        }

        changed
    }

    fn sync_parameter_control_warnings(&mut self, param_snapshots: &HashMap<NodeId, ParameterSnapshot>) -> bool {
        let mut params = param_snapshots.keys().copied().collect::<Vec<_>>();
        params.sort_by_key(|node| node.0);

        let mut pending = Vec::<(NodeId, crate::node::PresentationHint)>::new();
        for param in params {
            let Some(node) = self.nodes.get(param) else {
                continue;
            };
            let Some(control_state) = node.engine_param_control_state() else {
                continue;
            };

            let mut next_presentation = node.node_data().meta.presentation.clone();
            next_presentation
                .warnings
                .retain(|warning| !warning.id.starts_with(CONTROL_DIAGNOSTIC_WARNING_ID_PREFIX));

            let mut warning_id_occurrences = HashMap::<String, usize>::new();
            for diagnostic in &control_state.diagnostics {
                let diagnostic_code = diagnostic.code.trim();
                let normalized_code = if diagnostic_code.is_empty() {
                    "unknown"
                } else {
                    diagnostic_code
                };
                let mode_id = control_mode_id(control_state.mode);
                let base_warning_id = format!("{CONTROL_DIAGNOSTIC_WARNING_ID_PREFIX}{mode_id}:{normalized_code}");
                let occurrence = warning_id_occurrences.entry(base_warning_id.clone()).or_insert(0usize);
                let warning_id = if *occurrence == 0 {
                    base_warning_id
                } else {
                    format!("{base_warning_id}:{}", *occurrence + 1)
                };
                *occurrence += 1;

                let mut detail_lines = vec![format!("code: {normalized_code}")];
                if let Some(detail) = diagnostic.detail.as_deref() {
                    let detail = detail.trim();
                    if !detail.is_empty() {
                        detail_lines.push(detail.to_string());
                    }
                }
                let detail = Some(detail_lines.join("\n"));

                let diagnostic_message = diagnostic.message.trim();
                let message = if control_state.mode == ParameterControlMode::Expression
                    && normalized_code == "expression_error"
                {
                    if diagnostic_message.is_empty() {
                        "Expression error".to_string()
                    } else {
                        diagnostic_message.to_string()
                    }
                } else if diagnostic_message.is_empty() {
                    format!("{} control diagnostic", control_mode_label(control_state.mode))
                } else {
                    format!(
                        "{} control: {}",
                        control_mode_label(control_state.mode),
                        diagnostic_message
                    )
                };

                next_presentation.set_warning_message(Some(warning_id.as_str()), message, detail);
            }

            if next_presentation != node.node_data().meta.presentation {
                pending.push((param, next_presentation));
            }
        }

        for (node_id, presentation) in &pending {
            if let Some(node) = self.nodes.get_mut(*node_id) {
                node.node_data_mut().meta.presentation = presentation.clone();
            }
            self.emit_event(EventKind::MetaChanged {
                node: *node_id,
                patch: crate::node::NodeMetaPatch {
                    presentation: Some(presentation.clone()),
                    ..Default::default()
                },
            });
        }

        !pending.is_empty()
    }
}

fn build_expression_runtime_source(expression: &str) -> String {
    let expression_json = serde_json::to_string(expression).unwrap_or_else(|_| "\"\"".to_string());
    format!(
        r#"
const __gc_expression_source = {expression_json};
if (typeof Boolean.prototype.get !== "function") {{
  Object.defineProperty(Boolean.prototype, "get", {{
    value() {{ return this.valueOf(); }},
    configurable: true,
  }});
}}
if (typeof Number.prototype.get !== "function") {{
  Object.defineProperty(Number.prototype, "get", {{
    value() {{ return this.valueOf(); }},
    configurable: true,
  }});
}}
if (typeof String.prototype.get !== "function") {{
  Object.defineProperty(String.prototype, "get", {{
    value() {{ return this.valueOf(); }},
    configurable: true,
  }});
}}
globalThis.__gc_script_exports["{EXPRESSION_EXPORT_NAME}"] = function(__gc_symbols = {{}}) {{
  const __gc_scope = (__gc_symbols && typeof __gc_symbols === "object" && !Array.isArray(__gc_symbols)) ? __gc_symbols : {{}};
  const __gc_keys = Object.keys(__gc_scope).filter((key) => /^[A-Za-z_$][A-Za-z0-9_$]*$/.test(key));
  const __gc_values = __gc_keys.map((key) => __gc_scope[key]);
  const __gc_eval = Function(...__gc_keys, "return (" + __gc_expression_source + ");");
  return __gc_eval(...__gc_values);
}};
"#
    )
}

fn script_value_to_param_value(value: ScriptValue) -> Result<ParamValue, String> {
    match value {
        ScriptValue::Nil => Err("expression returned undefined/null".to_string()),
        ScriptValue::Bool(value) => Ok(ParamValue::Bool(value)),
        ScriptValue::Int(value) => {
            if let Ok(value) = i32::try_from(value) {
                Ok(ParamValue::Int(value))
            } else {
                Ok(ParamValue::Float(value as f64))
            }
        }
        ScriptValue::Float(value) => Ok(ParamValue::Float(value)),
        ScriptValue::Str(value) => Ok(ParamValue::Str(value)),
        ScriptValue::Json(value) => ParamValue::from_script_json(&value),
    }
}

fn is_expression_identifier_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_' || ch == '$'
}

fn is_expression_identifier_continue(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_' || ch == '$'
}

fn expression_mentions_symbol(expression: &str, symbol: &str) -> bool {
    let symbol = symbol.trim();
    if symbol.is_empty() {
        return false;
    }
    let mut symbol_chars = symbol.chars();
    let Some(first) = symbol_chars.next() else {
        return false;
    };
    if !is_expression_identifier_start(first) || !symbol_chars.all(is_expression_identifier_continue) {
        return false;
    }

    let chars = expression.chars().collect::<Vec<_>>();
    let mut index = 0usize;
    while index < chars.len() {
        if !is_expression_identifier_start(chars[index]) {
            index += 1;
            continue;
        }

        let start = index;
        index += 1;
        while index < chars.len() && is_expression_identifier_continue(chars[index]) {
            index += 1;
        }

        let token = chars[start..index].iter().collect::<String>();
        if token != symbol {
            continue;
        }

        let mut cursor = start;
        while cursor > 0 && chars[cursor - 1].is_whitespace() {
            cursor -= 1;
        }
        if cursor > 0 && chars[cursor - 1] == '.' {
            continue;
        }

        return true;
    }

    false
}

fn expression_should_run_continuously(expression: &str) -> bool {
    expression.contains("time(")
        || expression_mentions_symbol(expression, "deltaTime")
        || expression_mentions_symbol(expression, "root")
        || expression_mentions_symbol(expression, "local")
        || expression_mentions_symbol(expression, "script")
        || expression_mentions_symbol(expression, "gc")
}

fn parse_template_segments(template: &str) -> Vec<TemplateSegment> {
    let chars = template.chars().collect::<Vec<_>>();
    let mut out = Vec::<TemplateSegment>::new();
    let mut literal = String::new();
    let mut index = 0usize;

    while index < chars.len() {
        let ch = chars[index];
        if ch == '{' {
            if index + 1 < chars.len() && chars[index + 1] == '{' {
                literal.push('{');
                index += 2;
                continue;
            }

            if !literal.is_empty() {
                out.push(TemplateSegment::Literal(std::mem::take(&mut literal)));
            }

            index += 1;
            let mut token = String::new();
            while index < chars.len() && chars[index] != '}' {
                token.push(chars[index]);
                index += 1;
            }

            if index >= chars.len() {
                literal.push('{');
                literal.push_str(token.as_str());
                break;
            }

            index += 1;
            out.push(TemplateSegment::Token(token.trim().to_string()));
            continue;
        }

        if ch == '}' && index + 1 < chars.len() && chars[index + 1] == '}' {
            literal.push('}');
            index += 2;
            continue;
        }

        literal.push(ch);
        index += 1;
    }

    if !literal.is_empty() {
        out.push(TemplateSegment::Literal(literal));
    }
    if out.is_empty() {
        out.push(TemplateSegment::Literal(String::new()));
    }

    out
}

pub(crate) fn text_has_template_token(template: &str) -> bool {
    parse_template_segments(template)
        .into_iter()
        .any(|segment| matches!(segment, TemplateSegment::Token(token) if !token.is_empty()))
}

fn control_spec_matches_mode(mode: ParameterControlMode, spec: &ParameterControlSpec) -> bool {
    matches!(
        (mode, spec),
        (ParameterControlMode::Manual, ParameterControlSpec::Manual)
            | (
                ParameterControlMode::ContextLink,
                ParameterControlSpec::ContextLink { .. }
            )
            | (
                ParameterControlMode::TemplateText,
                ParameterControlSpec::TemplateText { .. }
            )
            | (ParameterControlMode::Expression, ParameterControlSpec::Expression)
            | (ParameterControlMode::Proxy, ParameterControlSpec::Proxy)
            | (ParameterControlMode::Binding, ParameterControlSpec::Binding)
            | (ParameterControlMode::Animation, ParameterControlSpec::Animation)
    )
}
