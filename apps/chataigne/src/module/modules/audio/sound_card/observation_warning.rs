use golden_core::{
    edit::Edit,
    node::{NodeId, NodeWarning},
    process_ctx::ProcessCtx,
};

use super::runtime::observation::ObservationCaptureError;
use crate::app::module::common::edit_receipt::EditReceipt;

const WARNING_ID: &str = "sound-card-observation";

#[derive(Default)]
pub(super) struct ObservationWarning {
    published: Option<Option<ObservationCaptureError>>,
    pending: Option<(Option<ObservationCaptureError>, EditReceipt)>,
}

impl ObservationWarning {
    pub(super) fn has_pending(&self) -> bool {
        self.pending.is_some()
    }

    /// Configuration/control admission is independent of metering pressure.
    /// Retain the receipt even after the native facade is retired.
    pub(super) fn service(&mut self, ctx: &mut ProcessCtx, node: NodeId, desired: Option<ObservationCaptureError>) {
        if let Some((value, mut receipt)) = self.pending.take() {
            if !receipt.service(ctx, || warning_edit(node, value)) {
                self.pending = Some((value, receipt));
                return;
            }
            self.published = Some(value);
        }
        if self.published != Some(desired) {
            let mut receipt = EditReceipt::default();
            receipt.service(ctx, || warning_edit(node, desired));
            self.pending = Some((desired, receipt));
        }
    }
}

fn warning_edit(node: NodeId, value: Option<ObservationCaptureError>) -> Edit {
    match value {
        Some(error) => Edit::SetNodeWarning {
            node,
            warning: NodeWarning {
                id: WARNING_ID.into(),
                message: error.message().into(),
                detail: None,
            },
        },
        None => Edit::ClearNodeWarning {
            node,
            warning_id: Some(WARNING_ID.into()),
        },
    }
}

#[cfg(test)]
mod tests;
