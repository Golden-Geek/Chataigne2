use std::collections::HashSet;
use std::net::IpAddr;

use golden_core::{
    node::Node,
    parameter::Parameter,
    parameter::{ParamValue, ParameterEnumOption},
    process_ctx::ProcessCtx,
};
use if_addrs::{get_if_addrs, IfAddr, IfOperStatus, Interface};

pub const ANY_INTERFACE_VARIANT: &str = "any";
pub const ANY_INTERFACE_BIND_HOST: &str = "0.0.0.0";

pub fn available_interface_options() -> Result<Vec<ParameterEnumOption>, String> {
    let interfaces = get_if_addrs().map_err(|error| format!("failed to enumerate network interfaces: {error}"))?;

    let mut seen = HashSet::<String>::new();
    let mut options = vec![ParameterEnumOption {
        variant_id: ANY_INTERFACE_VARIANT.to_string(),
        value: ParamValue::Enum(ANY_INTERFACE_VARIANT.to_string()),
        label: "Any".to_string(),
        tags: vec![],
        ordering: Some(0),
    }];

    let mut discovered = interfaces
        .into_iter()
        .filter_map(interface_to_option)
        .filter(|option| seen.insert(option.variant_id.clone()))
        .collect::<Vec<_>>();
    discovered.sort_by(|left, right| left.label.cmp(&right.label));
    options.extend(discovered);

    Ok(options)
}

pub fn bind_host_for_interface_variant(variant_id: &str) -> String {
    if variant_id.trim().is_empty() || variant_id == ANY_INTERFACE_VARIANT {
        ANY_INTERFACE_BIND_HOST.to_string()
    } else {
        variant_id.to_string()
    }
}

pub fn sync_interface_enum_options(
    ctx: &mut ProcessCtx,
    param_id: golden_core::node::NodeId,
    options: Vec<ParameterEnumOption>,
) {
    ctx.call_node_mutation(param_id, move |node, inner_ctx| {
        let Some(parameter) = node.as_any_mut().downcast_mut::<Parameter>() else {
            return Err("network interface target is not a parameter".to_string());
        };

        let next_value = parameter
            .value
            .as_enum()
            .filter(|current_variant| options.iter().any(|option| option.variant_id == *current_variant))
            .map(ParamValue::Enum)
            .unwrap_or_else(|| ParamValue::Enum(ANY_INTERFACE_VARIANT.to_string()));

        if parameter.constraints.enum_options == options && parameter.value == next_value {
            return Ok(());
        }

        let label = parameter.node_data().meta.label.clone();
        let change_check = parameter.change_check.clone();
        let mut replacement = Parameter::new(label.as_str(), next_value, change_check);
        *replacement.node_data_mut() = parameter.node_data().clone();
        replacement.default_value = parameter.default_value.clone();
        replacement.event_behaviour = parameter.event_behaviour;
        replacement.read_only = parameter.read_only;
        replacement.constraints = parameter.constraints.clone();
        replacement.constraints.enum_options = options.clone();
        replacement.ui_hints = parameter.ui_hints.clone();
        replacement.control = parameter.control.clone();
        replacement.control_modes_enabled = parameter.control_modes_enabled;

        inner_ctx.replace_node(param_id, replacement);

        Ok(())
    });
}

fn interface_to_option(interface: Interface) -> Option<ParameterEnumOption> {
    if matches!(
        interface.oper_status,
        IfOperStatus::Down | IfOperStatus::NotPresent | IfOperStatus::LowerLayerDown
    ) {
        return None;
    }

    let ip = match &interface.addr {
        IfAddr::V4(address) => IpAddr::V4(address.ip),
        IfAddr::V6(_) => return None,
    };

    let variant_id = ip.to_string();
    let mut tags = Vec::new();
    if interface.is_loopback() {
        tags.push("loopback".to_string());
    }

    Some(ParameterEnumOption {
        variant_id: variant_id.clone(),
        value: ParamValue::Enum(variant_id.clone()),
        label: format!("{} ({variant_id})", interface.name),
        tags,
        ordering: Some(10),
    })
}
