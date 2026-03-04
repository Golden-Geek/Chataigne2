use std::collections::{HashMap, HashSet};
use std::f64::consts::PI;

use crate::contexts::{UserContextLookup, UserContextValueType};
use crate::edit::Edit;
use crate::events::EventKind;
use crate::node::{Node, NodeId, NodeReference};
use crate::parameter::{
    AnimationWaveform, ParamValue, ParameterControlDiagnostic, ParameterControlMode, ParameterControlSpec, ParameterControlState, ParameterEventBehaviour,
    ParameterSnapshot,
};

use super::Engine;

#[derive(Clone, Debug, PartialEq)]
enum TemplateSegment {
    Literal(String),
    Token(String),
}

struct BindingEvaluation {
    pending_writes: Vec<(NodeId, ParamValue)>,
    diagnostics_by_param: HashMap<NodeId, Vec<ParameterControlDiagnostic>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct BindingWriteScore {
    source_counter: u64,
    source: NodeId,
}

impl<T: Node> Engine<T> {
    /// Evaluates all parameter-control states and queues resulting parameter writes.
    ///
    /// Returns `true` when it changed queued edits or updated control diagnostics.
    pub(crate) fn evaluate_parameter_controls(&mut self) -> bool {
        let param_snapshots = self
            .nodes
            .iter()
            .filter_map(|(node_id, node)| node.engine_param_snapshot().map(|snapshot| (node_id, snapshot)))
            .collect::<HashMap<_, _>>();

        if param_snapshots.is_empty() {
            return false;
        }

        let mut diagnostics_by_param = HashMap::<NodeId, Vec<ParameterControlDiagnostic>>::new();
        let mut binding_params = Vec::<NodeId>::new();
        let mut queued_writes = Vec::<(NodeId, ParamValue)>::new();

        for (param, snapshot) in &param_snapshots {
            let control = &snapshot.control;
            let mut diagnostics = Vec::<ParameterControlDiagnostic>::new();

            let maybe_value = match control.mode {
                ParameterControlMode::Manual => None,
                ParameterControlMode::ContextLink => match &control.spec {
                    ParameterControlSpec::ContextLink { symbol } => self.evaluate_context_link(*param, snapshot, symbol.as_str(), &param_snapshots, &mut diagnostics),
                    _ => {
                        diagnostics.push(ParameterControlDiagnostic::new(
                            "invalid_control_spec",
                            "control mode/context-link mismatch",
                        ));
                        None
                    }
                },
                ParameterControlMode::TemplateText => match &control.spec {
                    ParameterControlSpec::TemplateText { template } => self.evaluate_template_text(*param, snapshot, template.as_str(), &param_snapshots, &mut diagnostics),
                    _ => {
                        diagnostics.push(ParameterControlDiagnostic::new(
                            "invalid_control_spec",
                            "control mode/template-text mismatch",
                        ));
                        None
                    }
                },
                ParameterControlMode::Expression => match &control.spec {
                    ParameterControlSpec::Expression { expression } => {
                        self.evaluate_expression(*param, snapshot, expression.as_str(), &param_snapshots, &mut diagnostics)
                    }
                    _ => {
                        diagnostics.push(ParameterControlDiagnostic::new(
                            "invalid_control_spec",
                            "control mode/expression mismatch",
                        ));
                        None
                    }
                },
                ParameterControlMode::Proxy => match &control.spec {
                    ParameterControlSpec::Proxy { target } => self.evaluate_proxy(*param, snapshot, target, &param_snapshots, &mut diagnostics),
                    _ => {
                        diagnostics.push(ParameterControlDiagnostic::new(
                            "invalid_control_spec",
                            "control mode/proxy mismatch",
                        ));
                        None
                    }
                },
                ParameterControlMode::Binding => {
                    binding_params.push(*param);
                    None
                }
                ParameterControlMode::Animation => match &control.spec {
                    ParameterControlSpec::Animation { animation } => self.evaluate_animation(snapshot, animation, &mut diagnostics),
                    _ => {
                        diagnostics.push(ParameterControlDiagnostic::new(
                            "invalid_control_spec",
                            "control mode/animation mismatch",
                        ));
                        None
                    }
                },
            };

            if let Some(value) = maybe_value {
                if snapshot.value != value {
                    queued_writes.push((*param, value));
                }
            }

            diagnostics_by_param.insert(*param, diagnostics);
        }

        let binding_result = self.evaluate_bindings(&binding_params, &param_snapshots);
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
        queued_any || diagnostics_changed
    }

