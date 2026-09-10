use std::fmt;
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, mpsc};
use std::thread::{self, JoinHandle};
use std::time::Instant;

use crate::RuntimeMetrics;

type StateTask<S> = Box<dyn FnOnce(&mut S) + Send + 'static>;

enum ControlMessage<S> {
    Apply(StateTask<S>),
    Shutdown,
}

const DEFAULT_CONTROL_CAPACITY: NonZeroUsize = NonZeroUsize::new(1_024).expect("control capacity is non-zero");

/// Admission limits for one authoritative control actor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ControlActorConfig {
    /// Maximum operations retained while another operation is executing.
    pub pending_capacity: NonZeroUsize,
}

impl Default for ControlActorConfig {
    fn default() -> Self {
        Self {
            pending_capacity: DEFAULT_CONTROL_CAPACITY,
        }
    }
}

/// Lifecycle state of one admitted control operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlStatus {
    /// The actor accepted the operation into its bounded queue.
    Accepted,
    /// The actor applied the operation to the authoritative state.
    Applied,
    /// The operation could not be admitted or completed.
    Rejected,
}

/// Stable category for a control admission or completion failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlErrorKind {
    /// The actor's bounded pending-operation capacity is exhausted.
    Overloaded,
    /// The actor is shutting down or no longer available.
    Disconnected,
}

/// Failure to admit or complete a control operation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControlError {
    kind: ControlErrorKind,
    message: Arc<str>,
}

impl ControlError {
    fn disconnected() -> Self {
        Self {
            kind: ControlErrorKind::Disconnected,
            message: "control actor is not available".into(),
        }
    }

    fn overloaded() -> Self {
        Self {
            kind: ControlErrorKind::Overloaded,
            message: "control actor pending-operation capacity is exhausted".into(),
        }
    }

    /// Returns the stable failure category.
    pub const fn kind(&self) -> ControlErrorKind {
        self.kind
    }

    /// Returns the stable diagnostic.
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for ControlError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ControlError {}

/// Completion receipt for one typed control operation.
#[derive(Debug)]
pub struct ControlReceipt<R> {
    /// Monotonic control sequence.
    pub sequence: u64,
    /// Final lifecycle state.
    pub status: ControlStatus,
    /// Time spent in the admitted actor queue.
    pub queue_wait: std::time::Duration,
    /// Time spent applying the operation to authoritative state.
    pub apply_time: std::time::Duration,
    /// Result produced by the authoritative actor.
    pub output: R,
}

/// Accepted control operation that may be awaited independently of transport work.
pub struct PendingControl<R> {
    sequence: u64,
    receiver: mpsc::Receiver<(R, std::time::Duration, std::time::Duration)>,
}

impl<R> PendingControl<R> {
    /// Returns the operation sequence assigned before admission.
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    /// Returns the current admission lifecycle state.
    pub const fn status(&self) -> ControlStatus {
        ControlStatus::Accepted
    }

    /// Waits for authoritative application.
    pub fn wait(self) -> Result<ControlReceipt<R>, ControlError> {
        let (output, queue_wait, apply_time) = self.receiver.recv().map_err(|_| ControlError::disconnected())?;
        Ok(ControlReceipt {
            sequence: self.sequence,
            status: ControlStatus::Applied,
            queue_wait,
            apply_time,
            output,
        })
    }
}

/// Cloneable typed channel into an actor-owned authoritative state.
pub struct ControlHandle<S> {
    sender: mpsc::SyncSender<ControlMessage<S>>,
    next_sequence: Arc<AtomicU64>,
    metrics: Arc<RuntimeMetrics>,
    accepting: Arc<AtomicBool>,
}

impl<S> Clone for ControlHandle<S> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            next_sequence: self.next_sequence.clone(),
            metrics: self.metrics.clone(),
            accepting: self.accepting.clone(),
        }
    }
}

