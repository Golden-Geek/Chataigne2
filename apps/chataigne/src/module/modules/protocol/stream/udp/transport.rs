use super::outbound::UdpOutboundSend;
use super::outbound::UDP_COMMAND_BYTES;
#[cfg(test)]
use super::outbound::{UdpOutputTarget, UDP_MAX_PAYLOAD};
use crate::app::module::common::streaming::input::{self, InputPlan, PreparedStreamInput, StreamInputConfig};
use crate::app::module::common::streaming::parser::StreamingParser;
use golden_io::RetainedPayload;
mod replies;
use crate::app::module::common::streaming::native_replies::NativeReplies;
use replies::ReplySource;
use std::{
    net::{SocketAddr, ToSocketAddrs, UdpSocket},
    num::NonZeroUsize,
    sync::OnceLock,
    time::Duration,
};

use golden_io::{
    BoundedWorker, PendingDrain, PendingDrainState, PendingSendError, RetirementPool, WorkerContext, WorkerLimits,
    WorkerNotifier,
};

const UDP_EVENT_DRAIN_BUDGET: NonZeroUsize = NonZeroUsize::new(4).unwrap();
const UDP_COMMAND_TURN: NonZeroUsize = NonZeroUsize::new(16).unwrap();
const UDP_RECEIVE_TURN: usize = 16;
const UDP_HOST_BYTES: usize = 1024;
const UDP_EVENT_BYTES: usize = 32 * 1024 * 1024;

fn worker_lifetimes() -> &'static RetirementPool {
    static POOL: OnceLock<RetirementPool> = OnceLock::new();
    POOL.get_or_init(|| RetirementPool::new(NonZeroUsize::new(16).unwrap()))
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct UdpStreamingTransportConfig {
    pub bind_interface_host: String,
    pub bind_port: u16,
    pub receive_enabled: bool,
    pub input: StreamInputConfig,
}

impl UdpStreamingTransportConfig {
    pub(super) fn same_socket(&self, other: &Self) -> bool {
        self.bind_interface_host == other.bind_interface_host
            && self.bind_port == other.bind_port
            && self.receive_enabled == other.receive_enabled
    }
}

#[derive(Clone, Debug)]
pub(crate) enum StreamingWorkerEvent {
    Ready(SocketAddr),
    Input(RetainedPayload<PreparedStreamInput>),
    Interfaces(Result<Vec<golden_core::parameter::ParameterEnumOption>, String>),
    OutgoingLog(String),
    Error(String),
    Failed(String),
}

impl StreamingWorkerEvent {
    fn weight(&self) -> usize {
        std::mem::size_of::<Self>()
            + match self {
                Self::Ready(_) => 0,
                Self::Input(input) => input.weight,
                Self::Interfaces(Ok(options)) => interface_weight(options),
                Self::Interfaces(Err(error)) => error.capacity(),
                Self::OutgoingLog(message) | Self::Error(message) | Self::Failed(message) => message.capacity(),
            }
    }
}

pub(crate) struct UdpStreamingTransportHandle {
    worker: BoundedWorker<UdpOutboundSend, StreamingWorkerEvent>,
    input: InputPlan,
}

impl UdpStreamingTransportHandle {
    pub(crate) fn spawn(config: UdpStreamingTransportConfig, notifier: WorkerNotifier) -> Result<Self, String> {
        let binding = config.clone();
        Self::spawn_using(config, notifier, move || bind_socket(&binding))
    }

    fn spawn_using(
        config: UdpStreamingTransportConfig,
        notifier: WorkerNotifier,
        setup: impl FnOnce() -> Result<UdpSocket, String> + Send + 'static,
    ) -> Result<Self, String> {
        Self::spawn_with_setup(config, worker_lifetimes(), notifier, setup)
    }

    pub(super) fn spawn_with_setup(
        config: UdpStreamingTransportConfig,
        lifetimes: &RetirementPool,
        notifier: WorkerNotifier,
        setup: impl FnOnce() -> Result<UdpSocket, String> + Send + 'static,
    ) -> Result<Self, String> {
        if config.bind_interface_host.len() > UDP_HOST_BYTES {
            return Err("UDP bind host exceeds the 1024-byte limit".into());
        }
        let input = InputPlan::new(config.input.clone());
        let worker_input = input.clone();
        let worker = BoundedWorker::spawn_notified(
            "streaming-udp",
            lifetimes,
            WorkerLimits {
                command_items: 256,
                command_weight: UDP_COMMAND_BYTES,
                event_items: 64,
                event_weight: UDP_EVENT_BYTES,
            },
            notifier,
            move |context| worker_loop(context, config.receive_enabled, worker_input, setup),
        )
        .map_err(|error| format!("failed to start UDP worker: {error}"))?;
        Ok(Self { worker, input })
    }

    pub(super) fn update_input(&self, config: StreamInputConfig) {
        self.input.update(config);
    }

    pub(super) fn send(&self, command: UdpOutboundSend) -> Result<(), String> {
        let weight = command.weight();
        self.worker.send_weighted(command, weight).map_err(|error| match error {
            PendingSendError::Full(_) => "UDP command capacity exhausted; whole command was not admitted".into(),
            PendingSendError::Disconnected(_) => "UDP worker is not accepting commands".into(),
        })
    }

