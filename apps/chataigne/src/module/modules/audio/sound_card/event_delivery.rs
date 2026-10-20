use std::collections::VecDeque;

use golden_audio::AudioEvent;
use golden_core::{
    edit::Edit,
    node::{Node, NodeWarning},
    process_ctx::ProcessCtx,
};

use super::{
    connection_feedback::{self, ConnectionLogLevel, ConnectionWarning},
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
    logged: bool,
    phase: DeliveryPhase,
    selection: Option<SelectionFence>,
    #[cfg(test)]
    selection_checks: usize,
}

enum DeliveryPhase {
    Feedback,
    Callback,
}

struct SelectionFence {
    parameter: golden_core::node::NodeId,
    driver: String,
    value: String,
    matches: bool,
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
            logged: false,
            phase: DeliveryPhase::Feedback,
            selection: None,
            #[cfg(test)]
            selection_checks: 0,
        });
    }

    pub(super) fn service(&mut self, ctx: &mut ProcessCtx, module: &SoundCardModule) {
        while let Some(front) = self.0.front_mut() {
            if !front.service(ctx, module) {
                break;
            }
            self.0.pop_front();
        }
        if !self.0.is_empty() {
            if let Some(wake) = &module.runtime_wake {
                wake.wake();
            }
        }
    }
}

impl AudioEventDelivery {
    fn service(&mut self, ctx: &mut ProcessCtx, module: &SoundCardModule) -> bool {
        if matches!(self.phase, DeliveryPhase::Feedback) {
            let matches_selection = self.matches_selection(module);
            if !self.logged {
                if module.base.log_outgoing_enabled() {
                    if let Some(message) = &self.prepared.outgoing_log {
                        golden_core::log!(origin = module.id(); message.as_str());
                    }
                }
                if let Some(error) = &self.prepared.preparation_error {
                    golden_core::logerror!(origin = module.id(); error.as_str());
                }
                if let Some(connection) = &self.prepared.connection {
                    if matches_selection {
                        if let Some(log) = &connection.log {
                            match log.level {
                                ConnectionLogLevel::Info => {
                                    golden_core::log!(origin = module.id(); log.message.as_str());
                                }
                                ConnectionLogLevel::Success => {
                                    golden_core::logsuccess!(origin = module.id(); log.message.as_str());
                                }
                                ConnectionLogLevel::Error => {
                                    golden_core::logerror!(origin = module.id(); log.message.as_str());
                                }
                            }
                        }
                    }
                }
                self.logged = true;
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

    fn matches_selection(&mut self, module: &SoundCardModule) -> bool {
        let AudioEvent::DeviceStatusChanged(status) = &self.prepared.event else {
            return false;
        };
        let Some(parameter) = module.device_parameter(status.direction) else {
            return false;
        };
        let driver_value = module.audio_driver.get_ref().0.as_str();
        let (input, output) = module.selected_device_values();
        let selected = match status.direction {
            golden_audio::AudioDirection::Input => input,
            golden_audio::AudioDirection::Output => output,
        };
        if let Some(fence) = &self.selection {
            if fence.parameter == parameter && fence.driver == driver_value && fence.value == selected {
                return fence.matches;
            }
        }
        // Receipt waits must not repeat selection parsing. Compare the actual
        // cached parameter values rather than an inbox-driven revision, since a
        // wake can observe a new value before its change event is dispatched.
        #[cfg(test)]
        {
            self.selection_checks += 1;
        }
        let driver = golden_audio::BackendId::new(driver_value).ok();
        let matches = connection_feedback::status_matches_selection(status, selected, driver.as_ref());
        self.selection = Some(SelectionFence {
            parameter,
            driver: driver_value.to_owned(),
            value: selected.to_owned(),
            matches,
        });
        matches
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
