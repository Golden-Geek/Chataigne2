use super::NO_SERIAL_PORT_VARIANT;
use golden_core::{
    node::{Node, NodeId},
    parameter::{ParamValue, Parameter, ParameterEnumOption},
    process_ctx::ProcessCtx,
};
use serialport::{SerialPortInfo, SerialPortType, UsbPortInfo};
#[cfg(not(windows))]
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DiscoveredSerialPort {
    pub port_name: String,
    pub label: String,
    pub tags: Vec<String>,
    pub details: String,
}

pub(crate) fn serial_port_options(ports: &[DiscoveredSerialPort]) -> Vec<ParameterEnumOption> {
    let mut options = vec![no_serial_port_option()];
    options.extend(ports.iter().map(discovered_serial_port_to_option));
    options
}

pub(crate) fn sync_serial_port_enum_options(
    ctx: &mut ProcessCtx,
    param_id: NodeId,
    catalog: Option<golden_io::RetainedPayload<super::discovery::SerialCatalog>>,
) {
    ctx.call_node_mutation(param_id, move |node, inner_ctx| {
        let Some(parameter) = node.as_any_mut().downcast_mut::<Parameter>() else {
            return Err("serial port target is not a parameter".to_string());
        };

        let mut next_options = catalog
            .as_ref()
            .map_or_else(|| serial_port_options(&[]), |catalog| catalog.options.clone());
        let current_variant = match &parameter.value {
            ParamValue::Enum(value) if value.len() > 1024 => return Err("Serial selection exceeds 1024 bytes".into()),
            ParamValue::Enum(value) if !value.trim().is_empty() => value.clone(),
            _ => NO_SERIAL_PORT_VARIANT.to_string(),
        };

        if current_variant != NO_SERIAL_PORT_VARIANT
            && !next_options.iter().any(|option| option.variant_id == current_variant)
        {
            next_options.insert(1, missing_serial_port_option(current_variant.as_str()));
        }

        let next_value = if next_options.iter().any(|option| option.variant_id == current_variant) {
            ParamValue::Enum(current_variant.clone())
        } else {
            ParamValue::Enum(NO_SERIAL_PORT_VARIANT.to_string())
        };

        if parameter.constraints.enum_options == next_options && parameter.value == next_value {
            return Ok(());
        }

        let label = parameter.node_data().meta.label.clone();
        let change_check = parameter.change_check.clone();
        let mut replacement = Parameter::new(label.as_str(), next_value, change_check);
        *replacement.node_data_mut() = parameter.node_data().clone();
        replacement.default_value = parameter.default_value.clone();
        replacement.event_behaviour = parameter.event_behaviour;
        replacement.read_only = parameter.read_only;
        replacement.persist_read_only_value = parameter.persist_read_only_value;
        replacement.constraints = parameter.constraints.clone();
        replacement.constraints.enum_options = next_options;
        replacement.ui_hints = parameter.ui_hints.clone();
        replacement.control = parameter.control.clone();
        replacement.control_modes_enabled = parameter.control_modes_enabled;

        inner_ctx.replace_node(param_id, replacement);

        Ok(())
    });
}

