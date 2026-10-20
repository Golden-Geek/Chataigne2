use super::super::osc_message::{decode_packet_messages, encode_packet};
use super::*;
use crate::app::module::common::streaming::native_replies::{NativeReplies, NativeReply};
use golden_io::{PendingDrainState, WorkerContext};
use mio::{net::UdpSocket, Events, Interest, Poll, Token};
use rosc::{decoder, encoder};
use std::{collections::VecDeque, io, time::Duration};

const SOCKET_TOKEN: Token = Token(0);
const WAKE_TOKEN: Token = Token(1);
const FALLBACK_WAIT: Duration = Duration::from_millis(250);

pub(super) trait OscSocket: Send + 'static {
    fn address(&self) -> SocketAddr;
    fn receive(&mut self, bytes: &mut [u8]) -> io::Result<usize>;
    fn send(&mut self, bytes: &[u8], remote: SocketAddr) -> io::Result<usize>;
    fn wait(&mut self, duration: Duration) -> io::Result<()>;
}

pub(super) struct NativeSocket {
    socket: UdpSocket,
    poll: Poll,
    events: Events,
    address: SocketAddr,
    _waker: Arc<Waker>,
}

impl NativeSocket {
    pub(super) fn open(config: &OscTransportConfig, wake: &NativeWake) -> Result<Self, String> {
        let poll = Poll::new().map_err(|error| error.to_string())?;
        let waker = Arc::new(Waker::new(poll.registry(), WAKE_TOKEN).map_err(|error| error.to_string())?);
        *wake.source.lock().unwrap_or_else(|error| error.into_inner()) = Some(waker.clone());
        let address = resolve_socket_addr(&config.bind_interface_host, config.bind_port)?;
        let mut socket =
            UdpSocket::bind(address).map_err(|error| format!("failed to bind OSC socket on {address}: {error}"))?;
        poll.registry()
            .register(&mut socket, SOCKET_TOKEN, Interest::READABLE)
            .map_err(|error| error.to_string())?;
        let address = socket.local_addr().map_err(|error| error.to_string())?;
        Ok(Self {
            socket,
            poll,
            events: Events::with_capacity(16),
            address,
            _waker: waker,
        })
    }
}

