use chataigne_sound_card_protocol::{SoundCardUiControlRequest, SOUND_CARD_UI_CONTROL_TOPIC};
use golden_audio::{AudioCommand, AudioError, AudioErrorCategory, CommandSequence, PlayFileRequest};
use golden_core::node::{Node, NodeReference, NodeUuid};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::*;
use crate::app::module_modules_audio_sound_card_commands::SoundCardCommandRequest;

mod command_admission;
mod lifecycle;

use command_admission::*;
pub(super) use command_admission::{authored_gain_change as command_gain_change, command_error};
#[cfg(test)]
use runtime::DeviceChoice;
#[cfg(test)]
pub(super) fn device_options_for_current(
    current: &str,
    state: &golden_audio::AudioDeviceInspectorState,
    direction: golden_audio::AudioDirection,
    driver: Option<&golden_audio::BackendId>,
) -> Vec<golden_core::parameter::ParameterEnumOption> {
    let choice = match direction {
        golden_audio::AudioDirection::Input => DeviceChoice::Input,
        golden_audio::AudioDirection::Output => DeviceChoice::Output,
    };
    runtime::native_device_options(current, state, choice, driver).expect("fixture choices fit native admission")
}
#[cfg(test)]
pub(super) fn duplex_device_options_for_current(
    current: &str,
    state: &golden_audio::AudioDeviceInspectorState,
    driver: Option<&golden_audio::BackendId>,
) -> Vec<golden_core::parameter::ParameterEnumOption> {
    runtime::native_device_options(current, state, DeviceChoice::Duplex, driver)
        .expect("fixture choices fit native admission")
}

pub(crate) const SOUND_CARD_COMMAND_RESULT_TOPIC: &str = "chataigne.sound_card.command.result";

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub(crate) struct SoundCardCommandResultEvent {
    pub module_id: NodeId,
    pub command_id: NodeId,
    pub command_type: String,
    pub admitted_sequence: Option<u64>,
    pub error: Option<AudioError>,
}

impl SoundCardModule {
    pub(super) fn handle_sound_card_ui_control_event(
        &mut self,
        ctx: &mut ProcessCtx,
        event: &golden_core::events::CustomEvent,
    ) -> bool {
        if event.topic != SOUND_CARD_UI_CONTROL_TOPIC {
            return false;
        }
        let result = crate::app::module::common::json::retained_weight(&event.payload, 64 * 1024, 256, 8)
            .map_err(command_error)
            .and_then(|_| {
                event
                    .payload_as::<SoundCardUiControlRequest>()
                    .map_err(|_| command_error("invalid Sound Card UI control payload"))
            })
            .and_then(|request| {
                let snapshot = ctx
                    .tree_snapshot_arc()
                    .ok_or_else(|| command_error("Sound Card UI control requires a tree snapshot"))?;
                match request {
                    SoundCardUiControlRequest::StopFile { playback_id } => self
                        .admit_request(
                            snapshot.as_ref(),
                            SoundCardCommandRequest::StopFile {
                                playback_id: golden_audio::PlaybackId::new(playback_id)
                                    .map_err(|error| command_error(error.to_string()))?,
                            },
                        )
                        .map(|_| ()),
                    SoundCardUiControlRequest::StopAllFiles => self
                        .admit_request(snapshot.as_ref(), SoundCardCommandRequest::StopAllFiles)
                        .map(|_| ()),
                    SoundCardUiControlRequest::ConnectRoute {
                        direction,
                        physical_channel,
                        app_channel_uuid,
                    } => self.enqueue_route_intent(&snapshot, direction, physical_channel, app_channel_uuid, true),
                    SoundCardUiControlRequest::DisconnectRoute {
                        direction,
                        physical_channel,
                        app_channel_uuid,
                    } => self.enqueue_route_intent(&snapshot, direction, physical_channel, app_channel_uuid, false),
                    SoundCardUiControlRequest::RenameChannel {
                        direction,
                        app_channel_uuid,
                        label,
                    } => self.rename_channel(
                        ctx,
                        snapshot.as_ref(),
                        direction,
                        app_channel_uuid.as_str(),
                        label.as_str(),
                    ),
                }
            });
        if let Err(error) = result {
            golden_core::logerror!(origin = self.id(); format!(
                "Sound Card UI control was not admitted: {error}"
            ));
        }
        true
    }

