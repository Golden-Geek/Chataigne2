use super::{CommandCapture, PreparedCommand, Ticket};
use golden_core::{node::NodeId, process_ctx::ProcessTreeSnapshot};
use golden_io::{BoundedWorker, RetainedPayload, RetirementPool, WorkerLimits};
use std::{
    num::NonZeroUsize,
    sync::{Arc, LazyLock, Mutex},
};

pub(super) struct Job {
    pub snapshot: RetainedPayload<ProcessTreeSnapshot>,
    pub execution: Option<RetainedPayload<serde_json::Value>>,
    pub command: NodeId,
    pub capture: CommandCapture,
    pub ticket: Ticket,
    pub target: golden_core::engine::NodeWaker,
    pub module: NodeId,
}
impl Job {
    pub(super) fn run(&self) -> Result<RetainedPayload<PreparedCommand>, String> {
        let cancelled = || self.ticket.cancelled() || self.target.is_closed();
        if cancelled() {
            return Err("Stream command lifetime retired before preparation".into());
        }
        let execution = self
            .execution
            .as_ref()
            .map(|value| {
                serde_json::from_value::<crate::app::module_command::ModuleCommandExecuteEvent>((**value).clone())
                    .map_err(|error| format!("Invalid stream invocation: {error}"))
            })
            .transpose()?;
        if execution
            .as_ref()
            .is_some_and(|execution| execution.command_id != self.command)
        {
            return Err("Stream invocation targets another command".into());
        }
        let snapshot = execution.map(|execution| {
            crate::app::module_command::snapshot_with_command_overrides(&self.snapshot, execution.param_overrides)
        });
        let view = snapshot.as_deref().unwrap_or(&self.snapshot);
        let request = self.capture.materialize(view, &cancelled)?;
        if cancelled() {
            return Err("Stream command lifetime retired during preparation".into());
        }
        let weight =
            std::mem::size_of::<PreparedCommand>() + request.bytes.capacity() + request.description.capacity() + 1024;
        crate::app::module::common::streaming::admission::retain_payload(
            PreparedCommand {
                module: self.module,
                request,
                source: self.ticket.0.cancelled.clone(),
                target: self.target.clone(),
            },
            weight,
        )
    }
}
impl Drop for Job {
    fn drop(&mut self) {
        self.ticket.complete(Err(
            "Stream preparation worker stopped before completing its accepted command".into(),
        ));
    }
}

pub(super) struct Service {
    worker: Mutex<Option<BoundedWorker<Job, ()>>>,
    lifetimes: RetirementPool,
    pub(super) admission: RetirementPool,
    prepare: fn(&Job) -> Result<RetainedPayload<PreparedCommand>, String>,
}
impl Service {
    fn new() -> Self {
        Self {
            worker: Mutex::new(None),
            lifetimes: RetirementPool::new(NonZeroUsize::MIN),
            admission: RetirementPool::new(NonZeroUsize::new(256).unwrap()),
            prepare: Job::run,
        }
    }
    pub fn submit(&self, job: Job) -> Result<(), String> {
        let mut worker = self.worker.lock().unwrap_or_else(|p| p.into_inner());
        if worker.as_ref().is_some_and(|worker| worker.has_pending()) {
            *worker = None;
        }
        if worker.is_none() {
            let prepare = self.prepare;
            *worker = Some(
                BoundedWorker::<Job, ()>::spawn(
                    "stream-command-preparation",
                    &self.lifetimes,
                    WorkerLimits {
                        command_items: 64,
                        command_weight: 64 * std::mem::size_of::<Job>(),
                        event_items: 1,
                        event_weight: 1,
                    },
                    move |context| {
                        while let Some(job) = context.receive() {
                            job.ticket.complete(prepare(&job));
                        }
                    },
                )
                .map_err(|error| format!("Stream preparation worker unavailable: {error}"))?,
            );
        }
        worker
            .as_ref()
            .unwrap()
            .send_weighted(job, std::mem::size_of::<Job>())
            .map_err(|error| match error {
                golden_io::PendingSendError::Full(_) => {
                    "Stream preparation queue is full; command was not admitted".into()
                }
                golden_io::PendingSendError::Disconnected(_) => {
                    "Stream preparation worker stopped; command was not admitted".into()
                }
            })
    }
    #[cfg(test)]
    pub(super) fn with_prepare(prepare: fn(&Job) -> Result<RetainedPayload<PreparedCommand>, String>) -> Arc<Self> {
        Arc::new(Self { prepare, ..Self::new() })
    }
    #[cfg(test)]
    pub(super) fn native_lifetimes(&self) -> RetirementPool {
        self.lifetimes.clone()
    }
}
pub(super) static SERVICE: LazyLock<Arc<Service>> = LazyLock::new(|| Arc::new(Service::new()));
