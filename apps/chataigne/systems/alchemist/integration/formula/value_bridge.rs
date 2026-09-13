use super::*;

pub(super) fn runtime_value_type_id(value: &RuntimeValue) -> String {
    value.value_type().to_string()
}

pub(super) fn parameter_node_type(value: &ParamValue) -> &'static str {
    match value {
        ParamValue::Trigger() => "trigger",
        ParamValue::Int(_) => "int",
        ParamValue::Float(_) => "float",
        ParamValue::Str(_) => "str",
        ParamValue::File(_) => "file",
        ParamValue::Enum(_) => "enum",
        ParamValue::Bool(_) => "bool",
        ParamValue::CssValue(_) => "css_value",
        ParamValue::Vec2(_, _) => "vec2",
        ParamValue::Vec3(_, _, _) => "vec3",
        ParamValue::Color(_, _, _, _) => "color",
        ParamValue::Reference(_) => "reference",
    }
}

pub(crate) fn runtime_value_to_param(value: &RuntimeValue) -> Result<ParamValue, String> {
    let value = match value {
        RuntimeValue::Unit => ParamValue::Str(String::new()),
        RuntimeValue::Bool(value) => ParamValue::Bool(*value),
        RuntimeValue::Trigger(_) => ParamValue::Trigger(),
        RuntimeValue::Int(value) => ParamValue::Int(
            i32::try_from(*value)
                .map_err(|_| format!("integer value {value} exceeds Golden Core i32 range"))?,
        ),
        RuntimeValue::Float(value) => ParamValue::Float(*value),
        RuntimeValue::String(value) => ParamValue::Str(value.to_string()),
        RuntimeValue::Vec2(value) => ParamValue::Vec2(value[0], value[1]),
        RuntimeValue::Vec3(value) => {
            ParamValue::Vec3(value[0], value[1], value[2])
        }
        RuntimeValue::Color(value) => ParamValue::Color(
            value.red,
            value.green,
            value.blue,
            value.alpha,
        ),
        RuntimeValue::Duration(value) => ParamValue::Float(value.as_secs_f64()),
        RuntimeValue::Array(values) => ParamValue::Str(
            values
                .iter()
                .map(|value| match value {
                    RuntimeValue::String(value) => value.to_string(),
                    RuntimeValue::Int(value) => value.to_string(),
                    RuntimeValue::Float(value) => value.to_string(),
                    RuntimeValue::Bool(value) => value.to_string(),
                    _ => format!("{value:?}"),
                })
                .collect::<Vec<_>>()
                .join(","),
        ),
        RuntimeValue::Ref(value) => {
            let uuid = value
                .stable_id
                .parse::<uuid::Uuid>()
                .map(NodeUuid)
                .unwrap_or_else(|_| NodeUuid::nil());
            ParamValue::Reference(NodeReference::new(uuid))
        }
        RuntimeValue::Extension(value) => ParamValue::Str(
            value
                .payload
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect(),
        ),
    };
    if !value.has_only_finite_numbers() {
        return Err("runtime value contains a non-finite number that cannot cross the parameter protocol".to_string());
    }
    Ok(value)
}

pub(super) fn default_runtime_value(
    value_type: &ValueTypeId,
) -> Result<RuntimeValue, String> {
    value_types()
        .get(value_type)
        .map(|descriptor| (descriptor.default_value)())
        .ok_or_else(|| format!("unknown Alchemist value type `{value_type}`"))
}

pub(crate) fn param_to_runtime_value(
    value: &ParamValue,
    value_type: &ValueTypeId,
) -> Result<RuntimeValue, String> {
    match value_type.as_str() {
        "unit" => Ok(RuntimeValue::Unit),
        "trigger" => Ok(RuntimeValue::Trigger(TriggerValue {
            fired: value.as_bool().unwrap_or(false),
            ..TriggerValue::default()
        })),
        "bool" | "int" | "float" | "string" | "vec2" | "vec3" | "color" | "duration" => {
            param_to_untyped_runtime_value(value)?.convert_to(value_type)
        }
        "value_array" => match value {
            ParamValue::Str(value) | ParamValue::File(value) | ParamValue::Enum(value) => {
                Ok(RuntimeValue::Array(
                    value
                        .split(',')
                        .map(|part| RuntimeValue::String(Arc::from(part.trim())))
                        .collect(),
                ))
            }
            _ => Ok(RuntimeValue::Array(vec![param_to_untyped_runtime_value(value)?])),
        },
        _ => match value {
            ParamValue::Reference(reference) => {
                let stable_id = if reference.is_empty() {
                    String::new()
                } else {
                    reference.uuid().0.to_string()
                };
                Ok(RuntimeValue::Ref(StableRef::new(
                    value_type.clone(),
                    stable_id,
                )))
            }
            _ => Err(format!(
                "Alchemist value type `{value_type}` requires a node reference parameter"
            )),
        },
    }
}