    pub(super) fn rename_channel(
        &self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        direction: golden_audio::AudioDirection,
        channel_uuid: &str,
        label: &str,
    ) -> Result<(), AudioError> {
        let label = label.trim();
        if label.is_empty() {
            return Err(command_error("Sound Card channel name cannot be empty"));
        }
        let channel = self.resolve_routing_channel_node(snapshot, direction, channel_uuid)?;
        structure::patch_channel_label(ctx, channel, label.to_owned());
        Ok(())
    }

    pub(super) fn resolve_routing_channel(
        &self,
        snapshot: &ProcessTreeSnapshot,
        direction: golden_audio::AudioDirection,
        channel_uuid: &str,
    ) -> Result<(NodeId, NodeReference), AudioError> {
        let channel = self.resolve_routing_channel_node(snapshot, direction, channel_uuid)?;
        let state = snapshot.node(channel).expect("validated routing channel");
        let uuid = state.uuid;
        if state.label.len() > 64 * 1024 {
            return Err(command_error("Sound Card channel naming metadata exceeds 64 KiB"));
        }
        let mut reference = NodeReference::with_cached_id(uuid, Some(channel));
        reference.set_cached_name(Some(structure::channel_name(state)));
        Ok((channel, reference))
    }

    pub(super) fn resolve_routing_channel_node(
        &self,
        snapshot: &ProcessTreeSnapshot,
        direction: golden_audio::AudioDirection,
        channel_uuid: &str,
    ) -> Result<NodeId, AudioError> {
        let uuid =
            NodeUuid(Uuid::parse_str(channel_uuid).map_err(|_| command_error("Sound Card channel UUID is invalid"))?);
        let channel = snapshot
            .node_id_by_uuid(uuid)
            .ok_or_else(|| command_error("Sound Card channel is missing"))?;
        let expected_parent_path = match direction {
            golden_audio::AudioDirection::Input => INPUT_CHANNELS_PATH,
            golden_audio::AudioDirection::Output => OUTPUT_CHANNELS_PATH,
        };
        let state = snapshot
            .node(channel)
            .ok_or_else(|| command_error("Sound Card channel disappeared"))?;
        if !matches!(state.param_value, Some(ParamValue::Float(_)))
            || state.parent != find_path(snapshot, self.id(), expected_parent_path)
        {
            return Err(command_error(
                "Sound Card routing target is not a channel gain owned by this module",
            ));
        }
        Ok(channel)
    }

    pub(super) fn validate_physical_channel(
        &self,
        direction: golden_audio::AudioDirection,
        physical_channel: &str,
    ) -> Result<(), AudioError> {
        self.runtime
            .as_ref()
            .ok_or_else(|| command_error("Sound Card runtime is not available"))?
            .validate_physical_channel(direction, physical_channel)
            .map_err(|error| command_error(error.message()))
    }

    fn requested_sample_rate(&self) -> Result<golden_audio::SampleRate, String> {
        let value = &self.sample_rate.get_ref().0;
        let sample_rate = if value == AUTOMATIC_CONFIGURATION {
            return Ok(self
                .automatic_sample_rate
                .unwrap_or_else(|| golden_audio::SampleRate::new(48_000).expect("valid default")));
        } else {
            value
                .parse::<u32>()
                .map_err(|_| "Sound Card sample rate is invalid".to_owned())?
        };
        golden_audio::SampleRate::new(sample_rate).map_err(|_| "Sound Card engine sample rate is invalid".to_owned())
    }

