use golden_core::{node::NodeId, process_ctx::ProcessCtx};

use super::{capture::ObservationView, telemetry::PreparedTelemetry};
use crate::app::module::common::edit_receipt::EditReceipt;

#[derive(Default)]
pub(in super::super) struct TelemetryDelivery {
    published: Option<PreparedTelemetry>,
    pending: Option<(PreparedTelemetry, EditReceipt)>,
}

impl TelemetryDelivery {
    pub(in super::super) fn has_pending(&self) -> bool {
        self.pending.is_some()
    }

    pub(in super::super) fn service(&mut self, ctx: &mut ProcessCtx, module: NodeId, view: &ObservationView) {
        if let Some((prepared, mut receipt)) = self.pending.take() {
            if !receipt.service(ctx, || golden_core::edit::Edit::EmitCustomEvent {
                event: prepared.event(module),
            }) {
                self.pending = Some((prepared, receipt));
                return;
            }
            self.published = Some(prepared);
        }
        let Some(latest) = view.telemetry.as_ref().filter(|_| view.error.is_none()) else {
            return;
        };
        if self
            .published
            .as_ref()
            .is_some_and(|published| published.same_payload(latest))
        {
            return;
        }
        let mut receipt = EditReceipt::default();
        receipt.service(ctx, || golden_core::edit::Edit::EmitCustomEvent {
            event: latest.event(module),
        });
        self.pending = Some((latest.clone(), receipt));
    }
}

#[cfg(test)]
mod tests;