impl<S: Send + 'static> ControlHandle<S> {
    /// Admits a typed operation without waiting for it to execute.
    pub fn submit<R, F>(&self, operation: F) -> Result<PendingControl<R>, ControlError>
    where
        R: Send + 'static,
        F: FnOnce(&mut S) -> R + Send + 'static,
    {
        let sequence = self.next_sequence.fetch_add(1, Ordering::Relaxed);
        let admitted_at = Instant::now();
        let (response_tx, response_rx) = mpsc::sync_channel(1);
        let metrics = self.metrics.clone();
        let task = Box::new(move |state: &mut S| {
            let queue_wait = admitted_at.elapsed();
            metrics.control_started(duration_ns(queue_wait));
            let started_at = Instant::now();
            let output = operation(state);
            let apply_time = started_at.elapsed();
            metrics.control_finished(duration_ns(apply_time));
            let _ = response_tx.send((output, queue_wait, apply_time));
        });
        self.metrics.control_received();
        let admission = if self.accepting.load(Ordering::Acquire) {
            self.sender.try_send(ControlMessage::Apply(task))
        } else {
            Err(mpsc::TrySendError::Disconnected(ControlMessage::Apply(task)))
        };
        if let Err(error) = admission {
            self.metrics.control_started(0);
            self.metrics.control_rejected();
            return Err(match error {
                mpsc::TrySendError::Full(_) => ControlError::overloaded(),
                mpsc::TrySendError::Disconnected(_) => ControlError::disconnected(),
            });
        }
        Ok(PendingControl {
            sequence,
            receiver: response_rx,
        })
    }

    /// Admits and waits for a typed operation.
    pub fn call<R, F>(&self, operation: F) -> Result<ControlReceipt<R>, ControlError>
    where
        R: Send + 'static,
        F: FnOnce(&mut S) -> R + Send + 'static,
    {
        self.submit(operation)?.wait()
    }

    /// Returns the shared lock-free runtime metrics.
    pub fn metrics(&self) -> Arc<RuntimeMetrics> {
        self.metrics.clone()
    }
}

/// Owner of one authoritative state and its dedicated control thread.
pub struct ControlActor<S> {
    handle: ControlHandle<S>,
    stopping: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl<S: Send + 'static> ControlActor<S> {
    /// Starts an actor with the supplied authoritative state.
    pub fn spawn(name: impl Into<String>, state: S) -> std::io::Result<Self> {
        Self::spawn_with_config(name, state, ControlActorConfig::default())
    }

    /// Starts an actor with explicit bounded admission settings.
    pub fn spawn_with_config(name: impl Into<String>, state: S, config: ControlActorConfig) -> std::io::Result<Self> {
        Self::spawn_with_metrics_and_config(name, state, Arc::new(RuntimeMetrics::default()), config)
    }

    /// Starts an actor with a metrics source shared by the other runtime planes.
    pub fn spawn_with_metrics(
        name: impl Into<String>,
        state: S,
        metrics: Arc<RuntimeMetrics>,
    ) -> std::io::Result<Self> {
        Self::spawn_with_metrics_and_config(name, state, metrics, ControlActorConfig::default())
    }

    /// Starts an actor with shared metrics and explicit bounded admission settings.
    pub fn spawn_with_metrics_and_config(
        name: impl Into<String>,
        state: S,
        metrics: Arc<RuntimeMetrics>,
        config: ControlActorConfig,
    ) -> std::io::Result<Self> {
        let (sender, receiver) = mpsc::sync_channel(config.pending_capacity.get());
        let accepting = Arc::new(AtomicBool::new(true));
        let stopping = Arc::new(AtomicBool::new(false));
        let handle = ControlHandle {
            sender,
            next_sequence: Arc::new(AtomicU64::new(1)),
            metrics,
            accepting,
        };
        let actor_stopping = stopping.clone();
        let thread = thread::Builder::new()
            .name(name.into())
            .spawn(move || actor_loop(state, receiver, actor_stopping))?;
        Ok(Self {
            handle,
            stopping,
            thread: Some(thread),
        })
    }

    /// Returns a cloneable handle without exposing the actor-owned state.
    pub fn handle(&self) -> ControlHandle<S> {
        self.handle.clone()
    }

    /// Admits and waits for a typed operation.
    pub fn call<R, F>(&self, operation: F) -> Result<ControlReceipt<R>, ControlError>
    where
        R: Send + 'static,
        F: FnOnce(&mut S) -> R + Send + 'static,
    {
        self.handle.call(operation)
    }

    /// Returns the shared lock-free runtime metrics.
    pub fn metrics(&self) -> Arc<RuntimeMetrics> {
        self.handle.metrics()
    }
}

impl<S> Drop for ControlActor<S> {
    fn drop(&mut self) {
        self.handle.accepting.store(false, Ordering::Release);
        self.stopping.store(true, Ordering::Release);
        let _ = self.handle.sender.try_send(ControlMessage::Shutdown);
        self.handle.metrics.control_discard_pending();
        if let Some(thread) = self.thread.take()
            && thread.thread().id() != thread::current().id()
        {
            let _ = thread.join();
        }
    }
}

fn actor_loop<S>(mut state: S, receiver: mpsc::Receiver<ControlMessage<S>>, stopping: Arc<AtomicBool>) {
    while let Ok(message) = receiver.recv() {
        match message {
            ControlMessage::Apply(task) => task(&mut state),
            ControlMessage::Shutdown => break,
        }
        if stopping.load(Ordering::Acquire) {
            break;
        }
    }
}

fn duration_ns(duration: std::time::Duration) -> u64 {
    duration.as_nanos().min(u64::MAX as u128) as u64
}
