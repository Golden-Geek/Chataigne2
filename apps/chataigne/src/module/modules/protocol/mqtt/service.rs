use super::*;
use crate::app::module::common::worker_wake::ModuleWake;
use std::time::Instant;

impl MqttModule {
    pub(super) fn register_service(&mut self, ctx: &ProcessCtx) -> Result<(), String> {
        let source = ctx.node_waker().map_err(|error| error.to_string())?;
        if !self.wake.as_ref().is_some_and(|wake| wake.matches(&source)) {
            self.configuration = None;
            self.configuration_key = None;
            self.stop_transport();
            self.wake = Some(ModuleWake::new(source)?);
            self.transport_dirty = true;
        }
        Ok(())
    }
    pub(super) fn request_service(&self) {
        if let Some(wake) = &self.wake {
            wake.notifier.notify();
        }
    }
    pub(super) fn schedule_retry(&mut self) {
        let now = Instant::now();
        let at = self.retry_backoff.schedule(now);
        self.retry_at = Some(at);
        if let Some(wake) = &self.wake {
            wake.later(at.saturating_duration_since(now));
        }
    }
    pub(super) fn service_needs_snapshot(&self) -> bool {
        self.node_data().effective_enabled
            && (self.configuration_needs_snapshot()
                || self.incoming.requires_snapshot()
                || self.retry_at.is_some_and(|at| Instant::now() >= at))
    }
    pub(super) fn service_io(&mut self, ctx: &mut ProcessCtx) {
        if self.retry_at.is_some_and(|at| Instant::now() >= at) {
            self.retry_at = None;
            self.transport_dirty = true;
        }
        self.drain_transport_events(ctx);
        if !self.node_data().effective_enabled {
            return;
        }
        let snapshot = ctx.tree_snapshot_arc();
        if self.configuration_needs_snapshot() {
            let Some(snapshot) = &snapshot else {
                self.request_service();
                return;
            };
            self.refresh_data_capabilities(ctx, snapshot);
            if self.transport_dirty {
                self.refresh_transport(ctx, snapshot);
            }
        }
        if let Some(wake) = &self.wake {
            let (id, log, notifier) = (self.id(), self.base.log_incoming_enabled(), wake.notifier.clone());
            self.incoming.service(ctx, snapshot.as_deref(), id, log, notifier);
        }
        if let Some(error) = self.incoming.feedback.take() {
            self.incoming.clear_feedback = false;
            NodeHandle::new(self.id()).set_warning_with(ctx, Some("mqtt_incoming_pressure"), &error, None);
            logerror!(error);
        } else if std::mem::take(&mut self.incoming.clear_feedback) {
            NodeHandle::new(self.id()).clear_warning(ctx, Some("mqtt_incoming_pressure"));
        }
        if self.incoming.needs_service()
            || self.incoming.can_receive() && self.transport.as_ref().is_some_and(MqttTransportHandle::has_pending)
            || self
                .pending_publishes
                .iter()
                .any(|(receipt, _)| receipt.status() != PublishStatus::Pending)
        {
            self.request_service();
        }
        let retry = self.retry_at.into_iter().chain(self.incoming.retry_at()).min();
        if let (Some(wake), Some(at)) = (&self.wake, retry) {
            wake.later(at.saturating_duration_since(Instant::now()));
        }
    }
}