    fn requested_driver(&self) -> Result<Option<golden_audio::BackendId>, String> {
        let value = &self.audio_driver.get_ref().0;
        if value == NO_AUDIO_DRIVER {
            return Ok(None);
        }
        let driver = golden_audio::BackendId::new(value.clone())
            .map_err(|error| format!("Sound Card audio driver is invalid: {error}"))?;
        #[cfg(test)]
        if driver == golden_audio::NullBackend::backend_id() {
            return Ok(Some(driver));
        }
        if golden_audio::compiled_cpal_backend_catalog()
            .iter()
            .any(|backend| backend.id == driver)
        {
            Ok(Some(driver))
        } else {
            Err(format!(
                "Sound Card audio driver `{driver}` is not compiled into this build"
            ))
        }
    }

    pub(super) fn refresh_configuration(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &std::sync::Arc<ProcessTreeSnapshot>,
    ) {
        if self
            .runtime
            .as_ref()
            .is_some_and(runtime::SoundCardRuntime::is_applying_configuration)
        {
            return;
        }
        let module = self.id();
        if self
            .runtime
            .as_mut()
            .is_some_and(|runtime| runtime.has_current_preparation(snapshot, module))
        {
            self.service_configuration_preparation(ctx, snapshot);
            return;
        }
        match self.reset_routes_for_device_selection(ctx, snapshot) {
            Ok(true) => {
                self.configuration_dirty = true;
                return;
            }
            Ok(false) => {}
            Err(error) => {
                self.configuration_dirty = false;
                self.set_runtime_error(ctx, self.id(), &error);
                return;
            }
        }
        let Some(wake) = self.runtime_wake.clone() else { return };
        let prepared = self
            .runtime
            .as_mut()
            .unwrap()
            .prepare_selection(snapshot, module, wake.clone());
        let selection = match prepared {
            Ok(Some(selection)) => selection,
            Ok(None) => return,
            Err(error) => {
                self.handle_preparation_error(ctx, error, &wake);
                return;
            }
        };
        let runtime = self.runtime.as_mut().unwrap();
        let admission = (|| {
            if selection.disable_before_probe {
                runtime.set_enabled(false)?;
            }
            runtime.set_probe_interests(selection.clone())?;
            if selection.pending {
                runtime.set_enabled(false)?;
            }
            Ok::<(), String>(())
        })();
        if let Err(error) = admission {
            runtime.defer_preparation_retry(&wake);
            self.configuration_dirty = false;
            self.set_runtime_error(ctx, self.id(), &error);
            return;
        }
        if selection.pending {
            runtime.schedule_preparation_retry(&wake);
            self.configuration_dirty = false;
            self.clear_runtime_error(ctx);
            return;
        }
        match self.seed_default_routes_from_inventory(
            ctx,
            snapshot,
            &selection.input_physical,
            &selection.output_physical,
        ) {
            Ok(true) => {
                self.configuration_dirty = true;
                return;
            }
            Ok(false) => {}
            Err(error) => {
                self.configuration_dirty = false;
                self.set_runtime_error(ctx, self.id(), &error);
                return;
            }
        }
        self.service_configuration_preparation(ctx, snapshot);
    }

    fn service_configuration_preparation(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &std::sync::Arc<ProcessTreeSnapshot>,
    ) {
        let runtime_sample_rate = self.runtime.as_ref().expect("live configuration runtime").sample_rate();
        let Some(wake) = self.runtime_wake.clone() else {
            return;
        };
        let module = self.id();
        let prepared = self
            .runtime
            .as_mut()
            .unwrap()
            .prepare_configuration(snapshot, module, wake.clone());
        match prepared {
            Ok(None) => {}
            Ok(Some(built)) => {
                if built.sample_rate != runtime_sample_rate {
                    if let Some(runtime) = self.runtime.as_mut() {
                        runtime.cancel_preparation();
                    }
                    if self.sample_rate.get_ref().0 == AUTOMATIC_CONFIGURATION {
                        self.automatic_sample_rate = Some(built.sample_rate);
                    }
                    self.configuration_dirty = true;
                    return;
                }
                let module_enabled = snapshot.node(self.id()).is_some_and(|node| node.enabled);
                let Some(runtime) = self.runtime.as_mut() else {
                    return;
                };
                match runtime.submit(built) {
                    Ok(_) => {
                        let _ = runtime.set_enabled(module_enabled);
                        self.configuration_dirty = false;
                    }
                    Err(error) => {
                        runtime.defer_preparation_retry(&wake);
                        self.configuration_dirty = false;
                        self.set_runtime_error(ctx, self.id(), &error);
                    }
                }
            }
            Err(error) => self.handle_preparation_error(ctx, error, &wake),
        }
    }

