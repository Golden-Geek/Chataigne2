use super::{
    outbound::Outbound, socket::WireSocket, ReplySource, StreamingWorkerEvent as Event,
    WebSocketClientConnectionStatus as Status, WebSocketClientTransportConfig,
};
use crate::app::module::common::streaming::{
    input::{self, InputPlan},
    native_replies::{NativeReplies, NativeReply},
    parser::StreamingParser,
};
use golden_io::{PendingDrainState, ReconnectBackoff, WorkerCancellation, WorkerContext};
use std::{
    net::Shutdown,
    num::NonZeroUsize,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tokio_tungstenite::tungstenite::{Error, Message};

const POLL: Duration = Duration::from_millis(5);
const TURN: usize = 4;

struct Connection {
    socket: Option<WireSocket>,
    published: Arc<AtomicU64>,
    generation: u64,
    status: Status,
    backoff: ReconnectBackoff,
    retry_at: Instant,
    control_since: Option<Instant>,
    closing: bool,
}
impl Connection {
    fn new(published: Arc<AtomicU64>) -> Self {
        Self {
            socket: None,
            published,
            generation: 0,
            status: Status::Recovering {
                message: "WebSocket endpoint is starting".into(),
            },
            backoff: ReconnectBackoff::new(Duration::from_millis(250), Duration::from_secs(5)),
            retry_at: Instant::now(),
            control_since: None,
            closing: false,
        }
    }
    fn install(&mut self, socket: WireSocket) -> Result<(), String> {
        let remote_address = socket.get_ref().remote().map_err(|error| error.to_string())?;
        self.generation = self
            .generation
            .checked_add(1)
            .ok_or("WebSocket connection generation exhausted")?;
        self.socket = Some(socket);
        self.status = Status::Connected { remote_address };
        self.control_since = None;
        self.closing = false;
        self.backoff.reset();
        self.published.store(self.generation, Ordering::Release);
        Ok(())
    }
    fn recover(&mut self, message: String) {
        self.close();
        self.status = Status::Recovering {
            message: if message.capacity() <= 4096 {
                message
            } else {
                "WebSocket native error details exceed 4096 bytes".into()
            },
        };
        self.retry_at = self.backoff.schedule(Instant::now());
    }
    fn close(&mut self) {
        self.published.store(0, Ordering::Release);
        if let Some(socket) = self.socket.take() {
            let _ = socket.get_ref().stream.shutdown(Shutdown::Both);
        }
        self.closing = false;
        self.control_since = None;
    }
    fn peer_close(&mut self) {
        self.published.store(0, Ordering::Release);
        self.closing = true;
        self.control_since.get_or_insert_with(Instant::now);
        self.status = Status::Recovering {
            message: "WebSocket peer closed the connection".into(),
        };
    }
    fn flush_control(&mut self) {
        let Some(started) = self.control_since else {
            return;
        };
        let deadline = if self.closing {
            Duration::from_secs(1)
        } else {
            Duration::from_secs(5)
        };
        if started.elapsed() >= deadline {
            self.recover("WebSocket control write deadline exceeded".into());
            return;
        }
        let Some(socket) = &mut self.socket else {
            return;
        };
        match socket.flush() {
            Ok(()) => {
                self.control_since = None;
                if self.closing {
                    self.recover("WebSocket peer closed the connection".into());
                }
            }
            Err(Error::Io(error)) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(error) => self.recover(format!("WebSocket control completion: {error}")),
        }
    }
}
impl Drop for Connection {
    fn drop(&mut self) {
        self.close();
    }
}

pub(super) fn run(
    context: WorkerContext<Outbound, Event>,
    config: WebSocketClientTransportConfig,
    published: Arc<AtomicU64>,
    input: InputPlan,
    mut connector: impl FnMut(&WorkerCancellation, &dyn Fn() -> bool) -> Result<WireSocket, String>,
) {
    let mut connection = Connection::new(published);
    let mut replies = NativeReplies::<Event>::default();
    let mut last_status = None;
    let mut current: Option<Outbound> = None;
    let mut closed = false;
    let mut close_started = None;
    let mut parser = StreamingParser::default();
    let mut input_generation = None;
    loop {
        if context.cancellation().is_cancelled() || !replies.flush(&context) {
            return;
        }
        if let Some(socket) = &mut connection.socket {
            socket.get_mut().reset();
        }
        for _ in 0..TURN {
            if !replies.permits(ReplySource::Output) {
                break;
            }
            if current.is_none() {
                let mut commands = Vec::with_capacity(1);
                let turn = context.drain_commands(&mut commands, NonZeroUsize::MIN);
                closed = turn.state == PendingDrainState::Disconnected;
                if let Some(mut command) = commands.pop() {
                    let failure = if command.generation != connection.generation
                        || connection.socket.is_none()
                        || connection.closing
                    {
                        Some(command.failure("its original connection is unavailable"))
                    } else {
                        command.prepare().err().map(Event::Error)
                    };
                    if let Some(failure) = failure {
                        if !replies.offer(&context, ReplySource::Output, failure) {
                            return;
                        }
                        continue;
                    }
                    current = Some(command);
                }
            }
            let Some(command) = current.as_mut() else {
                break;
            };
            let outcome =
                if connection.socket.is_none() || connection.closing || command.generation != connection.generation {
                    Some(command.failure("its original connection is unavailable"))
                } else {
                    match command.write_turn(connection.socket.as_mut().expect("eligible WebSocket frame")) {
                        Ok(false) => break,
                        Ok(true) => {
                            let command = current.take().expect("completed WebSocket frame");
                            if let Some(event) = command.completion() {
                                if !replies.offer(&context, ReplySource::Output, event) {
                                    return;
                                }
                            }
                            continue;
                        }
                        Err(error) => {
                            let failure = command.failure(&error);
                            connection.recover(error);
                            Some(failure)
                        }
                    }
                };
            current = None;
            if let Some(event) = outcome {
                if !replies.offer(&context, ReplySource::Output, event) {
                    return;
                }
            }
        }
        if closed && current.is_none() {
            // A close frame follows the confirmed command prefix. Never wait for peer ACK.
            let started = *close_started.get_or_insert_with(Instant::now);
            let pending = connection.socket.as_mut().is_some_and(|socket| {
                matches!(socket.close(None), Err(Error::Io(error)) if error.kind() == std::io::ErrorKind::WouldBlock)
            });
            if !pending || started.elapsed() >= Duration::from_secs(1) {
                parser.discard_partial_input();
                let rejected = parser.take_rejected_line_count();
                replies.finish(&context);
                if rejected > 0 {
                    context
                        .publish_weighted(Event::RejectedInput(rejected), std::mem::size_of::<Event>())
                        .ok();
                }
                return;
            }
        }
        if connection.socket.is_none() || connection.closing {
            parser.discard_partial_input();
            if replies.permits(ReplySource::Input) {
                let rejected = parser.take_rejected_line_count();
                if rejected > 0 && !replies.offer(&context, ReplySource::Input, Event::RejectedInput(rejected)) {
                    return;
                }
            }
        }
        if connection.socket.is_none()
            && !context.command_admission_closed()
            && Instant::now() >= connection.retry_at
            && replies.permits(ReplySource::Status)
        {
            let result = connector(context.cancellation(), &|| context.command_admission_closed());
            if context.cancellation().is_cancelled() {
                return;
            }
            if context.command_admission_closed() {
                drop(result);
                continue;
            }
            if let Err(error) = result.and_then(|socket| connection.install(socket)) {
                connection.recover(error);
            }
        }
        if replies.permits(ReplySource::Status) && last_status.as_ref() != Some(&connection.status) {
            let status = connection.status.clone();
            if !replies.offer(&context, ReplySource::Status, Event::Status(status.clone())) {
                return;
            }
            last_status = Some(status);
        }
        if !context.command_admission_closed() && !connection.closing {
            for _ in 0..TURN {
                if !replies.permits(ReplySource::Input) {
                    break;
                }
                let Some(socket) = &mut connection.socket else {
                    break;
                };
                let event = match socket.read() {
                    Ok(message @ (Message::Text(_) | Message::Binary(_))) if config.receive_enabled => {
                        let snapshot = input.snapshot();
                        let generation = (connection.generation, snapshot.generation);
                        if input_generation != Some(generation) {
                            parser.discard_partial_input();
                            input_generation = Some(generation);
                        }
                        let text = matches!(&message, Message::Text(_));
                        let bytes = message.into_data();
                        Some(
                            match input::prepare(&mut parser, &snapshot, &bytes, "WebSocket", None, text) {
                                Ok(input) => Event::Input(input),
                                Err(error) => Event::Error(format!("WebSocket input was not admitted: {error}")),
                            },
                        )
                    }
                    Ok(Message::Ping(_)) => {
                        connection.control_since.get_or_insert_with(Instant::now);
                        None
                    }
                    Ok(Message::Close(_)) => {
                        connection.peer_close();
                        break;
                    }
                    Ok(_) => None,
                    Err(Error::Io(error)) if error.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(error) => {
                        connection.recover(format!("WebSocket receive failed: {error}"));
                        break;
                    }
                };
                if let Some(event) = event {
                    if event.weight() > Event::limit(ReplySource::Input) {
                        parser.discard_partial_input();
                    }
                    if !replies.offer(&context, ReplySource::Input, event) {
                        return;
                    }
                }
            }
        }
        connection.flush_control();
        context.cancellation().wait(POLL);
    }
}
