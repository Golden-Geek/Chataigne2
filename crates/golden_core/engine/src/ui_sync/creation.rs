use super::*;

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
impl<T: Node> Engine<T> {
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

    pub(super) fn ui_apply_set_text_param_smart(
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

    pub(super) fn ui_apply_create_dashboard_container_widget(
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

    pub(super) fn ui_apply_create_dashboard_node_widget(
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

    pub(super) fn ui_apply_create_dashboard_generic_widget(
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

    pub(super) fn ui_apply_bind_dashboard_node_widget_target(
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

    pub(super) fn ui_apply_bind_dashboard_generic_widget_target(
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

    pub(super) fn ui_apply_wrap_dashboard_widget_in_container(
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

    pub(super) fn ui_apply_create_user_item(
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

        let copied_count = prepared_duplicates.len() + prepared_created_items.len();
        let mut prepared = Vec::with_capacity(copied_count + prepared_dependent_items.len());
        prepared.extend(
            prepared_duplicates
                .into_iter()
                .map(|subtree| (subtree, NodeCreationContext::Duplicate)),
        );
        prepared.extend(
            prepared_created_items
                .into_iter()
                .map(|subtree| (subtree, NodeCreationContext::Fresh)),
        );
        prepared.extend(
            prepared_dependent_items
                .into_iter()
                .map(|subtree| (subtree, NodeCreationContext::Fresh)),
        );
        let mut copied_roots = self.commit_prepared_project_subtree_batch(prepared, OPERATION)?;
        copied_roots.truncate(copied_count);
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
}
