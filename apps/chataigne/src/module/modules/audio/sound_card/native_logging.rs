use std::{
    num::NonZeroUsize,
    panic::{catch_unwind, AssertUnwindSafe},
    sync::{
        atomic::{AtomicU8, Ordering},
        Arc, LazyLock, Mutex,
    },
    time::{Duration, Instant},
};

use golden_core::{logger::LogLevel, node::NodeId};
use golden_io::{BoundedWorker, RetirementPermit, RetirementPool, WorkerLimits};

use super::{connection_feedback::ConnectionLogLevel, prepared_event::PreparedAudioEvent, runtime::RuntimeWakeSender};

const TICKETS: usize = 32;
const MESSAGE_BYTES: usize = 16 * 1024;
const RETRY_DELAY: Duration = Duration::from_millis(250);
const PENDING: u8 = 0;
const APPLIED: u8 = 1;
const FAILED: u8 = 2;

#[derive(Clone, Copy)]
pub(super) struct LogSelection {
    pub outgoing: bool,
    pub connection: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum LogProgress {
    Waiting,
    Applied,
    Failed,
}

#[derive(Default)]
pub(super) struct LogDelivery {
    state: DeliveryState,
    retry_at: Option<Instant>,
}

#[derive(Default)]
enum DeliveryState {
    #[default]
    New,
    Pending(Arc<Completion>),
    Applied,
    Failed,
}

struct Completion {
    status: AtomicU8,
    wake: Option<RuntimeWakeSender>,
    _admission: RetirementPermit,
}

struct Job {
    event: PreparedAudioEvent,
    selection: LogSelection,
    origin: NodeId,
    completion: Arc<Completion>,
}

type Sink = Arc<dyn Fn(LogLevel, NodeId, &str) + Send + Sync>;

struct Service {
    worker: Mutex<Option<BoundedWorker<Job, ()>>>,
    lifetime: RetirementPool,
    admission: RetirementPool,
    sink: Sink,
}

impl LogDelivery {
    pub(super) fn service(
        &mut self,
        event: &PreparedAudioEvent,
        selection: LogSelection,
        origin: NodeId,
        wake: Option<&RuntimeWakeSender>,
    ) -> LogProgress {
        self.service_using(&SERVICE, event, selection, origin, wake)
    }

    fn service_using(
        &mut self,
        service: &Service,
        event: &PreparedAudioEvent,
        selection: LogSelection,
        origin: NodeId,
        wake: Option<&RuntimeWakeSender>,
    ) -> LogProgress {
        match &self.state {
            DeliveryState::Applied => return LogProgress::Applied,
            DeliveryState::Failed => return LogProgress::Failed,
            DeliveryState::Pending(completion) => {
                return match completion.status.load(Ordering::Acquire) {
                    PENDING => LogProgress::Waiting,
                    APPLIED => {
                        self.state = DeliveryState::Applied;
                        LogProgress::Applied
                    }
                    _ => {
                        self.state = DeliveryState::Failed;
                        LogProgress::Failed
                    }
                };
            }
            DeliveryState::New => {}
        }
        if !selection.has_messages(event) {
            self.state = DeliveryState::Applied;
            return LogProgress::Applied;
        }
        if let Some(deadline) = self.retry_at {
            let now = Instant::now();
            if now < deadline {
                // The shared timer keeps only the earliest deadline. An earlier
                // owner wake may have consumed it; preserve this remaining retry.
                if let Some(wake) = wake {
                    wake.wake_after(deadline.saturating_duration_since(now));
                }
                return LogProgress::Waiting;
            }
        }
        match service.submit(event.clone(), selection, origin, wake.cloned()) {
            Ok(completion) => self.state = DeliveryState::Pending(completion),
            Err(()) => {
                self.retry_at = Some(Instant::now() + RETRY_DELAY);
                if let Some(wake) = wake {
                    wake.wake_after(RETRY_DELAY);
                }
            }
        }
        LogProgress::Waiting
    }
}

impl LogSelection {
    fn has_messages(self, event: &PreparedAudioEvent) -> bool {
        (self.outgoing && event.outgoing_log.is_some())
            || event.preparation_error.is_some()
            || (self.connection
                && event
                    .connection
                    .as_ref()
                    .is_some_and(|connection| connection.log.is_some()))
    }
}

impl Job {
    fn publish(&self, sink: &Sink) {
        if self.selection.outgoing {
            if let Some(message) = &self.event.outgoing_log {
                sink(LogLevel::Info, self.origin, message);
            }
        }
        if let Some(error) = &self.event.preparation_error {
            sink(LogLevel::Error, self.origin, error);
        }
        if self.selection.connection {
            if let Some(log) = self
                .event
                .connection
                .as_ref()
                .and_then(|connection| connection.log.as_ref())
            {
                let level = match log.level {
                    ConnectionLogLevel::Info => LogLevel::Info,
                    ConnectionLogLevel::Success => LogLevel::Success,
                    ConnectionLogLevel::Error => LogLevel::Error,
                };
                sink(level, self.origin, &log.message);
            }
        }
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        // Unvisited queue suffixes and a failed writer settle explicitly. A
        // possibly committed log prefix is never replayed after failure.
        let _ = self
            .completion
            .status
            .compare_exchange(PENDING, FAILED, Ordering::AcqRel, Ordering::Acquire);
        if let Some(wake) = &self.completion.wake {
            wake.wake();
        }
    }
}

impl Service {
    fn new(sink: Sink) -> Self {
        Self {
            worker: Mutex::new(None),
            lifetime: RetirementPool::new(NonZeroUsize::MIN),
            admission: RetirementPool::new(NonZeroUsize::new(TICKETS).unwrap()),
            sink,
        }
    }

