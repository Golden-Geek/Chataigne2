use std::collections::VecDeque;

use golden_audio::AudioEvent;
use golden_core::{
    edit::Edit,
    node::{Node, NodeWarning},
    process_ctx::ProcessCtx,
};

use super::{
    connection_feedback::{self, ConnectionWarning},
    native_feedback::{LogDelivery, LogProgress, LogSelection, SelectionDelivery, SelectionProgress},
    prepared_event::PreparedAudioEvent,
    SoundCardModule,
};
use crate::app::module::{common::edit_receipt::EditReceipt, script_api::CallbackDelivery};

#[derive(Default)]
pub(super) struct PendingAudioEvents(VecDeque<AudioEventDelivery>);

struct AudioEventDelivery {
    prepared: PreparedAudioEvent,
    callback: Option<CallbackDelivery>,
    connection_warning: EditReceipt,
    rejection_warning: EditReceipt,
    logging: LogDelivery,
    logging_warning: EditReceipt,
    selection_warning: EditReceipt,
    external_wait: bool,
    phase: DeliveryPhase,
    selection: SelectionDelivery,
}

enum DeliveryPhase {
    Feedback,
    Callback,
}

impl PendingAudioEvents {
    pub(super) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub(super) fn push(&mut self, event: PreparedAudioEvent, module: golden_core::node::NodeId) {
        debug_assert!(self.0.len() < super::runtime::EVENT_SERVICE_ITEMS);
        self.0.push_back(AudioEventDelivery {
            callback: event.callback(module).map(CallbackDelivery::new),
            prepared: event,
            connection_warning: EditReceipt::default(),
            rejection_warning: EditReceipt::default(),
            logging: LogDelivery::default(),
            logging_warning: EditReceipt::default(),
            selection_warning: EditReceipt::default(),
            external_wait: false,
            phase: DeliveryPhase::Feedback,
            selection: SelectionDelivery::default(),
        });
    }

    pub(super) fn service(&mut self, ctx: &mut ProcessCtx, module: &SoundCardModule) {
        while let Some(front) = self.0.front_mut() {
            if !front.service(ctx, module) {
                break;
            }
            self.0.pop_front();
        }
        if self.0.front().is_some_and(|front| !front.external_wait) {
            if let Some(wake) = &module.runtime_wake {
                wake.wake();
            }
        }
    }
}

impl AudioEventDelivery {
    fn service(&mut self, ctx: &mut ProcessCtx, module: &SoundCardModule) -> bool {
        self.external_wait = false;
        if matches!(self.phase, DeliveryPhase::Feedback) {
            let matches_selection = match self.selection.service(&self.prepared, module) {
                SelectionProgress::Waiting => {
                    self.external_wait = true;
                    return false;
                }
                SelectionProgress::Ready(matches) => matches,
                SelectionProgress::Failed => {
                    if !self.selection_warning.service(ctx, || Edit::SetNodeWarning {
                        node: module.id(),
                        warning: NodeWarning {
                            id: "sound-card-feedback-selection".into(),
                            message: "An audio status selection could not be inspected".into(),
                            detail: Some(
                                "Connection feedback was suppressed; the original status callback is preserved.".into(),
                            ),
                        },
                    }) {
                        return false;
                    }
                    false
                }
            };
            match self.logging.service(
                &self.prepared,
                LogSelection {
                    outgoing: module.base.log_outgoing_enabled(),
                    connection: matches_selection,
                },
                module.id(),
                module.runtime_wake.as_ref(),
            ) {
                LogProgress::Waiting => {
                    self.external_wait = true;
                    return false;
                }
                LogProgress::Failed => {
                    if !self.logging_warning.service(ctx, || Edit::SetNodeWarning {
                        node: module.id(),
                        warning: NodeWarning {
                            id: "sound-card-event-logging".into(),
                            message: "Sound Card event logging failed".into(),
                            detail: Some(
                                "The native logger did not confirm every message. Accepted logs are not replayed."
                                    .into(),
                            ),
                        },
                    }) {
                        return false;
                    }
                }
                LogProgress::Applied => {}
            }
            if let Some(error) = &self.prepared.preparation_error {
                if !self.rejection_warning.service(ctx, || Edit::SetNodeWarning {
                    node: module.id(),
                    warning: NodeWarning {
                        id: "sound-card-event-preparation".into(),
                        message: error.clone(),
                        detail: None,
                    },
                }) {
                    return false;
                }
            }
            if !self.service_connection_warning(ctx, module, matches_selection) {
                return false;
            }
            self.phase = DeliveryPhase::Callback;
        }
        self.callback.as_mut().is_none_or(|callback| callback.service(ctx))
    }

    fn service_connection_warning(
        &mut self,
        ctx: &mut ProcessCtx,
        module: &SoundCardModule,
        matches_selection: bool,
    ) -> bool {
        // A failed warning must not be retried for a device the user has since
        // deselected. The original callback occurrence remains deliverable.
        if !matches_selection {
            return true;
        }
        let AudioEvent::DeviceStatusChanged(status) = &self.prepared.event else {
            return true;
        };
        let Some(connection) = &self.prepared.connection else {
            return true;
        };
        let Some(node) = module.device_parameter(status.direction) else {
            return true;
        };
        let id = connection_feedback::warning_id(status.direction);
        match &connection.warning {
            ConnectionWarning::Keep => true,
            ConnectionWarning::Clear => self.connection_warning.service(ctx, || Edit::ClearNodeWarning {
                node,
                warning_id: Some(id.into()),
            }),
            ConnectionWarning::Set { message, detail } => {
                self.connection_warning.service(ctx, || Edit::SetNodeWarning {
                    node,
                    warning: NodeWarning {
                        id: id.into(),
                        message: message.clone(),
                        detail: Some(detail.clone()),
                    },
                })
            }
        }
    }
}

#[cfg(test)]
mod tests;
