use buttplug::{
    connector::ButtplugRemoteClientConnector,
    device::{ButtplugClientDevice, ClientDeviceCommandValue, ClientDeviceOutputCommand},
    ButtplugClient, ButtplugClientEvent, ButtplugWebsocketClientTransport,
};
use buttplug_core::message::OutputType;
use futures_util::{Stream, StreamExt};
use std::{pin::Pin, time::Duration};
use tokio::{runtime::Builder, time::timeout};

use super::*;
use crate::app::module::common::buttplug::{
    ButtplugSetOutputRequest, BUTTPLUG_DEVICE_VARIANT_PREFIX, BUTTPLUG_OUTPUT_CONSTRICT,
    BUTTPLUG_OUTPUT_HW_POSITION_WITH_DURATION, BUTTPLUG_OUTPUT_LED, BUTTPLUG_OUTPUT_OSCILLATE,
    BUTTPLUG_OUTPUT_POSITION, BUTTPLUG_OUTPUT_ROTATE, BUTTPLUG_OUTPUT_SPRAY, BUTTPLUG_OUTPUT_TEMPERATURE,
    BUTTPLUG_OUTPUT_VIBRATE, BUTTPLUG_TARGET_ALL, BUTTPLUG_TARGET_NONE,
};

const BUTTPLUG_CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
type ClientEventStream = Pin<Box<dyn Stream<Item = ButtplugClientEvent> + Send>>;
struct ActiveButtplugClient {
    client: ButtplugClient,
    events: ClientEventStream,
}

