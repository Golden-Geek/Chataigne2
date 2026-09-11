use std::collections::{HashMap, HashSet};

use crate::contexts::UserContextValueType;
use crate::edit::{Edit, EditOrigin};
use crate::engine::{Engine, EngineTime, ProjectLoadRecoveryReport, ProjectPersistenceError};
use crate::events::{Event, EventKind};
use crate::node::{
    CurveNode, DASHBOARD_GENERIC_WIDGET_NODE_TYPE, DASHBOARD_NODE_WIDGET_NODE_TYPE, DASHBOARD_PAGE_NODE_TYPE,
    DASHBOARD_WIDGET_CONTAINER_NODE_TYPE, DeclId, FOLDER_NODE_TYPE, Node, NodeCreationContext, NodeId, NodeMeta,
    NodeMetaPatch as EngineNodeMetaPatch, NodeReference, NodeUuid, UserCreatableItem, UserCreatableItemInitialParam,
};
use crate::parameter::{
    ParamValue, ParameterConstraints, ParameterControlMode, ParameterControlSpec, ParameterControlState,
    ParameterEnumOption, ParameterEventBehaviour, RangeConstraint, available_control_modes_for_parameter,
    compatibility_for_binding_values, compatibility_for_values,
};
use crate::process_ctx::ProcessTreeSnapshot;
use crate::script::{ScriptNodeConfig, ScriptUiConfig, ScriptUiState};

pub use golden_protocol::*;

pub(crate) const UI_USER_CONTEXT_SCOPE_TOPIC: &str = "__user_context.scope_changed";
pub(crate) const UI_USER_CONTEXT_ENTRY_TOPIC: &str = "__user_context.entry_changed";

fn duplicate_unique_label_base(label: &str) -> (&str, u64) {
    let Some((base, suffix)) = label.rsplit_once(' ') else {
        return (label, 2);
    };
    if base.trim().is_empty() {
        return (label, 2);
    }
    match suffix.parse::<u64>() {
        Ok(suffix) if suffix >= 2 => (base, suffix.saturating_add(1)),
        _ => (label, 2),
    }
}

impl From<&ProjectLoadRecoveryReport> for UiProjectLoadRecoveryDto {
    fn from(report: &ProjectLoadRecoveryReport) -> Self {
        Self {
            problems: report
                .problems
                .iter()
                .map(|problem| UiProjectLoadProblemDto {
                    stage: problem.stage.as_str().to_string(),
                    message: problem.message.clone(),
                })
                .collect(),
        }
    }
}

impl From<ProjectLoadRecoveryReport> for UiProjectLoadRecoveryDto {
    fn from(report: ProjectLoadRecoveryReport) -> Self {
        Self::from(&report)
    }
}

impl From<UserCreatableItem> for UiCreatableUserItemDto {
    fn from(item: UserCreatableItem) -> Self {
        Self {
            node_type: item.node_type,
            item_kind: item.item_kind,
            label: item.label,
            menu_path: item.menu_path,
            initial_params: item.initial_params.into_iter().map(Into::into).collect(),
            select_when_created: item.select_when_created,
            separator_before: item.separator_before,
            icon: item.icon,
        }
    }
}

impl From<UserCreatableItemInitialParam> for UiCreateUserItemInitialParam {
    fn from(initial_param: UserCreatableItemInitialParam) -> Self {
        Self {
            decl_id: initial_param.decl_id,
            value: initial_param.value,
        }
    }
}

impl From<&EngineNodeMetaPatch> for UiNodeMetaPatch {
    fn from(patch: &EngineNodeMetaPatch) -> Self {
        Self {
            label: patch.label.clone(),
            short_name: patch.short_name.clone(),
            enabled: patch.enabled,
            can_be_disabled: patch.can_be_disabled,
            description: patch.description.clone(),
            user_permissions: patch.user_permissions.clone(),
            tags: patch.tags.clone(),
            presentation: patch.presentation.clone(),
        }
    }
}

impl From<EngineNodeMetaPatch> for UiNodeMetaPatch {
    fn from(patch: EngineNodeMetaPatch) -> Self {
        Self::from(&patch)
    }
}

impl From<UiNodeMetaPatch> for EngineNodeMetaPatch {
    fn from(patch: UiNodeMetaPatch) -> Self {
        Self {
            short_name: patch.short_name,
            enabled: patch.enabled,
            can_be_disabled: patch.can_be_disabled,
            label: patch.label,
            description: patch.description,
            tags: patch.tags,
            user_permissions: patch.user_permissions,
            semantics: None,
            presentation: patch.presentation,
        }
    }
}

impl From<&EngineNodeMetaPatch> for NodeMetaPatch {
    fn from(patch: &EngineNodeMetaPatch) -> Self {
        Self {
            short_name: patch.short_name.clone(),
            enabled: patch.enabled,
            can_be_disabled: patch.can_be_disabled,
            label: patch.label.clone(),
            description: patch.description.clone(),
            tags: patch.tags.clone(),
            user_permissions: patch.user_permissions.clone(),
            semantics: patch.semantics.as_ref().map(|semantics| SemanticsHint {
                intent: semantics.intent.clone(),
                unit: semantics.unit.clone(),
            }),
            presentation: patch.presentation.clone(),
        }
    }
}

impl From<NodeMetaPatch> for EngineNodeMetaPatch {
    fn from(patch: NodeMetaPatch) -> Self {
        Self {
            short_name: patch.short_name,
            enabled: patch.enabled,
            can_be_disabled: patch.can_be_disabled,
            label: patch.label,
            description: patch.description,
            tags: patch.tags,
            user_permissions: patch.user_permissions,
            semantics: patch.semantics.map(|semantics| crate::node::SemanticsHint {
                intent: semantics.intent,
                unit: semantics.unit,
            }),
            presentation: patch.presentation,
        }
    }
}

impl From<EngineNodeMetaPatch> for NodeMetaPatch {
    fn from(patch: EngineNodeMetaPatch) -> Self {
        Self::from(&patch)
    }
}

impl From<Event> for UiEventDto {
    fn from(event: Event) -> Self {
        let kind = match event.kind {
            EventKind::ParamChanged {
                param,
                old_value,
                new_value,
            } => UiEventKind::ParamChanged {
                param,
                old_value,
                new_value,
            },
            EventKind::ParamControlChanged {
                param,
                old_state,
                new_state,
            } => UiEventKind::ParamControlChanged {
                param,
                old_state: old_state.into(),
                new_state: new_state.into(),
            },
            EventKind::ParamConstraintsChanged {
                param,
                old_constraints,
                new_constraints,
            } => UiEventKind::ParamConstraintsChanged {
                param,
                old_constraints,
                new_constraints,
            },
            EventKind::ChildAdded { parent, child, decl_id } => UiEventKind::ChildAdded {
                parent,
                child,
                decl_id,
                parent_children: None,
            },
            EventKind::ChildRemoved { parent, child } => UiEventKind::ChildRemoved { parent, child },
            EventKind::ChildReplaced {
                parent,
                old,
                new,
                decl_id,
            } => UiEventKind::ChildReplaced {
                parent,
                old,
                new,
                decl_id,
            },
            EventKind::ChildMoved {
                child,
                old_parent,
                new_parent,
            } => UiEventKind::ChildMoved {
                child,
                old_parent,
                new_parent,
                old_parent_children: None,
                new_parent_children: None,
            },
            EventKind::ChildReordered { parent, child } => UiEventKind::ChildReordered {
                parent,
                child,
                parent_children: None,
            },
            EventKind::NodeCreated { node } => UiEventKind::NodeCreated { node, snapshot: None },
            EventKind::NodeDeleted { node } => UiEventKind::NodeDeleted { node },
            EventKind::MetaChanged { node, patch } => UiEventKind::MetaChanged {
                node,
                patch: NodeMetaPatch::from(&patch),
            },
            EventKind::GraphTransaction { transaction } => UiEventKind::GraphTransaction { transaction },
            EventKind::Custom(custom) => UiEventKind::Custom {
                topic: custom.topic,
                origin: custom.origin,
                payload: std::sync::Arc::unwrap_or_clone(custom.payload),
                retention: custom.retention,
            },
        };

        Self { time: event.time, kind }
    }
}

impl<T: Node> Engine<T> {
    fn ui_enum_options_fingerprint(options: &[ParameterEnumOption]) -> String {
        serde_json::to_string(options).unwrap_or_default()
    }

    fn ui_enum_definition_from_options(enum_id: String, options: Vec<ParameterEnumOption>) -> UiEnumDefinition {
        UiEnumDefinition {
            enum_id,
            variants: options
                .into_iter()
                .map(|option| UiEnumVariantDefinition {
                    variant_id: option.variant_id,
                    value: option.value,
                    label: option.label,
                    tags: option.tags,
                    ordering: option.ordering,
                })
                .collect(),
        }
    }

