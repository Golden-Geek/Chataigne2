use std::io;
use std::num::NonZeroUsize;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::Duration;

use crate::{
    PendingDrain, PendingDrainState, PendingReceiver, PendingSendError, PendingSender, RetirementCapacityError,
    RetirementPool, WorkerNotifier, bounded_pending_channel, notified_pending_channel,
};

/// Item and retained-weight bounds for both directions of a worker boundary.
#[derive(Clone, Copy, Debug)]
pub struct WorkerLimits {
    pub command_items: usize,
    pub command_weight: usize,
    pub event_items: usize,
    pub event_weight: usize,
}

/// Cancellation and notifications shared by a handle and its sole worker.
#[derive(Clone, Default)]
pub struct WorkerCancellation(Arc<WakeState>);

#[derive(Default)]
struct WakeState {
    state: Mutex<WakeVersion>,
    changed: Condvar,
}

#[derive(Clone, Copy, Default)]
struct WakeVersion {
    version: u64,
    cancelled: bool,
}

impl WorkerCancellation {
    pub fn is_cancelled(&self) -> bool {
        self.snapshot().cancelled
    }

    /// Waits through ordinary notifications; returns true only for cancellation.
    pub fn wait(&self, duration: Duration) -> bool {
        let state = self.0.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let (state, _) = self
            .0
            .changed
            .wait_timeout_while(state, duration, |state| !state.cancelled)
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.cancelled
    }

    fn cancel(&self) {
        let mut state = self.0.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        state.cancelled = true;
        drop(state);
        self.0.changed.notify_all();
    }

    fn notify(&self) {
        let mut state = self.0.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        state.version = state.version.wrapping_add(1);
        drop(state);
        self.0.changed.notify_all();
    }

    fn snapshot(&self) -> WakeVersion {
        *self.0.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn wait_for_change(&self, observed: WakeVersion) {
        let state = self.0.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        drop(
            self.0
                .changed
                .wait_while(state, |state| !state.cancelled && state.version == observed.version)
                .unwrap_or_else(|poisoned| poisoned.into_inner()),
        );
    }
}

/// Nonblocking owner of a bounded worker. Dropping it explicitly cancels queued work.
///
/// A lifetime slot is reserved before thread creation and held by that thread through resource
/// cleanup. Cancellation never needs mailbox admission, a join, or a new retirement thread.
/// An uninterruptible native operation therefore retains its slot rather than permitting an
/// unbounded series of replacement workers. The host may wait on the supplied pool at shutdown.
pub struct BoundedWorker<C, E> {
    commands: Option<PendingSender<C>>,
    events: PendingReceiver<E>,
    cancellation: WorkerCancellation,
}

/// Worker-owned endpoints. Only the worker may wait for commands or event capacity.
pub struct WorkerContext<C, E> {
    commands: PendingReceiver<C>,
    events: PendingSender<E>,
    cancellation: WorkerCancellation,
    maximum_event_weight: usize,
}

/// Worker startup failed before ownership of any commands was accepted.
#[derive(Debug)]
pub enum WorkerStartError {
    Capacity(RetirementCapacityError),
    Io(io::Error),
}

impl std::fmt::Display for WorkerStartError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Capacity(error) => error.fmt(f),
            Self::Io(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for WorkerStartError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Capacity(error) => Some(error),
            Self::Io(error) => Some(error),
        }
    }
}

impl<C: Send + 'static, E: Send + 'static> BoundedWorker<C, E> {
    pub fn spawn<F>(
        name: impl Into<String>,
        lifetimes: &RetirementPool,
        limits: WorkerLimits,
        run: F,
    ) -> Result<Self, WorkerStartError>
    where
        F: FnOnce(WorkerContext<C, E>) + Send + 'static,
    {
        Self::spawn_with_notifications(name, lifetimes, limits, None, run)
    }

    /// Notifies the owner after each reply publication and final producer disconnection.
    pub fn spawn_notified<F>(
        name: impl Into<String>,
        lifetimes: &RetirementPool,
        limits: WorkerLimits,
        notifier: WorkerNotifier,
        run: F,
    ) -> Result<Self, WorkerStartError>
    where
        F: FnOnce(WorkerContext<C, E>) + Send + 'static,
    {
        Self::spawn_with_notifications(name, lifetimes, limits, Some(notifier), run)
    }