pub(super) fn param_to_untyped_runtime_value(value: &ParamValue) -> Result<RuntimeValue, String> {
    Ok(match value {
        ParamValue::Trigger() => RuntimeValue::Trigger(TriggerValue::default()),
        ParamValue::Bool(value) => RuntimeValue::Bool(*value),
        ParamValue::Int(value) => RuntimeValue::Int(i64::from(*value)),
        ParamValue::Float(value) => RuntimeValue::Float(*value),
        ParamValue::Str(value) | ParamValue::File(value) | ParamValue::Enum(value) => {
            RuntimeValue::String(Arc::from(value.as_str()))
        }
        ParamValue::CssValue(value) => RuntimeValue::Float(value.value),
        ParamValue::Vec2(x, y) => RuntimeValue::Vec2([*x, *y]),
        ParamValue::Vec3(x, y, z) => RuntimeValue::Vec3([*x, *y, *z]),
        ParamValue::Color(r, g, b, a) => RuntimeValue::Color(ColorValue {
            red: *r,
            green: *g,
            blue: *b,
            alpha: *a,
        }),
        ParamValue::Reference(reference) => {
            let stable_id = if reference.is_empty() {
                String::new()
            } else {
                reference.uuid().0.to_string()
            };
            RuntimeValue::String(Arc::from(stable_id))
        }
    })
}

pub(super) fn config_decl_id(field: &str) -> String {
    format!("config/{field}")
}

pub(super) fn config_type_decl_id(field: &str) -> String {
    format!("config/{field}__type")
}

pub(super) fn socket_decl_id(direction: &str, socket: &str) -> String {
    format!("{direction}/{socket}")
}

pub(super) fn socket_value_decl_id(direction: &str, socket: &str) -> String {
    format!("{direction}/{socket}/value")
}

pub(super) fn numbered_socket_index(socket: &str, prefix: &str) -> Option<i32> {
    let index = socket.strip_prefix(prefix)?;
    index.parse::<i32>().ok().filter(|index| *index > 0)
}

pub(super) fn child_param<'a>(
    snapshot: &'a ProcessTreeSnapshot,
    parent: NodeId,
    decl_id: &str,
) -> Option<&'a ParamValue> {
    let child = snapshot.find_child_by_decl_id(parent, decl_id)?;
    snapshot.node(child)?.param_value.as_ref()
}

pub(super) fn child_string(
    snapshot: &ProcessTreeSnapshot,
    parent: NodeId,
    decl_id: &str,
) -> Option<String> {
    child_param(snapshot, parent, decl_id)
        .and_then(ParamValue::as_str)
        .map(|value| value.to_string())
}

pub(super) fn child_vec2(
    snapshot: &ProcessTreeSnapshot,
    parent: NodeId,
    decl_id: &str,
) -> Option<[f64; 2]> {
    child_param(snapshot, parent, decl_id)
        .and_then(ParamValue::as_vec2)
        .map(|value| [value.0, value.1])
}

pub(super) fn enabled_child_vec2(
    snapshot: &ProcessTreeSnapshot,
    parent: NodeId,
    decl_id: &str,
) -> Option<[f64; 2]> {
    let child = snapshot.find_child_by_decl_id(parent, decl_id)?;
    let node = snapshot.node(child)?;
    node.enabled.then_some(())?;
    node.param_value
        .as_ref()
        .and_then(ParamValue::as_vec2)
        .map(|value| [value.0, value.1])
}

pub(super) fn direct_child_under(
    snapshot: &ProcessTreeSnapshot,
    ancestor: NodeId,
    descendant: NodeId,
) -> Option<NodeId> {
    let mut child = descendant;
    let mut parent = snapshot.node(child)?.parent?;
    while parent != ancestor {
        child = parent;
        parent = snapshot.node(child)?.parent?;
    }
    Some(child)
}

pub(super) fn is_anode_layout_decl_id(decl_id: &str) -> bool {
    matches!(decl_id, ANODE_POSITION_DECL_ID | ANODE_SIZE_DECL_ID)
}