    /// Builds a UI snapshot for the requested scope.
    pub fn ui_snapshot(&self, scope: UiSubscriptionScope) -> UiSnapshot {
        let node_ids = self.collect_scope_nodes(scope.clone());
        let visible: HashSet<NodeId> = node_ids.iter().copied().collect();
        let catalog_snapshot = self.build_process_tree_snapshot();
        let mut nodes = Vec::with_capacity(node_ids.len());
        let mut known_node_types = HashMap::<String, Option<String>>::new();
        let mut known_declared_descriptions = HashMap::<String, String>::new();
        let mut enum_id_by_fingerprint = HashMap::<String, String>::new();
        let mut enums = Vec::<UiEnumDefinition>::new();

        for node_id in node_ids {
            let Some(node) = self.nodes.get(node_id) else {
                continue;
            };
            let node_data = node.node_data();
            let node_type = node.get_type().to_string();
            let type_description = node.type_description().map(str::to_string);
            known_node_types
                .entry(node_type.clone())
                .and_modify(|existing| {
                    if existing.is_none() && type_description.is_some() {
                        *existing = type_description.clone();
                    }
                })
                .or_insert(type_description);

            let mut children = Vec::new();
            let mut child = node_data.first_child;
            let mut seen_children = HashSet::<NodeId>::new();
            while let Some(child_id) = child {
                if !seen_children.insert(child_id) {
                    break;
                }
                if visible.contains(&child_id) {
                    children.push(child_id);
                }
                child = self.nodes.get(child_id).and_then(|next| next.node_data().next_sibling);
            }

            let data = if let Some(mut param) = node.engine_param_snapshot() {
                let enum_options_id = if param.constraints.enum_options.is_empty() {
                    None
                } else {
                    let enum_options = std::mem::take(&mut param.constraints.enum_options);
                    let fingerprint = Self::ui_enum_options_fingerprint(enum_options.as_slice());
                    if let Some(existing_id) = enum_id_by_fingerprint.get(&fingerprint) {
                        Some(existing_id.clone())
                    } else {
                        let enum_id = format!("enumOptions{}", enums.len() + 1);
                        enum_id_by_fingerprint.insert(fingerprint, enum_id.clone());
                        enums.push(Self::ui_enum_definition_from_options(enum_id.clone(), enum_options));
                        Some(enum_id)
                    }
                };
                let mut dto = UiParamDto::from(param);
                dto.enum_options_id = enum_options_id;
                UiNodeDataDto::Parameter { param: Box::new(dto) }
            } else {
                UiNodeDataDto::Node {
                    node_type: node.get_type().to_string(),
                }
            };

            let listed_user_items = node.user_creatable_items();
            let can_query_creatable_items = node.get_type() == FOLDER_NODE_TYPE
                || node.user_container_rules().is_some()
                || !listed_user_items.is_empty();

            let mut creatable_user_items = Vec::new();
            if can_query_creatable_items {
                for item in self
                    .catalog_creatable_items_with_snapshot(node_id, catalog_snapshot.as_ref())
                    .into_iter()
                {
                    creatable_user_items.push(UiCreatableUserItemDto::from(item));
                }
            }

            let mut accepted_user_item_kinds: Vec<String> = node
                .user_container_rules()
                .map(|rules| {
                    rules
                        .accepts_item_kinds
                        .iter()
                        .map(|kind| (*kind).to_string())
                        .collect()
                })
                .unwrap_or_default();
            for item in &listed_user_items {
                if !accepted_user_item_kinds.iter().any(|kind| kind == &item.item_kind) {
                    accepted_user_item_kinds.push(item.item_kind.clone());
                }
            }

            let declared_description_key = node_data
                .meta
                .declared_description_key
                .as_deref()
                .filter(|key| !key.trim().is_empty());
            let declared_description = node_data
                .meta
                .declared_description
                .as_deref()
                .filter(|description| !description.trim().is_empty());
            let effective_description = node_data.meta.description.as_deref();

            let (description, declared_description_key, description_overridden) =
                if let (Some(key), Some(description)) = (declared_description_key, declared_description) {
                    known_declared_descriptions
                        .entry(key.to_string())
                        .or_insert_with(|| description.to_string());
                    let overridden = effective_description != Some(description);
                    (
                        if overridden {
                            effective_description.map(str::to_string)
                        } else {
                            None
                        },
                        Some(key.to_string()),
                        overridden,
                    )
                } else {
                    (
                        match effective_description {
                            Some(description) if Some(description) != node.type_description() => {
                                Some(description.to_string())
                            }
                            _ => None,
                        },
                        None,
                        false,
                    )
                };

            nodes.push(UiNodeDto {
                node_id,
                uuid: node_data.meta.uuid,
                decl_id: node_data.meta.decl_id.clone(),
                node_type: node.get_type().to_string(),
                meta: UiNodeMetaDto {
                    short_name: node_data.meta.short_name.clone(),
                    label: node_data.meta.label.clone(),
                    enabled: node_data.meta.enabled,
                    can_be_disabled: node_data.meta.can_be_disabled,
                    user_permissions: node_data.meta.user_permissions.clone(),
                    description,
                    declared_description_key,
                    description_overridden,
                    tags: node_data.meta.tags.clone(),
                    presentation: node_data.meta.presentation.clone(),
                },
                data,
                user_role: node_data.user_role,
                user_item_kind: node.user_item_kind().to_string(),
                accepted_user_item_kinds,
                creatable_user_items,
                children,
            });
        }

        let mut node_types: Vec<UiNodeTypeDescriptor> = known_node_types
            .into_iter()
            .map(|(node_type, description)| UiNodeTypeDescriptor { node_type, description })
            .collect();
        node_types.sort_by(|left, right| left.node_type.cmp(&right.node_type));
        let mut declared_descriptions: Vec<UiDeclaredDescriptionDescriptor> = known_declared_descriptions
            .into_iter()
            .map(|(key, description)| UiDeclaredDescriptionDescriptor { key, description })
            .collect();
        declared_descriptions.sort_by(|left, right| left.key.cmp(&right.key));

        UiSnapshot {
            protocol_version: UI_PROTOCOL_VERSION.to_string(),
            scope,
            at: self.time,
            nodes,
            schema: UiSchemaView {
                node_types,
                declared_descriptions,
                enums,
            },
            history: self.ui_history_state(),
            logger: UiLoggerState {
                max_entries: crate::logger::max_entries(),
                records: crate::logger::records(),
            },
            project_file: UiProjectFileSpec::default(),
            user_contexts: self.ui_user_contexts(),
        }
    }

    /// Returns a scoped UI event replay batch starting strictly after `from`.
    pub fn ui_event_batch(&self, from: Option<EngineTime>, scope: UiSubscriptionScope) -> UiEventBatch {
        let start_index = self.ui_event_log_start_index(from);
        let mut events = Vec::<UiEventDto>::new();
        let source_events = &self.ui_event_log()[start_index..];
        events.reserve(source_events.len());
        let mut child_order_cache = HashMap::<NodeId, Option<Vec<NodeId>>>::new();

        for event in source_events {
            if self.event_matches_scope(&scope, event) {
                events.push(self.ui_event_dto_with_child_order_cache(event, &mut child_order_cache));
            }
        }

        let to = source_events.last().map(|event| event.time);

        UiEventBatch {
            from,
            to,
            runtime: None,
            events,
        }
    }

    /// Resolves custom-filter reference picker targets for one parameter node.
    ///
    /// Returns empty vectors when the parameter cannot be resolved or is not a reference parameter.
    pub fn ui_reference_targets_for_param(&self, param_node: NodeId) -> UiReferenceTargetsDto {
        let Some(node) = self.nodes.get(param_node) else {
            return UiReferenceTargetsDto::default();
        };
        let Some(snapshot) = node.engine_param_snapshot() else {
            return UiReferenceTargetsDto::default();
        };
        if !matches!(snapshot.value, ParamValue::Reference(_)) {
            return UiReferenceTargetsDto::default();
        }

        let constraints = self.reference_constraints_for_param(param_node);
        let mut allowed_targets = self.reference_allowed_targets_for_param(param_node);
        allowed_targets.sort_by_key(|node_id| node_id.0);
        let expected_parameter_values = self.expected_reference_parameter_values(param_node, &constraints);

        let mut candidates = allowed_targets
            .iter()
            .map(|target| {
                if let Some(compatibility) = self.reference_candidate_compatibility_for_expected_values(
                    param_node,
                    *target,
                    expected_parameter_values.as_slice(),
                    &constraints,
                ) {
                    UiReferenceTargetCandidateDto {
                        target: *target,
                        direct: compatibility.direct,
                        projections: compatibility.projections,
                    }
                } else {
                    UiReferenceTargetCandidateDto {
                        target: *target,
                        direct: true,
                        projections: Vec::new(),
                    }
                }
            })
            .collect::<Vec<_>>();
        candidates.sort_by_key(|candidate| candidate.target.0);

        UiReferenceTargetsDto {
            allowed_targets,
            visible_nodes: self.reference_visible_nodes_for_param(param_node),
            candidates,
        }
    }

    /// Returns control-related UI information for one parameter node.
    pub fn ui_param_control_info(&self, param_node: NodeId) -> Result<UiParamControlInfoDto, String> {
        let Some(node) = self.nodes.get(param_node) else {
            return Err(format!("node {} not found", param_node.0));
        };
        let Some(snapshot) = node.engine_param_snapshot() else {
            return Err(format!("node {} is not a parameter node", param_node.0));
        };

        let context_candidates = self.ui_context_candidates_for_param(param_node).candidates;
        let mut available_modes =
            available_control_modes_for_parameter(&snapshot.value, snapshot.control_modes_enabled);
        if context_candidates.is_empty() {
            available_modes.retain(|mode| *mode != ParameterControlMode::TemplateText);
        }

        let mut token_set = HashSet::<String>::new();
        token_set.insert("$name".to_string());
        token_set.insert("$type".to_string());
        token_set.insert("$id".to_string());
        token_set.insert("$uuid".to_string());
        for candidate in &context_candidates {
            token_set.insert(candidate.symbol.clone());
            if candidate.value_type == UserContextValueType::Reference {
                token_set.insert(format!("{}.$name", candidate.symbol));
                token_set.insert(format!("{}.$type", candidate.symbol));
                token_set.insert(format!("{}.$id", candidate.symbol));
                token_set.insert(format!("{}.$uuid", candidate.symbol));
            }
        }
        let mut token_suggestions = token_set
            .into_iter()
            .map(|token| UiTokenSuggestionDto { token })
            .collect::<Vec<_>>();
        token_suggestions.sort_by(|left, right| left.token.cmp(&right.token));

        let mut proxy_candidates = Vec::<UiParamCandidateDto>::new();
        let mut binding_candidates = Vec::<UiParamCandidateDto>::new();
        for (candidate_id, candidate_node) in self.nodes.iter() {
            if candidate_id == param_node {
                continue;
            }
            let Some(candidate_snapshot) = candidate_node.engine_param_snapshot() else {
                continue;
            };
            let compatibility = compatibility_for_values(&candidate_snapshot.value, &snapshot.value);
            proxy_candidates.push(UiParamCandidateDto {
                param: candidate_id,
                compatible: compatibility.is_compatible(),
                projections: compatibility.projections,
            });
            let binding_compatibility = compatibility_for_binding_values(&candidate_snapshot.value, &snapshot.value);
            binding_candidates.push(UiParamCandidateDto {
                param: candidate_id,
                compatible: binding_compatibility.is_compatible(),
                projections: binding_compatibility.projections,
            });
        }
        proxy_candidates.sort_by_key(|candidate| candidate.param.0);
        binding_candidates.sort_by_key(|candidate| candidate.param.0);

        Ok(UiParamControlInfoDto {
            param: param_node,
            active_mode: snapshot.control.mode,
            available_modes,
            context_candidates,
            token_suggestions,
            proxy_candidates,
            binding_candidates,
        })
    }

    /// Returns script runtime state for `node` when it is a script node.
    pub fn ui_script_state(&self, node: NodeId) -> Result<ScriptUiState, String> {
        let Some(target) = self.nodes.get(node) else {
            return Err(format!("node {} not found", node.0));
        };

        target
            .engine_script_state()
            .ok_or_else(|| format!("node {} does not expose script runtime state", node.0))
    }

    /// Replaces script configuration for `node`.
    pub fn ui_set_script_config(
        &mut self,
        node: NodeId,
        config: ScriptUiConfig,
        force_reload: bool,
    ) -> Result<(), String> {
        self.edits.push(Edit::SetScriptConfig {
            node,
            config: ScriptNodeConfig::from(config),
            force_reload,
        });
        self.apply_edits().map_err(|err| err.to_string())
    }

    /// Requests runtime reload for script node `node`.
    pub fn ui_reload_script(&mut self, node: NodeId) -> Result<(), String> {
        {
            let Some(target) = self.nodes.get_mut(node) else {
                return Err(format!("node {} not found", node.0));
            };

            target.engine_request_script_reload()?;
        }

        self.push_ui_custom_event(
            "__transport.resync_required",
            Some(node),
            serde_json::json!({
                "reason": "script_reload_requested",
                "node": node.0
            }),
        );

        Ok(())
    }

