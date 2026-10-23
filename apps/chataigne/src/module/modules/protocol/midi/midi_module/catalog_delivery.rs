use super::*;
use golden_core::edit::{EditCompletion, EditCompletionStatus, EditPreconditions, EditQueue, NodeReadGuard};
use midi_runtime::{CatalogEvent, PortCatalog};

#[cfg(test)]
mod tests;

pub(super) struct CatalogDelivery {
    pub event: CatalogEvent,
    receipt: Option<EditCompletion>,
}
impl CatalogDelivery {
    pub fn new(event: CatalogEvent) -> Self {
        Self { event, receipt: None }
    }
    pub fn requires_snapshot(&self) -> bool {
        self.event.result.is_ok()
            && self
                .receipt
                .as_ref()
                .is_none_or(|r| r.status() == EditCompletionStatus::Failed)
    }
    pub fn service(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: Option<&ProcessTreeSnapshot>,
        input: Option<NodeId>,
        output: Option<NodeId>,
    ) -> bool {
        match self.receipt.as_ref().map(EditCompletion::status) {
            Some(EditCompletionStatus::Applied) => return true,
            Some(EditCompletionStatus::Pending) => return false,
            None | Some(EditCompletionStatus::Failed) => {}
        }
        let mut edits = EditQueue::new();
        match &self.event.result {
            Ok(catalog) => {
                let Some(snapshot) = snapshot else { return false };
                for (id, port) in [(input, &catalog.input), (output, &catalog.output)] {
                    if let Some(id) = id {
                        publish(&mut edits, snapshot, id, port);
                    }
                }
            }
            Err(error) => {
                for id in [input, output].into_iter().flatten() {
                    edits.push(Edit::SetNodeWarning {
                        node: id,
                        warning: golden_core::node::NodeWarning::new(error.clone())
                            .with_id(MIDI_PORT_OPTIONS_WARNING_ID),
                    });
                }
            }
        }
        if edits.pending.is_empty() {
            return true;
        }
        self.receipt = Some(edits.completion());
        ctx.edits.pending.extend(edits.drain());
        false
    }
}
fn publish(edits: &mut EditQueue, snapshot: &ProcessTreeSnapshot, id: NodeId, catalog: &PortCatalog) {
    let Some(node) = snapshot.node(id) else { return };
    let expected = ParamValue::Enum(catalog.selection.clone());
    if node.param_value.as_ref() != Some(&expected) {
        return;
    }
    // Fence the warning clear as well as the choice-only update.
    edits.push(Edit::CheckPreconditions {
        preconditions: EditPreconditions::new(
            vec![NodeReadGuard::capture(snapshot, id).expect("catalog target exists")],
            vec![],
        )
        .expect("one catalog target"),
    });
    edits.push(Edit::SetParamEnumOptions {
        node: id,
        expected_uuid: node.uuid,
        expected_value: expected,
        selected_value: None,
        options: catalog.options.clone(),
    });
    edits.push(Edit::ClearNodeWarning {
        node: id,
        warning_id: Some(MIDI_PORT_OPTIONS_WARNING_ID.into()),
    });
}
