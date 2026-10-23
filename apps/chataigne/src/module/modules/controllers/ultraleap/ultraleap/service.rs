use super::*;
use crate::app::module::common::worker_wake::ModuleWake;
use golden_io::{ReconnectBackoff, RetirementPool};
use std::{
    num::NonZeroUsize,
    time::{Duration, Instant},
};

pub(super) struct Service {
    pub(super) runtime: Option<UltraleapRuntime>,
    pub(super) owner: RetirementPool,
    wake: Option<ModuleWake>,
    pub(super) retry_at: Option<Instant>,
    backoff: ReconnectBackoff,
    last_error: Option<String>,
    pressure: bool,
    pub(super) suppressed: bool,
    #[cfg(test)]
    pub(super) factory: Option<Factory>,
}
#[cfg(test)]
type Factory =
    Box<dyn Fn(&RetirementPool, golden_io::WorkerNotifier) -> Result<UltraleapRuntime, String> + Send + Sync>;
impl Service {
    pub(super) fn new() -> Self {
        Self {
            runtime: None,
            owner: RetirementPool::new(NonZeroUsize::MIN),
            wake: None,
            retry_at: None,
            backoff: ReconnectBackoff::new(Duration::from_millis(250), Duration::from_secs(5)),
            last_error: None,
            pressure: false,
            suppressed: false,
            #[cfg(test)]
            factory: None,
        }
    }
}
impl UltraleapModule {
    pub(super) fn register_service(&mut self, ctx: &ProcessCtx) -> Result<(), String> {
        let waker = match ctx.node_waker() {
            Ok(waker) => waker,
            Err(golden_core::engine::NodeWakeError::Unavailable) => return Ok(()),
            Err(error) => return Err(error.to_string()),
        };
        if !self.service.wake.as_ref().is_some_and(|wake| wake.matches(&waker)) {
            self.stop_runtime();
            self.service.wake = Some(ModuleWake::new(waker)?);
        }
        Ok(())
    }
    pub(super) fn request_service(&self) {
        if let Some(wake) = &self.service.wake {
            wake.notifier.notify();
        }
    }
    pub(super) fn stop_runtime(&mut self) {
        self.service.runtime = None;
        self.service.retry_at = None;
        self.service.pressure = false;
        if let Some(wake) = &self.service.wake {
            wake.cancel_deadline();
        }
    }
    fn failed(&mut self, ctx: &mut ProcessCtx, error: String) {
        self.stop_runtime();
        if self.service.last_error.as_ref() != Some(&error) {
            golden_core::logerror!(origin=self.id();format!("Ultraleap runtime stopped: {error}"));
            self.service.last_error = Some(error.clone());
        }
        self.set_runtime_warning(ctx, &error);
        self.reset_all_values(ctx);
        self.clear_pressure_warning(ctx);
        self.service.retry_at = Some(self.service.backoff.schedule(Instant::now()));
    }
    fn clear_pressure_warning(&self, ctx: &mut ProcessCtx) {
        NodeHandle::new(self.id()).clear_warning(ctx, Some("ultraleap_pressure"));
    }
    fn start_runtime(&self) -> Result<UltraleapRuntime, String> {
        let notifier = self
            .service
            .wake
            .as_ref()
            .ok_or("Ultraleap wake is unavailable")?
            .notifier
            .clone();
        #[cfg(test)]
        if let Some(factory) = &self.service.factory {
            return factory(&self.service.owner, notifier);
        }
        UltraleapRuntime::create(&self.service.owner, notifier)
    }
    pub(super) fn service_runtime(&mut self, ctx: &mut ProcessCtx) {
        if ctx.tree_snapshot_arc().is_none() {
            return;
        }
        if let Err(error) = self.register_service(ctx) {
            self.set_runtime_warning(ctx, &error);
            return;
        }
        if !self.node_data().effective_enabled {
            self.stop_runtime();
            return;
        }
        if self.service.runtime.is_none()
            && !self.service.suppressed
            && !self.service.retry_at.is_some_and(|at| Instant::now() < at)
        {
            match self.start_runtime() {
                Ok(runtime) => {
                    self.service.runtime = Some(runtime);
                    self.service.retry_at = None;
                }
                Err(error) => self.failed(ctx, error),
            }
        }
        if let Some(runtime) = self.service.runtime.as_mut() {
            let pressure = runtime.pressure();
            let result = runtime.poll();
            if pressure != self.service.pressure {
                self.service.pressure = pressure;
                if pressure {
                    NodeHandle::new(self.id()).set_warning_with(
                        ctx,
                        Some("ultraleap_pressure"),
                        "Ultraleap publication capacity is full; forwarding is paused",
                        None,
                    );
                } else {
                    self.clear_pressure_warning(ctx);
                }
            }
            match result {
                Ok(rows) => {
                    if !rows.is_empty() && self.guard_projection(ctx) {
                        for row in rows {
                            if !row.current() {
                                continue;
                            }
                            ctx.edits.push(golden_core::edit::Edit::CheckPreconditions {
                                preconditions: golden_core::edit::EditPreconditions::new(vec![], row.guards.to_vec())
                                    .unwrap(),
                            });
                            self.clear_runtime_warning(ctx);
                            self.service.last_error = None;
                            self.service.backoff.reset();
                            let prepared = &row.prepared;
                            let poll = &prepared.poll;
                            self.refresh_connection_state(ctx, poll.service_connected, poll.connected_devices);
                            if let Some(message) = &poll.last_event {
                                self.set_last_event(ctx, message.clone());
                            }
                            if !poll.service_connected || poll.connected_devices == 0 {
                                self.reset_tracking_values(ctx);
                            } else if let Some(frame) = poll.frame {
                                self.apply_frame(ctx, frame, prepared.distance);
                            }
                            ctx.edits.retain_payload(row);
                        }
                    } else if !rows.is_empty() {
                        self.set_runtime_warning(ctx, "Ultraleap publication cancelled; target hierarchy changed");
                    }
                }
                Err(error) => self.failed(ctx, error),
            }
        }
        if self
            .service
            .runtime
            .as_ref()
            .is_some_and(|runtime| runtime.has_pending() || !runtime.is_running())
        {
            self.request_service();
        }
        if let Some(wake) = &self.service.wake {
            if let Some(at) = self.service.retry_at {
                wake.later(at.saturating_duration_since(Instant::now()));
            } else {
                wake.cancel_deadline();
            }
        }
    }
}
#[cfg(test)]
pub(super) mod tests;
