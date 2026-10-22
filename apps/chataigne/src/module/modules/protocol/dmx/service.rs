use super::*;
use crate::app::module::common::worker_wake::ModuleWake;

impl DmxModuleBase {
    pub(super) fn register_service(&mut self, ctx: &ProcessCtx) -> Result<(), String> {
        let source = ctx.node_waker().map_err(|error| error.to_string())?;
        if !self.wake.as_ref().is_some_and(|wake| wake.matches(&source)) {
            self.clear_output();
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
    pub(super) fn service_needs_snapshot(&self) -> bool {
        self.node_data().effective_enabled
            && (self.transport_dirty
                || self.retry_due()
                || self.output_needs_snapshot()
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
        self.service_output(ctx);
        if self.transport.as_ref().is_some_and(DmxTransportHandle::has_pending)
            || self
                .pending_sends
                .iter()
                .any(|receipt| receipt.status() != SendStatus::Pending)
        {
            self.request_service();
        }
    }
}