impl OscSocket for NativeSocket {
    fn address(&self) -> SocketAddr {
        self.address
    }
    fn receive(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.socket.recv_from(bytes).map(|(length, _)| length)
    }
    fn send(&mut self, bytes: &[u8], remote: SocketAddr) -> io::Result<usize> {
        self.socket.send_to(bytes, remote)
    }
    fn wait(&mut self, duration: Duration) -> io::Result<()> {
        self.poll.poll(&mut self.events, Some(duration))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Source {
    Input,
    Output,
}

impl NativeReply for OscWorkerEvent {
    type Source = Source;
    const SOURCES: usize = 2;
    fn weight(&self) -> usize {
        admission::event_weight(self)
    }
    fn limit(source: Source) -> usize {
        match source {
            Source::Input => OSC_EVENT_BYTES,
            Source::Output => 64 * 1024,
        }
    }
    fn oversized(source: Source) -> Self {
        Self::Error(format!(
            "OSC {source:?} result exceeded its storage limit; native work is not replayed"
        ))
    }
}

struct ReadyGuard(Arc<AtomicBool>);
impl Drop for ReadyGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

struct WakeGuard(NativeWake);
impl Drop for WakeGuard {
    fn drop(&mut self) {
        self.0.clear();
    }
}

pub(super) fn run<S: OscSocket>(
    context: WorkerContext<OscOutboundMessage, OscWorkerEvent>,
    config: OscTransportConfig,
    wake: NativeWake,
    ready: Arc<AtomicBool>,
    setup: impl FnOnce(&OscTransportConfig, &NativeWake) -> Result<S, String>,
) {
    let _wake_guard = WakeGuard(wake.clone());
    let _ready_guard = ReadyGuard(ready.clone());
    if context.cancellation().is_cancelled() {
        return;
    }
    let mut socket = match setup(&config, &wake) {
        Ok(socket) => socket,
        Err(error) => {
            fail_commands(&context, error);
            return;
        }
    };
    if context.cancellation().is_cancelled() {
        return;
    }
    ready.store(true, Ordering::Release);
    if !publish(&context, OscWorkerEvent::Ready(socket.address())) {
        return;
    }
    if let Err(error) = service(&context, &mut socket, config.receive_enabled, &wake) {
        ready.store(false, Ordering::Release);
        fail_commands(&context, error);
    }
}

fn publish(context: &WorkerContext<OscOutboundMessage, OscWorkerEvent>, event: OscWorkerEvent) -> bool {
    let weight = admission::event_weight(&event);
    context.publish_weighted(event, weight).is_ok()
}

fn fail_commands(context: &WorkerContext<OscOutboundMessage, OscWorkerEvent>, error: String) {
    context.close_command_admission();
    let error = if error.capacity() > 32 * 1024 {
        "OSC native error exceeds diagnostic capacity".into()
    } else {
        error
    };
    if !publish(context, OscWorkerEvent::Failed(error.clone())) {
        return;
    }
    while let Some(command) = context.receive() {
        let message = format!(
            "OSC {} failed before send: {error}; command is not replayed",
            command.address
        );
        if !publish(context, OscWorkerEvent::Error(message)) {
            return;
        }
    }
}

fn service<S: OscSocket>(
    context: &WorkerContext<OscOutboundMessage, OscWorkerEvent>,
    socket: &mut S,
    receive_enabled: bool,
    wake: &NativeWake,
) -> Result<(), String> {
    let mut replies = NativeReplies::<OscWorkerEvent>::default();
    let mut incoming = VecDeque::new();
    let mut buffer = [0u8; 65_535];
    loop {
        if context.cancellation().is_cancelled() || !replies.flush(context) {
            return Ok(());
        }
        let mut worked = false;
        let mut closed = false;
        for _ in 0..OSC_COMMAND_TURN.get() {
            if !replies.permits(Source::Output) {
                break;
            }
            let mut next = Vec::with_capacity(1);
            let turn = context.drain_commands(&mut next, NonZeroUsize::MIN);
            closed = turn.state == PendingDrainState::Disconnected;
            let Some(command) = next.pop() else { break };
            worked = true;
            if let Err(error) = send(socket, command) {
                if !replies.offer(context, Source::Output, OscWorkerEvent::Error(error)) {
                    return Ok(());
                }
            }
        }
        for _ in 0..OSC_SOCKET_SERVICE_BUDGET {
            if !replies.permits(Source::Input) {
                break;
            }
            if incoming.is_empty() {
                if !receive_enabled || context.command_admission_closed() {
                    break;
                }
                match socket.receive(&mut buffer) {
                    Ok(length) => {
                        worked = true;
                        let bytes = &buffer[..length];
                        let decoded = admission::packet_shape(bytes).map_err(str::to_string).and_then(|()| {
                            decoder::decode_udp(bytes)
                                .map(|(_, packet)| decode_packet_messages(packet))
                                .map_err(|error| format!("failed to decode OSC packet: {error}"))
                        });
                        match decoded {
                            Ok(messages) => incoming.extend(messages.into_iter().map(|message| match message {
                                Ok(message) => OscWorkerEvent::Message(message),
                                Err(error) => OscWorkerEvent::Error(error),
                            })),
                            Err(error) => incoming.push_back(OscWorkerEvent::Error(error)),
                        }
                    }
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock || should_ignore_receive_error(&error) => {
                        break
                    }
                    Err(error) => {
                        context.close_command_admission();
                        finish_input_prefix(context, replies, incoming);
                        return Err(format!("OSC receive failed: {error}"));
                    }
                }
            }
            if let Some(event) = incoming.pop_front() {
                worked = true;
                if !replies.offer(context, Source::Input, event) {
                    return Ok(());
                }
            }
        }
        if closed && incoming.is_empty() {
            replies.finish(context);
            return Ok(());
        }
        if replies.permits(Source::Output)
            && wake.failed.swap(false, Ordering::AcqRel)
            && !replies.offer(
                context,
                Source::Output,
                OscWorkerEvent::Error(
                    "OSC native wake failed; accepted commands use the bounded fallback wait and are not resubmitted"
                        .into(),
                ),
            )
        {
            return Ok(());
        }
        if !worked {
            match socket.wait(FALLBACK_WAIT) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) => {
                    context.close_command_admission();
                    finish_input_prefix(context, replies, incoming);
                    return Err(format!("OSC poll failed: {error}"));
                }
            }
        }
    }
}

fn finish_input_prefix(
    context: &WorkerContext<OscOutboundMessage, OscWorkerEvent>,
    replies: NativeReplies<OscWorkerEvent>,
    incoming: VecDeque<OscWorkerEvent>,
) {
    if !replies.finish(context) {
        return;
    }
    for event in incoming {
        if !publish(context, event) {
            return;
        }
    }
}

fn send(socket: &mut impl OscSocket, command: OscOutboundMessage) -> Result<(), String> {
    let packet = encode_packet(&command.address, &command.payload)?;
    let bytes = encoder::encode(&packet).map_err(|error| format!("failed to encode OSC packet: {error}"))?;
    if bytes.len() > OSC_MAX_DATAGRAM {
        return Err("OSC encoded output exceeds 65507 bytes; native send was not attempted".into());
    }
    match socket.send(&bytes, command.remote_address) {
        Ok(length) if length == bytes.len() => Ok(()),
        Ok(_) => Err("OSC output reported a partial datagram; effect is unconfirmed and is not replayed".into()),
        Err(error) => Err(format!("OSC send failed: {error}; command is not replayed")),
    }
}
