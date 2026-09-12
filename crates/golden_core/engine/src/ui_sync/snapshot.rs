use super::*;

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
}

impl<T: Node> Engine<T> {
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
