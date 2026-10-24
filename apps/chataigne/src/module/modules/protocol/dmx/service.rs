use super::*;
use crate::app::module::common::worker_wake::ModuleWake;

impl DmxModuleBase {
    pub(super) fn register_service(&mut self, ctx: &ProcessCtx) -> Result<(), String> {
        let source = ctx.node_waker().map_err(|error| error.to_string())?;
        if !self.wake.as_ref().is_some_and(|wake| wake.matches(&source)) {
            self.clear_output();
            self.stop_transport();
            self.continuity.restart_from_authored();
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
    pub(super) fn service_needs_snapshot(&self) -> bool {
        self.node_data().effective_enabled
            && (self.transport_dirty
                || self.retry_due()
                || self.output_needs_snapshot()
                || self.initial_output_needs_snapshot()
                || self.transport.as_ref().is_some_and(DmxTransportHandle::has_frame))
    }
    pub(super) fn service_io(&mut self, ctx: &mut ProcessCtx) {
        self.drain_transport(ctx);
        if !self.node_data().effective_enabled {
            return;
        }
        if self.retry_due() {
            self.retry_at = None;
            self.transport_dirty = true;
        }
        if self.transport_dirty {
            if let Some(snapshot) = ctx.tree_snapshot_arc() {
                self.refresh_transport(ctx, &snapshot);
            } else {
                self.request_service();
            }
        }
        self.service_initial_output(ctx);
        self.service_output(ctx);
        if self.transport.as_ref().is_some_and(DmxTransportHandle::has_pending)
            || self
                .pending_sends
                .iter()
                .any(|receipt| receipt.status() != SendStatus::Pending)
        {
            self.request_service();
        }
        // The shared notifier retains only its earliest wake. If that wake belonged
        // to retired work, re-arm the still-owned later absolute deadline.
        if let Some(at) = self.retry_at.into_iter().chain(self.output_retry_at()).min() {
            if let Some(wake) = &self.wake {
                wake.later(at.saturating_duration_since(Instant::now()));
            }
        }
    }
}
