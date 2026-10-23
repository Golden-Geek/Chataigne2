use super::{osc_discovery::DiscoveryEvent, NETWORK_INTERFACE_WARNING_ID};
use golden_core::{
    edit::{Edit, EditCompletion, EditCompletionStatus, EditQueue},
    node::{NodeId, NodeWarning},
    parameter::ParamValue,
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

pub(super) struct CatalogDelivery {
    pub event: DiscoveryEvent,
    receipt: Option<EditCompletion>,
}
impl CatalogDelivery {
    pub(super) fn new(event: DiscoveryEvent) -> Self {
        Self { event, receipt: None }
    }
    pub(super) fn requires_snapshot(&self) -> bool {
        self.event.result.is_ok()
            && self
                .receipt
                .as_ref()
                .is_none_or(|receipt| receipt.status() == EditCompletionStatus::Failed)
    }
    pub(super) fn service(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: Option<&ProcessTreeSnapshot>,
        target: Option<NodeId>,
    ) -> bool {
        match self.receipt.as_ref().map(EditCompletion::status) {
            Some(EditCompletionStatus::Applied) => return true,
            Some(EditCompletionStatus::Pending) => return false,
            None | Some(EditCompletionStatus::Failed) => {}
        }
        let Some(target) = target else { return true };
        let mut edits = EditQueue::new();
        match &self.event.result {
            Ok(catalog) => {
                let Some(snapshot) = snapshot else { return false };
                let Some(node) = snapshot.node(target) else { return true };
                let expected = ParamValue::Enum(catalog.selection.variant());
                if node.param_value.as_ref() != Some(&expected) {
                    return true;
                }
                edits.push(Edit::SetParamEnumOptions {
                    node: target,
                    expected_uuid: node.uuid,
                    expected_value: expected,
                    selected_value: None,
                    options: catalog.options.clone(),
                });
                edits.push(Edit::ClearNodeWarning {
                    node: target,
                    warning_id: Some(NETWORK_INTERFACE_WARNING_ID.into()),
                });
            }
            Err(error) => edits.push(Edit::SetNodeWarning {
                node: target,
                warning: NodeWarning::new(error.clone()).with_id(NETWORK_INTERFACE_WARNING_ID),
            }),
        }
        self.receipt = Some(edits.completion());
        ctx.edits.pending.extend(edits.drain());
        false
    }
}
