use super::{
    handshake::{Outcome, Pending},
    outbound::{Broadcast, Command},
    peers::Peers,
    Event, ReplySource, WebSocketServerTransportConfig as Config, MAX_CLIENTS, TURN,
};
use crate::app::module::common::streaming::input::InputPlan;
use crate::app::module::common::streaming::native_replies::NativeReplies;
use golden_io::{PendingDrainState, ReconnectBackoff, WorkerContext};
use std::{
    collections::VecDeque,
    net::{TcpListener, ToSocketAddrs},
    num::NonZeroUsize,
    sync::{atomic::AtomicUsize, Arc},
    time::{Duration, Instant},
};

type Context = WorkerContext<Command, Event>;
pub(super) fn bind(config: &Config) -> Result<TcpListener, String> {
    let address = (config.bind_host.trim(), config.bind_port)
        .to_socket_addrs()
        .map_err(|_| "WebSocket server bind address could not be resolved")?
        .next()
        .ok_or("WebSocket server bind address has no endpoints")?;
    let listener = TcpListener::bind(address).map_err(|error| format!("WebSocket server bind failed: {error}"))?;
    listener.set_nonblocking(true).map_err(|error| error.to_string())?;
    Ok(listener)
}

pub(super) fn run(
    context: Context,
    config: Config,
    count: Arc<AtomicUsize>,
    input: InputPlan,
    mut binder: impl FnMut(&Config) -> Result<TcpListener, String>,
) {
    let mut peers = Peers::new(count);
    let mut handshakes = VecDeque::<Pending>::new();
    let mut listener = None;
    let mut retry_at = Instant::now();
    let mut backoff = ReconnectBackoff::new(Duration::from_millis(250), Duration::from_secs(5));
    let mut replies = NativeReplies::<Event>::default();
    let mut current: Option<Broadcast> = None;
    let mut closed = false;
    loop {
        if context.cancellation().is_cancelled() || !replies.flush(&context) {
            return;
        }
        peers.reset();
        if context.command_admission_closed() {
            listener = None;
            handshakes.clear();
        }
        // Output and input each retain at most one unpublished result. Neither waits on
        // the other's mailbox, and every wire turn has its own syscall/byte allowance.
        for _ in 0..TURN {
            if !replies.permits(ReplySource::Output) {
                break;
            }
            if current.is_none() {
                let mut commands = Vec::with_capacity(1);
                closed =
                    context.drain_commands(&mut commands, NonZeroUsize::MIN).state == PendingDrainState::Disconnected;
                if let Some(command) = commands.pop() {
                    let prepared = if peers.entries.is_empty() {
                        Err("WebSocket server command has no remaining clients; frame will not be replayed".into())
                    } else {
                        Broadcast::new(command, peers.entries.iter().map(|peer| peer.id))
                    };
                    match prepared {
                        Ok(command) => current = Some(command),
                        Err(error) => {
                            if !replies.offer(&context, ReplySource::Output, Event::Error(error)) {
                                return;
                            }
                            continue;
                        }
                    }
                }
            }
            let Some(command) = current.as_mut() else { break };
            let Some(recipient) = command.next_recipient() else {
                current = None;
                continue;
            };
            if let Some(event) = peers.write(command, recipient) {
                if !replies.offer(&context, ReplySource::Output, event) {
                    return;
                }
            }
        }
        if closed && current.is_none() {
            for _ in 0..TURN {
                if !replies.permits(ReplySource::Input) {
                    break;
                }
                if let Some(event) = peers.retire_one() {
                    if !replies.offer(&context, ReplySource::Input, event) {
                        return;
                    }
                }
            }
            if peers.entries.is_empty() {
                replies.finish(&context);
                return;
            }
        }
        if !context.command_admission_closed() {
            if listener.is_none() && Instant::now() >= retry_at && replies.permits(ReplySource::Status) {
                let bound = binder(&config).and_then(|socket| {
                    socket
                        .local_addr()
                        .map(|address| (socket, address))
                        .map_err(|error| error.to_string())
                });
                if context.cancellation().is_cancelled() {
                    return;
                }
                if context.command_admission_closed() {
                    continue;
                }
                let event = match bound {
                    Ok((socket, address)) => {
                        listener = Some(socket);
                        backoff.reset();
                        Event::Listening(address)
                    }
                    Err(error) => {
                        retry_at = backoff.schedule(Instant::now());
                        Event::Recovering(bounded_error(error))
                    }
                };
                if !replies.offer(&context, ReplySource::Status, event) {
                    return;
                }
            }
            if let Some(socket) = &listener {
                for _ in 0..TURN {
                    if !replies.permits(ReplySource::Input) || !replies.permits(ReplySource::Status) {
                        break;
                    }
                    match socket.accept() {
                        Ok((stream, address)) => {
                            if peers.entries.len() + handshakes.len() >= MAX_CLIENTS {
                                drop(stream);
                                if !replies.offer(
                                    &context,
                                    ReplySource::Status,
                                    Event::Error("WebSocket server client/handshake capacity exhausted".into()),
                                ) {
                                    return;
                                }
                                continue;
                            }
                            match Pending::start(stream, address, &config.path) {
                                Ok(pending) => handshakes.push_back(pending),
                                Err(event) => {
                                    if !replies.offer(&context, ReplySource::Status, event) {
                                        return;
                                    }
                                }
                            }
                        }
                        Err(error)
                            if matches!(
                                error.kind(),
                                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                            ) =>
                        {
                            break
                        }
                        Err(error) => {
                            listener = None;
                            retry_at = backoff.schedule(Instant::now());
                            if !replies.offer(
                                &context,
                                ReplySource::Status,
                                Event::Recovering(format!("WebSocket accept failed: {error}")),
                            ) {
                                return;
                            }
                            break;
                        }
                    }
                }
            }
            // Round robin interrupted handshakes; an idle HTTP peer cannot hold up upgrades,
            // existing clients, output, cancellation, or the combined connection limit.
            for _ in 0..TURN.min(handshakes.len()) {
                if handshakes.is_empty()
                    || !replies.permits(ReplySource::Input)
                    || !replies.permits(ReplySource::Status)
                {
                    break;
                }
                let mut pending = handshakes.pop_front().expect("bounded handshake turn");
                let outcome = pending.turn();
                if matches!(outcome, Outcome::Pending) {
                    handshakes.push_back(pending)
                } else if !publish_outcome(outcome, &mut peers, &mut replies, &context) {
                    return;
                }
            }
            for _ in 0..TURN {
                if !replies.permits(ReplySource::Input) {
                    break;
                }
                if let Some(event) = peers.receive(config.receive_enabled, &input) {
                    if !replies.offer(&context, ReplySource::Input, event) {
                        return;
                    }
                }
            }
        }
        context.cancellation().wait(Duration::from_millis(5));
    }
}

fn publish_outcome(outcome: Outcome, peers: &mut Peers, replies: &mut NativeReplies<Event>, context: &Context) -> bool {
    match outcome {
        Outcome::Pending => true,
        Outcome::Connected(socket, label, info) => {
            replies.offer(context, ReplySource::Input, peers.add(*socket, label, info))
        }
        Outcome::Failed(Event::Error(error)) => {
            replies.offer(context, ReplySource::Status, Event::Error(bounded_error(error)))
        }
        Outcome::Failed(event) => replies.offer(context, ReplySource::Status, event),
    }
}
fn bounded_error(error: String) -> String {
    if error.capacity() <= 4096 {
        error
    } else {
        "WebSocket server error details exceed 4096 bytes".into()
    }
}
