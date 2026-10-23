use super::*;
use crate::app::module::common::worker_wake::ModuleWake;
use golden_io::{ReconnectBackoff, RetirementPool};
use std::{
    num::NonZeroUsize,
    time::{Duration, Instant},
};
pub(super) struct Service {
    pub(super) runtime: Option<Kinect2Runtime>,
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
type Factory = Box<
    dyn Fn(&RetirementPool, golden_io::WorkerNotifier, KinectReferenceSpace) -> Result<Kinect2Runtime, String>
        + Send
        + Sync,
>;
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
impl Kinect2Module {
    pub(super) fn register_service(&mut self, ctx: &ProcessCtx) -> Result<(), String> {
        let waker = match ctx.node_waker() {
            Ok(w) => w,
            Err(golden_core::engine::NodeWakeError::Unavailable) => return Ok(()),
            Err(e) => return Err(e.to_string()),
        };
        if !self.service.wake.as_ref().is_some_and(|w| w.matches(&waker)) {
            self.stop_runtime();
            self.service.wake = Some(ModuleWake::new(waker)?);
        }
        Ok(())
    }
    pub(super) fn request_service(&self) {
        if let Some(w) = &self.service.wake {
            w.notifier.notify();
        }
    }
    pub(super) fn stop_runtime(&mut self) {
        self.service.runtime = None;
        self.service.retry_at = None;
        self.service.pressure = false;
        if let Some(w) = &self.service.wake {
            w.cancel_deadline();
        }
    }
    pub(super) fn configure_runtime(&mut self) {
        let space = self.space();
        if let Some(runtime) = self.service.runtime.as_mut() {
            runtime.configure(space);
        }
    }
    fn failed(&mut self, ctx: &mut ProcessCtx, error: String) {
        self.stop_runtime();
        if self.service.last_error.as_ref() != Some(&error) {
            golden_core::logerror!(origin=self.id();format!("Kinect runtime stopped: {error}"));
            self.service.last_error = Some(error.clone());
        }
        if self.guard_projection(ctx) {
            self.set_runtime_warning(ctx, &error);
            NodeHandle::new(self.id()).clear_warning(ctx, Some("kinect2_pressure"));
            self.reset_all_values(ctx);
        }
        self.service.retry_at = Some(self.service.backoff.schedule(Instant::now()));
    }
    fn start_runtime(&self) -> Result<Kinect2Runtime, String> {
        let notifier = self
            .service
            .wake
            .as_ref()
            .ok_or("Kinect wake unavailable")?
            .notifier
            .clone();
        #[cfg(test)]
        if let Some(factory) = &self.service.factory {
            return factory(&self.service.owner, notifier, self.space());
        }
        Kinect2Runtime::create(&self.service.owner, notifier, self.space())
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
        self.configure_runtime();
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
            match result {
                Ok(rows) => {
                    let mut ready = Vec::with_capacity(rows.len());
                    let mut rejected = false;
                    for row in rows {
                        if row.current() && self.guard_input(ctx, &row.prepared) {
                            ready.push(row);
                        } else {
                            rejected = true;
                        }
                    }
                    if rejected {
                        self.set_runtime_warning(
                            ctx,
                            "Kinect publication cancelled; source, settings or target changed",
                        );
                    }
                    for row in ready {
                        ctx.edits.push(golden_core::edit::Edit::CheckPreconditions {
                            preconditions: golden_core::edit::EditPreconditions::new(vec![], row.guards.to_vec())
                                .unwrap(),
                        });
                        self.clear_runtime_warning(ctx);
                        self.service.last_error = None;
                        self.service.backoff.reset();
                        self.project(ctx, &row.prepared);
                        ctx.edits.retain_payload(row);
                    }
                    if pressure != self.service.pressure {
                        self.service.pressure = pressure;
                        if pressure {
                            NodeHandle::new(self.id()).set_warning_with(
                                ctx,
                                Some("kinect2_pressure"),
                                "Kinect publication capacity is full; forwarding is paused",
                                None,
                            );
                        } else {
                            NodeHandle::new(self.id()).clear_warning(ctx, Some("kinect2_pressure"));
                        }
                    }
                }
                Err(error) => self.failed(ctx, error),
            }
        }
        if self
            .service
            .runtime
            .as_ref()
            .is_some_and(|r| r.has_pending() || !r.is_running())
        {
            self.request_service();
        }
        if let Some(w) = &self.service.wake {
            if let Some(at) = self.service.retry_at {
                w.later(at.saturating_duration_since(Instant::now()));
            } else {
                w.cancel_deadline();
            }
        }
    }
}
#[cfg(test)]
pub(super) mod tests;