    fn submit(
        &self,
        event: PreparedAudioEvent,
        selection: LogSelection,
        origin: NodeId,
        wake: Option<RuntimeWakeSender>,
    ) -> Result<Arc<Completion>, ()> {
        let admission = self.admission.try_reserve().map_err(|_| ())?;
        let mut worker = self.worker.lock().unwrap_or_else(|p| p.into_inner());
        if worker.as_ref().is_some_and(|worker| worker.has_pending()) {
            *worker = None;
        }
        if worker.is_none() {
            let sink = self.sink.clone();
            *worker = Some(
                BoundedWorker::<Job, ()>::spawn(
                    "sound-card-native-logging",
                    &self.lifetime,
                    WorkerLimits {
                        command_items: TICKETS,
                        command_weight: TICKETS * size_of::<Job>(),
                        event_items: 1,
                        event_weight: 1,
                    },
                    move |context| {
                        while let Some(job) = context.receive() {
                            if catch_unwind(AssertUnwindSafe(|| job.publish(&sink))).is_ok() {
                                job.completion.status.store(APPLIED, Ordering::Release);
                            }
                        }
                    },
                )
                .map_err(|_| ())?,
            );
        }
        let completion = Arc::new(Completion {
            status: AtomicU8::new(PENDING),
            wake,
            _admission: admission,
        });
        worker
            .as_ref()
            .unwrap()
            .send_weighted(
                Job {
                    event,
                    selection,
                    origin,
                    completion: completion.clone(),
                },
                size_of::<Job>(),
            )
            .map_err(|_| ())?;
        Ok(completion)
    }
}

fn bounded_message(message: &str) -> String {
    const MARKER: &str = " [truncated]";
    if message.len() <= MESSAGE_BYTES {
        return message.to_owned();
    }
    let mut end = MESSAGE_BYTES - MARKER.len();
    while !message.is_char_boundary(end) {
        end -= 1;
    }
    let mut result = String::with_capacity(end + MARKER.len());
    result.push_str(&message[..end]);
    result.push_str(MARKER);
    result
}

static SERVICE: LazyLock<Service> = LazyLock::new(|| {
    Service::new(Arc::new(|level, origin, message| {
        golden_core::logger::log_message(level, "general".into(), Some(origin), bounded_message(message));
    }))
});

#[cfg(test)]
mod tests;