pub(super) struct Native {
    runtime: tokio::runtime::Runtime,
    active: Option<ActiveButtplugClient>,
}
impl Native {
    pub(super) fn open() -> Result<Self, String> {
        if cfg!(test) {
            return Err("Native Buttplug endpoints are disabled in tests".into());
        }
        Ok(Self {
            runtime: Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| error.to_string())?,
            active: None,
        })
    }
}
impl worker::Provider for Native {
    fn connect(&mut self, config: &ButtplugTransportConfig) -> Result<Vec<ButtplugWorkerEvent>, String> {
        self.active = Some(self.runtime.block_on(connect_client(config))?);
        let active = self.active.as_ref().unwrap();
        let name = active.client.server_name().unwrap_or_else(|| "Buttplug Server".into());
        if name.len() > 1024 {
            return Err("Buttplug server name exceeds 1024 bytes".into());
        }
        let mut events = vec![
            ButtplugWorkerEvent::Status(ButtplugConnectionStatus::Connected { server_name: name }),
            devices(&active.client)?,
        ];
        if config.auto_scan {
            match self.runtime.block_on(timeout_call(active.client.start_scanning())) {
                Ok(()) => events.push(ButtplugWorkerEvent::Scanning(true)),
                Err(error) => events.push(ButtplugWorkerEvent::Error(error)),
            }
        }
        Ok(events)
    }
    fn poll(&mut self) -> Result<Vec<ButtplugWorkerEvent>, String> {
        let Some(active) = &mut self.active else {
            return Ok(Vec::new());
        };
        let event = self
            .runtime
            .block_on(async { timeout(Duration::from_millis(10), active.events.next()).await });
        let event = match event {
            Err(_) => return Ok(Vec::new()),
            Ok(Some(event)) => event,
            Ok(None) => return Err("Buttplug event stream closed".into()),
        };
        match event {
            ButtplugClientEvent::ServerDisconnect | ButtplugClientEvent::PingTimeout => {
                Err("Buttplug server disconnected".into())
            }
            ButtplugClientEvent::ServerConnect => Ok(Vec::new()),
            ButtplugClientEvent::DeviceListReceived => Ok(vec![devices(&active.client)?]),
            ButtplugClientEvent::DeviceAdded(device) => Ok(vec![
                ButtplugWorkerEvent::DeviceAdded(device_info(&device)?),
                devices(&active.client)?,
            ]),
            ButtplugClientEvent::DeviceRemoved(device) => Ok(vec![
                ButtplugWorkerEvent::DeviceRemoved(device_info(&device)?),
                devices(&active.client)?,
            ]),
            ButtplugClientEvent::ScanningFinished => Ok(vec![
                ButtplugWorkerEvent::ScanningFinished,
                ButtplugWorkerEvent::Scanning(false),
            ]),
            ButtplugClientEvent::Error(error) => Ok(vec![ButtplugWorkerEvent::Error(admission::diagnostic(
                error.to_string(),
            ))]),
        }
    }
    fn execute(&mut self, command: &ButtplugWorkerCommand) -> Result<String, String> {
        let active = self
            .active
            .as_mut()
            .ok_or("Buttplug transport is not connected; request was not sent")?;
        self.runtime.block_on(timeout_call(async {
            match command {
                ButtplugWorkerCommand::StartScanning => active
                    .client
                    .start_scanning()
                    .await
                    .map(|()| "started scanning".into())
                    .map_err(|error| error.to_string()),
                ButtplugWorkerCommand::StopScanning => active
                    .client
                    .stop_scanning()
                    .await
                    .map(|()| "stopped scanning".into())
                    .map_err(|error| error.to_string()),
                ButtplugWorkerCommand::StopAllDevices => active
                    .client
                    .stop_all_devices()
                    .await
                    .map(|()| "stopped all devices".into())
                    .map_err(|error| error.to_string()),
                ButtplugWorkerCommand::StopDevice { target } => stop_target_devices(&active.client, target).await,
                ButtplugWorkerCommand::SetOutput(request) => set_target_output(&active.client, request).await,
            }
        }))
    }
    fn disconnect(&mut self) {
        if let Some(active) = self.active.take() {
            self.runtime.block_on(async {
                let _ = timeout(Duration::from_secs(2), async {
                    let _ = active.client.stop_all_devices().await;
                    if active.client.connected() {
                        let _ = active.client.disconnect().await;
                    }
                })
                .await;
            });
        }
    }
}
impl Drop for Native {
    fn drop(&mut self) {
        worker::Provider::disconnect(self);
    }
}
async fn timeout_call<T, E: std::fmt::Display>(
    future: impl std::future::Future<Output = Result<T, E>>,
) -> Result<T, String> {
    timeout(Duration::from_secs(3), future)
        .await
        .map_err(|_| "Buttplug operation timed out; result unconfirmed".to_string())?
        .map_err(|error| admission::diagnostic(error.to_string()))
}
fn devices(client: &ButtplugClient) -> Result<ButtplugWorkerEvent, String> {
    let devices = client.devices();
    if devices.len() > 64 {
        return Err("Buttplug device inventory exceeds 64 entries".into());
    }
    Ok(ButtplugWorkerEvent::Devices(
        devices.values().map(device_info).collect::<Result<Vec<_>, _>>()?,
    ))
}

async fn connect_client(config: &ButtplugTransportConfig) -> Result<ActiveButtplugClient, String> {
    let client = ButtplugClient::new(config.client_name.as_str());
    let transport = if config.secure {
        ButtplugWebsocketClientTransport::new_secure_connector(
            buttplug_websocket_url(config).as_str(),
            config.bypass_certificate_verification,
        )
    } else {
        ButtplugWebsocketClientTransport::new_insecure_connector(buttplug_websocket_url(config).as_str())
    };
    let connector: ButtplugRemoteClientConnector<ButtplugWebsocketClientTransport> =
        ButtplugRemoteClientConnector::new(transport);

    timeout(BUTTPLUG_CONNECT_TIMEOUT, client.connect(connector))
        .await
        .map_err(|_| {
            format!(
                "timed out connecting to Buttplug server at {}",
                buttplug_websocket_url(config)
            )
        })?
        .map_err(|error| {
            format!(
                "failed to connect to Buttplug server at {}: {error}",
                buttplug_websocket_url(config)
            )
        })?;

    let events = Box::pin(client.event_stream());
    Ok(ActiveButtplugClient { client, events })
}

