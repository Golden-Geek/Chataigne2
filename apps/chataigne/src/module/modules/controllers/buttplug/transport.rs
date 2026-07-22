use std::{
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
        Arc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use buttplug::{
    connector::ButtplugRemoteClientConnector,
    device::{ButtplugClientDevice, ClientDeviceCommandValue, ClientDeviceOutputCommand},
    ButtplugClient, ButtplugClientEvent, ButtplugWebsocketClientTransport,
};
use buttplug_core::message::OutputType;
use futures_util::{Stream, StreamExt};
use golden_io::ReconnectBackoff;
use tokio::{runtime::Builder, time::timeout};

use crate::app::module::common::buttplug::{
    ButtplugSetOutputRequest, BUTTPLUG_DEVICE_VARIANT_PREFIX, BUTTPLUG_OUTPUT_CONSTRICT,
    BUTTPLUG_OUTPUT_HW_POSITION_WITH_DURATION, BUTTPLUG_OUTPUT_LED, BUTTPLUG_OUTPUT_OSCILLATE,
    BUTTPLUG_OUTPUT_POSITION, BUTTPLUG_OUTPUT_ROTATE, BUTTPLUG_OUTPUT_SPRAY, BUTTPLUG_OUTPUT_TEMPERATURE,
    BUTTPLUG_OUTPUT_VIBRATE, BUTTPLUG_TARGET_ALL, BUTTPLUG_TARGET_NONE,
};

const BUTTPLUG_WORKER_POLL_INTERVAL: Duration = Duration::from_millis(10);
const BUTTPLUG_RECONNECT_BASE_DELAY: Duration = Duration::from_millis(250);
const BUTTPLUG_RECONNECT_MAX_DELAY: Duration = Duration::from_secs(5);
const BUTTPLUG_CONNECT_TIMEOUT: Duration = Duration::from_secs(3);

type ClientEventStream = Pin<Box<dyn Stream<Item = ButtplugClientEvent> + Send>>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ButtplugTransportConfig {
    pub remote_host: String,
    pub remote_port: u16,
    pub path: String,
    pub secure: bool,
    pub bypass_certificate_verification: bool,
    pub client_name: String,
    pub auto_scan: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ButtplugDeviceInfo {
    pub index: u32,
    pub variant_id: String,
    pub name: String,
    pub display_name: Option<String>,
    pub outputs: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ButtplugConnectionStatus {
    Connected { server_name: String },
    Recovering { message: String },
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ButtplugWorkerEvent {
    Status(ButtplugConnectionStatus),
    Devices(Vec<ButtplugDeviceInfo>),
    DeviceAdded(ButtplugDeviceInfo),
    DeviceRemoved(ButtplugDeviceInfo),
    Scanning(bool),
    ScanningFinished,
    CommandResult(String),
    Error(String),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ButtplugWorkerCommand {
    StartScanning,
    StopScanning,
    StopAllDevices,
    StopDevice { target: String },
    SetOutput(ButtplugSetOutputRequest),
    Stop,
}

pub(crate) struct ButtplugTransportHandle {
    command_tx: Sender<ButtplugWorkerCommand>,
    event_rx: Receiver<ButtplugWorkerEvent>,
    connected: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl ButtplugTransportHandle {
    pub(crate) fn spawn(config: ButtplugTransportConfig) -> Result<Self, String> {
        let (command_tx, command_rx) = mpsc::channel();
        let (event_tx, event_rx) = mpsc::channel();
        let connected = Arc::new(AtomicBool::new(false));
        let worker_connected = Arc::clone(&connected);
        let thread_name = format!("buttplug-{}-{}", config.remote_host, config.remote_port);

        let worker = thread::Builder::new()
            .name(thread_name)
            .spawn(move || worker_loop(config, command_rx, event_tx, worker_connected))
            .map_err(|error| format!("failed to start Buttplug worker thread: {error}"))?;

        Ok(Self {
            command_tx,
            event_rx,
            connected,
            worker: Some(worker),
        })
    }

    pub(crate) fn send(&self, command: ButtplugWorkerCommand) -> Result<(), String> {
        if !matches!(command, ButtplugWorkerCommand::Stop) && !self.connected.load(Ordering::Acquire) {
            return Err("Buttplug transport is not connected".to_string());
        }

        self.command_tx
            .send(command)
            .map_err(|_| "Buttplug worker is no longer running".to_string())
    }

    pub(crate) fn try_recv(&self) -> Result<ButtplugWorkerEvent, mpsc::TryRecvError> {
        self.event_rx.try_recv()
    }

    pub(crate) fn stop(&mut self) {
        let _ = self.command_tx.send(ButtplugWorkerCommand::Stop);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Drop for ButtplugTransportHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

struct ActiveButtplugClient {
    client: ButtplugClient,
    events: ClientEventStream,
}

fn worker_loop(
    config: ButtplugTransportConfig,
    command_rx: Receiver<ButtplugWorkerCommand>,
    event_tx: Sender<ButtplugWorkerEvent>,
    connected: Arc<AtomicBool>,
) {
    let runtime = match Builder::new_current_thread().enable_all().build() {
        Ok(runtime) => runtime,
        Err(error) => {
            let _ = event_tx.send(ButtplugWorkerEvent::Error(format!(
                "failed to build Buttplug runtime: {error}"
            )));
            return;
        }
    };

    runtime.block_on(async_worker_loop(config, command_rx, event_tx, connected));
}

async fn async_worker_loop(
    config: ButtplugTransportConfig,
    command_rx: Receiver<ButtplugWorkerCommand>,
    event_tx: Sender<ButtplugWorkerEvent>,
    connected: Arc<AtomicBool>,
) {
    let mut active = None;
    let mut reconnect_delay = ReconnectBackoff::new(
        BUTTPLUG_RECONNECT_BASE_DELAY,
        BUTTPLUG_RECONNECT_MAX_DELAY,
    );
    let mut next_connect_at = Instant::now();
    let mut last_status = None;

    loop {
        if active.is_none() && Instant::now() >= next_connect_at {
            match connect_client(&config).await {
                Ok(next_active) => {
                    let server_name = next_active
                        .client
                        .server_name()
                        .unwrap_or_else(|| "Buttplug Server".to_string());
                    connected.store(true, Ordering::Release);
                    reconnect_delay.reset();
                    emit_status(
                        &event_tx,
                        &mut last_status,
                        ButtplugConnectionStatus::Connected { server_name },
                    );
                    emit_devices(&event_tx, &next_active.client);
                    if config.auto_scan {
                        match next_active.client.start_scanning().await {
                            Ok(()) => emit_event(&event_tx, ButtplugWorkerEvent::Scanning(true)),
                            Err(error) => emit_event(
                                &event_tx,
                                ButtplugWorkerEvent::Error(format!(
                                    "failed to start Buttplug scanning: {error}"
                                )),
                            ),
                        };
                    }
                    active = Some(next_active);
                }
                Err(error) => {
                    connected.store(false, Ordering::Release);
                    next_connect_at = schedule_reconnect(&mut reconnect_delay);
                    if !emit_status(
                        &event_tx,
                        &mut last_status,
                        ButtplugConnectionStatus::Recovering { message: error },
                    ) {
                        return;
                    }
                }
            }
        }

        if drain_commands(&command_rx, &event_tx, active.as_mut()).await {
            break;
        }

        let mut disconnect_reason = None;
        if let Some(active_client) = active.as_mut() {
            tokio::select! {
                event = active_client.events.next() => {
                    match event {
                        Some(event) => {
                            if !handle_client_event(&event_tx, active_client, event).await {
                                disconnect_reason = Some("Buttplug server disconnected".to_string());
                            }
                        }
                        None => disconnect_reason = Some("Buttplug event stream closed".to_string()),
                    }
                }
                _ = tokio::time::sleep(BUTTPLUG_WORKER_POLL_INTERVAL) => {}
            }
        } else {
            tokio::time::sleep(BUTTPLUG_WORKER_POLL_INTERVAL).await;
        }

        if let Some(reason) = disconnect_reason {
            close_active_client(&mut active).await;
            connected.store(false, Ordering::Release);
            next_connect_at = schedule_reconnect(&mut reconnect_delay);
            if !emit_status(
                &event_tx,
                &mut last_status,
                ButtplugConnectionStatus::Recovering { message: reason },
            ) {
                return;
            }
        }
    }

    close_active_client(&mut active).await;
    connected.store(false, Ordering::Release);
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

async fn drain_commands(
    command_rx: &Receiver<ButtplugWorkerCommand>,
    event_tx: &Sender<ButtplugWorkerEvent>,
    active: Option<&mut ActiveButtplugClient>,
) -> bool {
    let mut active = active;
    loop {
        let command = match command_rx.try_recv() {
            Ok(command) => command,
            Err(mpsc::TryRecvError::Empty) => return false,
            Err(mpsc::TryRecvError::Disconnected) => return true,
        };

        if matches!(command, ButtplugWorkerCommand::Stop) {
            return true;
        }

        let Some(active_client) = active.as_deref_mut() else {
            emit_event(
                event_tx,
                ButtplugWorkerEvent::Error("Buttplug transport is not connected".to_string()),
            );
            continue;
        };

        handle_command(event_tx, active_client, command).await;
    }
}

async fn handle_command(
    event_tx: &Sender<ButtplugWorkerEvent>,
    active: &mut ActiveButtplugClient,
    command: ButtplugWorkerCommand,
) {
    let result = match command {
        ButtplugWorkerCommand::StartScanning => match active.client.start_scanning().await {
            Ok(()) => {
                emit_event(event_tx, ButtplugWorkerEvent::Scanning(true));
                Ok("started scanning".to_string())
            }
            Err(error) => Err(format!("failed to start scanning: {error}")),
        },
        ButtplugWorkerCommand::StopScanning => match active.client.stop_scanning().await {
            Ok(()) => {
                emit_event(event_tx, ButtplugWorkerEvent::Scanning(false));
                Ok("stopped scanning".to_string())
            }
            Err(error) => Err(format!("failed to stop scanning: {error}")),
        },
        ButtplugWorkerCommand::StopAllDevices => active
            .client
            .stop_all_devices()
            .await
            .map(|()| "stopped all devices".to_string())
            .map_err(|error| format!("failed to stop all devices: {error}")),
        ButtplugWorkerCommand::StopDevice { target } => stop_target_devices(&active.client, target.as_str()).await,
        ButtplugWorkerCommand::SetOutput(request) => set_target_output(&active.client, &request).await,
        ButtplugWorkerCommand::Stop => Ok(String::new()),
    };

    match result {
        Ok(message) if !message.is_empty() => {
            emit_event(event_tx, ButtplugWorkerEvent::CommandResult(message));
        }
        Ok(_) => {}
        Err(error) => {
            emit_event(event_tx, ButtplugWorkerEvent::Error(error));
        }
    }
}

async fn handle_client_event(
    event_tx: &Sender<ButtplugWorkerEvent>,
    active: &mut ActiveButtplugClient,
    event: ButtplugClientEvent,
) -> bool {
    match event {
        ButtplugClientEvent::ServerConnect => {
            let server_name = active
                .client
                .server_name()
                .unwrap_or_else(|| "Buttplug Server".to_string());
            emit_event(
                event_tx,
                ButtplugWorkerEvent::Status(ButtplugConnectionStatus::Connected { server_name }),
            );
        }
        ButtplugClientEvent::ServerDisconnect | ButtplugClientEvent::PingTimeout => return false,
        ButtplugClientEvent::DeviceListReceived => {
            emit_devices(event_tx, &active.client);
        }
        ButtplugClientEvent::DeviceAdded(device) => {
            emit_event(event_tx, ButtplugWorkerEvent::DeviceAdded(device_info(&device)));
            emit_devices(event_tx, &active.client);
        }
        ButtplugClientEvent::DeviceRemoved(device) => {
            emit_event(event_tx, ButtplugWorkerEvent::DeviceRemoved(device_info(&device)));
            emit_devices(event_tx, &active.client);
        }
        ButtplugClientEvent::ScanningFinished => {
            emit_event(event_tx, ButtplugWorkerEvent::ScanningFinished);
            emit_event(event_tx, ButtplugWorkerEvent::Scanning(false));
        }
        ButtplugClientEvent::Error(error) => {
            emit_event(event_tx, ButtplugWorkerEvent::Error(error.to_string()));
        }
    }

    true
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

fn output_command(request: &ButtplugSetOutputRequest) -> Result<ClientDeviceOutputCommand, String> {
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

fn emit_devices(event_tx: &Sender<ButtplugWorkerEvent>, client: &ButtplugClient) {
    emit_event(
        event_tx,
        ButtplugWorkerEvent::Devices(client.devices().values().map(device_info).collect()),
    );
}

fn emit_event(event_tx: &Sender<ButtplugWorkerEvent>, event: ButtplugWorkerEvent) -> bool {
    event_tx.send(event).is_ok()
}

fn emit_status(
    event_tx: &Sender<ButtplugWorkerEvent>,
    last_status: &mut Option<ButtplugConnectionStatus>,
    status: ButtplugConnectionStatus,
) -> bool {
    if last_status.as_ref() == Some(&status) {
        return true;
    }

    *last_status = Some(status.clone());
    emit_event(event_tx, ButtplugWorkerEvent::Status(status))
}

fn schedule_reconnect(reconnect_delay: &mut ReconnectBackoff) -> Instant {
    reconnect_delay.schedule(Instant::now())
}

async fn close_active_client(active: &mut Option<ActiveButtplugClient>) {
    let Some(active_client) = active.take() else {
        return;
    };

    let _ = active_client.client.stop_all_devices().await;
    if active_client.client.connected() {
        let _ = active_client.client.disconnect().await;
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

fn normalize_path(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        "/".to_string()
    } else if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{trimmed}")
    }
}

fn device_info(device: &ButtplugClientDevice) -> ButtplugDeviceInfo {
    ButtplugDeviceInfo {
        index: device.index(),
        variant_id: format!("{BUTTPLUG_DEVICE_VARIANT_PREFIX}{}", device.index()),
        name: device.name().clone(),
        display_name: device.display_name().clone(),
        outputs: device_outputs(device),
    }
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
    device
        .display_name()
        .clone()
        .unwrap_or_else(|| device.name().clone())
}

#[cfg(test)]
mod tests;
