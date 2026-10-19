use super::{
    outbound::{Outbound, WriteTurn},
    ReplySource, StreamingWorkerEvent, TcpStreamingConnectionStatus as Status, TcpStreamingTransportConfig,
};
use crate::app::module::common::streaming::native_replies::NativeReplies;
use golden_io::{PendingDrainState, ReconnectBackoff, WorkerCancellation, WorkerContext};
use std::{
    io::Read,
    net::{Shutdown, TcpStream, ToSocketAddrs},
    num::NonZeroUsize,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

const POLL: Duration = Duration::from_millis(5);
const COMMAND_TURN: usize = 16;
const RECEIVE_TURN: usize = 4;

struct Connection {
    stream: Option<TcpStream>,
    published: Arc<AtomicU64>,
    generation: u64,
    status: Status,
    backoff: ReconnectBackoff,
    retry_at: Instant,
    health_at: Instant,
}

impl Connection {
    fn new(published: Arc<AtomicU64>) -> Self {
        Self {
            stream: None,
            published,
            generation: 0,
            status: Status::Recovering {
                message: "TCP endpoint is starting".into(),
            },
            backoff: ReconnectBackoff::new(Duration::from_millis(250), Duration::from_secs(5)),
            retry_at: Instant::now(),
            health_at: Instant::now(),
        }
    }
    fn install(&mut self, stream: TcpStream) -> Result<(), String> {
        stream.set_nonblocking(true).map_err(|error| error.to_string())?;
        let remote_address = stream.peer_addr().map_err(|error| error.to_string())?;
        self.generation = self
            .generation
            .checked_add(1)
            .ok_or("TCP connection generation exhausted")?;
        self.stream = Some(stream);
        self.status = Status::Connected { remote_address };
        self.backoff.reset();
        self.health_at = Instant::now() + Duration::from_millis(250);
        self.published.store(self.generation, Ordering::Release);
        Ok(())
    }
    fn recover(&mut self, message: String) {
        self.close();
        let message = if message.capacity() <= 4096 {
            message
        } else {
            "TCP native error details exceed 4096 bytes".into()
        };
        self.status = Status::Recovering { message };
        self.retry_at = self.backoff.schedule(Instant::now());
    }
    fn close(&mut self) {
        self.published.store(0, Ordering::Release);
        if let Some(stream) = self.stream.take() {
            let _ = stream.shutdown(Shutdown::Both);
        }
    }
}
impl Drop for Connection {
    fn drop(&mut self) {
        self.close();
    }
}

pub(super) fn run(
    context: WorkerContext<Outbound, StreamingWorkerEvent>,
    config: TcpStreamingTransportConfig,
    published: Arc<AtomicU64>,
    mut connector: impl FnMut(&WorkerCancellation) -> Result<TcpStream, String>,
) {
    let mut connection = Connection::new(published);
    let mut replies = NativeReplies::<StreamingWorkerEvent>::default();
    let mut last_status = None;
    let mut current: Option<Outbound> = None;
    let mut closed = false;
    let mut buffer = [0; 8192];
    loop {
        if context.cancellation().is_cancelled() || !replies.flush(&context) {
            return;
        }
        let mut write_blocked = false;
        for _ in 0..COMMAND_TURN {
            if !replies.permits(ReplySource::Output) {
                break;
            }
            if current.is_none() {
                let mut commands = Vec::with_capacity(1);
                let turn = context.drain_commands(&mut commands, NonZeroUsize::MIN);
                closed = turn.state == PendingDrainState::Disconnected;
                current = commands.pop();
            }
            let Some(command) = current.as_mut() else { break };
            let outcome = if command.generation != connection.generation || connection.stream.is_none() {
                Some(command.failure("its original connection is unavailable"))
            } else {
                match command.write_turn(connection.stream.as_mut().expect("connected write cursor")) {
                    Ok(WriteTurn::Complete) => {
                        let command = current.take().expect("completed write cursor");
                        if let Some(event) = command.completion() {
                            if !replies.offer(&context, ReplySource::Output, event) {
                                return;
                            }
                        }
                        continue;
                    }
                    Ok(WriteTurn::Pending { progressed }) => {
                        write_blocked = !progressed;
                        break;
                    }
                    Err(error) => {
                        let message = format!("TCP write failed: {error}");
                        let event = command.failure(&message);
                        connection.recover(message);
                        Some(event)
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
            replies.finish(&context);
            return;
        }
        if connection.stream.is_none()
            && !context.command_admission_closed()
            && Instant::now() >= connection.retry_at
            && replies.permits(ReplySource::Status)
        {
            let result = connector(context.cancellation());
            if context.cancellation().is_cancelled() {
                return;
            }
            if context.command_admission_closed() {
                drop(result);
                continue;
            }
            if let Err(error) = result.and_then(|stream| connection.install(stream)) {
                connection.recover(error);
            }
        }
        if replies.permits(ReplySource::Status) && last_status.as_ref() != Some(&connection.status) {
            let status = connection.status.clone();
            if !replies.offer(
                &context,
                ReplySource::Status,
                StreamingWorkerEvent::Status(status.clone()),
            ) {
                return;
            }
            last_status = Some(status);
        }
        if config.receive_enabled && !context.command_admission_closed() {
            for _ in 0..RECEIVE_TURN {
                if !replies.permits(ReplySource::Input)
                    || context.cancellation().is_cancelled()
                    || context.command_admission_closed()
                {
                    break;
                }
                let Some(stream) = connection.stream.as_mut() else {
                    break;
                };
                match stream.read(&mut buffer) {
                    Ok(0) => {
                        connection.recover("TCP peer closed the connection".into());
                        break;
                    }
                    Ok(count) => {
                        if !replies.offer(
                            &context,
                            ReplySource::Input,
                            StreamingWorkerEvent::Bytes(buffer[..count].to_vec()),
                        ) {
                            return;
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(error) if error.kind() == std::io::ErrorKind::Interrupted => break,
                    Err(error) => {
                        connection.recover(format!("TCP receive failed: {error}"));
                        break;
                    }
                }
            }
        }
        if let Some(stream) = &connection.stream {
            if Instant::now() >= connection.health_at {
                if let Err(error) = health(stream) {
                    connection.recover(error);
                }
                connection.health_at = Instant::now() + Duration::from_millis(250);
            }
        }
        if current.is_none() || write_blocked || !replies.permits(ReplySource::Output) {
            context.cancellation().wait(POLL);
        }
    }
}

pub(super) fn connect(config: &TcpStreamingTransportConfig, cancel: &WorkerCancellation) -> Result<TcpStream, String> {
    let addresses = (config.remote_host.trim(), config.remote_port)
        .to_socket_addrs()
        .map_err(|error| format!("TCP endpoint resolution failed: {error}"))?;
    let mut error = "TCP endpoint has no resolved address".to_owned();
    for address in addresses.take(8) {
        if cancel.is_cancelled() {
            return Err("TCP setup cancelled".into());
        }
        match TcpStream::connect_timeout(&address, Duration::from_millis(500)) {
            Ok(stream) => return Ok(stream),
            Err(failure) => error = format!("TCP connect to {address} failed: {failure}"),
        }
    }
    Err(error)
}

fn health(stream: &TcpStream) -> Result<(), String> {
    if let Some(error) = stream.take_error().map_err(|error| error.to_string())? {
        return Err(format!("TCP stream error: {error}"));
    }
    match stream.peek(&mut [0]) {
        Ok(0) => Err("TCP peer closed the connection".into()),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => Ok(()),
        Err(error) => Err(format!("TCP health check failed: {error}")),
    }
}
