use std::{
    collections::{HashSet, VecDeque},
    io::{Read, Write},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
        Arc, LazyLock, Mutex,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

#[cfg(not(windows))]
use std::path::Path;

use golden_core::{
    node::{Node, NodeId},
    parameter::{ParamValue, Parameter, ParameterEnumOption},
    process_ctx::ProcessCtx,
};
use serialport::{SerialPortInfo, SerialPortType, UsbPortInfo};

pub(crate) const NO_SERIAL_PORT_VARIANT: &str = "none";

const SERIAL_DISCOVERY_POLL_INTERVAL: Duration = Duration::from_secs(1);
const SERIAL_IO_TIMEOUT: Duration = Duration::from_millis(20);
const SERIAL_RECONNECT_BASE_DELAY: Duration = Duration::from_millis(250);
const SERIAL_RECONNECT_MAX_DELAY: Duration = Duration::from_secs(5);
const SERIAL_PENDING_WRITE_LIMIT: usize = 256;
const SERIAL_PENDING_WRITE_BYTES_LIMIT: usize = 256 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DiscoveredSerialPort {
    pub port_name: String,
    pub label: String,
    pub tags: Vec<String>,
    pub details: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct SerialDiscoverySnapshot {
    pub version: u64,
    pub ports: Vec<DiscoveredSerialPort>,
    pub error: Option<String>,
}

#[derive(Debug)]
pub(crate) struct SerialDiscoveryRegistration {
    id: u64,
}

struct SerialDiscoveryState {
    next_registration_id: u64,
    registrations: HashSet<u64>,
    snapshot: SerialDiscoverySnapshot,
    worker: Option<SerialDiscoveryWorker>,
}

struct SerialDiscoveryWorker {
    command_tx: Sender<SerialDiscoveryWorkerCommand>,
    worker: Option<JoinHandle<()>>,
}

enum SerialDiscoveryWorkerCommand {
    Stop,
}

static SERIAL_DISCOVERY_STATE: LazyLock<Mutex<SerialDiscoveryState>> =
    LazyLock::new(|| Mutex::new(SerialDiscoveryState::new()));

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SerialConnectionConfig {
    pub port_name: String,
    pub baud_rate: u32,
    pub receive_enabled: bool,
    pub send_enabled: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SerialConnectionStatus {
    Connected { port_name: String },
    Recovering { port_name: String, message: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SerialConnectionEvent {
    Bytes(Vec<u8>),
    Warning(String),
    Status(SerialConnectionStatus),
}

pub(crate) struct SerialConnectionHandle {
    command_tx: Sender<SerialConnectionCommand>,
    event_rx: Receiver<SerialConnectionEvent>,
    connected: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl SerialDiscoveryState {
    fn new() -> Self {
        Self {
            next_registration_id: 1,
            registrations: HashSet::new(),
            snapshot: SerialDiscoverySnapshot::default(),
            worker: None,
        }
    }
}

impl SerialDiscoveryRegistration {
    pub(crate) fn register() -> Result<Self, String> {
        let mut state = lock_serial_discovery_state();
        let registration_id = state.next_registration_id;
        state.next_registration_id = state.next_registration_id.saturating_add(1);
        state.registrations.insert(registration_id);

        if state.worker.is_none() {
            state.snapshot = SerialDiscoverySnapshot::default();
            match SerialDiscoveryWorker::spawn() {
                Ok(worker) => state.worker = Some(worker),
                Err(error) => {
                    state.registrations.remove(&registration_id);
                    return Err(error);
                }
            }
        }

        Ok(Self { id: registration_id })
    }

    pub(crate) fn snapshot_version(&self) -> u64 {
        lock_serial_discovery_state().snapshot.version
    }

    pub(crate) fn snapshot(&self) -> SerialDiscoverySnapshot {
        lock_serial_discovery_state().snapshot.clone()
    }
}

impl Drop for SerialDiscoveryRegistration {
    fn drop(&mut self) {
        let worker = {
            let mut state = lock_serial_discovery_state();
            if !state.registrations.remove(&self.id) {
                return;
            }

            if state.registrations.is_empty() {
                state.worker.take()
            } else {
                None
            }
        };

        if let Some(mut worker) = worker {
            worker.stop();
        }

        let mut state = lock_serial_discovery_state();
        if state.registrations.is_empty() {
            state.snapshot = SerialDiscoverySnapshot::default();
        }
    }
}

impl SerialDiscoveryWorker {
    fn spawn() -> Result<Self, String> {
        let (command_tx, command_rx) = mpsc::channel();
        let worker = thread::Builder::new()
            .name("serial-discovery".to_string())
            .spawn(move || serial_discovery_worker_loop(command_rx))
            .map_err(|error| format!("failed to start serial discovery thread: {error}"))?;

        Ok(Self {
            command_tx,
            worker: Some(worker),
        })
    }

    fn stop(&mut self) {
        let _ = self.command_tx.send(SerialDiscoveryWorkerCommand::Stop);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Drop for SerialDiscoveryWorker {
    fn drop(&mut self) {
        self.stop();
    }
}

impl SerialConnectionHandle {
    pub(crate) fn spawn(config: SerialConnectionConfig) -> Result<Self, String> {
        if config.port_name.trim().is_empty() {
            return Err("serial port is not selected".to_string());
        }

        let (command_tx, command_rx) = mpsc::channel();
        let (event_tx, event_rx) = mpsc::channel();
        let connected = Arc::new(AtomicBool::new(false));
        let worker_connected = Arc::clone(&connected);

        let worker = thread::Builder::new()
            .name(connection_worker_thread_name(config.port_name.as_str()))
            .spawn(move || serial_connection_worker_loop(config, command_rx, event_tx, worker_connected))
            .map_err(|error| format!("failed to start serial connection thread: {error}"))?;

        Ok(Self {
            command_tx,
            event_rx,
            connected,
            worker: Some(worker),
        })
    }

    pub(crate) fn send(&self, bytes: Vec<u8>) -> Result<(), String> {
        if !self.connected.load(Ordering::Acquire) {
            return Err("serial port is not connected".to_string());
        }

        self.command_tx
            .send(SerialConnectionCommand::Send(bytes))
            .map_err(|_| "serial connection is no longer running".to_string())
    }

    pub(crate) fn try_recv(&self) -> Result<SerialConnectionEvent, mpsc::TryRecvError> {
        self.event_rx.try_recv()
    }

    pub(crate) fn stop(&mut self) {
        let _ = self.command_tx.send(SerialConnectionCommand::Stop);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Drop for SerialConnectionHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

pub(crate) fn serial_port_options(ports: &[DiscoveredSerialPort]) -> Vec<ParameterEnumOption> {
    let mut options = vec![no_serial_port_option()];
    options.extend(ports.iter().map(discovered_serial_port_to_option));
    options
}

pub(crate) fn sync_serial_port_enum_options(
    ctx: &mut ProcessCtx,
    param_id: NodeId,
    options: Vec<ParameterEnumOption>,
) {
    ctx.call_node_mutation(param_id, move |node, inner_ctx| {
        let Some(parameter) = node.as_any_mut().downcast_mut::<Parameter>() else {
            return Err("serial port target is not a parameter".to_string());
        };

        let mut next_options = options.clone();
        let current_variant = parameter
            .value
            .as_enum()
            .filter(|variant| !variant.trim().is_empty())
            .unwrap_or_else(|| NO_SERIAL_PORT_VARIANT.to_string());

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

fn detect_available_serial_ports() -> Result<Vec<DiscoveredSerialPort>, String> {
    let ports = serialport::available_ports().map_err(|error| format!("failed to enumerate serial ports: {error}"))?;

    let mut seen = HashSet::<String>::new();
    let mut discovered = ports
        .into_iter()
        .filter(|port| seen.insert(port.port_name.clone()))
        .map(describe_serial_port)
        .collect::<Vec<_>>();
    discovered.sort_by(|left, right| left.label.cmp(&right.label));

    Ok(discovered)
}

fn lock_serial_discovery_state() -> std::sync::MutexGuard<'static, SerialDiscoveryState> {
    SERIAL_DISCOVERY_STATE
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
}

fn serial_discovery_worker_loop(command_rx: Receiver<SerialDiscoveryWorkerCommand>) {
    let mut initialized = false;
    let mut last_ports = Vec::<DiscoveredSerialPort>::new();
    let mut last_error = None::<String>;

    loop {
        match detect_available_serial_ports() {
            Ok(ports) => {
                if initialized {
                    log_serial_discovery_changes(last_ports.as_slice(), ports.as_slice());
                    if last_error.is_some() {
                        golden_core::log!(
                            tag = "serial";
                            format!("Serial port discovery recovered with {} port(s).", ports.len())
                        );
                    }
                }

                publish_serial_discovery_snapshot(ports.clone(), None);
                last_ports = ports;
                last_error = None;
                initialized = true;
            }
            Err(error) => {
                if last_error.as_deref() != Some(error.as_str()) {
                    golden_core::logerror!(tag = "serial"; format!("Failed to enumerate serial ports: {}", error));
                }

                publish_serial_discovery_snapshot(last_ports.clone(), Some(error.clone()));
                last_error = Some(error);
                initialized = true;
            }
        }

        match command_rx.recv_timeout(SERIAL_DISCOVERY_POLL_INTERVAL) {
            Ok(SerialDiscoveryWorkerCommand::Stop) => break,
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
    }
}

fn publish_serial_discovery_snapshot(ports: Vec<DiscoveredSerialPort>, error: Option<String>) {
    let mut state = lock_serial_discovery_state();
    if state.snapshot.ports == ports && state.snapshot.error == error {
        return;
    }

    state.snapshot.version = state.snapshot.version.wrapping_add(1);
    if state.snapshot.version == 0 {
        state.snapshot.version = 1;
    }
    state.snapshot.ports = ports;
    state.snapshot.error = error;
}

fn log_serial_discovery_changes(previous: &[DiscoveredSerialPort], current: &[DiscoveredSerialPort]) {
    for port in current {
        match previous.iter().find(|candidate| candidate.port_name == port.port_name) {
            None => {
                golden_core::log!(
                    tag = "serial";
                    format!("Serial port added: {} ({})", port.label, port.details)
                );
            }
            Some(existing) if existing != port => {
                golden_core::log!(
                    tag = "serial";
                    format!("Serial port updated: {} ({})", port.label, port.details)
                );
            }
            Some(_) => {}
        }
    }

    for port in previous {
        if current.iter().any(|candidate| candidate.port_name == port.port_name) {
            continue;
        }

        golden_core::log!(
            tag = "serial", level = warning;
            format!("Serial port removed: {} ({})", port.label, port.details)
        );
    }
}

enum SerialConnectionCommand {
    Send(Vec<u8>),
    Stop,
}

enum WorkerLoopControl {
    Continue,
    Stop,
}

fn serial_connection_worker_loop(
    config: SerialConnectionConfig,
    command_rx: Receiver<SerialConnectionCommand>,
    event_tx: Sender<SerialConnectionEvent>,
    connected: Arc<AtomicBool>,
) {
    let mut buffer = [0u8; 8192];
    let mut port = None;
    let mut pending_writes = VecDeque::<Vec<u8>>::new();
    let mut pending_write_bytes = 0usize;
    let mut reconnect_delay = SERIAL_RECONNECT_BASE_DELAY;
    let mut next_open_at = Instant::now();
    let mut last_status = None;

    loop {
        if port.is_none() {
            if Instant::now() >= next_open_at {
                match open_serial_port(&config) {
                    Ok(open_port) => {
                        port = Some(open_port);
                        reconnect_delay = SERIAL_RECONNECT_BASE_DELAY;
                        if !emit_status(
                            &event_tx,
                            &connected,
                            &mut last_status,
                            SerialConnectionStatus::Connected {
                                port_name: config.port_name.clone(),
                            },
                        ) {
                            return;
                        }
                        continue;
                    }
                    Err(error) => {
                        if !enter_recovery(
                            &event_tx,
                            &connected,
                            &mut last_status,
                            &mut reconnect_delay,
                            &mut next_open_at,
                            config.port_name.as_str(),
                            format_open_error(config.port_name.as_str(), &error),
                        ) {
                            return;
                        }
                    }
                }
            }

            let wait = next_open_at.saturating_duration_since(Instant::now());
            if wait.is_zero() {
                continue;
            }

            match command_rx.recv_timeout(wait) {
                Ok(command) => match handle_worker_command(
                    command,
                    &event_tx,
                    &mut pending_writes,
                    &mut pending_write_bytes,
                    config.send_enabled,
                ) {
                    WorkerLoopControl::Continue => {}
                    WorkerLoopControl::Stop => break,
                },
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }

            continue;
        }

        match drain_commands(
            &command_rx,
            &event_tx,
            &mut pending_writes,
            &mut pending_write_bytes,
            config.send_enabled,
        ) {
            WorkerLoopControl::Continue => {}
            WorkerLoopControl::Stop => break,
        }

        if let Some(active_port) = port.as_mut() {
            if let Some(error_message) = flush_pending_writes(
                active_port,
                &mut pending_writes,
                &mut pending_write_bytes,
                config.port_name.as_str(),
            ) {
                port = None;
                if !enter_recovery(
                    &event_tx,
                    &connected,
                    &mut last_status,
                    &mut reconnect_delay,
                    &mut next_open_at,
                    config.port_name.as_str(),
                    error_message,
                ) {
                    return;
                }
                continue;
            }

            if config.receive_enabled {
                match active_port.read(&mut buffer) {
                    Ok(length) if length > 0 => {
                        if event_tx
                            .send(SerialConnectionEvent::Bytes(buffer[..length].to_vec()))
                            .is_err()
                        {
                            return;
                        }
                    }
                    Ok(_) => {}
                    Err(error)
                        if error.kind() == std::io::ErrorKind::TimedOut
                            || error.kind() == std::io::ErrorKind::WouldBlock => {}
                    Err(error) => {
                        port = None;
                        if !enter_recovery(
                            &event_tx,
                            &connected,
                            &mut last_status,
                            &mut reconnect_delay,
                            &mut next_open_at,
                            config.port_name.as_str(),
                            format_read_error(config.port_name.as_str(), &error),
                        ) {
                            return;
                        }
                    }
                }
                continue;
            }
        }

        match command_rx.recv_timeout(SERIAL_IO_TIMEOUT) {
            Ok(command) => match handle_worker_command(
                command,
                &event_tx,
                &mut pending_writes,
                &mut pending_write_bytes,
                config.send_enabled,
            ) {
                WorkerLoopControl::Continue => {}
                WorkerLoopControl::Stop => break,
            },
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    connected.store(false, Ordering::Release);
}

fn drain_commands(
    command_rx: &Receiver<SerialConnectionCommand>,
    event_tx: &Sender<SerialConnectionEvent>,
    pending_writes: &mut VecDeque<Vec<u8>>,
    pending_write_bytes: &mut usize,
    send_enabled: bool,
) -> WorkerLoopControl {
    loop {
        match command_rx.try_recv() {
            Ok(command) => match handle_worker_command(
                command,
                event_tx,
                pending_writes,
                pending_write_bytes,
                send_enabled,
            ) {
                WorkerLoopControl::Continue => {}
                WorkerLoopControl::Stop => return WorkerLoopControl::Stop,
            },
            Err(mpsc::TryRecvError::Empty) => return WorkerLoopControl::Continue,
            Err(mpsc::TryRecvError::Disconnected) => return WorkerLoopControl::Stop,
        }
    }
}

fn handle_worker_command(
    command: SerialConnectionCommand,
    event_tx: &Sender<SerialConnectionEvent>,
    pending_writes: &mut VecDeque<Vec<u8>>,
    pending_write_bytes: &mut usize,
    send_enabled: bool,
) -> WorkerLoopControl {
    match command {
        SerialConnectionCommand::Stop => WorkerLoopControl::Stop,
        SerialConnectionCommand::Send(bytes) => {
            if !send_enabled {
                let _ = event_tx.send(SerialConnectionEvent::Warning(
                    "serial sender is disabled; outgoing bytes were dropped".to_string(),
                ));
                return WorkerLoopControl::Continue;
            }

            enqueue_pending_write(event_tx, pending_writes, pending_write_bytes, bytes);
            WorkerLoopControl::Continue
        }
    }
}

fn enqueue_pending_write(
    event_tx: &Sender<SerialConnectionEvent>,
    pending_writes: &mut VecDeque<Vec<u8>>,
    pending_write_bytes: &mut usize,
    bytes: Vec<u8>,
) {
    if pending_writes.len() >= SERIAL_PENDING_WRITE_LIMIT
        || pending_write_bytes.saturating_add(bytes.len()) > SERIAL_PENDING_WRITE_BYTES_LIMIT
    {
        let _ = event_tx.send(SerialConnectionEvent::Warning(
            "serial write queue is full; outgoing bytes were dropped".to_string(),
        ));
        return;
    }

    *pending_write_bytes = pending_write_bytes.saturating_add(bytes.len());
    pending_writes.push_back(bytes);
}

fn flush_pending_writes(
    port: &mut Box<dyn serialport::SerialPort>,
    pending_writes: &mut VecDeque<Vec<u8>>,
    pending_write_bytes: &mut usize,
    port_name: &str,
) -> Option<String> {
    while let Some(bytes) = pending_writes.pop_front() {
        *pending_write_bytes = pending_write_bytes.saturating_sub(bytes.len());

        if let Err(error) = port.write_all(bytes.as_slice()) {
            return Some(format_write_error(port_name, &error));
        }
    }

    None
}

fn open_serial_port(
    config: &SerialConnectionConfig,
) -> serialport::Result<Box<dyn serialport::SerialPort>> {
    serialport::new(config.port_name.as_str(), config.baud_rate)
        .timeout(SERIAL_IO_TIMEOUT)
        .open()
}

fn emit_status(
    event_tx: &Sender<SerialConnectionEvent>,
    connected: &Arc<AtomicBool>,
    last_status: &mut Option<SerialConnectionStatus>,
    next_status: SerialConnectionStatus,
) -> bool {
    connected.store(
        matches!(next_status, SerialConnectionStatus::Connected { .. }),
        Ordering::Release,
    );

    if last_status.as_ref() == Some(&next_status) {
        return true;
    }

    *last_status = Some(next_status.clone());
    event_tx.send(SerialConnectionEvent::Status(next_status)).is_ok()
}

fn enter_recovery(
    event_tx: &Sender<SerialConnectionEvent>,
    connected: &Arc<AtomicBool>,
    last_status: &mut Option<SerialConnectionStatus>,
    reconnect_delay: &mut Duration,
    next_open_at: &mut Instant,
    port_name: &str,
    message: String,
) -> bool {
    *next_open_at = Instant::now() + *reconnect_delay;
    *reconnect_delay = reconnect_delay
        .saturating_mul(2)
        .min(SERIAL_RECONNECT_MAX_DELAY);

    emit_status(
        event_tx,
        connected,
        last_status,
        SerialConnectionStatus::Recovering {
            port_name: port_name.to_string(),
            message,
        },
    )
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

fn describe_serial_port(port: SerialPortInfo) -> DiscoveredSerialPort {
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
    if trimmed.is_empty()
        || trimmed.eq_ignore_ascii_case("n/a")
        || trimmed.eq_ignore_ascii_case("unknown")
    {
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

fn connection_worker_thread_name(port_name: &str) -> String {
    let sanitized = port_name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '_'
            }
        })
        .take(48)
        .collect::<String>();
    format!("serial-manager-{sanitized}")
}

fn format_open_error(port_name: &str, error: &serialport::Error) -> String {
    format!(
        "Serial port {} is unavailable: {}. Auto-retrying.",
        human_serial_port_name(port_name),
        error
    )
}

fn format_read_error(port_name: &str, error: &std::io::Error) -> String {
    format!(
        "Lost serial port {} while reading: {}. Auto-retrying.",
        human_serial_port_name(port_name),
        error
    )
}

fn format_write_error(port_name: &str, error: &std::io::Error) -> String {
    format!(
        "Lost serial port {} while writing: {}. Auto-retrying.",
        human_serial_port_name(port_name),
        error
    )
}