pub(super) fn is_anode_layout_node(
    snapshot: &ProcessTreeSnapshot,
    formula_node: NodeId,
    node: NodeId,
) -> bool {
    let Some(anode) = direct_child_under(snapshot, formula_node, node) else {
        return false;
    };
    let Some(anode_snapshot) = snapshot.node(anode) else {
        return false;
    };
    if anode_snapshot.node_type != ANODE_NODE_TYPE {
        return false;
    }
    let Some(anode_child) = direct_child_under(snapshot, anode, node) else {
        return false;
    };
    snapshot
        .node(anode_child)
        .is_some_and(|node| is_anode_layout_decl_id(node.decl_id.as_str()))
}

pub(crate) fn formula_runtime_param_change_requires_rematerialization(
    snapshot: &ProcessTreeSnapshot,
    formula_node: NodeId,
    param: NodeId,
) -> bool {
    let Some(formula_child) = direct_child_under(snapshot, formula_node, param) else {
        return true;
    };
    if formula_child == param
        && snapshot.node(param).is_some_and(|node| {
            matches!(node.decl_id.as_str(), "is_valid" | "diagnostics_json" | "managed_regions_json")
        })
    {
        return false;
    }
    !is_anode_layout_node(snapshot, formula_node, param)
}

pub(super) fn node_removal_pending(ctx: &ProcessCtx, node: NodeId) -> bool {
    ctx.edits.pending.iter().any(|request| {
        matches!(&request.edit, Edit::RemoveNode { node: pending } if *pending == node)
    })
}

pub(super) fn child_add_pending(ctx: &ProcessCtx, parent: NodeId, decl_id: &str) -> bool {
    ctx.edits.pending.iter().any(|request| match &request.edit {
        Edit::AddNode {
            parent: pending_parent,
            node,
            ..
        } => {
            *pending_parent == parent
                && node.node_data().meta.decl_id.0.as_str() == decl_id
        }
        Edit::AddNodeTree {
            parent: pending_parent,
            tree,
            ..
        } => {
            *pending_parent == parent
                && tree.node.node_data().meta.decl_id.0.as_str() == decl_id
        }
        _ => false,
    })
}

pub(super) fn replace_child_tree_once(
    ctx: &mut ProcessCtx,
    parent: NodeId,
    existing: Option<NodeId>,
    tree: NodeTree,
) {
    let decl_id = tree.node.node_data().meta.decl_id.clone();
    if child_add_pending(ctx, parent, decl_id.0.as_str()) {
        if let Some(existing) = existing {
            if !node_removal_pending(ctx, existing) {
                ctx.edits.push(Edit::RemoveNode { node: existing });
            }
        }
        return;
    }
    if let Some(existing) = existing {
        if node_removal_pending(ctx, existing) {
            return;
        }
        ctx.edits.push(Edit::RemoveNode { node: existing });
    }
    ctx.add_child_tree(parent, tree, None);
}

pub(super) fn is_anode_type_variable_config_param(
    snapshot: &ProcessTreeSnapshot,
    anode: NodeId,
    param: NodeId,
) -> bool {
    let Some(anode_snapshot) = snapshot.node(anode) else {
        return false;
    };
    let Some(type_id) = anode_type_from_tags(&anode_snapshot.tags)
        .or_else(|| child_string(snapshot, anode, "anode_type"))
    else {
        return false;
    };
    let Some(config_folder) = snapshot.find_child_by_decl_id(anode, "config") else {
        return false;
    };
    let registry = registry();
    let Some(declaration) = registry.get(&ANodeTypeId::new(type_id)) else {
        return false;
    };
    declaration
        .config_fields()
        .into_iter()
        .filter(|field| field.type_variable.is_some())
        .any(|field| {
            snapshot.find_child_by_decl_id(config_folder, &config_decl_id(field.id.as_str()))
                == Some(param)
        })
}

pub(super) fn child_reference_uuid(
    snapshot: &ProcessTreeSnapshot,
    parent: NodeId,
    decl_id: &str,
) -> Option<NodeUuid> {
    child_param(snapshot, parent, decl_id).and_then(|value| match value {
        ParamValue::Reference(reference) => Some(reference.uuid()),
        _ => None,
    })
}

pub(super) fn property_meta_by_uuid(
    snapshot: &ProcessTreeSnapshot,
    properties: NodeId,
    property_uuid: NodeUuid,
) -> Option<(String, Option<Color>)> {
    let mut pending = snapshot.child_ids(properties);
    while let Some(candidate) = pending.pop() {
        let Some(node) = snapshot.node(candidate) else {
            continue;
        };
        if matches!(
            node.node_type.as_str(),
            PROPERTY_NODE_TYPE | PROPERTY_MANAGER_NODE_TYPE
        ) && node.uuid == property_uuid
        {
            let color = if node.node_type == PROPERTY_MANAGER_NODE_TYPE {
                node.presentation.color.or(node.presentation.default_color).or_else(|| {
                    child_string(snapshot, candidate, "role")
                        .and_then(|role| manager_role_color(&role))
                })
            } else {
                node.presentation.color.or(node.presentation.default_color)
            };
            return Some((node.label.clone(), color));
        }
        pending.extend(snapshot.child_ids(candidate));
    }
    None
}