async fn stop_target_devices(client: &ButtplugClient, target: &str) -> Result<String, String> {
    let devices = resolve_target_devices(client, target)?;
    for device in &devices {
        device
            .stop()
            .await
            .map_err(|error| format!("failed to stop Buttplug device '{}': {error}", device_label(device)))?;
    }

    Ok(format!("stopped {} Buttplug device(s)", devices.len()))
}

async fn set_target_output(client: &ButtplugClient, request: &ButtplugSetOutputRequest) -> Result<String, String> {
    let devices = resolve_target_devices(client, request.target.as_str())?;
    let command = output_command(request)?;
    let supported_devices = devices
        .into_iter()
        .filter(|device| device_supports_output(device, request.output.as_str()))
        .collect::<Vec<_>>();

    if supported_devices.is_empty() {
        return Err(format!(
            "no selected Buttplug device supports {} output",
            request.output
        ));
    }

    for device in &supported_devices {
        device.run_output(&command).await.map_err(|error| {
            format!(
                "failed to set {} on Buttplug device '{}': {error}",
                request.output,
                device_label(device)
            )
        })?;
    }

    Ok(format!(
        "set {} on {} Buttplug device(s)",
        request.output,
        supported_devices.len()
    ))
}

fn resolve_target_devices(client: &ButtplugClient, target: &str) -> Result<Vec<ButtplugClientDevice>, String> {
    let devices = client.devices();
    if devices.len() > 64 {
        return Err("Buttplug device inventory exceeds 64 entries".into());
    }
    if devices.is_empty() {
        return Err("no Buttplug devices are connected".to_string());
    }

    let target = target.trim();
    if target.is_empty() || target.eq_ignore_ascii_case(BUTTPLUG_TARGET_ALL) {
        return Ok(devices.into_values().collect());
    }
    if target.eq_ignore_ascii_case(BUTTPLUG_TARGET_NONE) {
        return Err("no Buttplug device is selected".to_string());
    }

    let index = target
        .strip_prefix(BUTTPLUG_DEVICE_VARIANT_PREFIX)
        .unwrap_or(target)
        .parse::<u32>()
        .ok();
    if let Some(index) = index {
        return devices
            .get(&index)
            .cloned()
            .map(|device| vec![device])
            .ok_or_else(|| format!("Buttplug device index {index} is not connected"));
    }

    let lower_target = target.to_ascii_lowercase();
    let matches = devices
        .into_values()
        .filter(|device| {
            device.name().eq_ignore_ascii_case(target)
                || device
                    .display_name()
                    .as_deref()
                    .is_some_and(|display_name| display_name.eq_ignore_ascii_case(target))
                || device_label(device).to_ascii_lowercase() == lower_target
        })
        .collect::<Vec<_>>();

    if matches.is_empty() {
        Err(format!("Buttplug device '{target}' is not connected"))
    } else {
        Ok(matches)
    }
}

