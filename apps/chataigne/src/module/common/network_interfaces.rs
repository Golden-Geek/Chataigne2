use std::collections::HashSet;
use std::net::IpAddr;

use golden_core::{
    edit::Edit,
    node::Node,
    parameter::Parameter,
    parameter::{ParamValue, ParameterEnumOption},
    process_ctx::ProcessCtx,
};
use if_addrs::{get_if_addrs, IfAddr, IfOperStatus, Interface};

#[cfg(test)]
mod tests;

pub const ANY_INTERFACE_VARIANT: &str = "any";
pub const ANY_INTERFACE_BIND_HOST: &str = "0.0.0.0";

pub fn available_interface_options() -> Result<Vec<ParameterEnumOption>, String> {
    let interfaces = get_if_addrs().map_err(|error| format!("failed to enumerate network interfaces: {error}"))?;
    Ok(options_for_interfaces(interfaces))
}

/// Native allocation and conversion stay on the caller's IO worker. Validate the
/// native inventory before constructing app-owned labels or option collections.
pub(crate) fn bounded_interface_options() -> Result<Vec<ParameterEnumOption>, String> {
    let interfaces = get_if_addrs().map_err(|error| format!("failed to enumerate network interfaces: {error}"))?;
    if interfaces.capacity() > 256 || interfaces.iter().any(|interface| interface.name.capacity() > 512) {
        return Err("network inventory exceeds 256 interfaces / 512-byte names".into());
    }
    let options = options_for_interfaces(interfaces);
    if options.len() > 64 {
        return Err("network interface catalog exceeds 64 options".into());
    }
    Ok(options)
}

fn options_for_interfaces(interfaces: Vec<Interface>) -> Vec<ParameterEnumOption> {
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

    options
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
    let options: golden_core::parameter::ParameterEnumOptions = options.into();
    ctx.call_node_mutation_without_snapshot(param_id, move |node, inner_ctx| {
        let Some(parameter) = node.as_any_mut().downcast_mut::<Parameter>() else {
            return Ok(());
        };
        // Retain a configured endpoint while its interface is absent. The native
        // transport owns retry/recovery; discovery must never select another NIC.
        if parameter.constraints.enum_options == options || !options.contains_value(&parameter.value) {
            return Ok(());
        }
        inner_ctx.edits.push(Edit::SetParamEnumOptions {
            node: param_id,
            expected_uuid: parameter.node_data().meta.uuid,
            expected_value: parameter.value.clone(),
            selected_value: None,
            options,
        });
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
