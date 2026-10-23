use super::*;

pub(super) fn mouse_device_options(devices: &[DiscoveredMouseDevice]) -> Vec<ParameterEnumOption> {
    let mut options = vec![
        enum_option(AUTO_MOUSE_VARIANT, "Auto", 0),
        enum_option(NO_MOUSE_VARIANT, "No Input Device", 1),
    ];

    options.extend(devices.iter().enumerate().map(|(index, device)| {
        let mut option = enum_option(device.variant_id.as_str(), device.label.as_str(), 10 + index as i32);
        option.tags = vec![device.details.clone()];
        option
    }));
    options
}

pub(super) fn sync_mouse_device_enum_options(
    ctx: &mut ProcessCtx,
    param_id: NodeId,
    options: Vec<ParameterEnumOption>,
) {
    ctx.call_node_mutation(param_id, move |node, inner_ctx| {
        let Some(parameter) = node.as_any_mut().downcast_mut::<Parameter>() else {
            return Err("mouse device target is not a parameter".to_string());
        };

        let mut next_options = options.clone();
        let current_variant = parameter
            .value
            .as_enum()
            .filter(|variant| !variant.trim().is_empty())
            .map(|variant| canonical_mouse_variant(variant.as_str(), options.as_slice()))
            .unwrap_or_else(|| AUTO_MOUSE_VARIANT.to_string());

        if mouse_specific_device_selected(current_variant.as_str())
            && !next_options.iter().any(|option| option.variant_id == current_variant)
        {
            next_options.insert(2, missing_mouse_option(current_variant.as_str()));
        }

        let next_value = if next_options.iter().any(|option| option.variant_id == current_variant) {
            ParamValue::Enum(current_variant.clone())
        } else {
            ParamValue::Enum(AUTO_MOUSE_VARIANT.to_string())
        };

        if parameter.constraints.enum_options.as_slice() == next_options.as_slice() && parameter.value == next_value {
            return Ok(());
        }

        let label = parameter.node_data().meta.label.clone();
        let change_check = parameter.change_check.clone();
        let mut replacement = Parameter::new(label.as_str(), next_value, change_check);
        *replacement.node_data_mut() = parameter.node_data().clone();
        replacement.default_value = parameter.default_value.clone();
        replacement.event_behaviour = ParameterEventBehaviour::Coalesce;
        replacement.read_only = parameter.read_only;
        replacement.persist_read_only_value = parameter.persist_read_only_value;
        replacement.constraints = parameter.constraints.clone();
        replacement.constraints.enum_options = (next_options).into();
        replacement.ui_hints = parameter.ui_hints.clone();
        replacement.control = parameter.control.clone();
        replacement.control_modes_enabled = parameter.control_modes_enabled;

        inner_ctx.replace_node(param_id, replacement);
        Ok(())
    });
}

pub(super) fn selected_mouse_device(
    selection: &str,
    devices: &[DiscoveredMouseDevice],
) -> Option<DiscoveredMouseDevice> {
    devices
        .iter()
        .find(|device| selection_matches_mouse(selection, device))
        .cloned()
}

pub(super) fn selection_matches_mouse(selection: &str, device: &DiscoveredMouseDevice) -> bool {
    match selection.trim() {
        AUTO_MOUSE_VARIANT => true,
        NO_MOUSE_VARIANT | "" => false,
        selected => {
            selected == device.variant_id
                || legacy_mouse_variant_identity(selected).is_some_and(|identity| identity == device.variant_id)
        }
    }
}

pub(super) fn canonical_mouse_variant(selection: &str, options: &[ParameterEnumOption]) -> String {
    let trimmed = selection.trim();
    if trimmed.is_empty() {
        return AUTO_MOUSE_VARIANT.to_string();
    }
    if options.iter().any(|option| option.variant_id == trimmed) {
        return trimmed.to_string();
    }
    if let Some(identity) = legacy_mouse_variant_identity(trimmed) {
        if options.iter().any(|option| option.variant_id == identity) {
            return identity.to_string();
        }
    }

    trimmed.to_string()
}

pub(super) fn legacy_mouse_variant_identity(selection: &str) -> Option<&str> {
    let (identity, label) = selection.split_once('|')?;
    if identity.trim().is_empty() || label.trim().is_empty() {
        return None;
    }
    Some(identity)
}

pub(super) fn mouse_specific_device_selected(selection: &str) -> bool {
    let trimmed = selection.trim();
    !trimmed.is_empty() && trimmed != AUTO_MOUSE_VARIANT && trimmed != NO_MOUSE_VARIANT
}

pub(super) fn missing_mouse_option(variant_id: &str) -> ParameterEnumOption {
    let mut option = enum_option(
        variant_id,
        format!("Missing: {}", human_mouse_variant(variant_id)).as_str(),
        5,
    );
    option.tags = vec!["missing".to_string()];
    option
}

pub(super) fn human_mouse_variant(variant_id: &str) -> String {
    if let Some((_, label)) = variant_id.rsplit_once('|') {
        if !label.trim().is_empty() {
            return label.to_string();
        }
    }
    variant_id
        .split('#')
        .nth(1)
        .or_else(|| variant_id.split('#').next())
        .map(str::to_string)
        .unwrap_or_else(|| variant_id.to_string())
}

pub(super) fn enum_option(variant_id: &str, label: &str, ordering: i32) -> ParameterEnumOption {
    ParameterEnumOption {
        variant_id: variant_id.to_string(),
        value: ParamValue::Enum(variant_id.to_string()),
        label: label.to_string(),
        tags: Vec::new(),
        ordering: Some(ordering),
    }
}