    pub(super) fn accepts_commands(&self) -> bool {
        self.worker.is_accepting_commands()
    }

    pub(crate) fn drain_events(&self, events: &mut Vec<StreamingWorkerEvent>) -> PendingDrain {
        self.worker.drain_events(events, UDP_EVENT_DRAIN_BUDGET)
    }

    pub(crate) fn has_pending(&self) -> bool {
        self.worker.has_pending()
    }

    /// Reconfiguration closes admission while retaining the worker and all admitted sends.
    pub(crate) fn finish_admission(&mut self) {
        self.worker.finish_admission();
    }

    /// Only destruction abandons pending work. Cancellation never joins native setup or I/O.
    pub(crate) fn stop(&mut self) {
        self.worker.cancel();
    }
}

fn interface_weight(options: &Vec<golden_core::parameter::ParameterEnumOption>) -> usize {
    options.capacity() * std::mem::size_of::<golden_core::parameter::ParameterEnumOption>()
        + options
            .iter()
            .map(|option| {
                option.variant_id.capacity()
                    + option.label.capacity()
                    + option.tags.capacity() * std::mem::size_of::<String>()
                    + option.tags.iter().map(String::capacity).sum::<usize>()
                    + match &option.value {
                        golden_core::parameter::ParamValue::Enum(value) => value.capacity(),
                        _ => 0,
                    }
            })
            .sum::<usize>()
}

fn discover_interfaces() -> Result<Vec<golden_core::parameter::ParameterEnumOption>, String> {
    let options = crate::app::module::common::network_interfaces::available_interface_options().map_err(|error| {
        if error.capacity() > 64 * 1024 {
            "UDP interface discovery error exceeds 64 KiB; existing choices retained".into()
        } else {
            error
        }
    })?;
    if options.len() > 256 || interface_weight(&options) > 64 * 1024 {
        return Err("UDP interface discovery exceeds 256 entries / 64 KiB; existing choices retained".into());
    }
    Ok(options)
}

fn bind_socket(config: &UdpStreamingTransportConfig) -> Result<UdpSocket, String> {
    let bind_address = resolve_socket_addr(config.bind_interface_host.as_str(), config.bind_port)?;
    let socket = UdpSocket::bind(bind_address)
        .map_err(|error| format!("failed to bind UDP socket on {bind_address}: {error}"))?;
    socket
        .set_nonblocking(true)
        .map_err(|error| format!("failed to set UDP socket to non-blocking mode: {error}"))?;
    Ok(socket)
}

fn publish(context: &WorkerContext<UdpOutboundSend, StreamingWorkerEvent>, event: StreamingWorkerEvent) -> bool {
    let weight = event.weight();
    context.publish_weighted(event, weight).is_ok()
}

