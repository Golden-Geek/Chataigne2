use super::*;

#[node("alchemist_input_socket", label = "Input")]
pub struct AlchemistInputSocket {}

#[node("alchemist_input_socket", from_struct)]
impl Node for AlchemistInputSocket {
    fn lifecycle_requires_tree_snapshot(&self) -> bool {
        false
    }

    fn init(&mut self, _ctx: &mut ProcessCtx) {
        self.node_data_mut().meta.user_permissions = NodeUserPermissions::none();
        self.node_data_mut().meta.can_be_disabled = false;
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

#[node("alchemist_output_socket", label = "Output")]
pub struct AlchemistOutputSocket {}

#[node("alchemist_output_socket", from_struct)]
impl Node for AlchemistOutputSocket {
    fn lifecycle_requires_tree_snapshot(&self) -> bool {
        false
    }

    fn init(&mut self, _ctx: &mut ProcessCtx) {
        self.node_data_mut().meta.user_permissions = NodeUserPermissions::none();
        self.node_data_mut().meta.can_be_disabled = false;
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

#[node("alchemist_anode", label = "ANode")]
#[children(
    position: golden_core::parameter::Vec2 = (0.0, 0.0) (
        label = "Position",
        show_in_inspector_content = false
    );
    size: golden_core::parameter::Vec2 = (13.0, 8.0) (
        label = "Size",
        enabled = false,
        can_be_disabled = true,
        show_in_inspector_content = false
    );
    folder(config, label = "Config") {}
    folder(inputs, label = "Inputs") {}
    folder(outputs, label = "Outputs") {}
)]
pub struct AlchemistANode {
    #[state(default = None)]
    numeric_constant_value_param: Option<NodeId>,
}

#[node("alchemist_anode", from_struct)]
impl Node for AlchemistANode {
    // Attachment binds declared children; ready reconciles the authored surface.
    // Init only adjusts metadata, so it must not force a whole-graph snapshot.
    fn init_requires_tree_snapshot(&self) -> bool {
        false
    }

    fn attached_snapshot_reusable_for_ready(&self) -> bool {
        // Constant reconciliation only reads its own persisted subtree and the parent formula's
        // read-only tag. Its init changes permissions, which are not in the process snapshot.
        anode_type_from_tags(&self.node_data().meta.tags).as_deref() == Some("constant")
    }

    fn inbox_requires_tree_snapshot(&self, events: &EventFrame) -> bool {
        let Some(param) = self.numeric_constant_value_param else {
            return true;
        };
        events.is_empty()
            || !events
                .iter()
                .all(|event| same_type_numeric_change_param(event) == Some(param))
    }

    fn user_item_kind(&self) -> &str {
        ANODE_ITEM_KIND
    }

    fn init(&mut self, _ctx: &mut ProcessCtx) {
        self.node_data_mut().meta.user_permissions = NodeUserPermissions::all();
        self.node_data_mut().meta.can_be_disabled = true;
    }

    fn on_node_ready(
        &mut self,
        ctx: &mut ProcessCtx,
        context: NodeCreationContext,
    ) {
        self.numeric_constant_value_param = ctx
            .tree_snapshot()
            .and_then(|snapshot| constant_value_param_from_snapshot(snapshot, self.id()));
        self.reconcile_structure(ctx);
        self.mirror_property_reference_presentation(ctx);
        if context == NodeCreationContext::Duplicate {
            let Some(snapshot) = ctx.tree_snapshot_arc() else {
                return;
            };
            if let Some([x, y]) = child_vec2(&snapshot, self.id(), "position") {
                if let Some(position) = snapshot.find_child_by_decl_id(self.id(), "position") {
                    ctx.set_param(position, ParamValue::Vec2(x + 1.5, y + 1.5));
                }
            }
        }
    }

    fn on_inbox(&mut self, ctx: &mut ProcessCtx) {
        if let Some(snapshot) = ctx.tree_snapshot() {
            self.numeric_constant_value_param =
                constant_value_param_from_snapshot(snapshot, self.id());
        }
        self.dispatch_inbox(ctx);
    }

    fn on_param_change(
        &mut self,
        ctx: &mut ProcessCtx,
        param: NodeId,
        _old_value: ParamValue,
    ) {
        if (self.numeric_constant_value_param == Some(param)
            && same_type_numeric_changes_for_param(&ctx.events, param))
            || constant_numeric_value_change_keeps_signature(ctx, param)
        {
            return;
        }
        let should_reconcile = ctx.tree_snapshot().is_some_and(|snapshot| {
            let Some(config_folder) =
                snapshot.find_child_by_decl_id(self.id(), "config")
            else {
                return false;
            };
            snapshot
                .node(param)
                .is_some_and(|node| node.parent == Some(config_folder))
                && !is_anode_type_variable_config_param(
                    snapshot,
                    self.id(),
                    param,
                )
        });
        if should_reconcile {
            self.reconcile_structure(ctx);
            self.mirror_property_reference_presentation(ctx);
        }
    }

    fn child_event_interest_depth(&self, _event: &Event) -> u32 {
        4
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

impl AlchemistANode {
    pub(super) fn for_type(type_id: &str, label: &str, category: &str) -> Self {
        let mut node = Self::new();
        let meta = &mut node.node_data_mut().meta;
        set_tag(&mut meta.tags, ANODE_TYPE_TAG_PREFIX, type_id);
        meta.label = label.to_owned();
        let color = anode_default_color(category, type_id);
        meta.presentation.default_color = Some(color);
        meta.presentation.color = Some(color);
        if type_id == chataigne_state_machine::alchemist::ROUTING_TYPE {
            meta.presentation.collapsed = true;
        }
        node
    }

    fn reconcile_structure(&mut self, ctx: &mut ProcessCtx) {
        let tagged_type = anode_type_from_tags(&self.node_data().meta.tags);
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            return;
        };
        let legacy_type = child_string(&snapshot, self.id(), "anode_type");
        let Some(type_id) = tagged_type.or(legacy_type) else {
            return;
        };
        if anode_type_from_tags(&self.node_data().meta.tags).is_none() {
            set_tag(
                &mut self.node_data_mut().meta.tags,
                ANODE_TYPE_TAG_PREFIX,
                &type_id,
            );
        }
        if let Some(legacy_type_child) =
            snapshot.find_child_by_decl_id(self.id(), "anode_type")
        {
            self.remove_child(ctx, legacy_type_child);
        }
        self.node_data_mut().meta.user_permissions = NodeUserPermissions::all();
        self.node_data_mut().meta.can_be_disabled = true;
        let registry = registry();
        let Some(declaration) = registry.get(&ANodeTypeId::new(&type_id)) else {
            ctx.set_node_warning_with(
                self.id(),
                Some("alchemist_anode"),
                "Unknown ANode type",
                Some(&type_id),
            );
            return;
        };
        ctx.clear_node_warning(self.id(), Some("alchemist_anode"));
        let default_color = anode_default_color(declaration.category(), &type_id);
        let presentation = &mut self.node_data_mut().meta.presentation;
        if presentation.default_color.is_none() {
            presentation.default_color = Some(default_color);
        }
        if presentation.color.is_none() {
            presentation.color = presentation.default_color.or(Some(default_color));
        }
        let Some(config_folder) =
            snapshot.find_child_by_decl_id(self.id(), "config")
        else {
            return;
        };
        let Some(inputs_folder) =
            snapshot.find_child_by_decl_id(self.id(), "inputs")
        else {
            return;
        };
        let Some(outputs_folder) =
            snapshot.find_child_by_decl_id(self.id(), "outputs")
        else {
            return;
        };
        sync_auto_input_count(ctx, &snapshot, self.id(), config_folder, &type_id);

        let value_types = value_types();
        let signature_ctx = SignatureCtx {
            value_types,
            properties: None,
        };
        let mut instance =
            ANodeInstance::new(declaration.type_id(), declaration.label());
        instance.config = existing_or_default_config(
            &snapshot,
            self.id(),
            declaration.as_ref(),
        )
        .unwrap_or_default();
        let config_signature = declaration.signature(
            &signature_ctx,
            &instance,
            &instance.type_bindings,
        );
        let config_read_only =
            anode_parent_formula_is_read_only(snapshot.as_ref(), self.id());

        let mut desired_config = HashSet::new();
        for field in config_fields_for_instance(declaration.as_ref(), &instance) {
            let value_decl = config_decl_id(field.id.as_str());
            desired_config.insert(value_decl.clone());
            if field.editor.as_deref() == Some("gradient") {
                self.ensure_gradient_config_node(
                    ctx,
                    &snapshot,
                    config_folder,
                    value_decl.as_str(),
                    &field.label,
                );
                continue;
            }
            if field.type_variable.is_some() {
                let type_options = field.resolved_type_options(&config_signature, value_types);
                let selected_type = child_string(&snapshot, config_folder, value_decl.as_str())
                    .unwrap_or_else(|| match &field.default_value {
                        RuntimeValue::String(value) => value.to_string(),
                        value => runtime_value_type_id(value),
                    });
                self.ensure_parameter_node(
                    ctx,
                    &snapshot,
                    config_folder,
                    value_type_parameter(
                        &field.label,
                        &value_decl,
                        &selected_type,
                        config_read_only,
                        &type_options,
                    ),
                );
                continue;
            }
            let value_type = if field.editor.as_deref() == Some("runtime_value") {
                let type_decl = config_type_decl_id(field.id.as_str());
                desired_config.insert(type_decl.clone());
                let type_options = field.resolved_type_options(&config_signature, value_types);
                let selected_type = child_string(
                    &snapshot,
                    config_folder,
                    type_decl.as_str(),
                )
                .unwrap_or_else(|| runtime_value_type_id(&field.default_value));
                let mut type_parameter = value_type_parameter(
                    &format!("{} Type", field.label),
                    &type_decl,
                    &selected_type,
                    config_read_only,
                    &type_options,
                );
                // A runtime value's type is explicit (there are no inputs to
                // infer it from), so it is shown in the header and stays
                // always-on rather than being a disable-to-infer selector.
                let meta = &mut type_parameter.node_data_mut().meta;
                meta.can_be_disabled = false;
                meta.enabled = true;
                self.ensure_parameter_node(
                    ctx,
                    &snapshot,
                    config_folder,
                    type_parameter,
                );
                ValueTypeId::new(selected_type)
            } else {
                field.default_value.value_type()
            };
            let default = if value_type == field.default_value.value_type() {
                field.default_value.clone()
            } else {
                default_runtime_value(&value_type)
                    .unwrap_or_else(|_| field.default_value.clone())
            };
            if let Ok(value) = runtime_value_to_param(&default) {
                let mut config_parameter = parameter(
                    &field.label,
                    &value_decl,
                    value,
                    config_read_only,
                );
                if !field.enum_options.is_empty() {
                    let selected = child_string(
                        &snapshot,
                        config_folder,
                        value_decl.as_str(),
                    )
                    .or_else(|| match &field.default_value {
                        RuntimeValue::String(value) => Some(value.to_string()),
                        _ => None,
                    })
                    .unwrap_or_default();
                    config_parameter.value = ParamValue::Enum(selected);
                    config_parameter.default_value =
                        ParamValue::Enum(match &field.default_value {
                            RuntimeValue::String(value) => value.to_string(),
                            _ => String::new(),
                        });
                    config_parameter.constraints.enum_options = field
                        .enum_options
                        .iter()
                        .map(|(variant_id, label)| ParameterEnumOption {
                            variant_id: variant_id.to_string(),
                            value: ParamValue::Enum(variant_id.to_string()),
                            label: label.clone(),
                            tags: Vec::new(),
                            ordering: None,
                        })
                        .collect();
                    config_parameter.constraints.policy =
                        ParameterConstraintPolicy::Reject;
                }
                if field.editor.as_deref() == Some("optional_count") {
                    config_parameter.node_data_mut().meta.can_be_disabled =
                        true;
                    config_parameter.node_data_mut().meta.enabled = false;
                }
                config_parameter
                    .node_data_mut()
                    .meta
                    .presentation
                    .color = Some(value_type_color(value_type.as_str()));
                self.ensure_parameter_node(
                    ctx,
                    &snapshot,
                    config_folder,
                    config_parameter,
                );
            }
        }
        self.remove_obsolete_children(
            ctx,
            &snapshot,
            config_folder,
            &desired_config,
        );

        apply_forced_type_bindings_from_config(
            &snapshot,
            config_folder,
            declaration.as_ref(),
            &config_signature,
            value_types,
            &mut instance,
        );
        let signature = declaration.signature(
            &signature_ctx,
            &instance,
            &instance.type_bindings,
        );
        let signature_bindings = local_signature_bindings(&signature, &instance);

        let mut desired_inputs = HashSet::new();
        for input in signature.inputs {
            let decl_id = socket_decl_id("inputs", input.id.as_str());
            desired_inputs.insert(decl_id.clone());
            let value_type =
                constraint_value_type(&input.constraint, &signature_bindings);
            let default = input
                .default_value
                .or_else(|| default_runtime_value(&value_type).ok())
                .unwrap_or(RuntimeValue::Float(0.0));
            let existing =
                snapshot.find_child_by_decl_id(inputs_folder, &decl_id);
            let needs_rebuild = existing.is_none_or(|socket| {
                !input_socket_matches(
                    &snapshot,
                    socket,
                    input.id.as_str(),
                    &value_type,
                    &default,
                )
            });
            if needs_rebuild {
                let tree = input_socket_tree(
                    input.id.as_str(),
                    &input.label,
                    &value_type,
                    &default,
                );
                replace_child_tree_once(ctx, inputs_folder, existing, tree);
            }
        }
        self.remove_obsolete_children(
            ctx,
            &snapshot,
            inputs_folder,
            &desired_inputs,
        );

        let mut desired_outputs = HashSet::new();
        for output in signature.outputs {
            let decl_id = socket_decl_id("outputs", output.id.as_str());
            desired_outputs.insert(decl_id.clone());
            let value_type =
                constraint_value_type(&output.constraint, &signature_bindings);
            let existing =
                snapshot.find_child_by_decl_id(outputs_folder, &decl_id);
            if existing.is_none_or(|socket| {
                !output_socket_matches(
                    &snapshot,
                    socket,
                    output.id.as_str(),
                    &value_type,
                )
            }) {
                replace_child_tree_once(
                    ctx,
                    outputs_folder,
                    existing,
                    output_socket_tree(
                        output.id.as_str(),
                        &output.label,
                        &value_type,
                    ),
                );
            }
        }
        self.remove_obsolete_children(
            ctx,
            &snapshot,
            outputs_folder,
            &desired_outputs,
        );
    }

    fn mirror_property_reference_presentation(&mut self, ctx: &mut ProcessCtx) {
        let Some(type_id) = anode_type_from_tags(&self.node_data().meta.tags) else {
            return;
        };
        let Some(config_decl_id) = anode_property_ref_config_decl_id(type_id.as_str()) else {
            return;
        };
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            return;
        };
        let Some(config) = snapshot.find_child_by_decl_id(self.id(), "config") else {
            return;
        };
        let Some(property_uuid) = child_reference_uuid(&snapshot, config, config_decl_id) else {
            return;
        };
        let Some(formula) = snapshot.node(self.id()).and_then(|node| node.parent) else {
            return;
        };
        let Some(properties) = snapshot.find_child_by_decl_id(formula, PROPERTIES_DECL_ID) else {
            return;
        };
        let Some((label, color)) = property_meta_by_uuid(&snapshot, properties, property_uuid) else {
            return;
        };
        let meta = &mut self.node_data_mut().meta;
        meta.label = label;
        meta.presentation.default_color = color;
    }

    fn ensure_parameter_node(
        &self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        parent: NodeId,
        parameter: Parameter,
    ) {
        let decl_id = parameter.node_data().meta.decl_id.clone();
        if let Some(existing) =
            snapshot.find_child_by_decl_id(parent, decl_id.0.as_str())
        {
            let Some(existing_snapshot) = snapshot.node(existing) else {
                return;
            };
            let expected_type = parameter.get_type();
            if existing_snapshot.node_type != expected_type {
                ctx.replace_node(existing, parameter);
            } else if parameter.read_only {
                let presentation =
                    parameter.node_data().meta.presentation.clone();
                let persist_read_only_value =
                    parameter.persist_read_only_value;
                ctx.call_node_mutation(existing, move |node, _ctx| {
                    let Some(existing) =
                        node.as_any_mut().downcast_mut::<Parameter>()
                    else {
                        return Err(
                            "expected an Alchemist config parameter".into(),
                        );
                    };
                    existing.read_only = true;
                    existing.persist_read_only_value =
                        persist_read_only_value;
                    existing.control_modes_enabled = false;
                    existing.node_data_mut().meta.presentation = presentation;
                    Ok(())
                });
            } else {
                let can_be_disabled =
                    parameter.node_data().meta.can_be_disabled;
                let presentation =
                    parameter.node_data().meta.presentation.clone();
                let constraints = parameter.constraints.clone();
                let default_value = parameter.default_value.clone();
                let fallback_value = parameter.value.clone();
                let read_only = parameter.read_only;
                let persist_read_only_value =
                    parameter.persist_read_only_value;
                let control_modes_enabled = parameter.control_modes_enabled;
                ctx.call_node_mutation(existing, move |node, _ctx| {
                    let Some(existing) =
                        node.as_any_mut().downcast_mut::<Parameter>()
                    else {
                        return Err(
                            "expected an Alchemist config parameter".into(),
                        );
                    };
                    existing.constraints = constraints;
                    existing.default_value = default_value;
                    existing.read_only = read_only;
                    existing.persist_read_only_value =
                        persist_read_only_value;
                    existing.control_modes_enabled = control_modes_enabled;
                    existing.node_data_mut().meta.can_be_disabled =
                        can_be_disabled;
                    existing.node_data_mut().meta.presentation = presentation;
                    existing.value = existing
                        .constraints
                        .normalize(existing.value.clone())
                        .unwrap_or_else(|_| fallback_value.clone());
                    Ok(())
                });
            }
            return;
        }
        ctx.add_child(parent, parameter, None);
    }

    fn ensure_gradient_config_node(
        &self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        config_folder: NodeId,
        decl_id: &str,
        label: &str,
    ) {
        if let Some(existing) =
            snapshot.find_child_by_decl_id(config_folder, decl_id)
        {
            if snapshot
                .node(existing)
                .is_some_and(|node| node.node_type == GRADIENT_NODE_TYPE)
            {
                return;
            }
            ctx.edits.push(Edit::RemoveNode { node: existing });
        }
        let mut gradient = GradientNode::new_with_label(label);
        gradient.node_data_mut().meta.decl_id = DeclId(decl_id.to_string());
        ctx.add_child(config_folder, gradient, None);
    }

    fn remove_obsolete_children(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        parent: NodeId,
        desired_decl_ids: &HashSet<String>,
    ) {
        let replaced_decl_ids = snapshot
            .child_ids(parent)
            .into_iter()
            .filter_map(|child| {
                let child_snapshot = snapshot.node(child)?;
                (desired_decl_ids.contains(child_snapshot.decl_id.as_str())
                    && node_removal_pending(ctx, child))
                .then(|| child_snapshot.decl_id.clone())
            })
            .collect::<HashSet<_>>();
        let mut retained_decl_ids = HashSet::<String>::new();
        for child in snapshot.child_ids(parent) {
            let Some(child_snapshot) = snapshot.node(child) else {
                continue;
            };
            let is_desired =
                desired_decl_ids.contains(child_snapshot.decl_id.as_str());
            let is_duplicate = is_desired
                && (replaced_decl_ids.contains(&child_snapshot.decl_id)
                    || !retained_decl_ids.insert(child_snapshot.decl_id.clone()));
            if (!is_desired || is_duplicate) && !node_removal_pending(ctx, child)
            {
                ctx.edits.push(Edit::RemoveNode { node: child });
            }
        }
    }
}

pub(super) fn value_type_parameter(
    label: &str,
    decl_id: &str,
    selected_type: &str,
    read_only: bool,
    type_options: &[ValueTypeId],
) -> Parameter {
    let registry = value_types();
    let options = registry
        .iter()
        .filter(|descriptor| {
            !matches!(
                descriptor.storage,
                chataigne_alchemist::ValueStorageKind::Extension
            ) && (type_options.is_empty()
                || type_options.iter().any(|id| id == &descriptor.id))
        })
        .map(|descriptor| ParameterEnumOption {
            variant_id: descriptor.id.to_string(),
            value: ParamValue::Enum(descriptor.id.to_string()),
            label: descriptor.label.clone(),
            tags: Vec::new(),
            ordering: None,
        })
        .collect::<Vec<_>>();
    let selected_type = options
        .iter()
        .any(|option| option.variant_id == selected_type)
        .then_some(selected_type)
        .or_else(|| options.first().map(|option| option.variant_id.as_str()))
        .unwrap_or(selected_type);
    let mut parameter = parameter(
        label,
        decl_id,
        ParamValue::Enum(selected_type.to_owned()),
        read_only,
    );
    if read_only {
        parameter
            .node_data_mut()
            .meta
            .presentation
            .show_in_inspector_content = false;
    } else {
        // ANode-level type selector: disabled by default (auto-inferred from inputs)
        parameter.node_data_mut().meta.can_be_disabled = true;
        parameter.node_data_mut().meta.enabled = false;
    }
    parameter.constraints.enum_options = options;
    parameter.constraints.policy = ParameterConstraintPolicy::Reject;
    parameter
}

pub(super) fn socket_value_type(
    snapshot: &ProcessTreeSnapshot,
    socket: NodeId,
    direction: &str,
    socket_id: &str,
) -> Option<String> {
    child_string(
        snapshot,
        socket,
        &format!("{direction}/{socket_id}/value_type"),
    )
}

pub(super) fn socket_id_matches(
    snapshot: &ProcessTreeSnapshot,
    socket: NodeId,
    direction: &str,
    socket_id: &str,
) -> bool {
    child_string(
        snapshot,
        socket,
        &format!("{direction}/{socket_id}/socket_id"),
    )
    .as_deref()
        == Some(socket_id)
}

pub(super) fn input_socket_matches(
    snapshot: &ProcessTreeSnapshot,
    socket: NodeId,
    socket_id: &str,
    value_type: &ValueTypeId,
    default: &RuntimeValue,
) -> bool {
    if !socket_id_matches(snapshot, socket, "inputs", socket_id) {
        return false;
    }
    if socket_value_type(snapshot, socket, "inputs", socket_id).as_deref()
        != Some(value_type.as_str())
    {
        return false;
    }
    let value = snapshot.find_child_by_decl_id(
        socket,
        &socket_value_decl_id("inputs", socket_id),
    );
    let expected_param = socket_default_param(value_type, default);
    match (value, expected_param) {
        (Some(value), Some(default)) => snapshot
            .node(value)
            .is_some_and(|node| node.node_type == parameter_node_type(&default)),
        (None, None) => true,
        _ => false,
    }
}

pub(super) fn output_socket_matches(
    snapshot: &ProcessTreeSnapshot,
    socket: NodeId,
    socket_id: &str,
    value_type: &ValueTypeId,
) -> bool {
    if !socket_id_matches(snapshot, socket, "outputs", socket_id) {
        return false;
    }
    socket_value_type(snapshot, socket, "outputs", socket_id).as_deref()
        == Some(value_type.as_str())
}

pub(super) fn input_socket_tree(
    socket_id: &str,
    label: &str,
    value_type: &ValueTypeId,
    default: &RuntimeValue,
) -> NodeTree {
    let decl_id = socket_decl_id("inputs", socket_id);
    let mut socket = AlchemistInputSocket::new();
    socket.node_data_mut().meta.label = label.to_owned();
    socket.node_data_mut().meta.decl_id = DeclId(decl_id.clone());
    socket.node_data_mut().meta.presentation.default_color =
        Some(value_type_color(value_type.as_str()));
    let mut socket_id_param = parameter(
        "Socket ID",
        format!("{decl_id}/socket_id"),
        ParamValue::Str(socket_id.to_owned()),
        true,
    );
    socket_id_param
        .node_data_mut()
        .meta
        .presentation
        .show_in_inspector_content = false;
    let mut value_type_param = parameter(
        "Value Type",
        format!("{decl_id}/value_type"),
        ParamValue::Str(value_type.to_string()),
        true,
    );
    value_type_param
        .node_data_mut()
        .meta
        .presentation
        .show_in_inspector_content = false;
    let mut tree = NodeTree::new(socket)
        .with_child(NodeTree::new(socket_id_param))
        .with_child(NodeTree::new(value_type_param));
    if let Some(default) = socket_default_param(value_type, default) {
        let mut value = parameter(
            "Value",
            socket_value_decl_id("inputs", socket_id),
            default,
            false,
        );
        value.node_data_mut().meta.presentation.default_color =
            Some(value_type_color(value_type.as_str()));
        tree = tree.with_child(NodeTree::new(value));
    }
    tree
}

pub(super) fn socket_default_param(value_type: &ValueTypeId, default: &RuntimeValue) -> Option<ParamValue> {
    let storage = value_types().get(value_type).map(|descriptor| descriptor.storage)?;
    if matches!(storage, chataigne_alchemist::ValueStorageKind::Extension) {
        return None;
    }
    runtime_value_to_param(default).ok()
}

pub(super) fn output_socket_tree(
    socket_id: &str,
    label: &str,
    value_type: &ValueTypeId,
) -> NodeTree {
    let decl_id = socket_decl_id("outputs", socket_id);
    let mut socket = AlchemistOutputSocket::new();
    socket.node_data_mut().meta.label = label.to_owned();
    socket.node_data_mut().meta.decl_id = DeclId(decl_id.clone());
    socket.node_data_mut().meta.presentation.default_color =
        Some(value_type_color(value_type.as_str()));
    let mut socket_id_param = parameter(
        "Socket ID",
        format!("{decl_id}/socket_id"),
        ParamValue::Str(socket_id.to_owned()),
        true,
    );
    socket_id_param
        .node_data_mut()
        .meta
        .presentation
        .show_in_inspector_content = false;
    let mut value_type_param = parameter(
        "Value Type",
        format!("{decl_id}/value_type"),
        ParamValue::Str(value_type.to_string()),
        true,
    );
    value_type_param
        .node_data_mut()
        .meta
        .presentation
        .show_in_inspector_content = false;
    NodeTree::new(socket)
        .with_child(NodeTree::new(socket_id_param))
        .with_child(NodeTree::new(value_type_param))
}
