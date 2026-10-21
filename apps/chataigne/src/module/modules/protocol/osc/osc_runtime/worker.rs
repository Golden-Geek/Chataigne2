use super::super::osc_message::decode_packet_messages;
use super::super::{osc_input::InputMode, osc_input_queue::InputQueue};
use super::*;
use crate::app::module::common::streaming::native_replies::{NativeReplies, NativeReply};
use golden_io::{PendingDrainState, WorkerContext};
use mio::{net::UdpSocket, Events, Interest, Poll, Token};
use rosc::decoder;
use std::{io, time::Duration};

const SOCKET_TOKEN: Token = Token(0);
const WAKE_TOKEN: Token = Token(1);
const FALLBACK_WAIT: Duration = Duration::from_millis(250);

pub(in super::super) trait OscSocket: Send + 'static {
    fn address(&self) -> SocketAddr;
    fn resolve(&mut self, host: &str, port: u16) -> Result<SocketAddr, String> {
        resolve_socket_addr(host, port)
    }
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
    context: WorkerContext<OscWorkerCommand, OscWorkerEvent>,
    config: OscTransportConfig,
    wake: NativeWake,
    ready: Arc<AtomicBool>,
    input: InputMode,
    status_notifier: WorkerNotifier,
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
    let incoming = match InputQueue::new(input) {
        Ok(incoming) => incoming,
        Err(error) => {
            fail_commands(&context, error);
            return;
        }
    };
    ready.store(true, Ordering::Release);
    if !publish(&context, OscWorkerEvent::Ready(socket.address())) {
        return;
    }
    if let Err(error) = service(
        &context,
        &mut socket,
        config.receive_enabled,
        &wake,
        incoming,
        &ready,
        &status_notifier,
    ) {
        ready.store(false, Ordering::Release);
        fail_commands(&context, error);
    }
}

fn publish(context: &WorkerContext<OscWorkerCommand, OscWorkerEvent>, event: OscWorkerEvent) -> bool {
    let weight = admission::event_weight(&event);
    context.publish_weighted(event, weight).is_ok()
}

fn fail_commands(context: &WorkerContext<OscWorkerCommand, OscWorkerEvent>, error: String) {
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
            command.description()
        );
        if !publish(context, OscWorkerEvent::Error(message)) {
            return;
        }
    }
}

fn service<S: OscSocket>(
    context: &WorkerContext<OscWorkerCommand, OscWorkerEvent>,
    socket: &mut S,
    receive_enabled: bool,
    wake: &NativeWake,
    mut incoming: InputQueue,
    ready: &AtomicBool,
    status_notifier: &WorkerNotifier,
) -> Result<(), String> {
    let mut replies = NativeReplies::<OscWorkerEvent>::default();
    let mut current: Option<super::super::osc_outbound_work::Current> = None;
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
            if current.is_none() {
                let mut next = Vec::with_capacity(1);
                let turn = context.drain_commands(&mut next, NonZeroUsize::MIN);
                closed = turn.state == PendingDrainState::Disconnected;
                let Some(command) = next.pop() else { break };
                match super::super::osc_outbound_work::Current::new(command) {
                    Ok(work) => current = Some(work),
                    Err(error) => {
                        worked = true;
                        if !replies.offer(context, Source::Output, OscWorkerEvent::Error(error)) {
                            return Ok(());
                        }
                        continue;
                    }
                }
            }
            worked = true;
            let work = current.as_mut().expect("admitted outgoing work");
            match work.service(socket, || context.cancellation().is_cancelled()) {
                Ok(None) => break,
                Ok(Some(outcome)) => {
                    let event = match outcome {
                        Ok(log) => OscWorkerEvent::Sent(log),
                        Err(error) => OscWorkerEvent::Error(error),
                    };
                    if work.complete() {
                        current = None;
                    }
                    if !replies.offer(context, Source::Output, event) {
                        return Ok(());
                    }
                }
                Err(error) => {
                    current = None;
                    if !replies.offer(context, Source::Output, OscWorkerEvent::Error(error)) {
                        return Ok(());
                    }
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
                            Ok(messages) => incoming.push(messages),
                            Err(error) => incoming.push(vec![Err(error)]),
                        }
                    }
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock || should_ignore_receive_error(&error) => {
                        break
                    }
                    Err(error) => {
                        ready.store(false, Ordering::Release);
                        status_notifier.notify();
                        context.close_command_admission();
                        finish_input_prefix(context, replies, incoming);
                        if let Some(work) = current {
                            publish(context, OscWorkerEvent::Error(format!("OSC {} interrupted before its remaining destinations; native effects are not replayed", work.description())));
                        }
                        return Err(format!("OSC receive failed: {error}"));
                    }
                }
            }
            if let Some(event) = incoming.next() {
                worked = true;
                if !replies.offer(context, Source::Input, event) {
                    return Ok(());
                }
            } else {
                break;
            }
        }
        if closed && current.is_none() && incoming.is_empty() {
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
                    ready.store(false, Ordering::Release);
                    status_notifier.notify();
                    context.close_command_admission();
                    finish_input_prefix(context, replies, incoming);
                    if let Some(work) = current {
                        publish(
                            context,
                            OscWorkerEvent::Error(format!(
                                "OSC {} interrupted before its remaining destinations; native effects are not replayed",
                                work.description()
                            )),
                        );
                    }
                    return Err(format!("OSC poll failed: {error}"));
                }
            }
        }
    }
}

fn finish_input_prefix(
    context: &WorkerContext<OscWorkerCommand, OscWorkerEvent>,
    replies: NativeReplies<OscWorkerEvent>,
    mut incoming: InputQueue,
) {
    if !replies.finish(context) {
        return;
    }
    while !incoming.is_empty() {
        if let Some(event) = incoming.next() {
            if !publish(context, event) {
                return;
            }
        } else if context.cancellation().wait(FALLBACK_WAIT) {
            return;
        }
    }
}
