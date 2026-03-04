use std::collections::{HashMap, HashSet};

use crate::contexts::{UserContextLookup, UserContextValueType};
use crate::edit::Edit;
use crate::events::EventKind;
use crate::node::{Node, NodeId, NodeReference, PARAMETER_ANIMATION_CONTROL_NODE_TYPE, PARAMETER_LINK_CONTROL_NODE_TYPE, PARAMETER_LINK_TARGET_DECL_ID, PARAMETER_LINK_TWO_WAY_DECL_ID, ParameterAnimationControlNode, ParameterLinkControlNode};
use crate::parameter::{ParamValue, ParameterControlDiagnostic, ParameterControlMode, ParameterControlSpec, ParameterControlState, ParameterEventBehaviour, ParameterSnapshot, available_control_modes_for_parameter};

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

#[derive(Clone, Debug)]
struct LinkControlConfig {
    target: NodeReference,
    two_way: bool,
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
        let param_snapshots = self.nodes.iter().filter_map(|(node_id, node)| node.engine_param_snapshot().map(|snapshot| (node_id, snapshot))).collect::<HashMap<_, _>>();

        if param_snapshots.is_empty() {
            return false;
        }

        let mut diagnostics_by_param = HashMap::<NodeId, Vec<ParameterControlDiagnostic>>::new();
        let mut two_way_links = HashMap::<NodeId, NodeId>::new();
        let mut queued_writes = Vec::<(NodeId, ParamValue)>::new();