    /// Sets control state on one parameter node.
    ///
    /// Returns `true` when the state changed.
    pub fn set_param_control_state(&mut self, param: NodeId, mut state: ParameterControlState) -> Result<bool, String> {
        if !control_spec_matches_mode(state.mode, &state.spec) {
            return Err("parameter control state has a mode/spec mismatch".to_string());
        }

        let Some(node) = self.nodes.get(param) else {
            return Err(format!("parameter node {} was not found", param.0));
        };
        if node.engine_param_snapshot().is_none() {
            return Err(format!("node {} is not a parameter node", param.0));
        }

        let Some(current_state) = node.engine_param_control_state() else {
            return Err(format!("node {} does not expose parameter control state", param.0));
        };
        state.diagnostics.clear();
        if current_state == state {
            return Ok(false);
        }

        let next_state = state.clone();
        let Some(node) = self.nodes.get_mut(param) else {
            return Err(format!("parameter node {} was not found", param.0));
        };
        node.engine_set_param_control_state(state)?;
        self.emit_event(EventKind::ParamControlChanged {
            param,
            old_state: current_state,
            new_state: next_state,
        });
        Ok(true)
    }

    fn evaluate_context_link(
        &mut self,
        consumer: NodeId,
        target_snapshot: &ParameterSnapshot,
        symbol: &str,
        param_snapshots: &HashMap<NodeId, ParameterSnapshot>,
        diagnostics: &mut Vec<ParameterControlDiagnostic>,
    ) -> Option<ParamValue> {
        let source = self.resolve_context_symbol_value(consumer, symbol, None, param_snapshots, diagnostics)?;
        self.convert_for_target(&source, target_snapshot, "context_link", diagnostics)
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
                    if let Some(resolved) = self.resolve_template_token(consumer, token.as_str(), param_snapshots, diagnostics) {
                        output.push_str(resolved.as_str());
                    }
                }
            }
        }

        self.convert_for_target(&ParamValue::Str(output), target_snapshot, "template_text", diagnostics)
    }

    fn evaluate_expression(
        &mut self,
        consumer: NodeId,
        target_snapshot: &ParameterSnapshot,
        expression: &str,
        param_snapshots: &HashMap<NodeId, ParameterSnapshot>,
        diagnostics: &mut Vec<ParameterControlDiagnostic>,
    ) -> Option<ParamValue> {
        let result = evaluate_expression_numeric(expression, |identifier| {
            let lookup = self.resolve_user_context_symbol(consumer, identifier, None);
            let source = match lookup {
                UserContextLookup::Resolved(resolution) => param_snapshots
                    .get(&resolution.entry_param)
                    .map(|snapshot| snapshot.value.clone())
                    .ok_or_else(|| format!("resolved symbol '{identifier}' references missing parameter node")),
                UserContextLookup::TypeMismatch(_) => Err(format!("symbol '{identifier}' type mismatch")),
                UserContextLookup::Missing { .. } => Err(format!("symbol '{identifier}' was not found")),
            }?;
            source
                .as_float()
                .ok_or_else(|| format!("symbol '{identifier}' cannot be coerced to numeric input"))
        });

        let value = match result {
            Ok(value) => value,
            Err(message) => {
                diagnostics.push(ParameterControlDiagnostic::new("expression_error", message));
                return None;
            }
        };

        self.convert_for_target(&ParamValue::Float(value), target_snapshot, "expression", diagnostics)
    }

    fn evaluate_proxy(
        &mut self,
        param: NodeId,
        target_snapshot: &ParameterSnapshot,
        target: &NodeReference,
        param_snapshots: &HashMap<NodeId, ParameterSnapshot>,
        diagnostics: &mut Vec<ParameterControlDiagnostic>,
    ) -> Option<ParamValue> {
        let target_param = self.resolve_control_target_param(target, param_snapshots).or_else(|| {
            diagnostics.push(ParameterControlDiagnostic::new(
                "proxy_target_missing",
                "proxy target parameter could not be resolved",
            ));
            None
        })?;

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

        self.convert_for_target(&source, target_snapshot, "proxy", diagnostics)
    }

    fn evaluate_animation(
        &self,
        target_snapshot: &ParameterSnapshot,
        animation: &crate::parameter::AnimationControlSpec,
        diagnostics: &mut Vec<ParameterControlDiagnostic>,
    ) -> Option<ParamValue> {
        if animation.frequency_hz.is_sign_negative() {
            diagnostics.push(ParameterControlDiagnostic::new(
                "animation_invalid_frequency",
                "animation frequency must be >= 0",
            ));
            return None;
        }

        let t = self.runtime_elapsed.as_secs_f64();
        let phase = animation.phase + t * animation.frequency_hz;
        let waveform = match animation.waveform {
            AnimationWaveform::Sine => (phase * (PI * 2.0)).sin(),
            AnimationWaveform::Triangle => 2.0 * (2.0 * (phase - (phase + 0.5).floor())).abs() - 1.0,
            AnimationWaveform::Saw => 2.0 * (phase - (phase + 0.5).floor()),
            AnimationWaveform::Square => {
                if (phase * (PI * 2.0)).sin().is_sign_negative() {
                    -1.0
                } else {
                    1.0
                }
            }
        };
        let value = animation.offset + animation.amplitude * waveform;
        self.convert_for_target(&ParamValue::Float(value), target_snapshot, "animation", diagnostics)
    }

    fn evaluate_bindings(
        &self,
        binding_params: &[NodeId],
        param_snapshots: &HashMap<NodeId, ParameterSnapshot>,
    ) -> BindingEvaluation {
        let mut diagnostics_by_param = HashMap::<NodeId, Vec<ParameterControlDiagnostic>>::new();
        if binding_params.is_empty() {
            return BindingEvaluation {
                pending_writes: Vec::new(),
                diagnostics_by_param,
            };
        }

        let binding_set = binding_params.iter().copied().collect::<HashSet<_>>();
        let mut pairs = HashSet::<(NodeId, NodeId)>::new();

        for param in binding_params {
            let mut diagnostics = Vec::<ParameterControlDiagnostic>::new();
            let Some(snapshot) = param_snapshots.get(param) else {
                diagnostics.push(ParameterControlDiagnostic::new(
                    "binding_param_missing",
                    "binding parameter snapshot is missing",
                ));
                diagnostics_by_param.insert(*param, diagnostics);
                continue;
            };

            let target = match &snapshot.control.spec {
                ParameterControlSpec::Binding { target } => target,
                _ => {
                    diagnostics.push(ParameterControlDiagnostic::new(
                        "invalid_control_spec",
                        "control mode/binding mismatch",
                    ));
                    diagnostics_by_param.insert(*param, diagnostics);
                    continue;
                }
            };

            let Some(target_param) = self.resolve_control_target_param(target, param_snapshots) else {
                diagnostics.push(ParameterControlDiagnostic::new(
                    "binding_target_missing",
                    "binding target parameter could not be resolved",
                ));
                diagnostics_by_param.insert(*param, diagnostics);
                continue;
            };

            if target_param == *param {
                diagnostics.push(ParameterControlDiagnostic::new(
                    "binding_cycle",
                    "binding target cannot reference the same parameter",
                ));
                diagnostics_by_param.insert(*param, diagnostics);
                continue;
            }

            if !binding_set.contains(&target_param) {
                diagnostics.push(ParameterControlDiagnostic::new(
                    "binding_target_not_binding",
                    "binding target is not configured in binding mode",
                ));
            }

            let key = if param.0 <= target_param.0 {
                (*param, target_param)
            } else {
                (target_param, *param)
            };
            pairs.insert(key);
            diagnostics_by_param.insert(*param, diagnostics);
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
            let Some(converted) = self.convert_for_target(&source_snapshot.value, target_snapshot, "binding", &mut target_diagnostics) else {
                if !target_diagnostics.is_empty() {
                    diagnostics_by_param.entry(target).or_default().extend(target_diagnostics);
                }
                continue;
            };

            if converted == target_snapshot.value {
                continue;
            }

            let score = BindingWriteScore {
                source_counter,
                source,
            };
            let replace = match best_write_by_target.get(&target) {
                Some((existing_score, _)) => {
                    score.source_counter > existing_score.source_counter
                        || (score.source_counter == existing_score.source_counter && score.source.0 < existing_score.source.0)
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
        let lookup = self.resolve_user_context_symbol(consumer, symbol, expected);
        match lookup {
            UserContextLookup::Resolved(resolution) => {
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

    fn resolve_reference_target_node(&self, reference: &NodeReference) -> Option<NodeId> {
        reference
            .cached_id()
            .filter(|node_id| self.nodes.contains(*node_id))
            .or_else(|| (!reference.uuid().is_nil()).then(|| self.node_id_by_uuid(reference.uuid())).flatten())
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

            let ParameterControlSpec::Proxy { target } = &snapshot.control.spec else {
                return false;
            };
            let Some(next) = self.resolve_control_target_param(target, param_snapshots) else {
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
        diagnostics: &mut Vec<ParameterControlDiagnostic>,
    ) -> Option<ParamValue> {
        let converted = match &target_snapshot.value {
            ParamValue::Trigger() => {
                if matches!(source, ParamValue::Trigger()) {
                    Some(ParamValue::Trigger())
                } else {
                    None
                }
            }
            ParamValue::Int(_) => source.as_int().map(ParamValue::Int),
            ParamValue::Float(_) => source.as_float().map(ParamValue::Float),
            ParamValue::Str(_) => source.as_str().map(ParamValue::Str),
            ParamValue::File(_) => source.as_str().map(ParamValue::File),
            ParamValue::Enum(_) => source.as_enum().map(ParamValue::Enum),
            ParamValue::Bool(_) => source.as_bool().map(ParamValue::Bool),
            ParamValue::Vec2(_, _) => source.as_vec2().map(|(x, y)| ParamValue::Vec2(x, y)),
            ParamValue::Vec3(_, _, _) => source.as_vec3().map(|(x, y, z)| ParamValue::Vec3(x, y, z)),
            ParamValue::Color(_, _, _, _) => source.as_color().map(|(r, g, b, a)| ParamValue::Color(r, g, b, a)),
            ParamValue::Reference(_) => match source {
                ParamValue::Reference(reference) => Some(ParamValue::Reference(reference.clone())),
                _ => None,
            },
        };

        let Some(converted) = converted else {
            diagnostics.push(ParameterControlDiagnostic::new(
                "control_type_incompatible",
                format!(
                    "control mode '{}' cannot convert value {:?} for target {:?}",
                    mode, source, target_snapshot.value
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

fn control_spec_matches_mode(mode: ParameterControlMode, spec: &ParameterControlSpec) -> bool {
    matches!(
        (mode, spec),
        (ParameterControlMode::Manual, ParameterControlSpec::Manual)
            | (ParameterControlMode::ContextLink, ParameterControlSpec::ContextLink { .. })
            | (ParameterControlMode::TemplateText, ParameterControlSpec::TemplateText { .. })
            | (ParameterControlMode::Expression, ParameterControlSpec::Expression { .. })
            | (ParameterControlMode::Proxy, ParameterControlSpec::Proxy { .. })
            | (ParameterControlMode::Binding, ParameterControlSpec::Binding { .. })
            | (ParameterControlMode::Animation, ParameterControlSpec::Animation { .. })
    )
}

fn evaluate_expression_numeric<F>(source: &str, mut resolve_identifier: F) -> Result<f64, String>
where
    F: FnMut(&str) -> Result<f64, String>,
{
    let tokens = lex_expression(source)?;
    let mut parser = ExpressionParser::new(tokens, &mut resolve_identifier);
    let value = parser.parse_expression()?;
    if parser.has_remaining_tokens() {
        return Err("unexpected trailing tokens in expression".to_string());
    }
    Ok(value)
}

#[derive(Clone, Debug, PartialEq)]
enum ExpressionToken {
    Number(f64),
    Identifier(String),
    Plus,
    Minus,
    Star,
    Slash,
    LParen,
    RParen,
}

fn lex_expression(source: &str) -> Result<Vec<ExpressionToken>, String> {
    let chars = source.chars().collect::<Vec<_>>();
    let mut out = Vec::<ExpressionToken>::new();
    let mut index = 0usize;

    while index < chars.len() {
        let ch = chars[index];
        if ch.is_whitespace() {
            index += 1;
            continue;
        }

        if ch.is_ascii_digit() || ch == '.' {
            let start = index;
            index += 1;
            while index < chars.len() && (chars[index].is_ascii_digit() || chars[index] == '.') {
                index += 1;
            }
            let number_text = chars[start..index].iter().collect::<String>();
            let number = number_text
                .parse::<f64>()
                .map_err(|_| format!("invalid number literal '{number_text}'"))?;
            out.push(ExpressionToken::Number(number));
            continue;
        }

        if ch.is_ascii_alphabetic() || ch == '_' {
            let start = index;
            index += 1;
            while index < chars.len() && (chars[index].is_ascii_alphanumeric() || chars[index] == '_') {
                index += 1;
            }
            let ident = chars[start..index].iter().collect::<String>();
            out.push(ExpressionToken::Identifier(ident));
            continue;
        }

        let token = match ch {
            '+' => ExpressionToken::Plus,
            '-' => ExpressionToken::Minus,
            '*' => ExpressionToken::Star,
            '/' => ExpressionToken::Slash,
            '(' => ExpressionToken::LParen,
            ')' => ExpressionToken::RParen,
            _ => return Err(format!("unsupported expression character '{ch}'")),
        };
        out.push(token);
        index += 1;
    }

    Ok(out)
}

struct ExpressionParser<'a, F>
where
    F: FnMut(&str) -> Result<f64, String>,
{
    tokens: Vec<ExpressionToken>,
    index: usize,
    resolve_identifier: &'a mut F,
}

impl<'a, F> ExpressionParser<'a, F>
where
    F: FnMut(&str) -> Result<f64, String>,
{
    fn new(tokens: Vec<ExpressionToken>, resolve_identifier: &'a mut F) -> Self {
        Self {
            tokens,
            index: 0,
            resolve_identifier,
        }
    }

    fn has_remaining_tokens(&self) -> bool {
        self.index < self.tokens.len()
    }

    fn parse_expression(&mut self) -> Result<f64, String> {
        let mut lhs = self.parse_term()?;

        loop {
            match self.peek() {
                Some(ExpressionToken::Plus) => {
                    self.index += 1;
                    lhs += self.parse_term()?;
                }
                Some(ExpressionToken::Minus) => {
                    self.index += 1;
                    lhs -= self.parse_term()?;
                }
                _ => break,
            }
        }

        Ok(lhs)
    }

    fn parse_term(&mut self) -> Result<f64, String> {
        let mut lhs = self.parse_unary()?;

        loop {
            match self.peek() {
                Some(ExpressionToken::Star) => {
                    self.index += 1;
                    lhs *= self.parse_unary()?;
                }
                Some(ExpressionToken::Slash) => {
                    self.index += 1;
                    let rhs = self.parse_unary()?;
                    lhs /= rhs;
                }
                _ => break,
            }
        }

        Ok(lhs)
    }

    fn parse_unary(&mut self) -> Result<f64, String> {
        match self.peek() {
            Some(ExpressionToken::Plus) => {
                self.index += 1;
                self.parse_unary()
            }
            Some(ExpressionToken::Minus) => {
                self.index += 1;
                self.parse_unary().map(|value| -value)
            }
            _ => self.parse_primary(),
        }
    }

    fn parse_primary(&mut self) -> Result<f64, String> {
        let Some(token) = self.peek().cloned() else {
            return Err("unexpected end of expression".to_string());
        };

        match token {
            ExpressionToken::Number(number) => {
                self.index += 1;
                Ok(number)
            }
            ExpressionToken::Identifier(identifier) => {
                self.index += 1;
                (self.resolve_identifier)(identifier.as_str())
            }
            ExpressionToken::LParen => {
                self.index += 1;
                let value = self.parse_expression()?;
                match self.peek() {
                    Some(ExpressionToken::RParen) => {
                        self.index += 1;
                        Ok(value)
                    }
                    _ => Err("missing ')' in expression".to_string()),
                }
            }
            _ => Err("expected number, identifier, or '('".to_string()),
        }
    }

    fn peek(&self) -> Option<&ExpressionToken> {
        self.tokens.get(self.index)
    }
}
