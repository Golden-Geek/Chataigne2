use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum DeviceChoice {
    Input,
    Output,
    Duplex,
}

impl DeviceChoice {
    fn supports(self, descriptor: &golden_audio::AudioDeviceDescriptor) -> bool {
        match self {
            Self::Input => descriptor.supports(golden_audio::AudioDirection::Input),
            Self::Output => descriptor.supports(golden_audio::AudioDirection::Output),
            Self::Duplex => {
                descriptor.supports(golden_audio::AudioDirection::Input)
                    && descriptor.supports(golden_audio::AudioDirection::Output)
            }
        }
    }

    fn direction(self) -> Option<golden_audio::AudioDirection> {
        match self {
            Self::Input => Some(golden_audio::AudioDirection::Input),
            Self::Output => Some(golden_audio::AudioDirection::Output),
            Self::Duplex => None,
        }
    }
}

pub(super) fn sync_device_enum_with_state(
    ctx: &mut ProcessCtx,
    parameter_id: NodeId,
    state: &golden_audio::AudioDeviceInspectorState,
    choice: DeviceChoice,
    driver: Option<&golden_audio::BackendId>,
) {
    let state = state.clone();
    let driver = driver.cloned();
    ctx.call_node_mutation_without_snapshot(parameter_id, move |node, inner_ctx| {
        let Some(parameter) = node.as_any_mut().downcast_mut::<Parameter>() else {
            return Err("Sound Card device selector is not a parameter".to_owned());
        };
        let current = parameter
            .value
            .as_enum()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| NO_AUDIO_DEVICE.to_owned());
        let options = device_options_for_current(current.as_str(), &state, choice, driver.as_ref());
        if parameter.constraints.enum_options == options {
            return Ok(());
        }
        let mut constraints = parameter.constraints.clone();
        constraints.enum_options = options;
        inner_ctx.edits.push(Edit::SetParamConstraints {
            node: parameter_id,
            constraints,
        });
        Ok(())
    });
}

pub(super) fn device_options_for_current(
    current: &str,
    state: &golden_audio::AudioDeviceInspectorState,
    choice: DeviceChoice,
    driver: Option<&golden_audio::BackendId>,
) -> Vec<golden_core::parameter::ParameterEnumOption> {
    let mut options = vec![enum_option(NO_AUDIO_DEVICE, "None", 0)];
    let mut option_targets = Vec::<(String, golden_audio::AudioDeviceTargetId)>::new();
    if driver.is_some_and(runtime::supports_system_default) {
        options.push(enum_option(SYSTEM_DEFAULT_DEVICE, "System Default", 10));
    }
    for (index, catalog_entry) in state
        .device_catalog
        .iter()
        .filter(|device| {
            driver.is_some_and(|driver| device.target.backend() == driver)
        })
        .enumerate()
    {
        let descriptors = state
            .devices
            .iter()
            .filter(|device| device.target == catalog_entry.target)
            .collect::<Vec<_>>();
        let descriptor = descriptors
            .iter()
            .copied()
            .find(|device| choice.supports(device));
        if !descriptors.is_empty() && descriptor.is_none() {
            continue;
        }
        let selection = descriptor.map_or_else(
            || {
                golden_audio::AudioDeviceSelection::from_catalog_entry(
                    catalog_entry,
                )
            },
            golden_audio::AudioDeviceSelection::from_descriptor,
        );
        let variant_id = runtime::device_selection_value(&selection);
        options.push(enum_option(
            variant_id.as_str(),
            catalog_entry.label.as_str(),
            100 + i32::try_from(index).unwrap_or(i32::MAX),
        ));
        option_targets.push((variant_id, catalog_entry.target.clone()));
    }
    options.sort_by(|left, right| {
        left.ordering
            .cmp(&right.ordering)
            .then_with(|| left.label.cmp(&right.label))
    });
    options.dedup_by(|left, right| left.variant_id == right.variant_id);

    let mut authored_selection = None;
    if !options.iter().any(|option| option.variant_id == current) {
        if let Ok(authored) = serde_json::from_str::<golden_audio::AudioDeviceSelection>(current) {
            let recovered = choice
                .direction()
                .and_then(|direction| {
                    match golden_audio::match_device_selection(
                        &authored,
                        direction,
                        state.devices.as_slice(),
                    ) {
                        golden_audio::AudioDeviceMatch::Matched(device) => {
                            Some((device.target.clone(), device.label.clone()))
                        }
                        golden_audio::AudioDeviceMatch::Missing
                        | golden_audio::AudioDeviceMatch::Ambiguous(_) => None,
                    }
                })
                .or_else(|| {
                    state
                        .devices
                        .iter()
                        .find(|device| device.target == authored.target && choice.supports(device))
                        .map(|device| (device.target.clone(), device.label.clone()))
                })
                .or_else(|| {
                    state
                        .device_catalog
                        .iter()
                        .find(|entry| entry.target == authored.target)
                        .map(|entry| (entry.target.clone(), entry.label.clone()))
                });
            if let Some((target, label)) = recovered {
                if let Some((variant, _)) = option_targets
                    .iter()
                    .find(|(_, candidate)| *candidate == target)
                {
                    if let Some(option) = options
                        .iter_mut()
                        .find(|option| option.variant_id == *variant)
                    {
                        // Keep the authored serialized identity while
                        // refreshing the user-facing live label.
                        option.variant_id = current.to_owned();
                        option.value = ParamValue::Enum(current.to_owned());
                        option.label = label;
                    }
                }
            }
            authored_selection = Some(authored);
        }
    }
    if !options.iter().any(|option| option.variant_id == current) {
        let label = authored_selection
            .as_ref()
            .map(|selection| selection.last_known_label.trim())
            .filter(|label| !label.is_empty())
            .unwrap_or("Unavailable device");
        let mut missing = enum_option(
            current,
            format!("Missing: {label}").as_str(),
            i32::MAX,
        );
        missing.tags.push("missing".to_owned());
        options.push(missing);
    }
    options
}

pub(super) fn sync_numeric_enum(ctx: &mut ProcessCtx, parameter_id: NodeId, unit: &'static str, values: &[u32]) {
    let mut options = vec![enum_option(AUTOMATIC_CONFIGURATION, "Automatic", 0)];
    options.extend(values.iter().enumerate().map(|(index, value)| {
        enum_option(
            value.to_string().as_str(),
            format!("{value} {unit}").as_str(),
            i32::try_from(index).unwrap_or(i32::MAX).saturating_add(1),
        )
    }));
    ctx.call_node_mutation_without_snapshot(parameter_id, move |node, inner_ctx| {
        let Some(parameter) = node.as_any_mut().downcast_mut::<Parameter>() else {
            return Err("Sound Card configuration selector is not a parameter".to_owned());
        };
        let current = parameter
            .value
            .as_enum()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| AUTOMATIC_CONFIGURATION.to_owned());
        if !options.iter().any(|option| option.variant_id == current) {
            let mut missing = enum_option(
                current.as_str(),
                format!("Unavailable: {current} {unit}").as_str(),
                i32::MAX,
            );
            missing.tags.push("missing".to_owned());
            options.push(missing);
        }
        if parameter.constraints.enum_options != options {
            let mut constraints = parameter.constraints.clone();
            constraints.enum_options = options;
            inner_ctx.edits.push(Edit::SetParamConstraints {
                node: parameter_id,
                constraints,
            });
        }
        Ok(())
    });
}

pub(super) fn direction_ready(status: &golden_audio::AudioStreamStatus) -> bool {
    status.enabled && status.readiness == golden_audio::AudioDeviceReadiness::Ready
}
