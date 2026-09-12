use super::*;

pub(crate) fn local_signature_bindings(
    signature: &chataigne_alchemist::ANodeSignature,
    instance: &ANodeInstance,
) -> TypeBindings {
    let mut bindings = signature.default_bindings.clone();
    let _ = bindings.merge_from(&instance.type_bindings);
    let _ = bindings.merge_from(&instance.forced_type_bindings);
    bindings
}

pub(super) fn sync_auto_type_variable_config_params(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    config_folder: NodeId,
    declaration: &dyn chataigne_alchemist::ANodeDeclaration,
    signature: &chataigne_alchemist::ANodeSignature,
    value_types: &chataigne_alchemist::ValueTypeRegistry,
    bindings: &TypeBindings,
) {
    for field in declaration.config_fields() {
        let Some(variable) = field.type_variable.clone() else {
            continue;
        };
        let Some(binding) = bindings.get(&variable) else {
            continue;
        };
        let type_options = field.resolved_type_options(signature, value_types);
        if !type_options.is_empty() && !type_options.contains(&binding.value_type) {
            continue;
        }
        let decl_id = config_decl_id(field.id.as_str());
        let Some(parameter_node_id) =
            snapshot.find_child_by_decl_id(config_folder, &decl_id)
        else {
            continue;
        };
        let Some(parameter_node) = snapshot.node(parameter_node_id) else {
            continue;
        };
        if parameter_node.enabled {
            continue;
        }
        let next_value = ParamValue::Enum(binding.value_type.to_string());
        if parameter_node.param_value.as_ref() == Some(&next_value) {
            continue;
        }
        ctx.edits.push(Edit::SetParam {
            node: parameter_node_id,
            value: next_value,
            behaviour: ParameterEventBehaviour::Coalesce,
        });
    }
}

