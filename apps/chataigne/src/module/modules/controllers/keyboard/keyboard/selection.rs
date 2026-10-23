use super::*;

pub(super) fn clamp_usize_to_i32(value: usize) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}

pub(super) fn keyboard_device_options(devices: &[DiscoveredKeyboardDevice]) -> Vec<ParameterEnumOption> {
    let mut options = vec![
        enum_option(AUTO_KEYBOARD_VARIANT, "Auto", 0),
        enum_option(NO_KEYBOARD_VARIANT, "No Input Device", 1),
    ];

    options.extend(devices.iter().enumerate().map(|(index, device)| {
        let mut option = enum_option(device.variant_id.as_str(), device.label.as_str(), 10 + index as i32);
        option.tags = vec![device.details.clone()];
        option
    }));
    options
}

pub(super) fn sync_keyboard_device_enum_options(
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
    let canonical = canonical_keyboard_variant(current, &options);
    if keyboard_specific_device_selected(&canonical) && !options.iter().any(|option| option.variant_id == canonical) {
        options.insert(2, missing_keyboard_option(&canonical));
    }
    if canonical != *current {
        // Preserve the existing narrow normalization of persisted label-suffixed device IDs.
        ctx.set_param_with_behaviour(
            param_id,
            ParamValue::Enum(canonical.clone()),
            ParameterEventBehaviour::Coalesce,
        );
    }
    ctx.edits.push(golden_core::edit::Edit::SetParamEnumOptions {
        node: param_id,
        expected_uuid: node.uuid,
        expected_value: ParamValue::Enum(canonical),
        options: options.into(),
    });
}

pub(super) fn selected_keyboard_device(
    selection: &str,
    devices: &[DiscoveredKeyboardDevice],
) -> Option<DiscoveredKeyboardDevice> {
    devices
        .iter()
        .find(|device| selection_matches_keyboard(selection, device))
        .cloned()
}

pub(super) fn selection_matches_keyboard(selection: &str, device: &DiscoveredKeyboardDevice) -> bool {
    match selection.trim() {
        AUTO_KEYBOARD_VARIANT => true,
        NO_KEYBOARD_VARIANT | "" => false,
        selected => {
            selected == device.variant_id
                || legacy_keyboard_variant_identity(selected).is_some_and(|identity| identity == device.variant_id)
        }
    }
}

pub(super) fn canonical_keyboard_variant(selection: &str, options: &[ParameterEnumOption]) -> String {
    let trimmed = selection.trim();
    if trimmed.is_empty() {
        return AUTO_KEYBOARD_VARIANT.to_string();
    }
    if options.iter().any(|option| option.variant_id == trimmed) {
        return trimmed.to_string();
    }
    if let Some(identity) = legacy_keyboard_variant_identity(trimmed) {
        if options.iter().any(|option| option.variant_id == identity) {
            return identity.to_string();
        }
    }

    trimmed.to_string()
}

pub(super) fn legacy_keyboard_variant_identity(selection: &str) -> Option<&str> {
    let (identity, label) = selection.split_once('|')?;
    if identity.trim().is_empty() || label.trim().is_empty() {
        return None;
    }
    Some(identity)
}

pub(super) fn keyboard_specific_device_selected(selection: &str) -> bool {
    let trimmed = selection.trim();
    !trimmed.is_empty() && trimmed != AUTO_KEYBOARD_VARIANT && trimmed != NO_KEYBOARD_VARIANT
}

pub(super) fn missing_keyboard_option(variant_id: &str) -> ParameterEnumOption {
    let mut option = enum_option(
        variant_id,
        format!("Missing: {}", human_keyboard_variant(variant_id)).as_str(),
        5,
    );
    option.tags = vec!["missing".to_string()];
    option
}

pub(super) fn human_keyboard_variant(variant_id: &str) -> String {
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