    fn ui_resolve_created_child(
        &self,
        parent: NodeId,
        known_children: &HashSet<NodeId>,
    ) -> Result<NodeId, crate::engine::EngineEditError> {
        const OPERATION: &str = "CreateUserItem";

        let mut created_children = self
            .ui_direct_children(parent)
            .unwrap_or_default()
            .into_iter()
            .filter(|child_id| !known_children.contains(child_id));

        let Some(created_child) = created_children.next() else {
            return Err(crate::engine::EngineEditError::NodeMutationRejected {
                edit_index: 0,
                operation: OPERATION,
                node: parent,
                node_type: self
                    .nodes
                    .get(parent)
                    .map(|node| node.get_type().to_string())
                    .unwrap_or_else(|| "unknown".to_string()),
                message: "createUserItem did not materialize a new direct child".to_string(),
            });
        };

        if created_children.next().is_some() {
            return Err(crate::engine::EngineEditError::NodeMutationRejected {
                edit_index: 0,
                operation: OPERATION,
                node: parent,
                node_type: self
                    .nodes
                    .get(parent)
                    .map(|node| node.get_type().to_string())
                    .unwrap_or_else(|| "unknown".to_string()),
                message: "createUserItem materialized multiple new direct children".to_string(),
            });
        }

        Ok(created_child)
    }

    fn ui_find_created_item_param_node(&self, created_child: NodeId, decl_id: &str) -> Option<NodeId> {
        let snapshot = self.build_process_tree_snapshot();
        if decl_id.contains('/')
            && let Some(node_id) = snapshot.resolve_path_from(created_child, decl_id)
        {
            return snapshot
                .node(node_id)
                .and_then(|node| node.is_parameter().then_some(node_id));
        }

        let mut stack = vec![created_child];
        while let Some(node_id) = stack.pop() {
            let mut child = snapshot.node(node_id).and_then(|node| node.first_child);
            while let Some(child_id) = child {
                let child_snapshot = snapshot.node(child_id)?;
                if child_snapshot.is_parameter()
                    && (child_snapshot.decl_id == decl_id || child_snapshot.decl_id.rsplit('/').next() == Some(decl_id))
                {
                    return Some(child_id);
                }
                stack.push(child_id);
                child = child_snapshot.next_sibling;
            }
        }

        None
    }

    fn ui_apply_set_text_param_smart(
        &mut self,
        node: NodeId,
        value: String,
        behaviour: ParameterEventBehaviour,
    ) -> Result<(), crate::engine::EngineEditError> {
        const OPERATION: &str = "SetTextParamSmart";

        let (node_type, snapshot) = {
            let Some(target) = self.nodes.get(node) else {
                return Err(crate::engine::EngineEditError::NodeNotFound {
                    edit_index: 0,
                    operation: OPERATION,
                    node,
                });
            };
            let node_type = target.get_type().to_string();
            let Some(snapshot) = target.engine_param_snapshot() else {
                return Err(crate::engine::EngineEditError::ParamEditTargetMismatch {
                    edit_index: 0,
                    node,
                    node_type,
                });
            };
            (node_type, snapshot)
        };

        if !matches!(snapshot.value, ParamValue::Str(_)) {
            return Err(crate::engine::EngineEditError::ParamConstraintViolation {
                edit_index: 0,
                node,
                node_type,
                message: "smart text entry requires a string parameter".to_string(),
            });
        }

        if self.text_param_input_uses_template_mode(value.as_str()) {
            let state = ParameterControlState::new(
                ParameterControlMode::TemplateText,
                ParameterControlSpec::TemplateText { template: value },
            );
            if let Some(effect) = self.apply_set_param_control_state(0, node, state)? {
                self.record_set_param_control_state_history(effect);
            }
            return Ok(());
        }

        if snapshot.control.mode != ParameterControlMode::Manual {
            let state = ParameterControlState::new(ParameterControlMode::Manual, ParameterControlSpec::Manual);
            if let Some(effect) = self.apply_set_param_control_state(0, node, state)? {
                self.record_set_param_control_state_history(effect);
            }
        }

        self.edits.push(Edit::SetParam {
            node,
            value: ParamValue::Str(value),
            behaviour,
        });
        self.apply_ui_stabilization_to_fixed_point(16)
    }

    fn ui_dashboard_initial_param(decl_id: &str, value: ParamValue) -> UiCreateUserItemInitialParam {
        UiCreateUserItemInitialParam {
            decl_id: DeclId(decl_id.to_string()),
            value,
        }
    }

    fn ui_dashboard_rejection(
        &self,
        operation: &'static str,
        node: NodeId,
        message: impl Into<String>,
    ) -> crate::engine::EngineEditError {
        crate::engine::EngineEditError::NodeMutationRejected {
            edit_index: 0,
            operation,
            node,
            node_type: self
                .nodes
                .get(node)
                .map(|node| node.get_type().to_string())
                .unwrap_or_else(|| "unknown".to_string()),
            message: message.into(),
        }
    }

    fn ui_node_reference_for(&self, node: NodeId) -> Result<NodeReference, crate::engine::EngineEditError> {
        const OPERATION: &str = "DashboardWidgetReference";
        let Some(target) = self.nodes.get(node) else {
            return Err(crate::engine::EngineEditError::NodeNotFound {
                edit_index: 0,
                operation: OPERATION,
                node,
            });
        };
        let node_data = target.node_data();
        let mut reference = NodeReference::with_cached_id(node_data.meta.uuid, Some(node));
        reference.cached_name = Some(node_data.meta.label.clone());
        Ok(reference)
    }

    fn ui_dashboard_parent_layout_kind(&self, parent: NodeId) -> String {
        let snapshot = self.build_process_tree_snapshot();
        let Some(parent_node) = snapshot.node(parent) else {
            return "free".to_string();
        };
        if !matches!(
            parent_node.node_type.as_str(),
            DASHBOARD_PAGE_NODE_TYPE | DASHBOARD_WIDGET_CONTAINER_NODE_TYPE
        ) {
            return "free".to_string();
        }
        snapshot
            .resolve_path_from(parent, "layout/layout_kind")
            .or_else(|| snapshot.resolve_path_from(parent, "layout_kind"))
            .and_then(|layout_kind| snapshot.node(layout_kind))
            .and_then(|layout_kind| layout_kind.param_value.as_ref())
            .and_then(|value| value.as_enum().or_else(|| value.as_str()))
            .unwrap_or_else(|| "free".to_string())
    }

    fn ui_dashboard_placement_initial_params(
        &self,
        parent: NodeId,
        placement: Option<&UiDashboardWidgetPlacement>,
    ) -> Vec<UiCreateUserItemInitialParam> {
        let Some(placement) = placement else {
            return Vec::new();
        };
        let parent_layout_kind = self.ui_dashboard_parent_layout_kind(parent);
        let mut params = Vec::new();

        if parent_layout_kind == "free" || parent_layout_kind == "horizontal" {
            params.push(Self::ui_dashboard_initial_param(
                "layout/width",
                ParamValue::CssValue(placement.width),
            ));
        }

        if parent_layout_kind == "free" || parent_layout_kind == "vertical" {
            params.push(Self::ui_dashboard_initial_param(
                "layout/height",
                ParamValue::CssValue(placement.height),
            ));
        }

        if parent_layout_kind == "free" {
            params.push(Self::ui_dashboard_initial_param(
                "layout/anchor",
                ParamValue::Enum(placement.anchor.clone()),
            ));
            params.push(Self::ui_dashboard_initial_param(
                "layout/position",
                ParamValue::Vec2(placement.position.0, placement.position.1),
            ));
        }

        params
    }

    fn ui_dashboard_apply_widget_sizing_mode(
        &mut self,
        widget: NodeId,
        parent: NodeId,
        placement: Option<&UiDashboardWidgetPlacement>,
    ) -> Result<(), crate::engine::EngineEditError> {
        let parent_layout_kind = self.ui_dashboard_parent_layout_kind(parent);
        let size_enabled = placement.and_then(|placement| placement.size_enabled);
        let patch_enabled =
            |engine: &mut Self, decl_id: &str, enabled: bool| -> Result<(), crate::engine::EngineEditError> {
                let Some(param) = engine.ui_find_created_item_param_node(widget, decl_id) else {
                    return Ok(());
                };
                engine.edits.push(Edit::PatchMeta {
                    node: param,
                    patch: EngineNodeMetaPatch {
                        enabled: Some(enabled),
                        ..Default::default()
                    },
                });
                engine.apply_ui_initialization_to_fixed_point(16)
            };

        match parent_layout_kind.as_str() {
            "free" => {
                patch_enabled(self, "width", true)?;
                patch_enabled(self, "height", true)?;
            }
            "horizontal" => {
                patch_enabled(self, "width", size_enabled.map(|value| value.width).unwrap_or(false))?;
            }
            "vertical" => {
                patch_enabled(self, "height", size_enabled.map(|value| value.height).unwrap_or(false))?;
            }
            _ => {}
        }
        Ok(())
    }

    fn ui_dashboard_slider_range(value: &ParamValue, constraints: &ParameterConstraints) -> (f64, f64) {
        let current = match value {
            ParamValue::Int(value) => f64::from(*value),
            ParamValue::Float(value) => *value,
            _ => 0.0,
        };
        if let Some(RangeConstraint::Uniform { min, max }) = &constraints.range {
            return (
                min.unwrap_or_else(|| current.min(0.0)),
                max.unwrap_or_else(|| current.max(1.0)),
            );
        }
        if (0.0..=1.0).contains(&current) {
            return (0.0, 1.0);
        }
        (current.min(0.0), current.max(1.0))
    }