    fn handle_preparation_error(
        &mut self,
        ctx: &mut ProcessCtx,
        error: runtime::ConfigurationError,
        wake: &runtime::RuntimeWakeSender,
    ) {
        let endpoint_error = error.node.0 != 0
            && [self.device.id(), self.input_device.id(), self.output_device.id()].contains(&error.node);
        if let Some(runtime) = self.runtime.as_mut() {
            if error.retryable {
                runtime.schedule_preparation_retry(wake);
            } else {
                runtime.cancel_preparation();
                let _ = runtime.set_enabled(false);
                if endpoint_error {
                    runtime.schedule_preparation_retry(wake);
                }
            }
        }
        self.set_runtime_error(ctx, error.node, &error.message);
        self.configuration_dirty = false;
    }

    pub(super) fn poll_runtime(&mut self, ctx: &mut ProcessCtx) {
        let had_pending = !self.pending_events.is_empty();
        if had_pending {
            self.service_pending_audio_events(ctx);
        }
        if self.pending_events.is_empty()
            && self
                .runtime
                .as_ref()
                .is_some_and(runtime::SoundCardRuntime::is_disconnected)
        {
            self.runtime = None;
            self.runtime_request = None;
            if let (Ok(sample_rate), Ok(driver)) = (self.requested_sample_rate(), self.requested_driver()) {
                self.schedule_runtime_retry(sample_rate, driver);
            }
            self.configuration_dirty = true;
            self.set_runtime_error(ctx, self.id(), "Sound Card native runtime stopped unexpectedly");
        }
        let module = self.id();
        let Some(runtime) = self.runtime.as_mut() else {
            self.choices.pause();
            self.service_device_choices(ctx);
            self.service_observation_warning(ctx, None);
            self.base.set_connected(ctx, false);
            self.base
                .set_data_capabilities(ctx, crate::app::module::ModuleDataCapabilities::new(false, false));
            return;
        };
        let (observation, events) = runtime.poll(ctx, module, !had_pending);
        if runtime.take_warning_failure() && !self.configuration_dirty {
            if let Some(wake) = &self.runtime_wake {
                runtime.schedule_preparation_retry(wake);
            }
        }
        if runtime.take_preparation_retry_due() {
            self.configuration_dirty = true;
        }
        let feedback = runtime.take_configuration_feedback();
        match feedback {
            Some(runtime::ConfigurationFeedback::Applied) => self.clear_runtime_error(ctx),
            Some(runtime::ConfigurationFeedback::Rejected(event)) => {
                if let golden_audio::AudioEvent::ConfigurationRejected { error, .. } = &event.event {
                    self.set_runtime_error(ctx, self.id(), &error.message);
                }
            }
            None => {}
        }
        self.service_observation_warning(ctx, observation.error);
        let input_ready = observation.header.input_ready;
        let output_ready = observation.header.output_ready;
        let enabled = self.node_data().effective_enabled;
        self.base.set_connected(ctx, enabled && observation.header.connected());
        self.base.set_data_capabilities(
            ctx,
            crate::app::module::ModuleDataCapabilities::new(enabled && input_ready, enabled && output_ready),
        );
        let inventory_changed = events.iter().any(|event| runtime_event_changes_inventory(&event.event));
        if inventory_changed {
            self.choices.invalidate();
        }
        self.service_device_choices(ctx);
        for event in events {
            if runtime_event_changes_inventory(&event.event) {
                self.configuration_dirty = true;
            }
            self.pending_events.push(event, self.id());
        }
        if !had_pending {
            self.service_pending_audio_events(ctx);
        }
    }