    fn spawn_with_notifications<F>(
        name: impl Into<String>,
        lifetimes: &RetirementPool,
        limits: WorkerLimits,
        notifier: Option<WorkerNotifier>,
        run: F,
    ) -> Result<Self, WorkerStartError>
    where
        F: FnOnce(WorkerContext<C, E>) + Send + 'static,
    {
        let permit = lifetimes.try_reserve().map_err(WorkerStartError::Capacity)?;
        let (commands, command_rx) = bounded_pending_channel(limits.command_items, limits.command_weight);
        let (event_tx, events) = match notifier {
            Some(notifier) => notified_pending_channel(limits.event_items, limits.event_weight, notifier),
            None => bounded_pending_channel(limits.event_items, limits.event_weight),
        };
        let cancellation = WorkerCancellation::default();
        let context = WorkerContext {
            commands: command_rx,
            events: event_tx,
            cancellation: cancellation.clone(),
            maximum_event_weight: limits.event_weight,
        };
        thread::Builder::new()
            .name(name.into())
            .spawn(move || {
                let _lifetime = permit;
                run(context);
            })
            .map_err(WorkerStartError::Io)?;
        Ok(Self {
            commands: Some(commands),
            events,
            cancellation,
        })
    }

    pub fn send_weighted(&self, command: C, weight: usize) -> Result<(), PendingSendError<C>> {
        if self.cancellation.is_cancelled() {
            return Err(PendingSendError::Disconnected(command));
        }
        let Some(commands) = &self.commands else {
            return Err(PendingSendError::Disconnected(command));
        };
        commands.send_weighted(command, weight)?;
        self.cancellation.notify();
        Ok(())
    }

    /// Cheap preparation preflight. True does not reserve capacity or guarantee a
    /// later send; cancellation and receiver closure may race with the caller.
    pub fn is_accepting_commands(&self) -> bool {
        !self.cancellation.is_cancelled()
            && self
                .commands
                .as_ref()
                .is_some_and(|commands| !commands.is_disconnected())
    }

    /// Closes admission while preserving every admitted command and its pending replies.
    /// Keep the handle and drain replies until disconnection; dropping it still cancels work.
    pub fn finish_admission(&mut self) {
        self.commands.take();
        self.cancellation.notify();
    }

    pub fn drain_events(&self, output: &mut Vec<E>, budget: NonZeroUsize) -> PendingDrain {
        let result = self.events.drain_into(output, budget);
        if result.received != 0 {
            self.cancellation.notify();
        }
        result
    }

    pub fn has_pending(&self) -> bool {
        self.events.has_pending() || self.events.is_disconnected()
    }

    /// Cancels this lifetime, including retry and capacity waits, without waiting for native I/O.
    pub fn cancel(&self) {
        self.cancellation.cancel();
    }
}

impl<C, E> Drop for BoundedWorker<C, E> {
    fn drop(&mut self) {
        self.cancellation.cancel();
    }
}

impl<C, E> WorkerContext<C, E> {
    /// Services one bounded, nonblocking command turn alongside unsolicited native input.
    /// Graceful admission closure drains every accepted command before disconnection;
    /// explicit cancellation abandons the remaining commands, as with `receive`.
    pub fn drain_commands(&self, output: &mut Vec<C>, budget: NonZeroUsize) -> PendingDrain {
        if self.cancellation.is_cancelled() {
            return PendingDrain {
                received: 0,
                state: PendingDrainState::Disconnected,
            };
        }
        self.commands.drain_into(output, budget)
    }

    /// Refuses further producer admission, retaining accepted commands for failure completion.
    pub fn close_command_admission(&self) {
        self.commands.close();
        self.cancellation.notify();
    }

    pub fn cancellation(&self) -> &WorkerCancellation {
        &self.cancellation
    }

    /// Receives one ordered command, or stops after cancellation/disconnection.
    pub fn receive(&self) -> Option<C> {
        loop {
            let observed = self.cancellation.snapshot();
            if observed.cancelled {
                return None;
            }
            let mut command = Vec::with_capacity(1);
            let drain = self.commands.drain_into(&mut command, NonZeroUsize::MIN);
            if let Some(command) = command.pop() {
                return Some(command);
            }
            if drain.state == PendingDrainState::Disconnected {
                return None;
            }
            self.cancellation.wait_for_change(observed);
        }
    }

    /// Retains a completed event while waiting for capacity; never silently drops accepted work.
    /// Oversized events return `Full` immediately. Cancellation returns the event as disconnected.
    pub fn publish_weighted(&self, mut event: E, weight: usize) -> Result<(), PendingSendError<E>> {
        if weight > self.maximum_event_weight {
            return Err(PendingSendError::Full(event));
        }
        loop {
            let observed = self.cancellation.snapshot();
            if observed.cancelled {
                return Err(PendingSendError::Disconnected(event));
            }
            match self.events.send_weighted(event, weight) {
                Ok(()) => return Ok(()),
                Err(PendingSendError::Disconnected(event)) => return Err(PendingSendError::Disconnected(event)),
                Err(PendingSendError::Full(returned)) => event = returned,
            }
            self.cancellation.wait_for_change(observed);
        }
    }
}
