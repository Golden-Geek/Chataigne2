use super::*;
use crate::app::module::common::worker_wake::ModuleWake;
use std::time::Instant;

impl MqttModule {
    pub(super) fn register_service(&mut self, ctx: &ProcessCtx) -> Result<(), String> {
        let source = ctx.node_waker().map_err(|error| error.to_string())?;
        if !self.wake.as_ref().is_some_and(|wake| wake.matches(&source)) {
            self.stop_transport();
            self.wake = Some(ModuleWake::new(source)?);
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
            && (self.transport_dirty
                || !self.pending_incoming_messages.is_empty()
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
        if self.transport_dirty || !self.pending_incoming_messages.is_empty() {
            let Some(snapshot) = ctx.tree_snapshot_arc() else {
                self.request_service();
                return;
            };
            self.refresh_data_capabilities(ctx, &snapshot);
            if self.transport_dirty {
                self.refresh_transport(ctx, &snapshot);
            }
            self.process_pending_incoming(ctx, &snapshot);
        }
        if !self.pending_incoming_messages.is_empty()
            || self.transport.as_ref().is_some_and(MqttTransportHandle::has_pending)
            || self
                .pending_publishes
                .iter()
                .any(|(receipt, _)| receipt.status() != PublishStatus::Pending)
        {
            self.request_service();
        }
        if let (Some(wake), Some(at)) = (&self.wake, self.retry_at) {
            wake.later(at.saturating_duration_since(Instant::now()));
        }
    }
}
