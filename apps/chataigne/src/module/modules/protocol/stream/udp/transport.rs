use std::{
    net::{SocketAddr, ToSocketAddrs, UdpSocket},
    num::NonZeroUsize,
    sync::OnceLock,
    time::Duration,
};

use golden_io::{
    BoundedWorker, PendingDrain, PendingDrainState, PendingSendError, RetirementPool, WorkerContext, WorkerLimits,
};

pub(super) const UDP_MAX_PAYLOAD: usize = 65_507;
pub(super) const UDP_COMMAND_BYTES: usize = 1024 * 1024;
const UDP_EVENT_DRAIN_BUDGET: NonZeroUsize = NonZeroUsize::new(4).unwrap();
const UDP_COMMAND_TURN: NonZeroUsize = NonZeroUsize::new(16).unwrap();
const UDP_RECEIVE_TURN: usize = 16;
const UDP_HOST_BYTES: usize = 1024;

fn worker_lifetimes() -> &'static RetirementPool {
    static POOL: OnceLock<RetirementPool> = OnceLock::new();
    POOL.get_or_init(|| RetirementPool::new(NonZeroUsize::new(16).unwrap()))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct UdpStreamingTransportConfig {
    pub bind_interface_host: String,
    pub bind_port: u16,
    pub receive_enabled: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct UdpStreamingOutboundPacket {
    pub bytes: Vec<u8>,
    pub remote_host: String,
    pub remote_port: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum StreamingWorkerEvent {
    Ready(SocketAddr),
    Bytes(Vec<u8>),
    Error(String),
    Failed(String),
}

impl StreamingWorkerEvent {
    fn weight(&self) -> usize {
        std::mem::size_of::<Self>()
            + match self {
                Self::Ready(_) => 0,
                Self::Bytes(bytes) => bytes.capacity(),
                Self::Error(message) | Self::Failed(message) => message.capacity(),
            }
    }
}

pub(crate) struct UdpStreamingTransportHandle {
    worker: BoundedWorker<UdpStreamingOutboundPacket, StreamingWorkerEvent>,
}

impl UdpStreamingTransportHandle {
    pub(crate) fn spawn(config: UdpStreamingTransportConfig) -> Result<Self, String> {
        let binding = config.clone();
        Self::spawn_using(config, move || bind_socket(&binding))
    }

    fn spawn_using(
        config: UdpStreamingTransportConfig,
        setup: impl FnOnce() -> Result<UdpSocket, String> + Send + 'static,
    ) -> Result<Self, String> {
        Self::spawn_with_setup(config, worker_lifetimes(), setup)
    }

    pub(super) fn spawn_with_setup(
        config: UdpStreamingTransportConfig,
        lifetimes: &RetirementPool,
        setup: impl FnOnce() -> Result<UdpSocket, String> + Send + 'static,
    ) -> Result<Self, String> {
        if config.bind_interface_host.len() > UDP_HOST_BYTES {
            return Err("UDP bind host exceeds the 1024-byte limit".into());
        }
        let worker = BoundedWorker::spawn(
            "streaming-udp",
            lifetimes,
            WorkerLimits {
                command_items: 256,
                command_weight: UDP_COMMAND_BYTES,
                event_items: 64,
                event_weight: 512 * 1024,
            },
            move |context| worker_loop(context, config.receive_enabled, setup),
        )
        .map_err(|error| format!("failed to start UDP worker: {error}"))?;
        Ok(Self { worker })
    }

    pub(crate) fn send(&self, packet: UdpStreamingOutboundPacket) -> Result<(), String> {
        if packet.bytes.len() > UDP_MAX_PAYLOAD {
            return Err(format!(
                "UDP payload exceeds {UDP_MAX_PAYLOAD} bytes; packet was not admitted"
            ));
        }
        if packet.remote_host.len() > UDP_HOST_BYTES {
            return Err("UDP destination host exceeds 1024 bytes; packet was not admitted".into());
        }
        let weight = std::mem::size_of::<UdpStreamingOutboundPacket>()
            .saturating_add(packet.bytes.capacity())
            .saturating_add(packet.remote_host.capacity());
        self.worker.send_weighted(packet, weight).map_err(|error| match error {
            PendingSendError::Full(_) => "UDP command capacity exhausted; packet was not admitted".into(),
            PendingSendError::Disconnected(_) => "UDP worker is not accepting packets".into(),
        })
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

fn bind_socket(config: &UdpStreamingTransportConfig) -> Result<UdpSocket, String> {
    let bind_address = resolve_socket_addr(config.bind_interface_host.as_str(), config.bind_port)?;
    let socket = UdpSocket::bind(bind_address)
        .map_err(|error| format!("failed to bind UDP socket on {bind_address}: {error}"))?;
    socket
        .set_nonblocking(true)
        .map_err(|error| format!("failed to set UDP socket to non-blocking mode: {error}"))?;
    Ok(socket)
}

fn publish(
    context: &WorkerContext<UdpStreamingOutboundPacket, StreamingWorkerEvent>,
    event: StreamingWorkerEvent,
) -> bool {
    let weight = event.weight();
    context.publish_weighted(event, weight).is_ok()
}

fn worker_loop(
    context: WorkerContext<UdpStreamingOutboundPacket, StreamingWorkerEvent>,
    receive_enabled: bool,
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
                for packet in commands.drain(..) {
                    let message = format!(
                        "UDP packet to {}:{} failed before send: {error}",
                        packet.remote_host, packet.remote_port
                    );
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
    let mut commands = Vec::new();
    loop {
        if context.cancellation().is_cancelled() {
            return;
        }
        let turn = context.drain_commands(&mut commands, UDP_COMMAND_TURN);
        for packet in commands.drain(..) {
            if context.cancellation().is_cancelled() || !send_packet(&socket, &context, packet) {
                return;
            }
        }
        if turn.state == PendingDrainState::Disconnected {
            return;
        }
        if receive_enabled {
            for _ in 0..UDP_RECEIVE_TURN {
                if context.cancellation().is_cancelled() {
                    return;
                }
                match socket.recv_from(&mut buffer) {
                    Ok((length, _source)) => {
                        if !publish(&context, StreamingWorkerEvent::Bytes(buffer[..length].to_vec())) {
                            return;
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(error) if should_ignore_receive_error(&error) => break,
                    Err(error) => {
                        context.close_command_admission();
                        // Keep servicing accepted sends, but stop receiving after a
                        // terminal socket error; the owner will reconnect this endpoint.
                        publish(
                            &context,
                            StreamingWorkerEvent::Failed(format!("UDP socket receive error: {error}")),
                        );
                        while let Some(packet) = context.receive() {
                            if !send_packet(&socket, &context, packet) {
                                return;
                            }
                        }
                        return;
                    }
                }
            }
        }
        if turn.state != PendingDrainState::BudgetExhausted {
            context.cancellation().wait(Duration::from_millis(5));
        }
    }
}

fn should_ignore_receive_error(error: &std::io::Error) -> bool {
    error.raw_os_error() == Some(10054) || (cfg!(windows) && error.kind() == std::io::ErrorKind::ConnectionReset)
}

fn send_packet(
    socket: &UdpSocket,
    context: &WorkerContext<UdpStreamingOutboundPacket, StreamingWorkerEvent>,
    packet: UdpStreamingOutboundPacket,
) -> bool {
    let outcome = resolve_socket_addr(&packet.remote_host, packet.remote_port).and_then(|address| {
        socket
            .send_to(&packet.bytes, address)
            .map_err(|error| error.to_string())
    });
    match outcome {
        Ok(_) => true,
        Err(error) => publish(
            context,
            StreamingWorkerEvent::Error(format!(
                "UDP packet to {}:{} failed: {error}",
                packet.remote_host, packet.remote_port
            )),
        ),
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