    fn ui_dashboard_generic_widget_initial_params(
        &self,
        target: NodeId,
        placement_parent: NodeId,
        placement: Option<&UiDashboardWidgetPlacement>,
    ) -> Result<Vec<UiCreateUserItemInitialParam>, crate::engine::EngineEditError> {
        const OPERATION: &str = "CreateDashboardGenericWidget";
        let Some(target_node) = self.nodes.get(target) else {
            return Err(crate::engine::EngineEditError::NodeNotFound {
                edit_index: 0,
                operation: OPERATION,
                node: target,
            });
        };
        let label = target_node.node_data().meta.label.clone();
        let Some(snapshot) = target_node.engine_param_snapshot() else {
            return Err(self.ui_dashboard_rejection(
                OPERATION,
                target,
                "generic dashboard widgets require a parameter target",
            ));
        };

        let mut params = vec![Self::ui_dashboard_initial_param(
            "binding/target_param",
            ParamValue::Reference(self.ui_node_reference_for(target)?),
        )];

        match &snapshot.value {
            ParamValue::Trigger() => {
                params.push(Self::ui_dashboard_initial_param(
                    "content/widget_kind",
                    ParamValue::Enum("button".to_string()),
                ));
                params.push(Self::ui_dashboard_initial_param("content/text", ParamValue::Str(label)));
            }
            ParamValue::Bool(value) => {
                params.push(Self::ui_dashboard_initial_param(
                    "content/widget_kind",
                    ParamValue::Enum("checkbox".to_string()),
                ));
                params.push(Self::ui_dashboard_initial_param("content/text", ParamValue::Str(label)));
                params.push(Self::ui_dashboard_initial_param(
                    "content/default_checked",
                    ParamValue::Bool(*value),
                ));
            }
            ParamValue::Int(_) | ParamValue::Float(_) => {
                let (min, max) = Self::ui_dashboard_slider_range(&snapshot.value, &snapshot.constraints);
                params.push(Self::ui_dashboard_initial_param(
                    "content/widget_kind",
                    ParamValue::Enum("slider".to_string()),
                ));
                params.push(Self::ui_dashboard_initial_param(
                    "content/value_range",
                    ParamValue::Vec2(min, max),
                ));
                params.push(Self::ui_dashboard_initial_param(
                    "content/step",
                    ParamValue::Float(if matches!(snapshot.value, ParamValue::Int(_)) {
                        1.0
                    } else {
                        0.01
                    }),
                ));
            }
            ParamValue::Str(value) => {
                params.push(Self::ui_dashboard_initial_param(
                    "content/widget_kind",
                    ParamValue::Enum("textInput".to_string()),
                ));
                params.push(Self::ui_dashboard_initial_param(
                    "content/placeholder",
                    ParamValue::Str(label),
                ));
                params.push(Self::ui_dashboard_initial_param(
                    "content/multiline",
                    ParamValue::Bool(value.contains('\n') || value.trim().len() > 48),
                ));
            }
            ParamValue::File(_) | ParamValue::Enum(_) | ParamValue::CssValue(_) => {
                params.push(Self::ui_dashboard_initial_param(
                    "content/widget_kind",
                    ParamValue::Enum("textInput".to_string()),
                ));
                params.push(Self::ui_dashboard_initial_param(
                    "content/placeholder",
                    ParamValue::Str(label),
                ));
            }
            _ => {
                params.push(Self::ui_dashboard_initial_param(
                    "content/widget_kind",
                    ParamValue::Enum("text".to_string()),
                ));
                params.push(Self::ui_dashboard_initial_param("content/text", ParamValue::Str(label)));
            }
        }

        params.extend(self.ui_dashboard_placement_initial_params(placement_parent, placement));
        Ok(params)
    }