pub(super) fn sync_auto_input_count(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    anode: NodeId,
    config_folder: NodeId,
    type_id: &str,
) {
    let socket_prefix = match type_id {
        "math" => "value",
        "concatenate" => "part",
        _ => return,
    };
    let Some(count_param) =
        snapshot.find_child_by_decl_id(config_folder, "config/num_inputs")
    else {
        return;
    };
    let Some(count_node) = snapshot.node(count_param) else {
        return;
    };
    if count_node.enabled {
        return;
    }
    let Some(anode_snapshot) = snapshot.node(anode) else {
        return;
    };
    let Some(formula) = anode_snapshot.parent else {
        return;
    };
    let target_uuid = anode_snapshot.uuid;
    let mut connected_indices = HashSet::<i32>::new();
    for child in snapshot.child_ids(formula) {
        let Some(connection) = snapshot.node(child) else {
            continue;
        };
        if connection.node_type != CONNECTION_NODE_TYPE {
            continue;
        }
        if child_reference_uuid(snapshot, child, "target_node")
            != Some(target_uuid)
        {
            continue;
        }
        let Some(socket) = child_string(snapshot, child, "target_socket")
        else {
            continue;
        };
        if let Some(index) = numbered_socket_index(&socket, socket_prefix) {
            connected_indices.insert(index);
        }
    }
    let current = match count_node.param_value.as_ref() {
        Some(ParamValue::Int(value)) => *value,
        _ => 2,
    }
    .clamp(2, 64);
    let all_current_inputs_connected =
        (1..=current).all(|index| connected_indices.contains(&index));
    let highest_connected = connected_indices.iter().copied().max().unwrap_or(0);
    let desired = if all_current_inputs_connected {
        (current + 1).clamp(2, 64)
    } else if highest_connected < current {
        (highest_connected + 1).clamp(2, current)
    } else {
        current
    };
    if count_node.param_value.as_ref() == Some(&ParamValue::Int(desired)) {
        return;
    }
    if ctx.edits.pending.iter().any(|request| {
        matches!(
            &request.edit,
            Edit::SetParam { node, value, .. }
                if *node == count_param && *value == ParamValue::Int(desired)
        )
    }) {
        return;
    }
    ctx.edits.push(Edit::SetParam {
        node: count_param,
        value: ParamValue::Int(desired),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
}

pub(crate) fn anode_from_snapshot(
    snapshot: &ProcessTreeSnapshot,
    anode: NodeId,
) -> Result<ANodeInstance, String> {
    let node = snapshot
        .node(anode)
        .ok_or_else(|| format!("ANode {anode:?} is missing"))?;
    let type_id = anode_type_from_tags(&node.tags)
        .or_else(|| child_string(snapshot, anode, "anode_type"))
        .ok_or_else(|| format!("ANode `{}` has no internal type", node.label))?;
    let registry = registry();
    let declaration = registry
        .get(&ANodeTypeId::new(type_id.clone()))
        .ok_or_else(|| format!("ANode type `{type_id}` is not registered"))?;
    let mut instance =
        ANodeInstance::new(ANodeTypeId::new(type_id), node.label.clone());
    instance.id = ANodeId::from_uuid(node.uuid.0);
    instance.enabled = node.enabled;
    let config_folder = snapshot
        .find_child_by_decl_id(anode, "config")
        .ok_or_else(|| "ANode Config folder is missing".to_string())?;
    instance.config =
        existing_or_default_config(snapshot, anode, declaration.as_ref())?;
    let value_types = value_types();
    let signature_ctx = SignatureCtx {
        value_types,
        properties: None,
    };
    let signature = declaration.signature(
        &signature_ctx,
        &instance,
        &instance.type_bindings,
    );
    apply_forced_type_bindings_from_config(
        snapshot,
        config_folder,
        declaration.as_ref(),
        &signature,
        value_types,
        &mut instance,
    );
    instance.ui.position =
        child_vec2(snapshot, anode, "position").unwrap_or([0.0, 0.0]);
    instance.ui.size = enabled_child_vec2(snapshot, anode, "size")
        .filter(|size| size[0] > 0.0 && size[1] > 0.0);
    instance.ui.collapsed = node.presentation.collapsed;

    let signature = declaration.signature(
        &signature_ctx,
        &instance,
        &instance.type_bindings,
    );
    let signature_bindings = local_signature_bindings(&signature, &instance);
    if let Some(inputs_folder) = snapshot.find_child_by_decl_id(anode, "inputs") {
        for input in signature.inputs {
            let Some(socket_node) = snapshot.find_child_by_decl_id(
                inputs_folder,
                &socket_decl_id("inputs", input.id.as_str()),
            ) else {
                continue;
            };
            let value_type =
                constraint_value_type(&input.constraint, &signature_bindings);
            let value = child_param(
                snapshot,
                socket_node,
                &socket_value_decl_id("inputs", input.id.as_str()),
            )
            .map(|value| param_to_runtime_value(value, &value_type))
            .transpose()?
            .or(input.default_value);
            if let Some(value) = value {
                instance.input_defaults.insert(input.id, value);
            }
        }
    }
    Ok(instance)
}

pub(crate) fn formula_from_snapshot(
	snapshot: &ProcessTreeSnapshot,
	formula_node: NodeId,
) -> Result<AlchemistFormula, String> {
    let formula_snapshot = snapshot
        .node(formula_node)
        .ok_or_else(|| format!("Formula {formula_node:?} is missing"))?;
    if formula_snapshot.node_type != AlchemistFormulaDefinition::NODE_TYPE {
        return Err(format!(
            "node `{}` is not an Alchemist Formula",
            formula_snapshot.label
        ));
    }

    let graph_id =
        chataigne_alchemist::AlchemistGraphId::from_uuid(formula_snapshot.uuid.0);
    let child_count = snapshot.child_ids_slice(formula_node).len();
    let trace = child_count >= 1000 && std::env::var_os("GOLDEN_PERF_TRACE").is_some();
    let phase_started = trace.then(std::time::Instant::now);
    let mut anodes_by_uuid = HashMap::<NodeUuid, ANodeId>::new();
    let mut anodes = Vec::new();
    let mut connections = Vec::new();

    for child in snapshot.child_ids(formula_node) {
        let Some(child_snapshot) = snapshot.node(child) else {
            continue;
        };
        if child_snapshot.node_type != ANODE_NODE_TYPE {
            continue;
        }
        let instance = anode_from_snapshot(snapshot, child)?;
        anodes_by_uuid.insert(child_snapshot.uuid, instance.id);
        anodes.push(instance);
    }

    let anodes_us = phase_started.map(|started| started.elapsed().as_micros()).unwrap_or(0);
    let phase_started = trace.then(std::time::Instant::now);

    for child in snapshot.child_ids(formula_node) {
        let Some(child_snapshot) = snapshot.node(child) else {
            continue;
        };
        if child_snapshot.node_type != CONNECTION_NODE_TYPE {
            continue;
        }
        let source = child_reference_uuid(snapshot, child, "source_node")
            .and_then(|uuid| anodes_by_uuid.get(&uuid).copied())
            .ok_or_else(|| {
                format!("connection `{}` has no valid source ANode", child_snapshot.label)
            })?;
        let target = child_reference_uuid(snapshot, child, "target_node")
            .and_then(|uuid| anodes_by_uuid.get(&uuid).copied())
            .ok_or_else(|| {
                format!("connection `{}` has no valid target ANode", child_snapshot.label)
            })?;
        let source_socket = child_string(snapshot, child, "source_socket")
            .ok_or_else(|| "connection source socket is missing".to_string())?;
        let target_socket = child_string(snapshot, child, "target_socket")
            .ok_or_else(|| "connection target socket is missing".to_string())?;
        connections.push((
            OutputSocketRef::new(source, source_socket),
            InputSocketRef::new(target, target_socket),
        ));
    }

    let connections_us = phase_started.map(|started| started.elapsed().as_micros()).unwrap_or(0);
    let phase_started = trace.then(std::time::Instant::now);

    let properties = formula_property_schema_from_snapshot(snapshot, formula_node);
    let surface = formula_surface_from_snapshot(
        snapshot,
        formula_node,
        &mut anodes,
    )?;
    let surface_us = phase_started.map(|started| started.elapsed().as_micros()).unwrap_or(0);
    let phase_started = trace.then(std::time::Instant::now);
    let domain = chataigne_alchemist::AlchemistGraphDomain::new(
        (*registry()).clone(),
        (*value_types()).clone(),
        Some(properties.clone()),
    );
    let mut graph =
        chataigne_alchemist::AlchemistGraphDomain::new_document_with_identity(
            graph_id,
            formula_snapshot.label.clone(),
        );
    let mut transaction =
        chataigne_alchemist::AlchemistGraphTransaction::for_document(&graph);
    for anode in anodes {
        chataigne_alchemist::AlchemistGraphDomain::insert_node(
            &mut transaction,
            anode,
        );
    }
    for (source, target) in connections {
        chataigne_alchemist::AlchemistGraphDomain::connect(
            &mut transaction,
            &graph,
            source,
            target,
        );
    }
    let transaction_us = phase_started.map(|started| started.elapsed().as_micros()).unwrap_or(0);
    let phase_started = trace.then(std::time::Instant::now);
    transaction
        .commit(&mut graph, &domain)
        .map_err(|error| error.to_string())?;
    if let Some(started) = phase_started {
        eprintln!(
            "[formula] materialize children={} anodes_us={} connections_us={} surface_us={} transaction_us={} commit_us={}",
            child_count,
            anodes_us,
            connections_us,
            surface_us,
            transaction_us,
            started.elapsed().as_micros()
        );
    }

    Ok(AlchemistFormula {
        id: FormulaId::new(formula_snapshot.uuid.0.to_string()),
        version: 1,
        label: formula_snapshot.label.clone(),
        description: None,
        tags: formula_snapshot.tags.clone(),
        graph,
        properties,
        surface,
        context_contract: FormulaContextContract {
            accepts_additional_dimensions: true,
            ..FormulaContextContract::default()
        },
        migrations: Vec::new(),
	})
}

pub(super) fn formula_property_schema_from_snapshot(
    snapshot: &ProcessTreeSnapshot,
    formula_node: NodeId,
) -> FormulaPropertySchema {
    let mut schema = FormulaPropertySchema::default();
    if let Some(properties) =
        snapshot.find_child_by_decl_id(formula_node, PROPERTIES_DECL_ID)
    {
        collect_property_declarations(snapshot, properties, &mut schema);
    }
    schema
}

pub(super) fn collect_property_declarations(
    snapshot: &ProcessTreeSnapshot,
    parent: NodeId,
    schema: &mut FormulaPropertySchema,
) {
    for child in snapshot.child_ids(parent) {
        let Some(node) = snapshot.node(child) else {
            continue;
        };
        match node.node_type.as_str() {
            PROPERTY_NODE_TYPE => {
                let property_type = child_string(snapshot, child, "property_type")
                    .unwrap_or_else(|| "float".to_owned());
                let value_type =
                    ValueTypeId::new(property_value_type(&property_type));
                let default_value = child_param(snapshot, child, "value")
                    .and_then(|value| {
                        param_to_runtime_value(value, &value_type).ok()
                    })
                    .or_else(|| default_runtime_value(&value_type).ok())
                    .unwrap_or(RuntimeValue::Unit);
                schema.insert(FormulaPropertyDecl {
                    id: FormulaPropertyId::new(node.uuid.0.to_string()),
                    label: node.label.clone(),
                    description: None,
                    value_type,
                    default_value,
                    ui: ParamUiHints::default(),
                });
            }
            PROPERTY_FOLDER_NODE_TYPE => {
                collect_property_declarations(snapshot, child, schema);
            }
            _ => {}
        }
    }
}

pub(super) fn formula_anode_node_ids(
    snapshot: &ProcessTreeSnapshot,
    formula_node: NodeId,
) -> HashMap<ANodeId, NodeId> {
    snapshot
        .child_ids(formula_node)
        .into_iter()
        .filter_map(|child| {
            let node = snapshot.node(child)?;
            (node.node_type == ANODE_NODE_TYPE)
                .then(|| (ANodeId::from_uuid(node.uuid.0), child))
        })
        .collect()
}

pub(super) fn diagnostic_origin_node_id(
    origin: &DiagnosticOrigin,
    anode_node_ids: &HashMap<ANodeId, NodeId>,
) -> Option<NodeId> {
    match origin {
        DiagnosticOrigin::Node(node)
        | DiagnosticOrigin::Socket { node, .. } => {
            anode_node_ids.get(node).copied()
        }
        DiagnosticOrigin::Graph
        | DiagnosticOrigin::Registry
        | DiagnosticOrigin::Runtime => None,
    }
}

pub(crate) fn node_has_warning(
    snapshot: &ProcessTreeSnapshot,
    node: NodeId,
    warning_id: &str,
) -> bool {
    snapshot.node(node).is_some_and(|node| {
        node.presentation
            .warnings
            .iter()
            .any(|warning| warning.id == warning_id)
    })
}

pub(crate) fn node_warning_detail(
    snapshot: &ProcessTreeSnapshot,
    node: NodeId,
    warning_id: &str,
) -> Option<String> {
    snapshot.node(node).and_then(|node| {
        node.presentation
            .warnings
            .iter()
            .find(|warning| warning.id == warning_id)
            .and_then(|warning| warning.detail.clone())
    })
}

pub(crate) fn node_warning_matches(
    snapshot: &ProcessTreeSnapshot,
    node: NodeId,
    warning_id: &str,
    message: &str,
    detail: Option<&str>,
) -> bool {
    snapshot.node(node).is_some_and(|node| {
        node.presentation
            .warnings
            .iter()
            .find(|warning| warning.id == warning_id)
            .is_some_and(|warning| {
                warning.message == message
                    && warning.detail.as_deref() == detail
            })
    })
}

pub(super) fn diagnostic_text_field<'a>(
    diagnostic: &'a serde_json::Value,
    key: &str,
) -> Option<&'a str> {
    diagnostic
        .get(key)
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

pub(super) fn formula_warning_detail(diagnostics: &[serde_json::Value]) -> String {
    let mut lines = Vec::new();

    for (index, diagnostic) in diagnostics.iter().enumerate() {
        if diagnostics.len() > 1 {
            if index > 0 {
                lines.push(String::new());
            }
            lines.push(format!("Issue {}:", index + 1));
        }

        if let Some(code) = diagnostic_text_field(diagnostic, "code") {
            lines.push(format!("Code: {code}"));
        }
        if let Some(origin) = diagnostic_text_field(diagnostic, "origin") {
            lines.push(format!("Origin: {origin}"));
        }

        let severity = diagnostic_text_field(diagnostic, "severity").unwrap_or("error");
        let severity_label = match severity {
            "warning" => "Warning",
            "info" => "Info",
            _ => "Error",
        };

        if let Some(message) = diagnostic_text_field(diagnostic, "message") {
            lines.push(format!("{severity_label}: {message}"));
        }
    }

    if lines.is_empty() {
        "Formula compilation failed".to_owned()
    } else {
        lines.join("\n")
    }
}