    fn service_pending_audio_events(&mut self, ctx: &mut ProcessCtx) {
        if self.pending_events.is_empty() {
            return;
        }
        let mut pending = std::mem::take(&mut self.pending_events);
        pending.service(ctx, self);
        self.pending_events = pending;
        if self.pending_events.is_empty() {
            if let Some(wake) = &self.runtime_wake {
                // One continuation lets the next batch drain after the last
                // receipt settles, even if the native mailbox is already full.
                wake.wake();
            }
        }
    }

    fn service_observation_warning(
        &mut self,
        ctx: &mut ProcessCtx,
        error: Option<runtime::observation::ObservationCaptureError>,
    ) {
        self.observation_warning.service(ctx, self.id(), error);
        if self.observation_warning.has_pending() {
            if let Some(wake) = &self.runtime_wake {
                wake.wake();
            }
        }
    }

    pub(super) fn admit_request(
        &self,
        snapshot: &ProcessTreeSnapshot,
        request: SoundCardCommandRequest,
    ) -> Result<CommandSequence, AudioError> {
        self.admit_prepared_request(Some(snapshot), request)
    }

    pub(super) fn admit_prepared_request(
        &self,
        snapshot: Option<&ProcessTreeSnapshot>,
        request: SoundCardCommandRequest,
    ) -> Result<CommandSequence, AudioError> {
        if !self.node_data().effective_enabled {
            return Err(command_error("Sound Card module is disabled"));
        }
        let runtime = self
            .runtime
            .as_ref()
            .ok_or_else(|| command_error("Sound Card runtime is not available"))?;
        let command = match request {
            SoundCardCommandRequest::PlayFile {
                path,
                playback_id,
                start_offset,
                force_restart,
            } => AudioCommand::PlayFile(
                PlayFileRequest::new(path, playback_id)
                    .with_start_offset(start_offset)
                    .with_force_restart(force_restart),
            ),
            SoundCardCommandRequest::StopFile { playback_id } => AudioCommand::StopFile { playback_id },
            SoundCardCommandRequest::StopAllFiles => AudioCommand::StopAllFiles,
            SoundCardCommandRequest::SetMasterVolume { gain } => AudioCommand::SetMasterGain { gain },
            SoundCardCommandRequest::SetChannelVolume { output_channel, gain } => AudioCommand::SetChannelGain {
                channel: resolve_output_channel(
                    snapshot.ok_or_else(|| command_error("Sound Card channel command requires a snapshot"))?,
                    self.id(),
                    &output_channel,
                )?,
                gain,
            },
        };
        runtime.admit(command)
    }

    pub(super) fn set_runtime_error(&mut self, ctx: &mut ProcessCtx, node: NodeId, message: &str) {
        if let Some(previous) = self.runtime_error_node {
            if previous != node {
                ctx.clear_node_warning(previous, Some("sound-card-runtime"));
            }
        }
        ctx.set_node_warning_with(
            node,
            Some("sound-card-runtime"),
            "Sound Card runtime configuration was not applied",
            Some(message),
        );
        self.runtime_error_node = Some(node);
    }

    fn clear_runtime_error(&mut self, ctx: &mut ProcessCtx) {
        if let Some(node) = self.runtime_error_node.take() {
            ctx.clear_node_warning(node, Some("sound-card-runtime"));
        }
    }

    pub(super) fn stop_runtime(&mut self) {
        self.routes.invalidate();
        self.derived.invalidate();
        self.commands = command_service::CommandService::default();
        self.choices.pause();
        self.runtime_request = None;
        self.runtime_retry_at = None;
        self.runtime_retry.reset();
        self.runtime = None;
        self.runtime_worker = None;
        self.pending_events = event_delivery::PendingAudioEvents::default();
    }
}

pub(super) fn runtime_event_changes_inventory(event: &golden_audio::AudioEvent) -> bool {
    matches!(event, golden_audio::AudioEvent::DeviceInventoryChanged { .. })
}