    fn ui_apply_create_dashboard_container_widget(
        &mut self,
        parent: NodeId,
        label: Option<String>,
        placement: Option<UiDashboardWidgetPlacement>,
        layout_kind: Option<String>,
        prev_sibling: Option<NodeId>,
    ) -> Result<NodeId, crate::engine::EngineEditError> {
        let mut initial_params = self.ui_dashboard_placement_initial_params(parent, placement.as_ref());
        if let Some(layout_kind) = layout_kind {
            initial_params.push(Self::ui_dashboard_initial_param(
                "layout/layout_kind",
                ParamValue::Enum(layout_kind),
            ));
        }
        let created = self.ui_apply_create_user_item_returning_node(
            parent,
            DASHBOARD_WIDGET_CONTAINER_NODE_TYPE.to_string(),
            Some(
                label
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty())
                    .unwrap_or_else(|| "Container".to_string()),
            ),
            initial_params,
        )?;
        if prev_sibling.is_some() {
            self.edits.push(Edit::MoveNode {
                node: created,
                new_parent: parent,
                new_prev_sibling: prev_sibling,
            });
            self.apply_ui_initialization_to_fixed_point(16)?;
        }
        self.ui_dashboard_apply_widget_sizing_mode(created, parent, placement.as_ref())?;
        Ok(created)
    }

    fn ui_apply_create_dashboard_node_widget(
        &mut self,
        parent: NodeId,
        target: NodeId,
        placement: Option<UiDashboardWidgetPlacement>,
        prev_sibling: Option<NodeId>,
    ) -> Result<NodeId, crate::engine::EngineEditError> {
        const OPERATION: &str = "CreateDashboardNodeWidget";
        let Some(target_node) = self.nodes.get(target) else {
            return Err(crate::engine::EngineEditError::NodeNotFound {
                edit_index: 0,
                operation: OPERATION,
                node: target,
            });
        };
        let label = target_node.node_data().meta.label.clone();
        let mut initial_params = vec![Self::ui_dashboard_initial_param(
            "widget/target_node",
            ParamValue::Reference(self.ui_node_reference_for(target)?),
        )];
        initial_params.extend(self.ui_dashboard_placement_initial_params(parent, placement.as_ref()));
        let created = self.ui_apply_create_user_item_returning_node(
            parent,
            DASHBOARD_NODE_WIDGET_NODE_TYPE.to_string(),
            Some(label),
            initial_params,
        )?;
        if prev_sibling.is_some() {
            self.edits.push(Edit::MoveNode {
                node: created,
                new_parent: parent,
                new_prev_sibling: prev_sibling,
            });
            self.apply_ui_initialization_to_fixed_point(16)?;
        }
        self.ui_dashboard_apply_widget_sizing_mode(created, parent, placement.as_ref())?;
        Ok(created)
    }

    fn ui_apply_create_dashboard_generic_widget(
        &mut self,
        parent: NodeId,
        target: NodeId,
        placement: Option<UiDashboardWidgetPlacement>,
        prev_sibling: Option<NodeId>,
    ) -> Result<NodeId, crate::engine::EngineEditError> {
        const OPERATION: &str = "CreateDashboardGenericWidget";
        let Some(target_node) = self.nodes.get(target) else {
            return Err(crate::engine::EngineEditError::NodeNotFound {
                edit_index: 0,
                operation: OPERATION,
                node: target,
            });
        };
        let label = target_node.node_data().meta.label.clone();
        let initial_params = self.ui_dashboard_generic_widget_initial_params(target, parent, placement.as_ref())?;
        let created = self.ui_apply_create_user_item_returning_node(
            parent,
            DASHBOARD_GENERIC_WIDGET_NODE_TYPE.to_string(),
            Some(label),
            initial_params,
        )?;
        if prev_sibling.is_some() {
            self.edits.push(Edit::MoveNode {
                node: created,
                new_parent: parent,
                new_prev_sibling: prev_sibling,
            });
            self.apply_ui_initialization_to_fixed_point(16)?;
        }
        self.ui_dashboard_apply_widget_sizing_mode(created, parent, placement.as_ref())?;
        Ok(created)
    }

    fn ui_apply_bind_dashboard_node_widget_target(
        &mut self,
        widget: NodeId,
        target: NodeId,
    ) -> Result<(), crate::engine::EngineEditError> {
        let initial_params = vec![Self::ui_dashboard_initial_param(
            "widget/target_node",
            ParamValue::Reference(self.ui_node_reference_for(target)?),
        )];
        self.ui_apply_initial_params_to_node(widget, initial_params, "BindDashboardNodeWidgetTarget")
    }

    fn ui_apply_bind_dashboard_generic_widget_target(
        &mut self,
        widget: NodeId,
        target: NodeId,
    ) -> Result<(), crate::engine::EngineEditError> {
        let parent = self
            .nodes
            .get(widget)
            .and_then(|node| node.node_data().parent)
            .unwrap_or(widget);
        let initial_params = self.ui_dashboard_generic_widget_initial_params(target, parent, None)?;
        self.ui_apply_initial_params_to_node(widget, initial_params, "BindDashboardGenericWidgetTarget")
    }

    fn ui_apply_wrap_dashboard_widget_in_container(
        &mut self,
        widget: NodeId,
        placement: Option<UiDashboardWidgetPlacement>,
        layout_kind: Option<String>,
    ) -> Result<(), crate::engine::EngineEditError> {
        const OPERATION: &str = "WrapDashboardWidgetInContainer";
        let Some(widget_node) = self.nodes.get(widget) else {
            return Err(crate::engine::EngineEditError::NodeNotFound {
                edit_index: 0,
                operation: OPERATION,
                node: widget,
            });
        };
        let widget_data = widget_node.node_data();
        let Some(parent) = widget_data.parent else {
            return Err(self.ui_dashboard_rejection(OPERATION, widget, "widget has no parent"));
        };
        let label = widget_data.meta.label.trim().to_string();
        let container_label = if label.is_empty() {
            "Container".to_string()
        } else {
            format!("{label} Container")
        };
        let prev_sibling = widget_data.prev_sibling;
        let container = self.ui_apply_create_dashboard_container_widget(
            parent,
            Some(container_label),
            placement,
            layout_kind.or_else(|| Some("vertical".to_string())),
            prev_sibling,
        )?;
        self.edits.push(Edit::MoveNode {
            node: widget,
            new_parent: container,
            new_prev_sibling: None,
        });
        self.apply_ui_initialization_to_fixed_point(16)
    }

    fn ui_apply_create_user_item(
        &mut self,
        parent: NodeId,
        node_type: String,
        label: Option<String>,
        initial_params: Vec<UiCreateUserItemInitialParam>,
    ) -> Result<(), crate::engine::EngineEditError> {
        self.ui_apply_create_user_item_returning_node(parent, node_type, label, initial_params)
            .map(|_| ())
    }

    fn ui_apply_create_user_item_returning_node(
        &mut self,
        parent: NodeId,
        node_type: String,
        label: Option<String>,
        initial_params: Vec<UiCreateUserItemInitialParam>,
    ) -> Result<NodeId, crate::engine::EngineEditError> {
        const OPERATION: &str = "CreateUserItem";

        let known_children: HashSet<NodeId> = self
            .ui_direct_children(parent)
            .unwrap_or_default()
            .into_iter()
            .collect();
        self.queue_catalog_create(parent, node_type, label, None)?;
        self.apply_ui_stabilization_to_fixed_point(16)?;

        let created_child = self.ui_resolve_created_child(parent, &known_children)?;
        if !initial_params.is_empty() {
            self.ui_apply_initial_params_to_node(created_child, initial_params, OPERATION)?;
        }
        Ok(created_child)
    }

    /// Duplicates/copied roots and then creates dependent user items within the same edit session.
    pub fn ui_apply_duplicate_nodes_with_dependent_user_items<Encode, Decode>(
        &mut self,
        nodes: Vec<UiDuplicateNodeSpec>,
        created_items: Vec<UiDuplicateCreateUserItemSpec>,
        dependent_items: Vec<UiDuplicateDependentUserItem>,
        mut encode_data: Encode,
        mut decode_node: Decode,
    ) -> Result<Vec<NodeId>, ProjectPersistenceError>
    where
        Encode: FnMut(&T) -> Result<serde_json::Value, String>,
        Decode: FnMut(&str, &serde_json::Value, &NodeMeta) -> Result<T, String>,
    {
        const OPERATION: &str = "DuplicateNodes";

        let mut copied_by_source = HashMap::<NodeId, (NodeUuid, String)>::new();
        let mut reserved_labels = HashMap::<NodeId, HashSet<String>>::new();
        let catalog_labels_by_parent = self.ui_duplicate_catalog_labels(&created_items, &dependent_items);
        let mut prepared_duplicates = Vec::with_capacity(nodes.len());
        let mut prepared_created_items = Vec::with_capacity(created_items.len());
        let mut prepared_dependent_items = Vec::with_capacity(dependent_items.len());
        let mut copied_roots = Vec::with_capacity(nodes.len() + created_items.len());

        for spec in nodes {
            if copied_by_source.contains_key(&spec.source) {
                return Err(Self::ui_duplicate_source_collision(spec.source));
            }

            let source = spec.source;
            let mut prepared = self.prepare_duplicate_subtree_with(
                source,
                spec.new_parent,
                spec.new_prev_sibling,
                None,
                &mut encode_data,
                &mut decode_node,
            )?;
            let resolved_label =
                self.ui_reserve_duplicate_label(spec.new_parent, prepared.root_label(), &mut reserved_labels);
            prepared.set_root_label(resolved_label.clone());
            let prepared = self.prepare_project_subtree_initial_params(
                prepared,
                Self::ui_initial_param_pairs(spec.initial_params),
                OPERATION,
            )?;
            copied_by_source.insert(source, (prepared.root_uuid(), resolved_label));
            prepared_duplicates.push(prepared);
        }

        for spec in created_items {
            if copied_by_source.contains_key(&spec.source) {
                return Err(Self::ui_duplicate_source_collision(spec.source));
            }

            let source = spec.source;
            let preferred_label = Self::ui_catalog_item_preferred_label(
                &catalog_labels_by_parent,
                spec.parent,
                spec.node_type.as_str(),
                spec.label,
                OPERATION,
            )?;
            let resolved_label =
                self.ui_reserve_duplicate_label(spec.parent, preferred_label.as_str(), &mut reserved_labels);
            let prepared = self.prepare_prevalidated_catalog_item_subtree(
                spec.parent,
                None,
                spec.node_type.as_str(),
                resolved_label.clone(),
                Self::ui_initial_param_pairs(spec.initial_params),
                OPERATION,
            )?;
            copied_by_source.insert(source, (prepared.root_uuid(), resolved_label));
            prepared_created_items.push(prepared);
        }

        for item in dependent_items {
            let initial_params =
                self.ui_resolve_duplicate_dependent_initial_params(item.initial_params, &copied_by_source)?;
            let preferred_label = Self::ui_catalog_item_preferred_label(
                &catalog_labels_by_parent,
                item.parent,
                item.node_type.as_str(),
                item.label,
                OPERATION,
            )?;
            let resolved_label =
                self.ui_reserve_duplicate_label(item.parent, preferred_label.as_str(), &mut reserved_labels);
            prepared_dependent_items.push(self.prepare_prevalidated_catalog_item_subtree(
                item.parent,
                None,
                item.node_type.as_str(),
                resolved_label,
                Self::ui_initial_param_pairs(initial_params),
                OPERATION,
            )?);
        }

        let committed_capacity =
            prepared_duplicates.len() + prepared_created_items.len() + prepared_dependent_items.len();
        let commit_checkpoint = self.project_subtree_commit_checkpoint();
        let mut committed = Vec::with_capacity(committed_capacity);
        let mut committed_roots = Vec::with_capacity(committed_capacity);
        let mut structure_roots = Vec::with_capacity(committed_capacity);
        for prepared in prepared_duplicates {
            let subtree = match self.commit_prepared_project_subtree(
                prepared,
                NodeCreationContext::Duplicate,
                false,
                OPERATION,
            ) {
                Ok(subtree) => subtree,
                Err(error) => {
                    self.rollback_committed_project_subtrees(committed_roots.iter().copied(), commit_checkpoint);
                    return Err(error);
                }
            };
            copied_roots.push(subtree.root);
            structure_roots.push(subtree.root);
            committed_roots.push(subtree.root);
            committed.push(subtree);
        }
        for prepared in prepared_created_items {
            let subtree =
                match self.commit_prepared_project_subtree(prepared, NodeCreationContext::Fresh, false, OPERATION) {
                    Ok(subtree) => subtree,
                    Err(error) => {
                        self.rollback_committed_project_subtrees(committed_roots.iter().copied(), commit_checkpoint);
                        return Err(error);
                    }
                };
            copied_roots.push(subtree.root);
            structure_roots.push(subtree.root);
            committed_roots.push(subtree.root);
            committed.push(subtree);
        }
        for prepared in prepared_dependent_items {
            let subtree =
                match self.commit_prepared_project_subtree(prepared, NodeCreationContext::Fresh, false, OPERATION) {
                    Ok(subtree) => subtree,
                    Err(error) => {
                        self.rollback_committed_project_subtrees(committed_roots.iter().copied(), commit_checkpoint);
                        return Err(error);
                    }
                };
            structure_roots.push(subtree.root);
            committed_roots.push(subtree.root);
            committed.push(subtree);
        }

        if let Err(error) = self.queue_loaded_subtree_structure_events(structure_roots.as_slice()) {
            self.rollback_committed_project_subtrees(committed_roots.iter().copied(), commit_checkpoint);
            return Err(error);
        }
        if let Err(error) = self.finalize_committed_project_subtrees(committed) {
            self.rollback_committed_project_subtrees(structure_roots, commit_checkpoint);
            return Err(error);
        }
        Ok(copied_roots)
    }

    fn ui_resolve_duplicate_dependent_initial_params(
        &self,
        initial_params: Vec<UiDuplicateDependentUserItemInitialParam>,
        copied_by_source: &HashMap<NodeId, (NodeUuid, String)>,
    ) -> Result<Vec<UiCreateUserItemInitialParam>, ProjectPersistenceError> {
        initial_params
            .into_iter()
            .map(|initial_param| {
                let value = match initial_param.value {
                    UiDuplicateDependentInitialParamValue::Literal { value } => value,
                    UiDuplicateDependentInitialParamValue::DuplicatedNodeReference { source } => {
                        let (copied_uuid, copied_label) =
                            copied_by_source
                                .get(&source)
                                .ok_or_else(|| ProjectPersistenceError::Codec {
                                    node_type: "duplicateNodes".to_string(),
                                    message: format!(
                                        "dependent item references source node {:?} that was not copied",
                                        source
                                    ),
                                })?;
                        let mut reference = NodeReference::with_cached_id(*copied_uuid, None);
                        reference.cached_name = Some(copied_label.clone());
                        ParamValue::Reference(reference)
                    }
                };
                Ok(UiCreateUserItemInitialParam {
                    decl_id: initial_param.decl_id,
                    value,
                })
            })
            .collect()
    }

    fn ui_initial_param_pairs(initial_params: Vec<UiCreateUserItemInitialParam>) -> Vec<(DeclId, ParamValue)> {
        initial_params
            .into_iter()
            .map(|initial_param| (initial_param.decl_id, initial_param.value))
            .collect()
    }

    fn ui_duplicate_source_collision(source: NodeId) -> ProjectPersistenceError {
        ProjectPersistenceError::Codec {
            node_type: "duplicateNodes".to_string(),
            message: format!(
                "source node {:?} appears more than once in one duplication request",
                source
            ),
        }
    }

    fn ui_duplicate_catalog_labels(
        &self,
        created_items: &[UiDuplicateCreateUserItemSpec],
        dependent_items: &[UiDuplicateDependentUserItem],
    ) -> HashMap<NodeId, HashMap<String, String>> {
        let parents = created_items
            .iter()
            .map(|item| item.parent)
            .chain(dependent_items.iter().map(|item| item.parent))
            .collect::<HashSet<_>>();
        let shared_snapshot = parents
            .iter()
            .copied()
            .any(|parent| self.catalog_creatable_items_require_tree_snapshot(parent))
            .then(|| self.build_process_tree_snapshot());

        parents
            .into_iter()
            .map(|parent| {
                let items = match shared_snapshot.as_deref() {
                    Some(snapshot) if self.catalog_creatable_items_require_tree_snapshot(parent) => {
                        self.catalog_creatable_items_with_snapshot(parent, snapshot)
                    }
                    _ => self.catalog_creatable_items_without_snapshot(parent),
                };
                let labels = items.into_iter().map(|item| (item.node_type, item.label)).collect();
                (parent, labels)
            })
            .collect()
    }

    fn ui_catalog_item_preferred_label(
        catalog_labels_by_parent: &HashMap<NodeId, HashMap<String, String>>,
        parent: NodeId,
        node_type: &str,
        label: Option<String>,
        operation: &'static str,
    ) -> Result<String, ProjectPersistenceError> {
        let catalog_label = catalog_labels_by_parent
            .get(&parent)
            .and_then(|labels| labels.get(node_type))
            .ok_or_else(|| {
                ProjectPersistenceError::Engine(crate::engine::EngineEditError::UserItemTypeUnavailable {
                    edit_index: 0,
                    operation,
                    parent,
                    node_type: node_type.to_string(),
                })
            })?;
        Ok(label.unwrap_or_else(|| catalog_label.clone()))
    }

    fn ui_reserve_duplicate_label(
        &self,
        parent: NodeId,
        preferred_label: &str,
        reserved_labels: &mut HashMap<NodeId, HashSet<String>>,
    ) -> String {
        reserved_labels.entry(parent).or_insert_with(|| {
            let mut labels = HashSet::new();
            for child in self.ui_direct_children(parent).unwrap_or_default() {
                if let Some(label) = self
                    .nodes
                    .get(child)
                    .map(|node| node.node_data().meta.label.trim())
                    .filter(|label| !label.is_empty())
                {
                    labels.insert(label.to_string());
                }
            }
            labels
        });

        let labels = reserved_labels
            .get_mut(&parent)
            .expect("reserved duplicate labels should be initialized");
        let preferred_label = preferred_label.trim();
        let preferred_label = if preferred_label.is_empty() {
            "Item"
        } else {
            preferred_label
        };
        if labels.insert(preferred_label.to_string()) {
            return preferred_label.to_string();
        }

        let (base_label, mut suffix) = duplicate_unique_label_base(preferred_label);
        loop {
            let candidate = format!("{base_label} {suffix}");
            if labels.insert(candidate.clone()) {
                return candidate;
            }
            suffix = suffix.saturating_add(1);
        }
    }

    /// Applies direct parameter initializers to an already-created user item root.
    pub fn ui_apply_initial_params_to_node(
        &mut self,
        created_child: NodeId,
        initial_params: Vec<UiCreateUserItemInitialParam>,
        operation: &'static str,
    ) -> Result<(), crate::engine::EngineEditError> {
        for initial_param in initial_params {
            let Some(param_node) =
                self.ui_find_created_item_param_node(created_child, initial_param.decl_id.0.as_str())
            else {
                return Err(crate::engine::EngineEditError::NodeMutationRejected {
                    edit_index: 0,
                    operation,
                    node: created_child,
                    node_type: self
                        .nodes
                        .get(created_child)
                        .map(|node| node.get_type().to_string())
                        .unwrap_or_else(|| "unknown".to_string()),
                    message: format!(
                        "parameter '{}' is unavailable on the created item",
                        initial_param.decl_id.0
                    ),
                });
            };

            self.edits.push(Edit::SetParam {
                node: param_node,
                value: initial_param.value,
                behaviour: ParameterEventBehaviour::Coalesce,
            });
            self.apply_ui_initialization_to_fixed_point(16)?;
        }

        Ok(())
    }

    /// Applies one UI edit intent and returns an acknowledgement payload.
    pub fn apply_ui_intent(&mut self, intent: UiEditIntent) -> UiAck {
        self.apply_ui_intent_from_client(intent, None)
    }

    /// Applies one UI edit intent while attributing any opened edit session to a stable client.
    pub fn apply_ui_intent_from_client(&mut self, intent: UiEditIntent, ui_client_instance_id: Option<&str>) -> UiAck {
        let before_event_time = self.ui_event_log().last().map(|event| event.time);
        // eprintln!("[gc-ui] intent recv: {intent:?} | undo_len={} redo_len={} active_session={}", self.undo_len(), self.redo_len(), self.has_active_edit_session());

        let ack = match intent {
            UiEditIntent::BeginEdit { client_edit_id, label } => {
                self.edits.push(Edit::BeginEditSession {
                    origin: EditOrigin::Ui,
                    label,
                    client_edit_id,
                    ui_client_instance_id: ui_client_instance_id.map(str::to_owned),
                });
                let result = self.apply_edits();
                self.finish_ui_apply_now(before_event_time, result)
            }
            UiEditIntent::EndEdit { client_edit_id } => {
                self.edits.push(Edit::EndEditSession { client_edit_id });
                let result = self.apply_edits();
                self.finish_ui_apply_now(before_event_time, result)
            }
            UiEditIntent::SetParam { node, value, behaviour } => {
                self.edits.push(Edit::SetParam { node, value, behaviour });
                let result = self.apply_ui_stabilization_to_fixed_point(16);
                self.finish_ui_apply_now(before_event_time, result)
            }
            UiEditIntent::SetTextParamSmart { node, value, behaviour } => {
                let result = self.apply_implicit_ui_edit_session(
                    "Set text parameter",
                    "__ui-set-text-param-smart",
                    ui_client_instance_id,
                    |engine| engine.ui_apply_set_text_param_smart(node, value, behaviour),
                );
                self.finish_ui_apply_now(before_event_time, result)
            }
            UiEditIntent::SetParamControlState { node, state } => {
                match self.apply_set_param_control_state(0, node, state.into()) {
                    Ok(Some(effect)) => {
                        self.record_set_param_control_state_history(effect);
                        self.finish_ui_apply_now(before_event_time, Ok(()))
                    }
                    Ok(None) => self.finish_ui_apply_now(before_event_time, Ok(())),
                    Err(err) => self.finish_ui_apply_now(before_event_time, Err(err)),
                }
            }
            UiEditIntent::SetParamConstraints { node, constraints } => {
                match self.apply_set_param_constraints(0, node, constraints) {
                    Ok(Some(effect)) => {
                        self.record_set_param_constraints_history(effect);
                        self.finish_ui_apply_now(before_event_time, Ok(()))
                    }
                    Ok(None) => self.finish_ui_apply_now(before_event_time, Ok(())),
                    Err(err) => self.finish_ui_apply_now(before_event_time, Err(err)),
                }
            }
            UiEditIntent::MoveNode {
                node,
                new_parent,
                new_prev_sibling,
            } => {
                self.edits.push(Edit::MoveNode {
                    node,
                    new_parent,
                    new_prev_sibling,
                });
                let result = self.apply_edits();
                self.finish_ui_apply_now(before_event_time, result)
            }
            UiEditIntent::RemoveNode { node } => {
                let result = self.apply_implicit_ui_edit_session(
                    "Remove node",
                    "__ui-remove-node",
                    ui_client_instance_id,
                    |engine| {
                        engine.edits.push(Edit::RemoveNode { node });
                        engine.apply_ui_stabilization_to_fixed_point(16)
                    },
                );
                self.finish_ui_apply_now(before_event_time, result)
            }
            UiEditIntent::RemoveNodes { nodes } => {
                let result = self.apply_implicit_ui_edit_session(
                    "Remove nodes",
                    "__ui-remove-nodes",
                    ui_client_instance_id,
                    |engine| {
                        let mut seen = HashSet::<NodeId>::new();
                        for node in nodes {
                            if seen.insert(node) {
                                engine.edits.push(Edit::RemoveNode { node });
                            }
                        }
                        engine.apply_ui_stabilization_to_fixed_point(16)
                    },
                );
                self.finish_ui_apply_now(before_event_time, result)
            }
            UiEditIntent::CreateUserItem {
                parent,
                node_type,
                label,
                initial_params,
            } => {
                let edit_label = label.clone().unwrap_or_else(|| "Create user item".to_string());
                let result = self.apply_implicit_ui_edit_session(
                    &edit_label,
                    "__ui-create-user-item",
                    ui_client_instance_id,
                    |engine| engine.ui_apply_create_user_item(parent, node_type, label, initial_params),
                );
                self.finish_ui_apply_now(before_event_time, result)
            }
            UiEditIntent::CreateDashboardContainerWidget {
                parent,
                label,
                placement,
                layout_kind,
                prev_sibling,
            } => {
                let result = self.apply_implicit_ui_edit_session(
                    "Create dashboard container",
                    "__ui-create-dashboard-container-widget",
                    ui_client_instance_id,
                    |engine| {
                        engine
                            .ui_apply_create_dashboard_container_widget(
                                parent,
                                label,
                                placement,
                                layout_kind,
                                prev_sibling,
                            )
                            .map(|_| ())
                    },
                );
                self.finish_ui_apply_now(before_event_time, result)
            }
            UiEditIntent::CreateDashboardNodeWidget {
                parent,
                target,
                placement,
                prev_sibling,
            } => {
                let result = self.apply_implicit_ui_edit_session(
                    "Create dashboard node widget",
                    "__ui-create-dashboard-node-widget",
                    ui_client_instance_id,
                    |engine| {
                        engine
                            .ui_apply_create_dashboard_node_widget(parent, target, placement, prev_sibling)
                            .map(|_| ())
                    },
                );
                self.finish_ui_apply_now(before_event_time, result)
            }
            UiEditIntent::CreateDashboardGenericWidget {
                parent,
                target,
                placement,
                prev_sibling,
            } => {
                let result = self.apply_implicit_ui_edit_session(
                    "Create dashboard generic widget",
                    "__ui-create-dashboard-generic-widget",
                    ui_client_instance_id,
                    |engine| {
                        engine
                            .ui_apply_create_dashboard_generic_widget(parent, target, placement, prev_sibling)
                            .map(|_| ())
                    },
                );
                self.finish_ui_apply_now(before_event_time, result)
            }
            UiEditIntent::BindDashboardNodeWidgetTarget { widget, target } => {
                let result = self.apply_implicit_ui_edit_session(
                    "Bind dashboard node widget",
                    "__ui-bind-dashboard-node-widget-target",
                    ui_client_instance_id,
                    |engine| engine.ui_apply_bind_dashboard_node_widget_target(widget, target),
                );
                self.finish_ui_apply_now(before_event_time, result)
            }
            UiEditIntent::BindDashboardGenericWidgetTarget { widget, target } => {
                let result = self.apply_implicit_ui_edit_session(
                    "Bind dashboard generic widget",
                    "__ui-bind-dashboard-generic-widget-target",
                    ui_client_instance_id,
                    |engine| engine.ui_apply_bind_dashboard_generic_widget_target(widget, target),
                );
                self.finish_ui_apply_now(before_event_time, result)
            }
            UiEditIntent::WrapDashboardWidgetInContainer {
                widget,
                placement,
                layout_kind,
            } => {
                let result = self.apply_implicit_ui_edit_session(
                    "Wrap dashboard widget in container",
                    "__ui-wrap-dashboard-widget-in-container",
                    ui_client_instance_id,
                    |engine| engine.ui_apply_wrap_dashboard_widget_in_container(widget, placement, layout_kind),
                );
                self.finish_ui_apply_now(before_event_time, result)
            }
            UiEditIntent::DuplicateNode {
                source,
                new_parent,
                new_prev_sibling,
                initial_params: _,
            } => self.rejected_ui_ack(
                "duplicate_node_transport_required",
                format!(
                    "duplicateNode requires transport-level project codec support (source={}, new_parent={}, new_prev_sibling={:?})",
                    source.0, new_parent.0, new_prev_sibling
                ),
            ),
            UiEditIntent::DuplicateNodes {
                nodes,
                created_items,
                dependent_items,
            } => self.rejected_ui_ack(
                "duplicate_nodes_transport_required",
                format!(
                    "duplicateNodes requires transport-level project codec support (nodes={}, created_items={}, dependent_items={})",
                    nodes.len(),
                    created_items.len(),
                    dependent_items.len()
                ),
            ),
            UiEditIntent::FitAnimationCurvePath { curve, points, options } => {
                self.edits.push(Edit::CallNodeMutation {
                    node: curve,
                    needs_tree_snapshot: true,
                    callback: Box::new(move |node, ctx| {
                        let Some(curve_node) = node.as_any_mut().downcast_mut::<CurveNode>() else {
                            return Err("target node should be CurveNode".to_string());
                        };
                        curve_node.replace_range_with_fitted_samples(ctx, points.as_slice(), options)?;
                        Ok(())
                    }),
                });
                let result = self.apply_edits_to_fixed_point(16);
                self.finish_ui_apply_now(before_event_time, result)
            }
            UiEditIntent::PatchMeta { node, patch } => {
                self.edits.push(Edit::PatchMeta {
                    node,
                    patch: patch.into(),
                });
                let result = self.apply_edits();
                self.finish_ui_apply_now(before_event_time, result)
            }
            UiEditIntent::EnsureUserContextScope { owner } => match self.ensure_user_context_scope(owner) {
                Ok(changed) => {
                    if changed {
                        self.push_ui_custom_event(
                            UI_USER_CONTEXT_SCOPE_TOPIC,
                            Some(owner),
                            serde_json::json!({
                                "action": "ensure_scope",
                                "owner": owner.0,
                            }),
                        );
                    }
                    self.applied_ui_ack_since(before_event_time)
                }
                Err(message) => self.rejected_ui_ack("user_context_scope_error", message),
            },
            UiEditIntent::RemoveUserContextScope { owner } => {
                let removed = self.remove_user_context_scope(owner);
                if removed {
                    self.push_ui_custom_event(
                        UI_USER_CONTEXT_SCOPE_TOPIC,
                        Some(owner),
                        serde_json::json!({
                            "action": "remove_scope",
                            "owner": owner.0,
                        }),
                    );
                }
                self.applied_ui_ack_since(before_event_time)
            }
            UiEditIntent::UpsertUserContextEntry { owner, symbol, param } => {
                match self.upsert_user_context_entry(owner, symbol.as_str(), param) {
                    Ok(changed) => {
                        if changed {
                            self.push_ui_custom_event(
                                UI_USER_CONTEXT_ENTRY_TOPIC,
                                Some(owner),
                                serde_json::json!({
                                    "action": "upsert_entry",
                                    "owner": owner.0,
                                    "symbol": symbol,
                                    "param": param.0,
                                }),
                            );
                        }
                        self.applied_ui_ack_since(before_event_time)
                    }
                    Err(message) => self.rejected_ui_ack("user_context_entry_error", message),
                }
            }
            UiEditIntent::RemoveUserContextEntry { owner, symbol } => {
                let removed = self.remove_user_context_entry(owner, symbol.as_str());
                if removed {
                    self.push_ui_custom_event(
                        UI_USER_CONTEXT_ENTRY_TOPIC,
                        Some(owner),
                        serde_json::json!({
                            "action": "remove_entry",
                            "owner": owner.0,
                            "symbol": symbol,
                        }),
                    );
                }
                self.applied_ui_ack_since(before_event_time)
            }
            UiEditIntent::SendNodeEvent { node, topic, payload } => {
                let result = if !self.nodes.contains(node) {
                    Err(crate::engine::EngineEditError::NodeNotFound {
                        edit_index: 0,
                        operation: "SendNodeEvent",
                        node,
                    })
                } else {
                    self.emit_inbox_event(EventKind::Custom(crate::events::CustomEvent::new(
                        topic,
                        Some(node),
                        payload,
                    )));
                    self.apply_ui_stabilization_to_fixed_point(16)
                };
                self.finish_ui_apply_now(before_event_time, result)
            }
            UiEditIntent::ReevaluateGraph => {
                self.edits.push(Edit::ReevaluateGraph);
                let result = self.apply_edits();
                self.finish_ui_apply_now(before_event_time, result)
            }
            UiEditIntent::ClearLogs => {
                crate::logger::clear();
                self.push_ui_custom_event(crate::logger::UI_LOG_CLEARED_TOPIC, None, serde_json::json!({}));
                self.applied_ui_ack_since(before_event_time)
            }
            UiEditIntent::SetLogMaxEntries { max_entries } => {
                let applied_max_entries = crate::logger::set_max_entries(max_entries);
                self.push_ui_custom_event(
                    crate::logger::UI_LOG_MAX_ENTRIES_TOPIC,
                    None,
                    serde_json::json!({ "max_entries": applied_max_entries }),
                );
                self.applied_ui_ack_since(before_event_time)
            }
            UiEditIntent::Undo => match self.undo() {
                Ok(true) => self.applied_ui_ack_since(before_event_time),
                Ok(false) => self.rejected_ui_ack("undo_unavailable", "there is no transaction to undo".to_string()),
                Err(err) => self.rejected_ui_ack(ui_error_code(&err), err.to_string()),
            },
            UiEditIntent::Redo => match self.redo() {
                Ok(true) => self.applied_ui_ack_since(before_event_time),
                Ok(false) => self.rejected_ui_ack("redo_unavailable", "there is no transaction to redo".to_string()),
                Err(err) => self.rejected_ui_ack(ui_error_code(&err), err.to_string()),
            },
        };

        // eprintln!(
        //     "[gc-ui] intent ack: success={} status={:?} code={:?} earliest={:?} history={{undo_len:{}, redo_len:{}, can_undo:{}, can_redo:{}, active_session:{}}}",
        //     ack.success, ack.status, ack.error_code, ack.earliest_event_time, ack.history.undo_len, ack.history.redo_len, ack.history.can_undo, ack.history.can_redo, ack.history.active_edit_session
        // );

        ack
    }

    fn apply_implicit_ui_edit_session<F>(
        &mut self,
        label: &str,
        client_edit_id: &str,
        ui_client_instance_id: Option<&str>,
        operation: F,
    ) -> Result<(), crate::engine::EngineEditError>
    where
        F: FnOnce(&mut Self) -> Result<(), crate::engine::EngineEditError>,
    {
        if self.has_active_edit_session() {
            return operation(self);
        }

        let client_edit_id = client_edit_id.to_string();
        self.edits.push(Edit::BeginEditSession {
            origin: EditOrigin::Ui,
            label: Some(label.to_string()),
            client_edit_id: client_edit_id.clone(),
            ui_client_instance_id: ui_client_instance_id.map(str::to_owned),
        });
        self.apply_edits()?;

        let operation_result = operation(self);
        self.edits.push(Edit::EndEditSession { client_edit_id });
        let end_result = self.apply_edits();

        operation_result?;
        end_result
    }

    fn applied_ui_ack_since(&self, previous_event_time: Option<EngineTime>) -> UiAck {
        let start = self.ui_event_log_start_index(previous_event_time);
        let events = &self.ui_event_log()[start..];
        UiAck {
            success: true,
            status: UiAckStatus::Applied,
            error_code: None,
            error_message: None,
            earliest_event_time: events.first().map(|event| event.time),
            latest_event_time: events.last().map(|event| event.time),
            history: self.ui_history_state(),
        }
    }

    fn rejected_ui_ack(&self, error_code: &str, error_message: String) -> UiAck {
        UiAck {
            success: false,
            status: UiAckStatus::Rejected,
            error_code: Some(error_code.to_string()),
            error_message: Some(error_message),
            earliest_event_time: None,
            latest_event_time: None,
            history: self.ui_history_state(),
        }
    }

    fn finish_ui_apply_now(
        &self,
        previous_event_time: Option<EngineTime>,
        result: Result<(), crate::engine::EngineEditError>,
    ) -> UiAck {
        match result {
            Ok(()) => self.applied_ui_ack_since(previous_event_time),
            Err(err) => self.rejected_ui_ack(ui_error_code(&err), err.to_string()),
        }
    }

    fn apply_edits_to_fixed_point(&mut self, max_passes: usize) -> Result<(), crate::engine::EngineEditError> {
        let pass_limit = max_passes.max(1);
        for _ in 0..pass_limit {
            self.apply_edits()?;
            if self.edits.pending.is_empty() {
                return Ok(());
            }
        }

        Err(crate::engine::EngineEditError::NodeMutationRejected {
            edit_index: 0,
            operation: "UiApplyFixedPoint",
            node: self.root,
            node_type: self
                .nodes
                .get(self.root)
                .map(|node| node.get_type().to_string())
                .unwrap_or_else(|| "unknown".to_string()),
            message: format!("ui intent left pending edits after {pass_limit} stabilization passes"),
        })
    }

    fn apply_ui_stabilization_to_fixed_point(
        &mut self,
        max_passes: usize,
    ) -> Result<(), crate::engine::EngineEditError> {
        let pass_limit = max_passes.max(1);
        for _ in 0..pass_limit {
            if !self.edits.pending.is_empty() {
                self.apply_edits()?;
            }

            if !self.inbox.events.is_empty() {
                self.dispatch_inbox(crate::process_ctx::ExecutionPhase::EndOfTickStabilization)?;
            }

            if self.edits.pending.is_empty() && self.inbox.events.is_empty() {
                return Ok(());
            }
        }

        Err(crate::engine::EngineEditError::NodeMutationRejected {
            edit_index: 0,
            operation: "UiApplyStabilizationFixedPoint",
            node: self.root,
            node_type: self
                .nodes
                .get(self.root)
                .map(|node| node.get_type().to_string())
                .unwrap_or_else(|| "unknown".to_string()),
            message: format!("ui intent left pending edits or inbox events after {pass_limit} stabilization passes"),
        })
    }

    fn apply_ui_initialization_to_fixed_point(
        &mut self,
        max_passes: usize,
    ) -> Result<(), crate::engine::EngineEditError> {
        let pass_limit = max_passes.max(1);
        for _ in 0..pass_limit {
            if !self.edits.pending.is_empty() {
                self.apply_edits_without_history()?;
            }

            if !self.inbox.events.is_empty() {
                self.dispatch_inbox(crate::process_ctx::ExecutionPhase::EndOfTickStabilization)?;
            }

            if self.edits.pending.is_empty() && self.inbox.events.is_empty() {
                return Ok(());
            }
        }

        Err(crate::engine::EngineEditError::NodeMutationRejected {
            edit_index: 0,
            operation: "UiApplyInitializationFixedPoint",
            node: self.root,
            node_type: self
                .nodes
                .get(self.root)
                .map(|node| node.get_type().to_string())
                .unwrap_or_else(|| "unknown".to_string()),
            message: format!(
                "ui initializer left pending edits or inbox events after {pass_limit} stabilization passes"
            ),
        })
    }

    /// Returns the current UI history summary for transport acknowledgements.
    pub fn ui_history_state(&self) -> UiHistoryState {
        UiHistoryState {
            can_undo: self.undo_len() > 0,
            can_redo: self.redo_len() > 0,
            undo_len: self.undo_len(),
            redo_len: self.redo_len(),
            active_edit_session: self.has_active_edit_session(),
            current_history_state_id: self.current_history_state_id(),
        }
    }

    pub(crate) fn ui_direct_children(&self, parent: NodeId) -> Option<Vec<NodeId>> {
        let mut children = Vec::new();
        let mut child = self.nodes.get(parent)?.node_data().first_child;
        while let Some(child_id) = child {
            children.push(child_id);
            child = self.nodes.get(child_id).and_then(|node| node.node_data().next_sibling);
        }
        Some(children)
    }

    fn ui_direct_children_cached(
        &self,
        parent: NodeId,
        cache: &mut HashMap<NodeId, Option<Vec<NodeId>>>,
    ) -> Option<Vec<NodeId>> {
        if let Some(children) = cache.get(&parent) {
            return children.clone();
        }
        let children = self.ui_direct_children(parent);
        cache.insert(parent, children.clone());
        children
    }

    /// Builds the UI-facing DTO for a single node, e.g. for apps that need
    /// to serialize a node (or subtree, by walking `.children` recursively)
    /// outside the normal UI sync/event pipeline.
    pub fn ui_node_dto_for_event(&self, node_id: NodeId) -> Option<UiNodeDto> {
        if self.catalog_creatable_items_require_tree_snapshot(node_id) {
            let catalog_snapshot = self.build_process_tree_snapshot();
            self.ui_node_dto_for_event_impl(node_id, Some(catalog_snapshot.as_ref()))
        } else {
            self.ui_node_dto_for_event_impl(node_id, None)
        }
    }

    /// As `ui_node_dto_for_event`, reusing an already-built snapshot â€” use
    /// this when converting many nodes at once to avoid rebuilding it per call.
    pub fn ui_node_dto_for_event_with_catalog_snapshot(
        &self,
        node_id: NodeId,
        catalog_snapshot: &ProcessTreeSnapshot,
    ) -> Option<UiNodeDto> {
        self.ui_node_dto_for_event_impl(node_id, Some(catalog_snapshot))
    }

    pub(crate) fn ui_node_dto_for_event_without_catalog_snapshot(&self, node_id: NodeId) -> Option<UiNodeDto> {
        self.ui_node_dto_for_event_impl(node_id, None)
    }

    fn ui_node_dto_for_event_impl(
        &self,
        node_id: NodeId,
        catalog_snapshot: Option<&ProcessTreeSnapshot>,
    ) -> Option<UiNodeDto> {
        let node = self.nodes.get(node_id)?;
        let node_data = node.node_data();
        let node_type = node.get_type().to_string();
        let children = self.ui_direct_children(node_id).unwrap_or_default();

        let data = if let Some(param) = node.engine_param_snapshot() {
            UiNodeDataDto::Parameter {
                param: Box::new(UiParamDto::from(param)),
            }
        } else {
            UiNodeDataDto::Node {
                node_type: node_type.clone(),
            }
        };

        let listed_user_items = node.user_creatable_items();
        let can_query_creatable_items = node.get_type() == FOLDER_NODE_TYPE
            || node.user_container_rules().is_some()
            || !listed_user_items.is_empty();

        let mut creatable_user_items = Vec::new();
        if can_query_creatable_items {
            let items = if self.catalog_creatable_items_require_tree_snapshot(node_id) {
                self.catalog_creatable_items_with_snapshot(node_id, catalog_snapshot?)
            } else {
                self.catalog_creatable_items_without_snapshot(node_id)
            };
            for item in items {
                creatable_user_items.push(UiCreatableUserItemDto::from(item));
            }
        }

        let mut accepted_user_item_kinds: Vec<String> = node
            .user_container_rules()
            .map(|rules| {
                rules
                    .accepts_item_kinds
                    .iter()
                    .map(|kind| (*kind).to_string())
                    .collect()
            })
            .unwrap_or_default();
        for item in &listed_user_items {
            if !accepted_user_item_kinds.iter().any(|kind| kind == &item.item_kind) {
                accepted_user_item_kinds.push(item.item_kind.clone());
            }
        }

        let description = node_data
            .meta
            .description
            .as_deref()
            .or(node_data.meta.declared_description.as_deref())
            .or_else(|| node.type_description())
            .filter(|description| !description.trim().is_empty())
            .map(str::to_string);

        Some(UiNodeDto {
            node_id,
            uuid: node_data.meta.uuid,
            decl_id: node_data.meta.decl_id.clone(),
            node_type,
            meta: UiNodeMetaDto {
                short_name: node_data.meta.short_name.clone(),
                label: node_data.meta.label.clone(),
                enabled: node_data.meta.enabled,
                can_be_disabled: node_data.meta.can_be_disabled,
                user_permissions: node_data.meta.user_permissions.clone(),
                description,
                declared_description_key: None,
                description_overridden: false,
                tags: node_data.meta.tags.clone(),
                presentation: node_data.meta.presentation.clone(),
            },
            data,
            user_role: node_data.user_role,
            user_item_kind: node.user_item_kind().to_string(),
            accepted_user_item_kinds,
            creatable_user_items,
            children,
        })
    }

    fn ui_event_dto_with_child_order_cache(
        &self,
        event: &Event,
        child_order_cache: &mut HashMap<NodeId, Option<Vec<NodeId>>>,
    ) -> UiEventDto {
        let kind = match &event.kind {
            EventKind::ParamChanged {
                param,
                old_value,
                new_value,
            } => UiEventKind::ParamChanged {
                param: *param,
                old_value: old_value.clone(),
                new_value: new_value.clone(),
            },
            EventKind::ParamControlChanged {
                param,
                old_state,
                new_state,
            } => UiEventKind::ParamControlChanged {
                param: *param,
                old_state: old_state.clone().into(),
                new_state: new_state.clone().into(),
            },
            EventKind::ParamConstraintsChanged {
                param,
                old_constraints,
                new_constraints,
            } => UiEventKind::ParamConstraintsChanged {
                param: *param,
                old_constraints: old_constraints.clone(),
                new_constraints: new_constraints.clone(),
            },
            EventKind::ChildAdded { parent, child, decl_id } => UiEventKind::ChildAdded {
                parent: *parent,
                child: *child,
                decl_id: decl_id.clone(),
                parent_children: self.ui_direct_children_cached(*parent, child_order_cache),
            },
            EventKind::ChildRemoved { parent, child } => UiEventKind::ChildRemoved {
                parent: *parent,
                child: *child,
            },
            EventKind::ChildReplaced {
                parent,
                old,
                new,
                decl_id,
            } => UiEventKind::ChildReplaced {
                parent: *parent,
                old: *old,
                new: *new,
                decl_id: decl_id.clone(),
            },
            EventKind::ChildMoved {
                child,
                old_parent,
                new_parent,
            } => UiEventKind::ChildMoved {
                child: *child,
                old_parent: *old_parent,
                new_parent: *new_parent,
                old_parent_children: self.ui_direct_children_cached(*old_parent, child_order_cache),
                new_parent_children: self.ui_direct_children_cached(*new_parent, child_order_cache),
            },
            EventKind::ChildReordered { parent, child } => UiEventKind::ChildReordered {
                parent: *parent,
                child: *child,
                parent_children: self.ui_direct_children_cached(*parent, child_order_cache),
            },
            EventKind::NodeCreated { node } => UiEventKind::NodeCreated {
                node: *node,
                snapshot: self.ui_node_dto_for_event(*node).map(Box::new),
            },
            EventKind::NodeDeleted { node } => UiEventKind::NodeDeleted { node: *node },
            EventKind::MetaChanged { node, patch } => UiEventKind::MetaChanged {
                node: *node,
                patch: NodeMetaPatch::from(patch),
            },
            EventKind::GraphTransaction { transaction } => UiEventKind::GraphTransaction {
                transaction: transaction.clone(),
            },
            EventKind::Custom(custom) => UiEventKind::Custom {
                topic: custom.topic.clone(),
                origin: custom.origin,
                payload: custom.payload.as_ref().clone(),
                retention: custom.retention,
            },
        };

        UiEventDto { time: event.time, kind }
    }

    fn collect_scope_nodes(&self, scope: UiSubscriptionScope) -> Vec<NodeId> {
        match scope {
            UiSubscriptionScope::WholeGraph => self.collect_subtree_nodes(self.root, u32::MAX),
            UiSubscriptionScope::Subtree { root, max_depth } => self.collect_subtree_nodes(root, max_depth),
        }
    }

    fn collect_subtree_nodes(&self, root: NodeId, max_depth: u32) -> Vec<NodeId> {
        let mut nodes = Vec::new();
        if !self.nodes.contains(root) {
            return nodes;
        }

        let mut stack = vec![(root, 0u32)];
        let mut visited = HashSet::<NodeId>::new();
        while let Some((node_id, depth)) = stack.pop() {
            if !visited.insert(node_id) {
                continue;
            }

            nodes.push(node_id);

            if depth >= max_depth {
                continue;
            }

            let mut child = self.nodes.get(node_id).and_then(|node| node.node_data().first_child);
            let mut children = Vec::new();
            let mut sibling_chain = HashSet::<NodeId>::new();
            while let Some(child_id) = child {
                if !sibling_chain.insert(child_id) {
                    break;
                }
                if self.nodes.contains(child_id) {
                    children.push(child_id);
                }
                child = self.nodes.get(child_id).and_then(|node| node.node_data().next_sibling);
            }

            for child_id in children.into_iter().rev() {
                stack.push((child_id, depth.saturating_add(1)));
            }
        }

        nodes
    }

    fn event_matches_scope(&self, scope: &UiSubscriptionScope, event: &Event) -> bool {
        match scope {
            UiSubscriptionScope::WholeGraph => true,
            UiSubscriptionScope::Subtree { root, max_depth } => {
                if matches!(
                    event.kind,
                    EventKind::ChildAdded { .. }
                        | EventKind::ChildRemoved { .. }
                        | EventKind::ChildReplaced { .. }
                        | EventKind::ChildMoved { .. }
                        | EventKind::ChildReordered { .. }
                        | EventKind::NodeCreated { .. }
                        | EventKind::NodeDeleted { .. }
                        | EventKind::GraphTransaction { .. }
                ) {
                    return true;
                }

                let candidate_nodes: Vec<NodeId> = match &event.kind {
                    EventKind::ParamChanged { param, .. } => vec![*param],
                    EventKind::ParamControlChanged { param, .. } => vec![*param],
                    EventKind::ParamConstraintsChanged { param, .. } => vec![*param],
                    EventKind::ChildAdded { parent, child, .. } => vec![*parent, *child],
                    EventKind::ChildRemoved { parent, child } => vec![*parent, *child],
                    EventKind::ChildReplaced { parent, old, new, .. } => vec![*parent, *old, *new],
                    EventKind::ChildMoved {
                        child,
                        old_parent,
                        new_parent,
                    } => vec![*child, *old_parent, *new_parent],
                    EventKind::ChildReordered { parent, child } => vec![*parent, *child],
                    EventKind::NodeCreated { node } => vec![*node],
                    EventKind::NodeDeleted { node } => vec![*node],
                    EventKind::MetaChanged { node, .. } => vec![*node],
                    EventKind::GraphTransaction { .. } => return true,
                    EventKind::Custom(custom) => custom.origin.into_iter().collect(),
                };

                candidate_nodes
                    .into_iter()
                    .any(|node| self.node_within_subtree(node, *root, *max_depth))
            }
        }
    }

    fn node_within_subtree(&self, node: NodeId, root: NodeId, max_depth: u32) -> bool {
        let mut current = Some(node);
        let mut depth = 0u32;
        let mut visited = HashSet::<NodeId>::new();

        while let Some(node_id) = current {
            if !visited.insert(node_id) {
                return false;
            }
            if node_id == root {
                return depth <= max_depth;
            }
            if depth >= max_depth {
                return false;
            }
            current = self.nodes.get(node_id).and_then(|entry| entry.node_data().parent);
            depth = depth.saturating_add(1);
        }

        false
    }
}