pub(super) fn manager_anode_uses_property_ref(type_id: &str) -> bool {
    matches!(
        type_id,
        chataigne_state_machine::alchemist::CONDITIONS_MANAGER_TYPE
            | chataigne_state_machine::alchemist::FILTERS_MANAGER_TYPE
            | chataigne_state_machine::alchemist::INPUTS_MANAGER_TYPE
            | chataigne_state_machine::alchemist::OUTPUTS_MANAGER_TYPE
    )
}

pub(super) fn anode_property_ref_config_decl_id(type_id: &str) -> Option<&'static str> {
    if type_id == PROPERTY_ANODE_TYPE {
        return Some("config/property_id");
    }
    manager_anode_uses_property_ref(type_id).then_some("config/manager_id")
}

pub(super) fn anode_parent_formula_is_read_only(
    snapshot: &ProcessTreeSnapshot,
    anode: NodeId,
) -> bool {
    snapshot
        .node(anode)
        .and_then(|node| node.parent)
        .and_then(|formula| snapshot.node(formula))
        .is_some_and(|formula| {
            formula
                .tags
                .iter()
                .any(|tag| tag == FORMULA_EXTERNAL_READ_ONLY_TAG)
        })
}

pub(crate) fn constraint_value_type(
    constraint: &TypeConstraint,
    bindings: &TypeBindings,
) -> ValueTypeId {
    match constraint {
        TypeConstraint::Exact(value_type) => value_type.clone(),
        TypeConstraint::Generic(variable) => bindings
            .get(variable)
            .map(|binding| binding.value_type.clone())
            .unwrap_or_else(|| ValueTypeId::new("float")),
        TypeConstraint::Facet(_)
        | TypeConstraint::Any
        | TypeConstraint::Primitive
        | TypeConstraint::NumericLike
        | TypeConstraint::OneOf(_) => ValueTypeId::new("float"),
    }
}

pub(super) fn log_config_field() -> chataigne_alchemist::ANodeConfigFieldDecl {
    chataigne_alchemist::ANodeConfigFieldDecl::new(
        "log",
        "Log",
        RuntimeValue::Bool(false),
    )
    .with_description("Emit a debug log entry whenever this node processes successfully.")
}

pub(super) fn process_on_input_change_only_config_field(
    declaration: &dyn chataigne_alchemist::ANodeDeclaration,
) -> chataigne_alchemist::ANodeConfigFieldDecl {
    chataigne_alchemist::ANodeConfigFieldDecl::new(
        PROCESS_ON_INPUT_CHANGE_ONLY_CONFIG,
        "Process on input change only",
        RuntimeValue::Bool(declaration.default_process_on_input_change_only()),
    )
    .with_description("Skip this ANode while all of its runtime inputs are unchanged.")
}

pub(super) fn send_on_output_change_only_config_field(
    declaration: &dyn chataigne_alchemist::ANodeDeclaration,
) -> chataigne_alchemist::ANodeConfigFieldDecl {
    chataigne_alchemist::ANodeConfigFieldDecl::new(
        SEND_ON_OUTPUT_CHANGE_ONLY_CONFIG,
        "Send on output change only",
        RuntimeValue::Bool(declaration.default_send_on_output_change_only()),
    )
    .with_description("Suppress output sends and preview activity while the output value is unchanged.")
}

pub(super) fn config_fields_for_instance(
    declaration: &dyn chataigne_alchemist::ANodeDeclaration,
    instance: &ANodeInstance,
) -> Vec<chataigne_alchemist::ANodeConfigFieldDecl> {
    let mut fields = declaration.config_fields_for(instance);
    if declaration.process_on_input_change_only_configurable()
        && !fields
            .iter()
            .any(|field| field.id.as_str() == PROCESS_ON_INPUT_CHANGE_ONLY_CONFIG)
    {
        fields.push(process_on_input_change_only_config_field(declaration));
    }
    if !fields
        .iter()
        .any(|field| field.id.as_str() == SEND_ON_OUTPUT_CHANGE_ONLY_CONFIG)
    {
        fields.push(send_on_output_change_only_config_field(declaration));
    }
    if !fields.iter().any(|field| field.id.as_str() == "log") {
        fields.push(log_config_field());
    }
    fields
}