pub(super) fn output_command(request: &ButtplugSetOutputRequest) -> Result<ClientDeviceOutputCommand, String> {
    if !(0.0..=1.0).contains(&request.value) {
        return Err(format!(
            "Buttplug output value must be between 0.0 and 1.0, got {}",
            request.value
        ));
    }

    let value = ClientDeviceCommandValue::Percent(request.value);
    match request.output.as_str() {
        BUTTPLUG_OUTPUT_VIBRATE => Ok(ClientDeviceOutputCommand::Vibrate(value)),
        BUTTPLUG_OUTPUT_ROTATE => Ok(ClientDeviceOutputCommand::Rotate(value)),
        BUTTPLUG_OUTPUT_OSCILLATE => Ok(ClientDeviceOutputCommand::Oscillate(value)),
        BUTTPLUG_OUTPUT_CONSTRICT => Ok(ClientDeviceOutputCommand::Constrict(value)),
        BUTTPLUG_OUTPUT_POSITION => Ok(ClientDeviceOutputCommand::Position(value)),
        BUTTPLUG_OUTPUT_HW_POSITION_WITH_DURATION => Ok(ClientDeviceOutputCommand::HwPositionWithDuration(
            value,
            request.duration_ms.max(1),
        )),
        BUTTPLUG_OUTPUT_SPRAY => Ok(ClientDeviceOutputCommand::Spray(value)),
        BUTTPLUG_OUTPUT_LED => Ok(ClientDeviceOutputCommand::Led(value)),
        BUTTPLUG_OUTPUT_TEMPERATURE => Ok(ClientDeviceOutputCommand::Temperature(value)),
        other => Err(format!("unsupported Buttplug output '{other}'")),
    }
}

fn buttplug_websocket_url(config: &ButtplugTransportConfig) -> String {
    let scheme = if config.secure { "wss" } else { "ws" };
    format!(
        "{scheme}://{}:{}{}",
        config.remote_host,
        config.remote_port,
        normalize_path(config.path.as_str())
    )
}

pub(super) fn normalize_path(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        "/".to_string()
    } else if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{trimmed}")
    }
}

fn device_info(device: &ButtplugClientDevice) -> Result<ButtplugDeviceInfo, String> {
    if device.name().len() > 1024 || device.display_name().as_ref().is_some_and(|name| name.len() > 1024) {
        return Err("Buttplug device metadata exceeds 1024 bytes".into());
    }
    Ok(ButtplugDeviceInfo {
        index: device.index(),
        variant_id: format!("{BUTTPLUG_DEVICE_VARIANT_PREFIX}{}", device.index()),
        name: device.name().clone(),
        display_name: device.display_name().clone(),
        outputs: device_outputs(device),
    })
}

fn device_outputs(device: &ButtplugClientDevice) -> Vec<String> {
    let mut outputs = Vec::new();
    push_output(
        &mut outputs,
        device.output_available(OutputType::Vibrate),
        BUTTPLUG_OUTPUT_VIBRATE,
    );
    push_output(
        &mut outputs,
        device.output_available(OutputType::Rotate),
        BUTTPLUG_OUTPUT_ROTATE,
    );
    push_output(
        &mut outputs,
        device.output_available(OutputType::Oscillate),
        BUTTPLUG_OUTPUT_OSCILLATE,
    );
    push_output(
        &mut outputs,
        device.output_available(OutputType::Constrict),
        BUTTPLUG_OUTPUT_CONSTRICT,
    );
    push_output(
        &mut outputs,
        device.output_available(OutputType::Position),
        BUTTPLUG_OUTPUT_POSITION,
    );
    push_output(
        &mut outputs,
        device.output_available(OutputType::HwPositionWithDuration),
        BUTTPLUG_OUTPUT_HW_POSITION_WITH_DURATION,
    );
    push_output(
        &mut outputs,
        device.output_available(OutputType::Spray),
        BUTTPLUG_OUTPUT_SPRAY,
    );
    push_output(
        &mut outputs,
        device.output_available(OutputType::Led),
        BUTTPLUG_OUTPUT_LED,
    );
    push_output(
        &mut outputs,
        device.output_available(OutputType::Temperature),
        BUTTPLUG_OUTPUT_TEMPERATURE,
    );
    outputs
}

fn push_output(outputs: &mut Vec<String>, supported: bool, output: &str) {
    if supported {
        outputs.push(output.to_string());
    }
}

fn device_supports_output(device: &ButtplugClientDevice, output: &str) -> bool {
    device_outputs(device).iter().any(|supported| supported == output)
}

fn device_label(device: &ButtplugClientDevice) -> String {
    device.display_name().clone().unwrap_or_else(|| device.name().clone())
}