fn ui_error_code(error: &crate::engine::EngineEditError) -> &'static str {
    match error {
        crate::engine::EngineEditError::NodeTypeMismatch { .. } => "node_type_mismatch",
        crate::engine::EngineEditError::ParamEditTargetMismatch { .. } => "param_edit_target_mismatch",
        crate::engine::EngineEditError::ParamConstraintViolation { .. } => "param_constraint_violation",
        crate::engine::EngineEditError::ParamConstraintsRejected { .. } => "param_constraints_rejected",
        crate::engine::EngineEditError::ParamControlStateRejected { .. } => "param_control_error",
        crate::engine::EngineEditError::ScriptConfigRejected { .. } => "script_config_rejected",
        crate::engine::EngineEditError::ScriptPropertyRejected { .. } => "script_property_rejected",
        crate::engine::EngineEditError::ScriptMethodRejected { .. } => "script_method_rejected",
        crate::engine::EngineEditError::NodeMutationRejected { .. } => "node_mutation_rejected",
        crate::engine::EngineEditError::NodeNotFound { .. } => "node_not_found",
        crate::engine::EngineEditError::ParentNotFound { .. } => "parent_not_found",
        crate::engine::EngineEditError::SiblingNotFound { .. } => "sibling_not_found",
        crate::engine::EngineEditError::InvalidSiblingParent { .. } => "invalid_sibling_parent",
        crate::engine::EngineEditError::InvalidSiblingReference { .. } => "invalid_sibling_reference",
        crate::engine::EngineEditError::CannotMutateRoot { .. } => "cannot_mutate_root",
        crate::engine::EngineEditError::CycleDetected { .. } => "cycle_detected",
        crate::engine::EngineEditError::EditSessionAlreadyActive { .. } => "edit_session_already_active",
        crate::engine::EngineEditError::EditSessionNotActive { .. } => "edit_session_not_active",
        crate::engine::EngineEditError::EditSessionIdMismatch { .. } => "edit_session_id_mismatch",
        crate::engine::EngineEditError::UserItemContainerRequired { .. } => "user_item_container_required",
        crate::engine::EngineEditError::UserItemKindRejected { .. } => "user_item_kind_rejected",
        crate::engine::EngineEditError::UserItemTypeUnavailable { .. } => "user_item_type_unavailable",
    }
}
