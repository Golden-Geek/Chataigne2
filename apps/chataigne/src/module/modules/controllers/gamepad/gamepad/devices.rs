use super::*;
pub(super) struct GamepadConnectionWarning {
    pub(super) message: String,
    pub(super) detail: Option<String>,
}

pub(super) fn gamepad_connection_warning(error: &str) -> GamepadConnectionWarning {
    if permission_denied_error(error) {
        return GamepadConnectionWarning {
            message: format!(
                "{error}. On Linux, GilRs reads /dev/input/event* directly; add udev rules or grant access to the input device."
            ),
            detail: Some(format!("GilRs Linux evdev notes: {GAMEPAD_UDEV_HELP_URL}")),
        };
    }

    GamepadConnectionWarning {
        message: error.to_string(),
        detail: None,
    }
}

fn permission_denied_error(error: &str) -> bool {
    let lower = error.to_ascii_lowercase();
    lower.contains("permission denied") || lower.contains("os error 13") || lower.contains("access is denied")
}
pub(super) fn gamepad_device_options(gamepads: &[DiscoveredGamepad]) -> Vec<ParameterEnumOption> {
    let mut options = vec![
        enum_option(AUTO_GAMEPAD_VARIANT, "Auto", 0),
        enum_option(NO_GAMEPAD_VARIANT, "No Gamepad", 1),
    ];

    options.extend(gamepads.iter().enumerate().map(|(index, gamepad)| {
        let mut option = enum_option(gamepad.variant_id.as_str(), gamepad.label.as_str(), 10 + index as i32);
        option.tags = vec![gamepad.details.clone()];
        option
    }));
    options
}

pub(super) fn sync_gamepad_device_enum_options(
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
    let canonical = if current.trim().is_empty() {
        AUTO_GAMEPAD_VARIANT.to_string()
    } else {
        current.clone()
    };
    if gamepad_specific_device_selected(&canonical) && !options.iter().any(|o| o.variant_id == canonical) {
        options.insert(2, missing_gamepad_option(&canonical));
    }
    ctx.edits.push(golden_core::edit::Edit::SetParamEnumOptions {
        node: param_id,
        expected_uuid: node.uuid,
        expected_value: ParamValue::Enum(current.clone()),
        selected_value: Some(ParamValue::Enum(canonical)),
        options: options.into(),
    });
}

pub(super) fn gamepad_specific_device_selected(selection: &str) -> bool {
    let trimmed = selection.trim();
    !trimmed.is_empty() && trimmed != AUTO_GAMEPAD_VARIANT && trimmed != NO_GAMEPAD_VARIANT
}

pub(super) fn missing_gamepad_option(variant_id: &str) -> ParameterEnumOption {
    let mut option = enum_option(
        variant_id,
        format!("Missing: {}", human_gamepad_variant(variant_id)).as_str(),
        5,
    );
    option.tags = vec!["missing".to_string()];
    option
}

pub(super) fn human_gamepad_variant(variant_id: &str) -> String {
    variant_id
        .split_once('|')
        .map(|(_, label)| label.to_string())
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