fn worker_loop(
    context: WorkerContext<UdpOutboundSend, StreamingWorkerEvent>,
    mut receive_enabled: bool,
    input: InputPlan,
    setup: impl FnOnce() -> Result<UdpSocket, String>,
) {
    let (socket, address) = match setup().and_then(|socket| {
        socket.set_nonblocking(true).map_err(|error| error.to_string())?;
        let address = socket.local_addr().map_err(|error| error.to_string())?;
        Ok((socket, address))
    }) {
        Ok(socket) => socket,
        Err(error) => {
            context.close_command_admission();
            if !publish(&context, StreamingWorkerEvent::Failed(error.clone())) {
                return;
            }
            // Every accepted send receives an explicit setup failure, even when
            // startup failed before its owner observed the readiness event.
            let mut commands = Vec::new();
            loop {
                let turn = context.drain_commands(&mut commands, UDP_COMMAND_TURN);
                for mut command in commands.drain(..) {
                    command.fail_preparation(error.clone());
                    let message = format!("UDP command failed before send: {error}");
                    if !publish(&context, StreamingWorkerEvent::Error(message)) {
                        return;
                    }
                }
                if turn.state == PendingDrainState::Disconnected {
                    return;
                }
            }
        }
    };
    if !publish(&context, StreamingWorkerEvent::Ready(address)) {
        return;
    }
    let mut buffer = [0u8; 65_535];
    let mut last_interfaces = None;
    let mut next_interfaces = std::time::Instant::now();
    let mut current: Option<(UdpOutboundSend, usize)> = None;
    let mut commands_closed = false;
    let mut parser = StreamingParser::default();
    let mut input_generation = None;
    let mut replies = NativeReplies::<StreamingWorkerEvent>::default();
    loop {
        if context.cancellation().is_cancelled() || !replies.flush(&context) {
            return;
        }
        for _ in 0..UDP_COMMAND_TURN.get() {
            if !replies.permits(ReplySource::Output) {
                break;
            }
            if current.is_none() {
                let mut next = Vec::with_capacity(1);
                let turn = context.drain_commands(&mut next, NonZeroUsize::MIN);
                commands_closed = turn.state == PendingDrainState::Disconnected;
                if let Some(command) = next.pop() {
                    current = Some((command, 0));
                }
            }
            let Some((command, index)) = current.as_mut() else {
                break;
            };
            match command.prepare_turn(|| context.cancellation().is_cancelled()) {
                Ok(false) => break,
                Err(error) => {
                    current = None;
                    if !replies.offer(
                        &context,
                        ReplySource::Output,
                        StreamingWorkerEvent::Error(format!("UDP output preparation failed before send: {error}")),
                    ) {
                        return;
                    }
                    continue;
                }
                Ok(true) => {}
            }
            if context.cancellation().is_cancelled() {
                return;
            }
            let outcome = send_target(&socket, command, *index);
            *index += 1;
            if *index == command.len() {
                current = None;
            }
            if let Some(event) = outcome {
                if !replies.offer(&context, ReplySource::Output, event) {
                    return;
                }
            }
        }
        if commands_closed && current.is_none() {
            if !replies.finish(&context) {
                return;
            }
            parser.discard_partial_input();
            let rejected = parser.take_rejected_line_count();
            if rejected > 0 {
                publish(
                    &context,
                    StreamingWorkerEvent::Error(format!(
                        "UDP retired with {rejected} incomplete input lines; data was not replayed"
                    )),
                );
            }
            return;
        }
        if !context.command_admission_closed()
            && replies.permits(ReplySource::Interfaces)
            && std::time::Instant::now() >= next_interfaces
        {
            let options = discover_interfaces();
            if last_interfaces.as_ref() != Some(&options) {
                if !replies.offer(
                    &context,
                    ReplySource::Interfaces,
                    StreamingWorkerEvent::Interfaces(options.clone()),
                ) {
                    return;
                }
                last_interfaces = Some(options);
            }
            next_interfaces = std::time::Instant::now() + Duration::from_secs(1);
        }
        if receive_enabled && !context.command_admission_closed() {
            for _ in 0..UDP_RECEIVE_TURN {
                if !replies.permits(ReplySource::Input) || context.command_admission_closed() {
                    break;
                }
                if context.cancellation().is_cancelled() {
                    return;
                }
                match socket.recv_from(&mut buffer) {
                    Ok((length, _source)) => {
                        let snapshot = input.snapshot();
                        if input_generation.is_some_and(|generation| generation != snapshot.generation) {
                            parser.discard_partial_input();
                        }
                        input_generation = Some(snapshot.generation);
                        let event = match input::prepare(&mut parser, &snapshot, &buffer[..length], "UDP", None) {
                            Ok(input) => StreamingWorkerEvent::Input(input),
                            Err(error) => StreamingWorkerEvent::Error(format!("UDP input was not admitted: {error}")),
                        };
                        if !replies.offer(&context, ReplySource::Input, event) {
                            return;
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(error) if should_ignore_receive_error(&error) => break,
                    Err(error) => {
                        context.close_command_admission();
                        // Keep servicing accepted sends, but stop receiving after a
                        // terminal socket error; the owner will reconnect this endpoint.
                        if !replies.offer(
                            &context,
                            ReplySource::Input,
                            StreamingWorkerEvent::Failed(format!("UDP socket receive error: {error}")),
                        ) {
                            return;
                        }
                        receive_enabled = false;
                        break;
                    }
                }
            }
        }
        if current.is_none() || !replies.permits(ReplySource::Output) {
            context.cancellation().wait(Duration::from_millis(5));
        }
    }
}

fn should_ignore_receive_error(error: &std::io::Error) -> bool {
    error.raw_os_error() == Some(10054) || (cfg!(windows) && error.kind() == std::io::ErrorKind::ConnectionReset)
}

fn send_target(socket: &UdpSocket, command: &UdpOutboundSend, index: usize) -> Option<StreamingWorkerEvent> {
    let target = command.target(index);
    let outcome = resolve_socket_addr(&target.remote_host, target.remote_port).and_then(|address| {
        socket
            .send_to(command.bytes(), address)
            .map_err(|error| error.to_string())
    });
    match outcome {
        Ok(_) if command.log_outgoing() => {
            let bytes = command.bytes();
            let prefix = &bytes[..bytes.len().min(1024)];
            let message = format!(
                "Sent UDP {} ({} bytes) to {}:{}",
                crate::app::module::common::streaming::module_helpers::format_bytes_for_log(prefix),
                bytes.len(),
                target.remote_host,
                target.remote_port
            );
            Some(StreamingWorkerEvent::OutgoingLog(message))
        }
        Ok(_) => None,
        Err(error) => Some(StreamingWorkerEvent::Error(format!(
            "UDP output {}/{} to {}:{} failed: {error}",
            index + 1,
            command.len(),
            target.remote_host,
            target.remote_port
        ))),
    }
}

fn resolve_socket_addr(host: &str, port: u16) -> Result<SocketAddr, String> {
    (host.trim(), port)
        .to_socket_addrs()
        .map_err(|error| format!("failed to resolve UDP address '{host}:{port}': {error}"))?
        .next()
        .ok_or_else(|| format!("UDP address '{host}:{port}' did not resolve to a socket address"))
}

#[cfg(test)]
mod tests;
