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
    mut options: Vec<ParameterEnumOption>,
) {
    let Some(snapshot) = ctx.tree_snapshot_arc() else {
        return;
    };
    let Some(node) = snapshot.node(param_id) else {
        return;
    };
    let Some(ParamValue::Enum(current)) = &node.param_value else {
        return;
    };
    if current.len() > 1024 {
        return;
    }
    let canonical = canonical_mouse_variant(current, &options);
    if mouse_specific_device_selected(&canonical) && !options.iter().any(|option| option.variant_id == canonical) {
        options.insert(2, missing_mouse_option(&canonical));
    }
    ctx.edits.push(golden_core::edit::Edit::SetParamEnumOptions {
        node: param_id,
        expected_uuid: node.uuid,
        expected_value: ParamValue::Enum(current.clone()),
        selected_value: Some(ParamValue::Enum(canonical)),
        options: options.into(),
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
