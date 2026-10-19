use super::DiscoveredSerialPort;
use crate::app::module::common::{serial::metadata::serial_port_options, streaming::admission::retain_payload};
use golden_core::parameter::{ParamValue, ParameterEnumOption};
use golden_io::RetainedPayload;
use std::mem::size_of;

pub(super) const MAX_PORTS: usize = 128;
const MAX_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug)]
pub(crate) struct SerialCatalog {
    pub ports: Vec<DiscoveredSerialPort>,
    pub options: Vec<ParameterEnumOption>,
}
impl SerialCatalog {
    pub(super) fn prepare(ports: Vec<DiscoveredSerialPort>) -> Result<RetainedPayload<Self>, String> {
        Self::validate(&ports)?;
        let options = serial_port_options(&ports);
        let mut weight = size_of::<Self>()
            + ports.capacity() * size_of::<DiscoveredSerialPort>()
            + options.capacity() * size_of::<ParameterEnumOption>();
        for port in &ports {
            weight += port.port_name.capacity()
                + port.label.capacity()
                + port.details.capacity()
                + port.tags.capacity() * size_of::<String>()
                + port.tags.iter().map(String::capacity).sum::<usize>();
        }
        for option in &options {
            weight += option.variant_id.capacity()
                + option.label.capacity()
                + option.tags.capacity() * size_of::<String>()
                + option.tags.iter().map(String::capacity).sum::<usize>();
            if let ParamValue::Enum(value) = &option.value {
                weight += value.capacity();
            }
        }
        if weight > MAX_BYTES {
            return Err("Serial catalog exceeds 2 MiB retained storage".into());
        }
        retain_payload(Self { ports, options }, weight)
    }
    pub(super) fn validate(ports: &Vec<DiscoveredSerialPort>) -> Result<(), String> {
        if ports.capacity() > MAX_PORTS {
            return Err("Serial catalog exceeds 128 retained ports".into());
        }
        for port in ports {
            if port.port_name.capacity() > 1024
                || port.port_name.trim().is_empty()
                || port.port_name == super::super::NO_SERIAL_PORT_VARIANT
                || port.port_name.contains('\0')
                || port.label.capacity() > 512
                || port.details.capacity() > 4096
                || port.tags.capacity() > 8
                || port.tags.iter().any(|tag| tag.capacity() > 64)
            {
                return Err("Serial port metadata exceeds retained field bounds".into());
            }
        }
        Ok(())
    }
}
