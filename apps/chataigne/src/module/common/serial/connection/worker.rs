use super::{
    outbound::Outbound, Event, ReplySource, SerialConnectionConfig as Config, SerialConnectionStatus as Status, TURN,
};
use crate::app::module::common::streaming::native_replies::NativeReplies;
use golden_io::{PendingDrainState, ReconnectBackoff, WorkerContext};
use std::{
    io::{self, Read, Write},
    num::NonZeroUsize,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

pub(super) fn open(config: &Config) -> Result<Box<dyn serialport::SerialPort>, String> {
    let mut port = serialport::new(&config.port_name, config.baud_rate)
        .timeout(Duration::from_millis(20))
        .open()
        .map_err(|error| format!("Serial port {} is unavailable: {error}", config.port_name))?;
    port.write_data_terminal_ready(config.dtr)
        .map_err(|error| format!("Serial DTR setup failed: {error}"))?;
    port.write_request_to_send(config.rts)
        .map_err(|error| format!("Serial RTS setup failed: {error}"))?;
    Ok(port)
}
struct Connection<P> {
    port: Option<P>,
    published: Arc<AtomicU64>,
    generation: u64,
    status: Status,
    retry_at: Instant,
    backoff: ReconnectBackoff,
}
impl<P> Connection<P> {
    fn new(published: Arc<AtomicU64>, config: &Config) -> Self {
        Self {
            port: None,
            published,
            generation: 0,
            status: Status::Recovering {
                port_name: config.port_name.clone(),
                message: "Serial endpoint is starting".into(),
            },
            retry_at: Instant::now(),
            backoff: ReconnectBackoff::new(Duration::from_millis(250), Duration::from_secs(5)),
        }
    }
    fn install(&mut self, port: P, config: &Config) {
        self.generation = self
            .generation
            .checked_add(1)
            .expect("serial connection generation exhausted");
        self.port = Some(port);
        self.status = Status::Connected {
            port_name: config.port_name.clone(),
        };
        self.backoff.reset();
        self.published.store(self.generation, Ordering::Release);
    }
    fn recover(&mut self, config: &Config, message: String) {
        self.close();
        self.status = Status::Recovering {
            port_name: config.port_name.clone(),
            message: bounded_error(message),
        };
        self.retry_at = self.backoff.schedule(Instant::now());
    }
    fn close(&mut self) {
        self.published.store(0, Ordering::Release);
        self.port.take();
    }
}
impl<P> Drop for Connection<P> {
    fn drop(&mut self) {
        self.close();
    }
}

pub(super) fn run<P: Read + Write>(
    context: WorkerContext<Outbound, Event>,
    config: Config,
    published: Arc<AtomicU64>,
    mut opener: impl FnMut(&Config) -> Result<P, String>,
) {
    let mut connection = Connection::new(published, &config);
    let mut replies = NativeReplies::<Event>::default();
    let mut last_status = None;
    let mut current: Option<Outbound> = None;
    let mut closed = false;
    let mut buffer = [0; 8192];
    loop {
        if context.cancellation().is_cancelled() || !replies.flush(&context) {
            return;
        }
        for _ in 0..TURN {
            if context.cancellation().is_cancelled() {
                return;
            }
            if !replies.permits(ReplySource::Output) {
                break;
            }
            if current.is_none() {
                let mut commands = Vec::with_capacity(1);
                closed =
                    context.drain_commands(&mut commands, NonZeroUsize::MIN).state == PendingDrainState::Disconnected;
                current = commands.pop();
            }
            let Some(command) = current.as_mut() else { break };
            let failure = if command.generation != connection.generation || connection.port.is_none() {
                Some(command.failure("its original connection is unavailable"))
            } else {
                match command.write_turn(connection.port.as_mut().expect("connected serial cursor")) {
                    Ok(true) => {
                        current = None;
                        continue;
                    }
                    Ok(false) => break,
                    Err(error) => {
                        let failure = command.failure(&error);
                        connection.recover(&config, error);
                        Some(failure)
                    }
                }
            };
            current = None;
            if let Some(event) = failure {
                if !replies.offer(&context, ReplySource::Output, event) {
                    return;
                }
            }
        }
        if closed && current.is_none() {
            connection.close();
            replies.finish(&context);
            return;
        }
        if connection.port.is_none()
            && !context.command_admission_closed()
            && Instant::now() >= connection.retry_at
            && replies.permits(ReplySource::Status)
        {
            let opened = opener(&config);
            if context.cancellation().is_cancelled() || context.command_admission_closed() {
                return;
            }
            match opened {
                Ok(port) => connection.install(port, &config),
                Err(error) => connection.recover(&config, error),
            }
        }
        if last_status.as_ref() != Some(&connection.status) && replies.permits(ReplySource::Status) {
            last_status = Some(connection.status.clone());
            if !replies.offer(&context, ReplySource::Status, Event::Status(connection.status.clone())) {
                return;
            }
        }
        if !context.cancellation().is_cancelled()
            && !context.command_admission_closed()
            && replies.permits(ReplySource::Input)
        {
            if let Some(port) = &mut connection.port {
                match port.read(&mut buffer) {
                    Ok(0) => {}
                    Ok(length) => {
                        if !replies.offer(&context, ReplySource::Input, Event::Bytes(buffer[..length].to_vec())) {
                            return;
                        }
                    }
                    Err(error)
                        if matches!(
                            error.kind(),
                            io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                        ) => {}
                    Err(error) => connection.recover(&config, format!("Serial read failed: {error}")),
                }
            }
        }
        context.cancellation().wait(Duration::from_millis(5));
    }
}
fn bounded_error(message: String) -> String {
    if message.capacity() <= 4096 {
        message
    } else {
        "Serial native error details exceed 4096 bytes".into()
    }
}