        for (param, snapshot) in &param_snapshots {
            let control = &snapshot.control;
            let mut diagnostics = Vec::<ParameterControlDiagnostic>::new();

            let maybe_value = match control.mode {
                ParameterControlMode::Manual => None,
                ParameterControlMode::ContextLink => match &control.spec {
                    ParameterControlSpec::ContextLink { symbol } => self.evaluate_context_link(*param, snapshot, symbol.as_str(), &param_snapshots, &mut diagnostics),
                    _ => {
                        diagnostics.push(ParameterControlDiagnostic::new("invalid_control_spec", "control mode/context-link mismatch"));
                        None
                    }
                },
                ParameterControlMode::TemplateText => match &control.spec {
                    ParameterControlSpec::TemplateText { template } => self.evaluate_template_text(*param, snapshot, template.as_str(), &param_snapshots, &mut diagnostics),
                    _ => {
                        diagnostics.push(ParameterControlDiagnostic::new("invalid_control_spec", "control mode/template-text mismatch"));
                        None
                    }
                },
                ParameterControlMode::Expression => match &control.spec {
                    ParameterControlSpec::Expression { expression } => self.evaluate_expression(*param, snapshot, expression.as_str(), &param_snapshots, &mut diagnostics),
                    _ => {
                        diagnostics.push(ParameterControlDiagnostic::new("invalid_control_spec", "control mode/expression mismatch"));
                        None
                    }
                },
                ParameterControlMode::Link => match &control.spec {
                    ParameterControlSpec::Link => {
                        if let Some(config) = self.read_link_control_config(*param, &mut diagnostics) {
                            if let Some(target_param) = self.resolve_control_target_param(&config.target, &param_snapshots) {
                                if config.two_way {
                                    two_way_links.insert(*param, target_param);
                                    None
                                } else {
                                    self.evaluate_link_one_way(*param, snapshot, target_param, &param_snapshots, &mut diagnostics)
                                }
                            } else {
                                diagnostics.push(ParameterControlDiagnostic::new("link_target_missing", "link target parameter could not be resolved"));
                                None
                            }
                        } else {
                            None
                        }
                    }
                    _ => {
                        diagnostics.push(ParameterControlDiagnostic::new("invalid_control_spec", "control mode/link mismatch"));
                        None
                    }
                },
                ParameterControlMode::Animation => match &control.spec {
                    ParameterControlSpec::Animation => None,
                    _ => {
                        diagnostics.push(ParameterControlDiagnostic::new("invalid_control_spec", "control mode/animation mismatch"));
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

        let binding_result = self.evaluate_bindings(&two_way_links, &param_snapshots);
        for (node, value) in binding_result.pending_writes {
            if param_snapshots.get(&node).is_some_and(|snapshot| snapshot.value != value) {
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
        let Some(snapshot) = node.engine_param_snapshot() else {
            return Err(format!("node {} is not a parameter node", param.0));
        };
        if !available_control_modes_for_parameter(&snapshot.value, snapshot.control_modes_enabled).contains(&state.mode) {
            return Err(format!("control mode '{:?}' is not supported for parameter type '{}'", state.mode, node.get_type()));
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
        self.emit_event(EventKind::ParamControlChanged { param, old_state: current_state, new_state: next_state });
        Ok(true)
    }

    fn sync_parameter_control_nodes(&mut self, param: NodeId, mode: ParameterControlMode) -> Result<(), String> {
        let children = self.direct_children(param);
        let mut link_nodes = Vec::<NodeId>::new();
        let mut animation_nodes = Vec::<NodeId>::new();

        for child in children {
            let Some(child_node) = self.nodes.get(child) else {
                continue;
            };

            match child_node.get_type() {
                PARAMETER_LINK_CONTROL_NODE_TYPE => link_nodes.push(child),
                PARAMETER_ANIMATION_CONTROL_NODE_TYPE => animation_nodes.push(child),
                _ => {}
            }
        }

        let mut changed = false;
        let mut to_remove = Vec::<NodeId>::new();

        match mode {
            ParameterControlMode::Link => {
                if link_nodes.is_empty() {
                    self.edits.push(Edit::AddNode {
                        parent: param,
                        node: Box::new(ParameterLinkControlNode::new("Link")),
                        prev_sibling: None,
                    });
                    changed = true;
                } else {
                    to_remove.extend(link_nodes.into_iter().skip(1));
                }
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
                to_remove.extend(link_nodes);
            }
            _ => {
                to_remove.extend(link_nodes);
                to_remove.extend(animation_nodes);
            }
        }

        for node in to_remove {
            self.edits.push(Edit::RemoveNode { node });
            changed = true;
        }

        if changed {
            self.apply_edits_without_history().map_err(|err| format!("failed to sync control nodes for parameter {}: {err}", param.0))?;
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

    fn find_direct_child_by_type(&self, parent: NodeId, node_type: &str) -> Option<NodeId> {
        self.direct_children(parent).into_iter().find(|child| self.nodes.get(*child).is_some_and(|node| node.get_type() == node_type))
    }

    fn find_direct_child_by_decl_id(&self, parent: NodeId, decl_id: &str) -> Option<NodeId> {
        self.direct_children(parent).into_iter().find(|child| self.nodes.get(*child).is_some_and(|node| node.node_data().meta.decl_id.0 == decl_id))
    }

    fn read_parameter_value(&self, param: NodeId) -> Option<ParamValue> {
        self.nodes.get(param).and_then(|node| node.engine_param_snapshot()).map(|snapshot| snapshot.value)
    }

    fn read_link_control_config(&self, param: NodeId, diagnostics: &mut Vec<ParameterControlDiagnostic>) -> Option<LinkControlConfig> {
        let Some(link_node) = self.find_direct_child_by_type(param, PARAMETER_LINK_CONTROL_NODE_TYPE) else {
            diagnostics.push(ParameterControlDiagnostic::new("link_control_missing", "link control node is missing"));
            return None;
        };

        let Some(target_param) = self.find_direct_child_by_decl_id(link_node, PARAMETER_LINK_TARGET_DECL_ID) else {
            diagnostics.push(ParameterControlDiagnostic::new("link_target_param_missing", "link target parameter is missing"));
            return None;
        };

        let Some(target_value) = self.read_parameter_value(target_param) else {
            diagnostics.push(ParameterControlDiagnostic::new("link_target_param_invalid", "link target node is not a parameter"));
            return None;
        };

        let ParamValue::Reference(target) = target_value else {
            diagnostics.push(ParameterControlDiagnostic::new("link_target_param_type_mismatch", "link target parameter must be a reference parameter"));
            return None;
        };

        let mut two_way = false;
        if let Some(two_way_param) = self.find_direct_child_by_decl_id(link_node, PARAMETER_LINK_TWO_WAY_DECL_ID) {
            let Some(two_way_value) = self.read_parameter_value(two_way_param) else {
                diagnostics.push(ParameterControlDiagnostic::new("link_two_way_param_invalid", "link two-way node is not a parameter"));
                return None;
            };
            let Some(parsed) = two_way_value.as_bool() else {
                diagnostics.push(ParameterControlDiagnostic::new("link_two_way_param_type_mismatch", "link two-way parameter must be boolean-compatible"));
                return None;
            };
            two_way = parsed;
        }

        Some(LinkControlConfig { target, two_way })
    }

    fn evaluate_context_link(&mut self, consumer: NodeId, target_snapshot: &ParameterSnapshot, symbol: &str, param_snapshots: &HashMap<NodeId, ParameterSnapshot>, diagnostics: &mut Vec<ParameterControlDiagnostic>) -> Option<ParamValue> {
        let source = self.resolve_context_symbol_value(consumer, symbol, None, param_snapshots, diagnostics)?;
        self.convert_for_target(&source, target_snapshot, "context_link", diagnostics)
    }

    fn evaluate_template_text(&mut self, consumer: NodeId, target_snapshot: &ParameterSnapshot, template: &str, param_snapshots: &HashMap<NodeId, ParameterSnapshot>, diagnostics: &mut Vec<ParameterControlDiagnostic>) -> Option<ParamValue> {
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

    fn evaluate_expression(&mut self, consumer: NodeId, target_snapshot: &ParameterSnapshot, expression: &str, param_snapshots: &HashMap<NodeId, ParameterSnapshot>, diagnostics: &mut Vec<ParameterControlDiagnostic>) -> Option<ParamValue> {
        let result = evaluate_expression_numeric(expression, |identifier| {
            let lookup = self.resolve_user_context_symbol(consumer, identifier, None);
            let source = match lookup {
                UserContextLookup::Resolved(resolution) => param_snapshots.get(&resolution.entry_param).map(|snapshot| snapshot.value.clone()).ok_or_else(|| format!("resolved symbol '{identifier}' references missing parameter node")),
                UserContextLookup::TypeMismatch(_) => Err(format!("symbol '{identifier}' type mismatch")),
                UserContextLookup::Missing { .. } => Err(format!("symbol '{identifier}' was not found")),
            }?;
            source.as_float().ok_or_else(|| format!("symbol '{identifier}' cannot be coerced to numeric input"))
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

    fn evaluate_link_one_way(&mut self, param: NodeId, target_snapshot: &ParameterSnapshot, target_param: NodeId, param_snapshots: &HashMap<NodeId, ParameterSnapshot>, diagnostics: &mut Vec<ParameterControlDiagnostic>) -> Option<ParamValue> {
        if target_param == param {
            diagnostics.push(ParameterControlDiagnostic::new("link_cycle", "one-way link target cannot reference the same parameter"));
            return None;
        }

        if self.link_chain_contains(param, target_param, param_snapshots) {
            diagnostics.push(ParameterControlDiagnostic::new("link_cycle", "one-way link chain contains a cycle"));
            return None;
        }

        let source = match param_snapshots.get(&target_param) {
            Some(snapshot) => snapshot.value.clone(),
            None => {
                diagnostics.push(ParameterControlDiagnostic::new("link_target_not_parameter", "one-way link target is not a parameter node"));
                return None;
            }
        };

        self.convert_for_target(&source, target_snapshot, "link", diagnostics)
    }

    fn evaluate_bindings(&self, two_way_links: &HashMap<NodeId, NodeId>, param_snapshots: &HashMap<NodeId, ParameterSnapshot>) -> BindingEvaluation {
        let mut diagnostics_by_param = HashMap::<NodeId, Vec<ParameterControlDiagnostic>>::new();
        if two_way_links.is_empty() {
            return BindingEvaluation { pending_writes: Vec::new(), diagnostics_by_param };
        }

        let mut pairs = HashSet::<(NodeId, NodeId)>::new();

        for (param, target_param) in two_way_links {
            let param = *param;
            let target_param = *target_param;
            let mut diagnostics = Vec::<ParameterControlDiagnostic>::new();
            let Some(_) = param_snapshots.get(&param) else {
                diagnostics.push(ParameterControlDiagnostic::new("link_param_missing", "two-way link parameter snapshot is missing"));
                diagnostics_by_param.insert(param, diagnostics);
                continue;
            };

            if !param_snapshots.contains_key(&target_param) {
                diagnostics.push(ParameterControlDiagnostic::new("link_target_missing", "two-way link target parameter could not be resolved"));
                diagnostics_by_param.insert(param, diagnostics);
                continue;
            }

            if target_param == param {
                diagnostics.push(ParameterControlDiagnostic::new("link_cycle", "two-way link target cannot reference the same parameter"));
                diagnostics_by_param.insert(param, diagnostics);
                continue;
            }

            let key = if param.0 <= target_param.0 { (param, target_param) } else { (target_param, param) };
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

            let (source, source_snapshot, source_counter, target, target_snapshot) = if a_counter > b_counter || (a_counter == b_counter && a.0 <= b.0) {
                (a, a_snapshot, a_counter, b, b_snapshot)
            } else {
                (b, b_snapshot, b_counter, a, a_snapshot)
            };

            let mut target_diagnostics = Vec::<ParameterControlDiagnostic>::new();
            let Some(converted) = self.convert_for_target(&source_snapshot.value, target_snapshot, "link_two_way", &mut target_diagnostics) else {
                if !target_diagnostics.is_empty() {
                    diagnostics_by_param.entry(target).or_default().extend(target_diagnostics);
                }
                continue;
            };

            if converted == target_snapshot.value {
                continue;
            }

            let score = BindingWriteScore { source_counter, source };
            let replace = match best_write_by_target.get(&target) {
                Some((existing_score, _)) => score.source_counter > existing_score.source_counter || (score.source_counter == existing_score.source_counter && score.source.0 < existing_score.source.0),
                None => true,
            };
            if replace {
                best_write_by_target.insert(target, (score, converted));
            }
        }

        let mut pending_writes = best_write_by_target.into_iter().map(|(target, (_, value))| (target, value)).collect::<Vec<_>>();
        pending_writes.sort_by_key(|(node, _)| node.0);

        BindingEvaluation { pending_writes, diagnostics_by_param }
    }

    fn resolve_template_token(&mut self, consumer: NodeId, token: &str, param_snapshots: &HashMap<NodeId, ParameterSnapshot>, diagnostics: &mut Vec<ParameterControlDiagnostic>) -> Option<String> {
        let token = token.trim();
        if token.is_empty() {
            diagnostics.push(ParameterControlDiagnostic::new("template_token_empty", "template token cannot be empty"));
            return None;
        }

        if let Some(stripped) = token.strip_prefix('$') {
            return self.node_metadata_value(consumer, stripped).or_else(|| {
                diagnostics.push(ParameterControlDiagnostic::new("template_meta_field_unknown", format!("unknown metadata field '${stripped}'")));
                None
            });
        }

        if let Some((symbol, field)) = token.split_once(".$") {
            let source = self.resolve_context_symbol_value(consumer, symbol, None, param_snapshots, diagnostics)?;
            let ParamValue::Reference(reference) = source else {
                diagnostics.push(ParameterControlDiagnostic::new("template_meta_requires_reference", format!("symbol '{}' does not resolve to a reference parameter", symbol.trim())));
                return None;
            };
            let Some(target_node) = self.resolve_reference_target_node(&reference) else {
                diagnostics.push(ParameterControlDiagnostic::new("template_meta_target_missing", format!("symbol '{}' references a missing node", symbol.trim())));
                return None;
            };

            return self.node_metadata_value(target_node, field).or_else(|| {
                diagnostics.push(ParameterControlDiagnostic::new("template_meta_field_unknown", format!("unknown metadata field '${field}'")));
                None
            });
        }

        let source = self.resolve_context_symbol_value(consumer, token, None, param_snapshots, diagnostics)?;
        source.as_str().or_else(|| Some(source.to_string()))
    }

    fn resolve_context_symbol_value(&mut self, consumer: NodeId, symbol: &str, expected: Option<UserContextValueType>, param_snapshots: &HashMap<NodeId, ParameterSnapshot>, diagnostics: &mut Vec<ParameterControlDiagnostic>) -> Option<ParamValue> {
        let lookup = self.resolve_user_context_symbol(consumer, symbol, expected);
        match lookup {
            UserContextLookup::Resolved(resolution) => param_snapshots.get(&resolution.entry_param).map(|snapshot| snapshot.value.clone()).or_else(|| {
                diagnostics.push(ParameterControlDiagnostic::new("context_target_missing", format!("symbol '{}' resolved to missing parameter node {}", resolution.symbol, resolution.entry_param.0)));
                None
            }),
            UserContextLookup::TypeMismatch(mismatch) => {
                diagnostics
                    .push(ParameterControlDiagnostic::new("context_type_mismatch", format!("symbol '{}' resolved to {:?} but expected {:?}", mismatch.symbol, mismatch.found, mismatch.expected)).with_detail(format!("scope_owner={}, lexical_depth={}", mismatch.scope_owner.0, mismatch.lexical_depth)));
                None
            }
            UserContextLookup::Missing { symbol } => {
                diagnostics.push(ParameterControlDiagnostic::new("context_symbol_missing", format!("context symbol '{symbol}' was not found")));
                None
            }
        }
    }

    fn resolve_reference_target_node(&self, reference: &NodeReference) -> Option<NodeId> {
        reference.cached_id().filter(|node_id| self.nodes.contains(*node_id)).or_else(|| (!reference.uuid().is_nil()).then(|| self.node_id_by_uuid(reference.uuid())).flatten())
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

    fn resolve_control_target_param(&self, target: &NodeReference, param_snapshots: &HashMap<NodeId, ParameterSnapshot>) -> Option<NodeId> {
        let target_node = self.resolve_reference_target_node(target)?;
        param_snapshots.contains_key(&target_node).then_some(target_node)
    }

    fn link_chain_contains(&self, needle: NodeId, mut start: NodeId, param_snapshots: &HashMap<NodeId, ParameterSnapshot>) -> bool {
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
            if snapshot.control.mode != ParameterControlMode::Link {
                return false;
            }

            let ParameterControlSpec::Link = &snapshot.control.spec else {
                return false;
            };

            let mut diagnostics = Vec::<ParameterControlDiagnostic>::new();
            let Some(config) = self.read_link_control_config(start, &mut diagnostics) else {
                return false;
            };
            if config.two_way {
                return false;
            }

            let Some(next) = self.resolve_control_target_param(&config.target, param_snapshots) else {
                return false;
            };
            start = next;
        }
    }

    fn convert_for_target(&self, source: &ParamValue, target_snapshot: &ParameterSnapshot, mode: &str, diagnostics: &mut Vec<ParameterControlDiagnostic>) -> Option<ParamValue> {
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
            diagnostics.push(ParameterControlDiagnostic::new("control_type_incompatible", format!("control mode '{}' cannot convert value {:?} for target {:?}", mode, source, target_snapshot.value)));
            return None;
        };

        match target_snapshot.constraints.normalize(converted) {
            Ok(normalized) => Some(normalized),
            Err(message) => {
                diagnostics.push(ParameterControlDiagnostic::new("control_constraints_violation", format!("control mode '{}' produced an invalid value: {message}", mode)));
                None
            }
        }
    }

    fn apply_parameter_control_diagnostics(&mut self, param_snapshots: &HashMap<NodeId, ParameterSnapshot>, mut diagnostics_by_param: HashMap<NodeId, Vec<ParameterControlDiagnostic>>) -> bool {
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
                self.emit_event(EventKind::ParamControlChanged { param, old_state, new_state });
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
            | (ParameterControlMode::Link, ParameterControlSpec::Link)
            | (ParameterControlMode::Animation, ParameterControlSpec::Animation)
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
            let number = number_text.parse::<f64>().map_err(|_| format!("invalid number literal '{number_text}'"))?;
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
        Self { tokens, index: 0, resolve_identifier }
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
