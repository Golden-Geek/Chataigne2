use super::*;

pub(super) fn config(value: &ButtplugTransportConfig) -> Result<(), String> {
    if value.remote_host.capacity() > 1024 || value.path.capacity() > 4096 || value.client_name.capacity() > 256 {
        return Err("Buttplug host/path/client name exceeds 1024 / 4096 / 256 bytes".into());
    }
    Ok(())
}
pub(super) fn command(value: &ButtplugWorkerCommand) -> Result<(), String> {
    match value {
        ButtplugWorkerCommand::StopDevice { target } if target.capacity() > 1024 => {
            Err("Buttplug target exceeds 1024 retained bytes".into())
        }
        ButtplugWorkerCommand::SetOutput(value)
            if value.target.capacity() > 1024 || value.output.capacity() > 64 || value.description.capacity() > 256 =>
        {
            Err("Buttplug target/output/description exceeds 1024 / 64 / 256 retained bytes".into())
        }
        ButtplugWorkerCommand::SetOutput(value) if !value.value.is_finite() => {
            Err("Buttplug output must be finite".into())
        }
        _ => Ok(()),
    }
}
pub(super) fn event(value: &ButtplugWorkerEvent) -> Result<(), String> {
    fn device(value: &ButtplugDeviceInfo) -> bool {
        value.name.capacity() <= 1024
            && value.variant_id.capacity() <= 64
            && value.display_name.as_ref().is_none_or(|value| value.capacity() <= 1024)
            && value.outputs.capacity() <= 16
            && value.outputs.iter().all(|value| value.capacity() <= 64)
    }
    let valid = match value {
        ButtplugWorkerEvent::Devices(devices) => devices.capacity() <= 64 && devices.iter().all(device),
        ButtplugWorkerEvent::DeviceAdded(value) | ButtplugWorkerEvent::DeviceRemoved(value) => device(value),
        ButtplugWorkerEvent::Status(ButtplugConnectionStatus::Connected { server_name }) => {
            server_name.capacity() <= 1024
        }
        ButtplugWorkerEvent::Status(ButtplugConnectionStatus::Recovering { message })
        | ButtplugWorkerEvent::Error(message) => message.capacity() <= 1024,
        _ => true,
    };
    if valid {
        Ok(())
    } else {
        Err("Buttplug event exceeds bounded metadata/catalog admission".into())
    }
}
pub(super) fn diagnostic(mut value: String) -> String {
    let mut end = value.len().min(1024);
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value.truncate(end);
    value.shrink_to_fit();
    value
}
