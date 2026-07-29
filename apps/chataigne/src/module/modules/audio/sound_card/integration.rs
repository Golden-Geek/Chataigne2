use chataigne_sound_card_protocol::{
    SOUND_CARD_TELEMETRY_TOPIC, SOUND_CARD_UI_CONTROL_TOPIC, SoundCardUiControlRequest, SoundCardUiTelemetryDto,
};
use golden_audio::{AudioCommand, AudioError, AudioErrorCategory, CommandSequence, PlayFileRequest};
use golden_core::{
    edit::NodeTree,
    node::{Node, NodeHandle, NodeReference, NodeUuid},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::*;
use crate::app::module_modules_audio_sound_card_commands::{
    SOUND_CARD_COMMAND_TYPES, SOUND_CARD_PLAY_FILE_COMMAND_NODE_TYPE, SOUND_CARD_SET_CHANNEL_VOLUME_COMMAND_NODE_TYPE,
    SOUND_CARD_SET_MASTER_VOLUME_COMMAND_NODE_TYPE, SOUND_CARD_STOP_ALL_FILES_COMMAND_NODE_TYPE,
    SOUND_CARD_STOP_FILE_COMMAND_NODE_TYPE, SoundCardCommandRequest,
};
use crate::app::module_modules_audio_sound_card_schema::{SoundCardInputRoute, SoundCardOutputRoute};

mod command_admission;
mod device_choices;

use command_admission::*;
use device_choices::{DeviceChoice, direction_ready, sync_device_enum_with_state, sync_numeric_enum};
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
    device_choices::device_options_for_current(current, state, choice, driver)
}
#[cfg(test)]
pub(super) fn duplex_device_options_for_current(
    current: &str,
    state: &golden_audio::AudioDeviceInspectorState,
    driver: Option<&golden_audio::BackendId>,
) -> Vec<golden_core::parameter::ParameterEnumOption> {
    device_choices::device_options_for_current(current, state, DeviceChoice::Duplex, driver)
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
        &self,
        ctx: &mut ProcessCtx,
        event: &golden_core::events::CustomEvent,
    ) -> bool {
        if event.topic != SOUND_CARD_UI_CONTROL_TOPIC {
            return false;
        }
        let result = event
            .payload_as::<SoundCardUiControlRequest>()
            .map_err(|error| command_error(format!("invalid Sound Card UI control payload: {error}")))
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
                    } => self.connect_route(
                        ctx,
                        snapshot.as_ref(),
                        direction,
                        physical_channel.as_str(),
                        app_channel_uuid.as_str(),
                    ),
                    SoundCardUiControlRequest::DisconnectRoute {
                        direction,
                        physical_channel,
                        app_channel_uuid,
                    } => self.disconnect_route(
                        ctx,
                        snapshot.as_ref(),
                        direction,
                        physical_channel.as_str(),
                        app_channel_uuid.as_str(),
                    ),
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

    pub(super) fn sync_device_choices(&self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) {
        if let Some(parameter) = find_path(snapshot, self.id(), "connection/device") {
            sync_enum_options(ctx, parameter, device_options());
        }
        if let Some(parameter) = find_path(snapshot, self.id(), "connection/input_device") {
            sync_enum_options(ctx, parameter, input_device_options());
        }
        if let Some(parameter) = find_path(snapshot, self.id(), "connection/output_device") {
            sync_enum_options(ctx, parameter, output_device_options());
        }
    }

    fn connect_route(
        &self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        direction: golden_audio::AudioDirection,
        physical_channel: &str,
        channel_uuid: &str,
    ) -> Result<(), AudioError> {
        self.validate_physical_channel(direction, physical_channel)?;
        let (channel, reference) = self.resolve_routing_channel(snapshot, direction, channel_uuid)?;
        let path = route_path(direction);
        let parent = find_path(snapshot, self.id(), path)
            .ok_or_else(|| command_error("Sound Card route container is missing"))?;
        if find_route(snapshot, parent, physical_channel, reference.uuid()).is_some() {
            return Ok(());
        }
        let module_uuid = self.node_data().meta.uuid;
        let tree = match direction {
            golden_audio::AudioDirection::Input => {
                let mut route = SoundCardInputRoute::connected(physical_channel, reference);
                structure::set_route_identity(
                    &mut route,
                    module_uuid,
                    "input",
                    physical_channel,
                    snapshot.node(channel).expect("channel exists").uuid,
                );
                NodeTree::new(route)
            }
            golden_audio::AudioDirection::Output => {
                let mut route = SoundCardOutputRoute::connected(reference, physical_channel);
                structure::set_route_identity(
                    &mut route,
                    module_uuid,
                    "output",
                    physical_channel,
                    snapshot.node(channel).expect("channel exists").uuid,
                );
                NodeTree::new(route)
            }
        };
        ctx.add_child_tree(parent, tree, None);
        Ok(())
    }

    fn disconnect_route(
        &self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        direction: golden_audio::AudioDirection,
        physical_channel: &str,
        channel_uuid: &str,
    ) -> Result<(), AudioError> {
        let (_, reference) = self.resolve_routing_channel(snapshot, direction, channel_uuid)?;
        let parent = find_path(snapshot, self.id(), route_path(direction))
            .ok_or_else(|| command_error("Sound Card route container is missing"))?;
        if let Some(route) = find_route(snapshot, parent, physical_channel, reference.uuid()) {
            NodeHandle::new(route).remove(ctx);
        }
        Ok(())
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
        let (channel, _) = self.resolve_routing_channel(snapshot, direction, channel_uuid)?;
        structure::patch_channel_label(ctx, channel, label.to_owned());
        Ok(())
    }

    fn resolve_routing_channel(
        &self,
        snapshot: &ProcessTreeSnapshot,
        direction: golden_audio::AudioDirection,
        channel_uuid: &str,
    ) -> Result<(NodeId, NodeReference), AudioError> {
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
        let mut reference = NodeReference::with_cached_id(uuid, Some(channel));
        reference.set_cached_name(Some(structure::channel_name(state)));
        Ok((channel, reference))
    }

    fn validate_physical_channel(
        &self,
        direction: golden_audio::AudioDirection,
        physical_channel: &str,
    ) -> Result<(), AudioError> {
        let state = self
            .runtime
            .as_ref()
            .ok_or_else(|| command_error("Sound Card runtime is not available"))?
            .inspector_state();
        let status = match direction {
            golden_audio::AudioDirection::Input => &state.input,
            golden_audio::AudioDirection::Output => &state.output,
        };
        let selected = status
            .active_target
            .as_ref()
            .or(status.selected_target.as_ref())
            .ok_or_else(|| command_error("Sound Card device is not ready"))?;
        let descriptor = state.devices.iter().find(|device| {
            device.supports(direction)
                && match selected {
                    golden_audio::AudioDeviceTargetId::Device { .. } => device.target == *selected,
                    golden_audio::AudioDeviceTargetId::SystemDefault { backend } => {
                        device.target.backend() == backend
                            && match direction {
                                golden_audio::AudioDirection::Input => device.is_system_default_input,
                                golden_audio::AudioDirection::Output => device.is_system_default_output,
                            }
                    }
                }
        });
        let channels = descriptor.map(|device| match direction {
            golden_audio::AudioDirection::Input => device.input_channels.as_slice(),
            golden_audio::AudioDirection::Output => device.output_channels.as_slice(),
        });
        if channels.is_some_and(|channels| channels.iter().any(|channel| channel.key.as_str() == physical_channel)) {
            Ok(())
        } else {
            Err(command_error(
                "Sound Card physical channel is not available on the selected device",
            ))
        }
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

    pub(super) fn runtime_matches_requested_sample_rate(&self) -> bool {
        let Ok(sample_rate) = self.requested_sample_rate() else {
            return false;
        };
        let Ok(driver) = self.requested_driver() else {
            return false;
        };
        if self
            .runtime
            .as_ref()
            .is_some_and(|runtime| runtime.sample_rate() == sample_rate && runtime.driver() == driver.as_ref())
        {
            return true;
        }
        false
    }

    pub(super) fn drive_runtime(&mut self, ctx: &mut ProcessCtx) {
        self.poll_runtime_worker(ctx);
        if self.configuration_dirty {
            if let Some(snapshot) = ctx.tree_snapshot_arc() {
                self.synchronize_derived_structure(ctx, snapshot.as_ref());
            }
        }
        self.request_runtime_start(ctx);
        self.poll_runtime(ctx);

        if !self.configuration_dirty {
            return;
        }
        if self.runtime_matches_requested_sample_rate() {
            if let Some(snapshot) = ctx.tree_snapshot_arc() {
                self.refresh_configuration(ctx, snapshot.as_ref());
                if self.configuration_dirty && !self.runtime_matches_requested_sample_rate() {
                    self.request_runtime_start(ctx);
                }
            } else if let Some(wake) = &self.runtime_wake {
                wake.wake();
            }
        } else {
            self.request_runtime_start(ctx);
        }
    }

    pub(super) fn request_runtime_start(&mut self, ctx: &mut ProcessCtx) {
        let sample_rate = match self.requested_sample_rate() {
            Ok(sample_rate) => sample_rate,
            Err(error) => {
                self.set_runtime_error(ctx, self.id(), error.as_str());
                return;
            }
        };
        let driver = match self.requested_driver() {
            Ok(driver) => driver,
            Err(error) => {
                self.set_runtime_error(ctx, self.id(), error.as_str());
                return;
            }
        };
        if driver.is_none() {
            self.runtime_request = None;
            self.runtime_retry_at = None;
            self.runtime_retry.reset();
            if let Some(runtime) = self.runtime.take() {
                self.retire_runtime(runtime);
            }
            self.configuration_dirty = false;
            self.clear_device_connection_warnings(ctx);
            self.clear_runtime_error(ctx);
            return;
        }
        if self
            .runtime
            .as_ref()
            .is_some_and(|runtime| runtime.sample_rate() == sample_rate && runtime.driver() == driver.as_ref())
        {
            self.runtime_request = None;
            self.runtime_retry_at = None;
            self.runtime_retry.reset();
            return;
        }
        if self
            .runtime_request
            .as_ref()
            .is_some_and(|request| request.sample_rate() == sample_rate && request.driver() == driver.as_ref())
        {
            return;
        }
        if let Some((retry_sample_rate, retry_driver, retry_at)) = self.runtime_retry_at.as_ref() {
            if *retry_sample_rate != sample_rate || retry_driver != &driver {
                self.runtime_retry_at = None;
                self.runtime_retry.reset();
            } else if Instant::now() < *retry_at {
                return;
            } else {
                self.runtime_retry_at = None;
            }
        }
        if self
            .runtime
            .as_ref()
            .is_some_and(|runtime| runtime.sample_rate() != sample_rate || runtime.driver() != driver.as_ref())
        {
            if let Some(runtime) = self.runtime.take() {
                self.retire_runtime(runtime);
            }
        }
        if self.runtime_worker.is_none() {
            let Some(wake) = self.runtime_wake.clone() else {
                self.set_runtime_error(
                    ctx,
                    self.id(),
                    "Sound Card runtime cannot start without an engine wake sender",
                );
                return;
            };
            match runtime::SoundCardRuntimeWorker::spawn(wake) {
                Ok(worker) => self.runtime_worker = Some(worker),
                Err(error) => {
                    self.schedule_runtime_retry(sample_rate, driver.clone());
                    self.set_runtime_error(ctx, self.id(), error.as_str());
                    return;
                }
            }
        }
        let request = self
            .runtime_worker
            .as_mut()
            .expect("Sound Card runtime worker was initialized")
            .request_start_for_driver(sample_rate, driver.clone());
        match request {
            Ok(request) => self.runtime_request = Some(request),
            Err(error) => {
                self.runtime_worker = None;
                self.schedule_runtime_retry(sample_rate, driver);
                self.set_runtime_error(ctx, self.id(), error.as_str());
            }
        }
    }

    pub(super) fn poll_runtime_worker(&mut self, ctx: &mut ProcessCtx) {
        loop {
            let poll = self.runtime_worker.as_ref().map(runtime::SoundCardRuntimeWorker::poll);
            match poll {
                None | Some(runtime::SoundCardRuntimeWorkerPoll::Pending) => return,
                Some(runtime::SoundCardRuntimeWorkerPoll::Started(started)) => {
                    self.handle_runtime_started(ctx, started);
                }
                Some(runtime::SoundCardRuntimeWorkerPoll::Disconnected) => {
                    self.runtime_worker = None;
                    let retry_sample_rate = self
                        .runtime_request
                        .take()
                        .map(|request| (request.sample_rate(), request.driver().cloned()))
                        .or_else(|| Some((self.requested_sample_rate().ok()?, self.requested_driver().ok()?)));
                    if let Some((sample_rate, driver)) = retry_sample_rate {
                        self.schedule_runtime_retry(sample_rate, driver);
                    }
                    self.set_runtime_error(ctx, self.id(), "Sound Card runtime worker stopped unexpectedly");
                    return;
                }
            }
        }
    }

    fn handle_runtime_started(&mut self, ctx: &mut ProcessCtx, started: runtime::SoundCardRuntimeStarted) {
        if self.runtime_request.as_ref() != Some(&started.request) {
            if let Ok(runtime) = started.result {
                self.retire_runtime(*runtime);
            }
            return;
        }
        self.runtime_request = None;
        if self.requested_sample_rate().ok() != Some(started.request.sample_rate())
            || self.requested_driver().ok() != Some(started.request.driver().cloned())
        {
            if let Ok(runtime) = started.result {
                self.retire_runtime(*runtime);
            }
            return;
        }
        match started.result {
            Ok(runtime) => {
                let mut runtime = *runtime;
                let Some(wake) = self.runtime_wake.clone() else {
                    self.retire_runtime(runtime);
                    self.set_runtime_error(ctx, self.id(), "Sound Card runtime lost its engine wake sender");
                    return;
                };
                if let Err(error) = runtime.enable_notifications(wake) {
                    self.retire_runtime(runtime);
                    self.schedule_runtime_retry(started.request.sample_rate(), started.request.driver().cloned());
                    self.set_runtime_error(ctx, self.id(), error.as_str());
                    return;
                }
                self.clear_device_connection_warnings(ctx);
                if let Some(previous) = self.runtime.replace(runtime) {
                    self.retire_runtime(previous);
                }
                self.runtime_retry.reset();
                self.runtime_retry_at = None;
                self.configuration_dirty = true;
            }
            Err(error) => {
                self.schedule_runtime_retry(started.request.sample_rate(), started.request.driver().cloned());
                self.set_runtime_error(ctx, self.id(), error.as_str());
            }
        }
    }

    fn schedule_runtime_retry(
        &mut self,
        sample_rate: golden_audio::SampleRate,
        driver: Option<golden_audio::BackendId>,
    ) {
        let now = Instant::now();
        let retry_at = self.runtime_retry.schedule(now);
        self.runtime_retry_at = Some((sample_rate, driver, retry_at));
        if let Some(wake) = &self.runtime_wake {
            wake.wake_after(retry_at.saturating_duration_since(now));
        }
    }

    fn retire_runtime(&self, runtime: runtime::SoundCardRuntime) {
        if let Some(worker) = self.runtime_worker.as_ref() {
            worker.retire(runtime);
        } else {
            runtime::retire_detached(runtime);
        }
    }

    pub(super) fn refresh_configuration(&mut self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) {
        if self.reset_routes_for_device_selection(ctx, snapshot) {
            self.configuration_dirty = true;
            return;
        }
        let driver = self.requested_driver().ok().flatten();
        let (input_device, output_device) = self.selected_device_values();
        let input_device = input_device.to_owned();
        let output_device = output_device.to_owned();
        let probe_targets = match runtime::selected_probe_targets(
            driver.as_ref(),
            input_device.as_str(),
            output_device.as_str(),
        ) {
            Ok(targets) => targets,
            Err(error) => {
                if let Some(runtime) = self.runtime.as_mut() {
                    let _ = runtime.set_enabled(false);
                }
                self.set_runtime_error(ctx, self.id(), error.as_str());
                self.configuration_dirty = false;
                return;
            }
        };
        let Some(runtime) = self.runtime.as_mut() else {
            return;
        };
        let inspector = runtime.inspector_state();
        if runtime::requires_disable_before_probe(
            driver.as_ref(),
            &inspector,
            probe_targets.as_slice(),
        ) {
            if let Err(error) = runtime.set_enabled(false) {
                self.set_runtime_error(ctx, self.id(), error.as_str());
                self.configuration_dirty = false;
                return;
            }
        }
        if let Err(error) = runtime.set_probe_interests(probe_targets.clone()) {
            self.set_runtime_error(ctx, self.id(), error.as_str());
            self.configuration_dirty = false;
            return;
        }
        let inspector = runtime.inspector_state();
        let runtime_sample_rate = runtime.sample_rate();
        if runtime::probe_targets_are_pending(
            &inspector,
            probe_targets.as_slice(),
        ) {
            let _ = runtime.set_enabled(false);
            self.configuration_dirty = false;
            self.clear_runtime_error(ctx);
            return;
        }
        let input_physical = runtime::selected_physical_channels(
            driver.as_ref(),
            &inspector,
            input_device.as_str(),
            golden_audio::AudioDirection::Input,
        );
        let output_physical = runtime::selected_physical_channels(
            driver.as_ref(),
            &inspector,
            output_device.as_str(),
            golden_audio::AudioDirection::Output,
        );
        if self.seed_default_routes_from_inventory(ctx, snapshot, input_physical.as_slice(), output_physical.as_slice())
        {
            self.configuration_dirty = true;
            return;
        }
        match runtime::build_configuration(snapshot, self.id(), &inspector) {
            Ok(built) => {
                if built.sample_rate != runtime_sample_rate {
                    if self.sample_rate.get_ref().0 == AUTOMATIC_CONFIGURATION {
                        self.automatic_sample_rate = Some(built.sample_rate);
                    }
                    self.configuration_dirty = true;
                    return;
                }
                self.apply_runtime_warnings(ctx, built.warnings.as_slice());
                let module_enabled = snapshot.node(self.id()).is_some_and(|node| node.enabled);
                let Some(runtime) = self.runtime.as_mut() else {
                    return;
                };
                match runtime.submit(built, snapshot) {
                    Ok(_) => {
                        let _ = runtime.set_enabled(module_enabled);
                        self.configuration_dirty = false;
                        self.clear_runtime_error(ctx);
                    }
                    Err(error) => self.set_runtime_error(ctx, self.id(), error.as_str()),
                }
            }
            Err(error) => {
                if let Some(runtime) = self.runtime.as_mut() {
                    let _ = runtime.set_enabled(false);
                }
                self.set_runtime_error(ctx, error.node, error.message.as_str());
                self.configuration_dirty = false;
            }
        }
    }

    pub(super) fn poll_runtime(&mut self, ctx: &mut ProcessCtx) {
        let selected_parameter_ids = self.selected_device_parameter_ids();
        let (input_device, output_device) = self.selected_device_values();
        let input_device = input_device.to_owned();
        let output_device = output_device.to_owned();
        let Some(runtime) = self.runtime.as_mut() else {
            self.base.set_connected(ctx, false);
            self.base
                .set_data_capabilities(ctx, crate::app::module::ModuleDataCapabilities::new(false, false));
            return;
        };
        let (observation, telemetry, events) = runtime.poll(ctx);
        if selected_parameter_ids
            .iter()
            .any(|node| self.runtime_error_node == Some(*node))
        {
            let driver = self.requested_driver().ok().flatten();
            if runtime::selected_devices_are_available(
                driver.as_ref(),
                &observation.device,
                input_device.as_str(),
                output_device.as_str(),
            ) {
                self.configuration_dirty = true;
            }
        }
        let input_ready = direction_ready(&observation.device.input);
        let output_ready = direction_ready(&observation.device.output);
        self.base
            .set_connected(ctx, observation.enabled && any_direction_connected(&observation.device));
        self.base.set_data_capabilities(
            ctx,
            crate::app::module::ModuleDataCapabilities::new(input_ready, output_ready),
        );
        let inventory_changed = events.iter().any(runtime_event_changes_inventory);
        if inventory_changed {
            self.sync_live_device_choices(ctx, &observation.device);
        }
        if let Some(telemetry) = telemetry {
            self.emit_telemetry(ctx, &telemetry);
        }
        for event in &events {
            if runtime_event_changes_inventory(event) {
                self.configuration_dirty = true;
            }
            if let golden_audio::AudioEvent::DeviceStatusChanged(status) = event {
                self.apply_device_connection_feedback(ctx, status);
            }
            if self.base.log_outgoing_enabled() {
                if let Some(message) =
                    super::traffic_logging::outgoing_audio_log_message(event)
                {
                    golden_core::log!(origin = self.id(); message);
                }
            }
            self.emit_audio_event_callback(ctx, event);
        }
    }

    pub(super) fn handle_sound_card_command_event(
        &mut self,
        ctx: &mut ProcessCtx,
        event: &golden_core::events::CustomEvent,
    ) {
        let Some(request_event) = crate::app::module_command::decode_module_command_request(event) else {
            return;
        };
        if request_event.module_id != self.id()
            || !SOUND_CARD_COMMAND_TYPES.contains(&request_event.command_type.as_str())
        {
            return;
        }

        let result = serde_json::from_value::<SoundCardCommandRequest>(request_event.payload.clone())
            .map_err(|error| command_error(format!("invalid Sound Card command payload: {error}")))
            .and_then(|request| {
                validate_request_type(request_event.command_type.as_str(), &request)?;
                let snapshot = ctx
                    .tree_snapshot_arc()
                    .ok_or_else(|| command_error("Sound Card command admission requires a tree snapshot"))?;
                self.admit_and_apply_request(ctx, snapshot.as_ref(), request)
            });

        let response = match result {
            Ok(sequence) => {
                self.base.emit_outgoing_traffic(ctx);
                SoundCardCommandResultEvent {
                    module_id: self.id(),
                    command_id: request_event.command_id,
                    command_type: request_event.command_type,
                    admitted_sequence: Some(sequence.get()),
                    error: None,
                }
            }
            Err(error) => {
                golden_core::logerror!(origin = self.id(); format!(
                    "Sound Card command {:?} was not admitted: {error}",
                    request_event.command_id
                ));
                SoundCardCommandResultEvent {
                    module_id: self.id(),
                    command_id: request_event.command_id,
                    command_type: request_event.command_type,
                    admitted_sequence: None,
                    error: Some(error),
                }
            }
        };
        if let Err(error) = ctx.emit_custom_payload(SOUND_CARD_COMMAND_RESULT_TOPIC, Some(self.id()), &response) {
            golden_core::logerror!(origin = self.id(); format!(
                "Failed to emit Sound Card command result: {error}"
            ));
        }
    }

    pub(super) fn admit_request(
        &self,
        snapshot: &ProcessTreeSnapshot,
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
                channel: resolve_output_channel(snapshot, self.id(), &output_channel)?,
                gain,
            },
        };
        runtime.admit(command)
    }

    pub(super) fn admit_and_apply_request(
        &self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        request: SoundCardCommandRequest,
    ) -> Result<CommandSequence, AudioError> {
        let authored_change = authored_gain_change(snapshot, self.id(), &request)?;
        let sequence = self.admit_request(snapshot, request)?;
        if let Some((parameter, value)) = authored_change {
            ctx.set_param(parameter, value);
        }
        Ok(sequence)
    }

    pub(super) fn sync_live_device_choices(
        &self,
        ctx: &mut ProcessCtx,
        state: &golden_audio::AudioDeviceInspectorState,
    ) {
        let driver = self.requested_driver().ok().flatten();
        if self.uses_duplex_device() {
            sync_device_enum_with_state(
                ctx,
                self.device.id(),
                state,
                DeviceChoice::Duplex,
                driver.as_ref(),
            );
        } else {
            sync_device_enum_with_state(
                ctx,
                self.input_device.id(),
                state,
                DeviceChoice::Input,
                driver.as_ref(),
            );
            sync_device_enum_with_state(
                ctx,
                self.output_device.id(),
                state,
                DeviceChoice::Output,
                driver.as_ref(),
            );
        }
        let (input_device, output_device) = self.selected_device_values();
        let (sample_rates, buffer_sizes) = runtime::configuration_capabilities(
            driver.as_ref(),
            state,
            input_device,
            output_device,
            self.sample_rate.get_ref().0.as_str(),
            self.buffer_size.get_ref().0.as_str(),
        );
        sync_numeric_enum(ctx, self.sample_rate.id(), "Hz", sample_rates.as_slice());
        sync_numeric_enum(ctx, self.buffer_size.id(), "frames", buffer_sizes.as_slice());
    }

    fn emit_telemetry(&self, ctx: &mut ProcessCtx, telemetry: &SoundCardUiTelemetryDto) {
        let _ = ctx.emit_latest_custom_payload(SOUND_CARD_TELEMETRY_TOPIC, Some(self.id()), telemetry);
    }

    pub(super) fn apply_runtime_warnings(
        &mut self,
        ctx: &mut ProcessCtx,
        warnings: &[runtime::RuntimeWarning],
    ) {
        let next = warnings
            .iter()
            .map(|warning| (warning.node, warning.id.clone()))
            .collect::<HashSet<_>>();
        for (node, warning_id) in self.active_runtime_warnings.difference(&next) {
            ctx.clear_node_warning(*node, Some(warning_id.as_str()));
        }
        for warning in warnings {
            if !self
                .active_runtime_warnings
                .contains(&(warning.node, warning.id.clone()))
            {
                ctx.set_node_warning_with(
                    warning.node,
                    Some(warning.id.as_str()),
                    warning.message.as_str(),
                    warning.detail.as_deref(),
                );
            }
        }
        self.active_runtime_warnings = next;
    }

    fn set_runtime_error(&mut self, ctx: &mut ProcessCtx, node: NodeId, message: &str) {
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
        self.runtime_request = None;
        self.runtime_retry_at = None;
        self.runtime_retry.reset();
        if let Some(runtime) = self.runtime.take() {
            self.retire_runtime(runtime);
        }
        self.runtime_worker = None;
    }
}

pub(super) fn runtime_event_changes_inventory(event: &golden_audio::AudioEvent) -> bool {
    matches!(
        event,
        golden_audio::AudioEvent::DeviceInventoryChanged { .. }
    )
}

pub(super) fn any_direction_connected(state: &golden_audio::AudioDeviceInspectorState) -> bool {
    direction_ready(&state.input) || direction_ready(&state.output)
}
