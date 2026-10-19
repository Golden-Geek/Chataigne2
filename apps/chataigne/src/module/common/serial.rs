mod connection;
mod discovery;
mod metadata;

pub(crate) use connection::{
    SerialConnectionConfig, SerialConnectionEvent, SerialConnectionHandle, SerialConnectionStatus,
};
pub(crate) use discovery::{SerialDiscoveryRegistration, SerialDiscoverySnapshot};
pub(crate) use metadata::{serial_port_name_for_variant, sync_serial_port_enum_options, DiscoveredSerialPort};

pub(crate) const NO_SERIAL_PORT_VARIANT: &str = "none";

#[cfg(test)]
pub(crate) use connection::spawn_for_test;
