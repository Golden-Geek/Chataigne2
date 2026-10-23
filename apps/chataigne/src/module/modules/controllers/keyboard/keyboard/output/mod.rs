use super::*;
use crate::app::module::common::keyboard::{KeyboardKey, KeyboardKeyAction};
use crate::app::module::common::worker_wake::ModuleWake;
use golden_io::{ReconnectBackoff, RetainedPayload, RevisionGuard, WorkerNotifier};
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

pub(super) mod native;
mod receipts;
mod route;
mod runtime;
mod source;
use receipts::{Command, Outcome, Receipt};
pub(crate) use route::dispatch;
use runtime::{Provider, Request, Runtime};
use source::{Input, Source};

pub(super) struct Service {
    runtime: Option<Runtime>,
    wake: Option<ModuleWake>,
    owner: RetirementPool,
    admission: RetirementPool,
    ingress: RetirementPool,
    outgoing: VecDeque<RetainedPayload<Source>>,
    receipts: VecDeque<Receipt>,
    retry_at: Option<Instant>,
    submit_at: Option<Instant>,
    backoff: ReconnectBackoff,
    observed_ready: bool,
    last_error: Option<String>,
}
impl Service {
    pub(super) fn new() -> Self {
        Self {
            runtime: None,
            wake: None,
            owner: RetirementPool::new(NonZeroUsize::MIN),
            admission: RetirementPool::new(NonZeroUsize::new(64).unwrap()),
            ingress: RetirementPool::new(NonZeroUsize::new(64).unwrap()),
            outgoing: VecDeque::new(),
            receipts: VecDeque::new(),
            retry_at: None,
            submit_at: None,
            backoff: ReconnectBackoff::new(Duration::from_millis(250), Duration::from_secs(5)),
            observed_ready: false,
            last_error: None,
        }
    }
    pub(super) fn is_ready(&self) -> bool {
        self.runtime.as_ref().is_some_and(Runtime::is_ready)
    }
    pub(super) fn has_results(&self) -> bool {
        !self.receipts.is_empty()
    }
}
impl KeyboardModule {
    pub(super) fn register_output(&mut self, ctx: &ProcessCtx) -> Result<(), String> {
        let waker = match ctx.node_waker() {
            Ok(waker) => waker,
            Err(golden_core::engine::NodeWakeError::Unavailable) => return Ok(()),
            Err(error) => return Err(error.to_string()),
        };
        if !self.output.wake.as_ref().is_some_and(|wake| wake.matches(&waker)) {
            self.stop_output_backend();
            self.output.wake = Some(ModuleWake::new(waker)?);
        }
        Ok(())
    }
    pub(super) fn request_output_service(&self) {
        if let Some(wake) = &self.output.wake {
            wake.notifier.notify();
        }
    }
    pub(super) fn stop_output_backend(&mut self) {
        self.output.runtime = None;
        if !self.output.outgoing.is_empty() {
            golden_core::logerror!(origin=self.id(); format!("Keyboard retirement cancelled {} captured requests; not sent", self.output.outgoing.len()));
        }
        self.output.outgoing.clear();
        self.output.retry_at = None;
        self.output.submit_at = None;
        self.output.observed_ready = false;
    }
    fn ensure_output(&mut self, ctx: &mut ProcessCtx) -> Result<(), String> {
        if !self.node_data().effective_enabled || !self.send_enabled.is_bound() || !self.send_enabled.get() {
            self.stop_output_backend();
            return Err("Keyboard output is disabled".into());
        }
        self.register_output(ctx)?;
        if self.output.runtime.is_some() {
            return Ok(());
        }
        if self.suppress_output_backend {
            return Err("Keyboard output backend is suppressed for tests".into());
        }
        if self.output.retry_at.is_some_and(|at| Instant::now() < at) {
            return Err("Keyboard output is waiting for recovery".into());
        }
        let notifier = self
            .output
            .wake
            .as_ref()
            .ok_or("Keyboard output wake is unavailable")?
            .notifier
            .clone();
        match Runtime::create(&self.output.owner, notifier) {
            Ok(runtime) => {
                self.output.runtime = Some(runtime);
                self.output.retry_at = None;
                Ok(())
            }
            Err(error) => {
                self.output.retry_at = Some(self.output.backoff.schedule(Instant::now()));
                self.set_backend_warning(ctx, KEYBOARD_OUTPUT_WARNING_ID, &error);
                Err(error)
            }
        }
    }
    pub(super) fn service_output(&mut self, ctx: &mut ProcessCtx) {
        if ctx.tree_snapshot_arc().is_none() {
            return;
        }
        if let Err(error) = self.register_output(ctx) {
            self.set_backend_warning(ctx, KEYBOARD_OUTPUT_WARNING_ID, &error);
            return;
        }
        let can_project = self.guard_output(ctx);
        for _ in 0..self.output.receipts.len().min(16) {
            let receipt = self.output.receipts.pop_front().unwrap();
            let Some(outcome) = receipt.take() else {
                self.output.receipts.push_back(receipt);
                continue;
            };
            if can_project {
                match outcome {
                    Outcome::Completed(Ok(message)) => {
                        self.clear_backend_warning(ctx, KEYBOARD_OUTPUT_WARNING_ID);
                        self.set_last_event(ctx, message);
                    }
                    Outcome::Completed(Err(error)) => {
                        self.set_backend_warning(ctx, KEYBOARD_OUTPUT_WARNING_ID, &error);
                        self.set_last_event(ctx, format!("Keyboard command failed: {error}"));
                    }
                    Outcome::NotSent => self.set_last_event(ctx, "Keyboard command cancelled; not sent".into()),
                    Outcome::Unconfirmed => {
                        self.set_last_event(ctx, "Keyboard command interrupted; result unconfirmed".into())
                    }
                }
                if receipt.attempted() {
                    self.base.emit_outgoing_traffic(ctx);
                }
            } else {
                golden_core::logerror!(format!(
                    "Keyboard completion target unavailable; disposition: {outcome:?}"
                ));
            }
            ctx.edits.retain_payload(receipt.source());
        }
        if self
            .output
            .runtime
            .as_ref()
            .is_some_and(|runtime| !runtime.is_running())
        {
            let error = self.output.runtime.as_ref().unwrap().error();
            self.stop_output_backend();
            if self.output.last_error.as_ref() != Some(&error) {
                golden_core::logerror!(origin=self.id(); format!("Keyboard output stopped: {error}"));
                self.output.last_error = Some(error.clone());
            }
            if can_project {
                self.set_backend_warning(ctx, KEYBOARD_OUTPUT_WARNING_ID, &error);
            }
            self.output.retry_at = Some(self.output.backoff.schedule(Instant::now()));
        }
        if self.node_data().effective_enabled && self.send_enabled.get() {
            let _ = self.ensure_output(ctx);
            if self.output.is_ready() && !self.output.observed_ready {
                self.output.observed_ready = true;
                self.output.backoff.reset();
                self.output.last_error = None;
                if can_project {
                    self.clear_backend_warning(ctx, KEYBOARD_OUTPUT_WARNING_ID);
                }
            }
            self.submit_keyboard_outputs();
        } else {
            self.stop_output_backend();
        }
        if can_project && self.guard_output_settings(ctx) {
            let output_ready = self.output.is_ready();
            if let Some(runtime) = self.output.runtime.as_ref().filter(|_| output_ready) {
                // Completion messages survive native retirement; readiness does not.
                ctx.edits.push(golden_core::edit::Edit::CheckPreconditions {
                    preconditions: golden_core::edit::EditPreconditions::new(vec![], runtime.status_guards().to_vec())
                        .unwrap(),
                });
            }
            self.refresh_connection_state_with_output(ctx, output_ready);
        }
        if self.output.receipts.iter().any(Receipt::complete) {
            self.request_output_service();
        }
        if let Some(wake) = &self.output.wake {
            let deadline = [self.output.retry_at, self.output.submit_at]
                .into_iter()
                .flatten()
                .min();
            if let Some(at) = deadline {
                wake.later(at.saturating_duration_since(Instant::now()));
            } else {
                wake.cancel_deadline();
            }
        }
    }
}
#[cfg(test)]
mod tests;