pub(super) fn existing_or_default_config(
    snapshot: &ProcessTreeSnapshot,
    anode: NodeId,
    declaration: &dyn chataigne_alchemist::ANodeDeclaration,
) -> Result<chataigne_alchemist::ANodeConfig, String> {
    let config_folder = snapshot
        .find_child_by_decl_id(anode, "config")
        .ok_or_else(|| "ANode Config folder is missing".to_string())?;
    let mut config = chataigne_alchemist::ANodeConfig::default();
    for field in declaration.config_fields() {
        let value = config_field_value(snapshot, config_folder, &field);
        config.set(field.id, value);
    }
    let mut instance =
        ANodeInstance::new(declaration.type_id(), declaration.label());
    instance.config = config.clone();
    for field in config_fields_for_instance(declaration, &instance) {
        let value = config_field_value(snapshot, config_folder, &field);
        config.set(field.id, value);
    }
    Ok(config)
}

pub(super) fn config_field_value(
    snapshot: &ProcessTreeSnapshot,
    config_folder: NodeId,
    field: &chataigne_alchemist::ANodeConfigFieldDecl,
) -> RuntimeValue {
    if field.editor.as_deref() == Some("gradient") {
        return gradient_config_value(snapshot, config_folder, field);
    }
    let value_type = if field.editor.as_deref() == Some("runtime_value") {
        child_string(
            snapshot,
            config_folder,
            &config_type_decl_id(field.id.as_str()),
        )
        .map(ValueTypeId::new)
        .unwrap_or_else(|| field.default_value.value_type())
    } else {
        field.default_value.value_type()
    };
    child_param(
        snapshot,
        config_folder,
        &config_decl_id(field.id.as_str()),
    )
    .and_then(|value| param_to_runtime_value(value, &value_type).ok())
    .unwrap_or_else(|| {
        if value_type == field.default_value.value_type() {
            field.default_value.clone()
        } else {
            default_runtime_value(&value_type)
                .unwrap_or_else(|_| field.default_value.clone())
        }
    })
}

/// Reads a hosted gradient node subtree into the structured `Array` config value
/// consumed by the Gradient Sampler ANode evaluator.
pub(super) fn gradient_config_value(
    snapshot: &ProcessTreeSnapshot,
    config_folder: NodeId,
    field: &chataigne_alchemist::ANodeConfigFieldDecl,
) -> RuntimeValue {
    let stops = snapshot
        .find_child_by_decl_id(config_folder, config_decl_id(field.id.as_str()).as_str())
        .and_then(|gradient_node| gradient_from_snapshot(snapshot, gradient_node))
        .map(|gradient| {
            gradient
                .stops()
                .iter()
                .map(gradient_stop_to_runtime_value)
                .collect::<Vec<_>>()
        });
    match stops {
        Some(stops) if !stops.is_empty() => RuntimeValue::Array(stops),
        _ => field.default_value.clone(),
    }
}

pub(super) fn gradient_stop_to_runtime_value(stop: &GradientStop) -> RuntimeValue {
    RuntimeValue::Array(vec![
        RuntimeValue::Float(stop.position),
        RuntimeValue::Color(ColorValue {
            red: stop.color.r(),
            green: stop.color.g(),
            blue: stop.color.b(),
            alpha: stop.color.a(),
        }),
        RuntimeValue::String(Arc::from(stop.interpolation.variant_id())),
    ])
}

pub(super) fn apply_forced_type_bindings_from_config(
    snapshot: &ProcessTreeSnapshot,
    config_folder: NodeId,
    declaration: &dyn chataigne_alchemist::ANodeDeclaration,
    signature: &chataigne_alchemist::ANodeSignature,
    value_types: &chataigne_alchemist::ValueTypeRegistry,
    instance: &mut ANodeInstance,
) {
    for field in config_fields_for_instance(declaration, instance) {
        let Some(variable) = field.type_variable.clone() else {
            continue;
        };
        let decl_id = config_decl_id(field.id.as_str());
        let Some(parameter_node_id) = snapshot.find_child_by_decl_id(config_folder, &decl_id) else {
            continue;
        };
        let Some(parameter_node) = snapshot.node(parameter_node_id) else {
            continue;
        };
        if !parameter_node.enabled {
            continue;
        }
        let Some(selected_type) = parameter_node
            .param_value
            .as_ref()
            .and_then(ParamValue::as_str)
        else {
            continue;
        };
        let selected_type = ValueTypeId::new(selected_type);
        let type_options = field.resolved_type_options(signature, value_types);
        if !type_options.contains(&selected_type) {
            continue;
        }
        instance.forced_type_bindings.insert(
            variable,
            selected_type,
            TypeBindingSource::ForcedByUser,
        );
    }
}