pub(crate) fn serial_port_name_for_variant(variant_id: &str) -> Option<String> {
    let trimmed = variant_id.trim();
    if trimmed.is_empty() || trimmed == NO_SERIAL_PORT_VARIANT {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn no_serial_port_option() -> ParameterEnumOption {
    ParameterEnumOption {
        variant_id: NO_SERIAL_PORT_VARIANT.to_string(),
        value: ParamValue::Enum(NO_SERIAL_PORT_VARIANT.to_string()),
        label: "No Port".to_string(),
        tags: vec![],
        ordering: Some(0),
    }
}

fn missing_serial_port_option(port_name: &str) -> ParameterEnumOption {
    let variant_id = port_name.to_string();
    ParameterEnumOption {
        variant_id: variant_id.clone(),
        value: ParamValue::Enum(variant_id.clone()),
        label: format!("Missing: {}", human_serial_port_name(variant_id.as_str())),
        tags: vec!["missing".to_string()],
        ordering: Some(5),
    }
}

pub(super) fn describe_serial_port(port: SerialPortInfo) -> DiscoveredSerialPort {
    DiscoveredSerialPort {
        port_name: port.port_name.clone(),
        label: format_serial_port_label(&port),
        tags: serial_port_tags(&port.port_type),
        details: format_serial_port_details(&port),
    }
}

fn discovered_serial_port_to_option(port: &DiscoveredSerialPort) -> ParameterEnumOption {
    let variant_id = port.port_name.clone();
    ParameterEnumOption {
        variant_id: variant_id.clone(),
        value: ParamValue::Enum(variant_id.clone()),
        label: port.label.clone(),
        tags: port.tags.clone(),
        ordering: Some(10),
    }
}

fn serial_port_tags(port_type: &SerialPortType) -> Vec<String> {
    match port_type {
        SerialPortType::UsbPort(_) => vec!["usb".to_string()],
        SerialPortType::BluetoothPort => vec!["bluetooth".to_string()],
        SerialPortType::PciPort => vec!["pci".to_string()],
        SerialPortType::Unknown => vec![],
    }
}

fn format_serial_port_label(port: &SerialPortInfo) -> String {
    let endpoint = human_serial_port_name(port.port_name.as_str());
    match &port.port_type {
        SerialPortType::UsbPort(info) => {
            let identity = format_usb_port_identity(info);
            format!("{identity} - {endpoint}")
        }
        SerialPortType::BluetoothPort => format!("Bluetooth Serial - {endpoint}"),
        SerialPortType::PciPort => format!("System Serial - {endpoint}"),
        SerialPortType::Unknown => endpoint,
    }
}

fn format_serial_port_details(port: &SerialPortInfo) -> String {
    let mut details = vec![format!("port={}", port.port_name)];

    match &port.port_type {
        SerialPortType::UsbPort(info) => {
            details.push("transport=usb".to_string());
            details.push(format!("vid={:04X}", info.vid));
            details.push(format!("pid={:04X}", info.pid));
            push_serial_port_detail(&mut details, "manufacturer", info.manufacturer.as_deref());
            push_serial_port_detail(&mut details, "product", info.product.as_deref());
            push_serial_port_detail(&mut details, "serial", info.serial_number.as_deref());
        }
        SerialPortType::BluetoothPort => {
            details.push("transport=bluetooth".to_string());
        }
        SerialPortType::PciPort => {
            details.push("transport=system".to_string());
        }
        SerialPortType::Unknown => {
            details.push("transport=unknown".to_string());
        }
    }

    details.join(", ")
}

fn format_usb_port_identity(info: &UsbPortInfo) -> String {
    let mut parts = Vec::<String>::new();

    if let Some(manufacturer) = clean_port_metadata(info.manufacturer.as_deref()) {
        parts.push(manufacturer);
    }

    if let Some(product) = clean_port_metadata(info.product.as_deref()) {
        if !parts.iter().any(|part| part.eq_ignore_ascii_case(product.as_str())) {
            parts.push(product);
        }
    }

    if parts.is_empty() {
        if let Some(serial_number) = clean_port_metadata(info.serial_number.as_deref()) {
            return format!("USB Serial {serial_number}");
        }

        return format!("USB Serial {:04x}:{:04x}", info.vid, info.pid);
    }

    parts.join(" ")
}

fn clean_port_metadata(value: Option<&str>) -> Option<String> {
    let trimmed = value?.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("n/a") || trimmed.eq_ignore_ascii_case("unknown") {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn push_serial_port_detail(target: &mut Vec<String>, key: &str, value: Option<&str>) {
    if let Some(value) = clean_port_metadata(value) {
        target.push(format!("{key}={value}"));
    }
}

fn human_serial_port_name(port_name: &str) -> String {
    #[cfg(windows)]
    {
        port_name.to_string()
    }

    #[cfg(not(windows))]
    {
        let short_name = Path::new(port_name)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(port_name);
        if short_name == port_name {
            port_name.to_string()
        } else {
            format!("{short_name} ({port_name})")
        }
    }
}
