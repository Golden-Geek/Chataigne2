use chataigne_sound_card_protocol::{
    SOUND_CARD_TELEMETRY_TOPIC, SOUND_CARD_UI_CONTROL_TOPIC,
    SoundCardUiControlRequest, SoundCardUiTelemetryDto,
};
use golden_audio::{
    AudioCommand, AudioError, AudioErrorCategory, CommandSequence, PlayFileRequest,
};
use serde::{Deserialize, Serialize};

use super::*;
use crate::app::module_modules_audio_sound_card_commands::{
    SOUND_CARD_COMMAND_TYPES, SOUND_CARD_PLAY_FILE_COMMAND_NODE_TYPE,
    SOUND_CARD_SET_CHANNEL_VOLUME_COMMAND_NODE_TYPE,
    SOUND_CARD_SET_MASTER_VOLUME_COMMAND_NODE_TYPE,
    SOUND_CARD_STOP_ALL_FILES_COMMAND_NODE_TYPE,
    SOUND_CARD_STOP_FILE_COMMAND_NODE_TYPE, SoundCardCommandRequest,
};

pub(crate) const SOUND_CARD_COMMAND_RESULT_TOPIC: &str =
    "chataigne.sound_card.command.result";

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
            .map_err(|error| {
                command_error(format!(
                    "invalid Sound Card UI control payload: {error}"
                ))
            })
            .and_then(|request| {
                let snapshot = ctx.tree_snapshot_arc().ok_or_else(|| {
                    command_error(
                        "Sound Card UI control requires a tree snapshot",
                    )
                })?;
                let request = match request {
                    SoundCardUiControlRequest::StopFile { playback_id } => {
                        SoundCardCommandRequest::StopFile {
                            playback_id: golden_audio::PlaybackId::new(playback_id)
                                .map_err(|error| command_error(error.to_string()))?,
                        }
                    }
                    SoundCardUiControlRequest::StopAllFiles => {
                        SoundCardCommandRequest::StopAllFiles
                    }
                };
                self.admit_request(snapshot.as_ref(), request)
            });
        if let Err(error) = result {
            golden_core::logerror!(origin = self.id(); format!(
                "Sound Card UI control was not admitted: {error}"
            ));
        }
        true
    }

    pub(super) fn sync_device_choices(
        &self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
    ) {
        if let Some(parameter) =
            find_path(snapshot, self.id(), "connection/input_device")
        {
            sync_device_enum(
                ctx,
                parameter,
                SYSTEM_DEFAULT_INPUT,
                "System Default Input",
            );
        }
        if let Some(parameter) =
            find_path(snapshot, self.id(), "connection/output_device")
        {
            sync_device_enum(
                ctx,
                parameter,
                SYSTEM_DEFAULT_OUTPUT,
                "System Default Output",
            );
        }
    }

    pub(super) fn ensure_runtime(&mut self, ctx: &mut ProcessCtx) {
        let sample_rate = golden_audio::SampleRate::new(
            u32::try_from(self.engine_sample_rate.get()).unwrap_or_default(),
        );
        let Ok(sample_rate) = sample_rate else {
            self.set_runtime_error(
                ctx,
                self.id(),
                "Sound Card engine sample rate is invalid",
            );
            return;
        };
        if self
            .runtime
            .as_ref()
            .is_some_and(|runtime| runtime.sample_rate() == sample_rate)
        {
            return;
        }
        self.stop_runtime();
        match runtime::SoundCardRuntime::start(sample_rate) {
            Ok(runtime) => {
                self.runtime = Some(runtime);
                self.configuration_dirty = true;
            }
            Err(error) => self.set_runtime_error(ctx, self.id(), error.as_str()),
        }
    }

    pub(super) fn refresh_configuration(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
    ) {
        self.ensure_runtime(ctx);
        let Some(runtime) = self.runtime.as_ref() else {
            return;
        };
        let inspector = runtime.inspector_state();
        match runtime::build_configuration(snapshot, self.id(), &inspector) {
            Ok(built) => {
                if built.sample_rate != runtime.sample_rate() {
                    self.stop_runtime();
                    self.ensure_runtime(ctx);
                    return;
                }
                self.apply_runtime_warnings(ctx, built.warnings.as_slice());
                let Some(runtime) = self.runtime.as_mut() else {
                    return;
                };
                match runtime.submit(built, snapshot) {
                    Ok(_) => {
                        self.configuration_dirty = false;
                        self.clear_runtime_error(ctx);
                    }
                    Err(error) => self.set_runtime_error(ctx, self.id(), error.as_str()),
                }
            }
            Err(error) => {
                self.set_runtime_error(ctx, error.node, error.message.as_str());
                self.configuration_dirty =
                    error.message.contains("backend is currently available");
            }
        }
    }

    pub(super) fn poll_runtime(&mut self, ctx: &mut ProcessCtx) {
        let Some(runtime) = self.runtime.as_mut() else {
            self.base.set_connected(ctx, false);
            self.base.set_data_capabilities(
                ctx,
                crate::app::module::ModuleDataCapabilities::new(false, false),
            );
            return;
        };
        let (observation, telemetry, events) = runtime.poll(ctx);
        let input_ready = direction_ready(&observation.device.input);
        let output_ready = direction_ready(&observation.device.output);
        let any_enabled = observation.device.input.enabled || observation.device.output.enabled;
        self.base.set_connected(
            ctx,
            observation.enabled
                && any_enabled
                && (!observation.device.input.enabled || input_ready)
                && (!observation.device.output.enabled || output_ready),
        );
        self.base.set_data_capabilities(
            ctx,
            crate::app::module::ModuleDataCapabilities::new(input_ready, output_ready),
        );
        self.sync_live_device_choices(ctx, &observation.device);
        if let Some(telemetry) = telemetry {
            self.emit_telemetry(ctx, &telemetry);
        }
        for event in &events {
            self.emit_audio_event_callback(ctx, event);
        }
    }

    pub(super) fn handle_sound_card_command_event(
        &mut self,
        ctx: &mut ProcessCtx,
        event: &golden_core::events::CustomEvent,
    ) {
        let Some(request_event) =
            crate::app::module_command::decode_module_command_request(event)
        else {
            return;
        };
        if request_event.module_id != self.id()
            || !SOUND_CARD_COMMAND_TYPES.contains(&request_event.command_type.as_str())
        {
            return;
        }

        let result = serde_json::from_value::<SoundCardCommandRequest>(
            request_event.payload.clone(),
        )
        .map_err(|error| {
            command_error(format!("invalid Sound Card command payload: {error}"))
        })
        .and_then(|request| {
            validate_request_type(request_event.command_type.as_str(), &request)?;
            let snapshot = ctx.tree_snapshot_arc().ok_or_else(|| {
                command_error("Sound Card command admission requires a tree snapshot")
            })?;
            self.admit_request(snapshot.as_ref(), request)
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
        if let Err(error) = ctx.emit_custom_payload(
            SOUND_CARD_COMMAND_RESULT_TOPIC,
            Some(self.id()),
            &response,
        ) {
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
            } => AudioCommand::PlayFile(PlayFileRequest::new(path, playback_id)),
            SoundCardCommandRequest::StopFile { playback_id } => {
                AudioCommand::StopFile { playback_id }
            }
            SoundCardCommandRequest::StopAllFiles => AudioCommand::StopAllFiles,
            SoundCardCommandRequest::SetMasterVolume { gain } => {
                AudioCommand::SetMasterGain { gain }
            }
            SoundCardCommandRequest::SetChannelVolume {
                virtual_output,
                gain,
            } => AudioCommand::SetChannelGain {
                channel: resolve_virtual_output(snapshot, self.id(), &virtual_output)?,
                gain,
            },
        };
        runtime.admit(command)
    }

    fn sync_live_device_choices(
        &self,
        ctx: &mut ProcessCtx,
        state: &golden_audio::AudioDeviceInspectorState,
    ) {
        let Some(runtime) = self.runtime.as_ref() else {
            return;
        };
        if let Some(parameter) = runtime.bindings().input_device() {
            sync_device_enum_with_state(
                ctx,
                parameter,
                SYSTEM_DEFAULT_INPUT,
                "Platform Default Input",
                state,
                golden_audio::AudioDirection::Input,
            );
        }
        if let Some(parameter) = runtime.bindings().output_device() {
            sync_device_enum_with_state(
                ctx,
                parameter,
                SYSTEM_DEFAULT_OUTPUT,
                "Platform Default Output",
                state,
                golden_audio::AudioDirection::Output,
            );
        }
    }

    fn emit_telemetry(&self, ctx: &mut ProcessCtx, telemetry: &SoundCardUiTelemetryDto) {
        let _ = ctx.emit_latest_custom_payload(
            SOUND_CARD_TELEMETRY_TOPIC,
            Some(self.id()),
            telemetry,
        );
    }

    fn apply_runtime_warnings(
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
            ctx.set_node_warning_with(
                warning.node,
                Some(warning.id.as_str()),
                warning.message.as_str(),
                warning.detail.as_deref(),
            );
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
        if let Some(mut runtime) = self.runtime.take() {
            runtime.stop();
        }
    }
}

fn validate_request_type(
    command_type: &str,
    request: &SoundCardCommandRequest,
) -> Result<(), AudioError> {
    let matches = matches!(
        (command_type, request),
        (
            SOUND_CARD_PLAY_FILE_COMMAND_NODE_TYPE,
            SoundCardCommandRequest::PlayFile { .. }
        ) | (
            SOUND_CARD_STOP_FILE_COMMAND_NODE_TYPE,
            SoundCardCommandRequest::StopFile { .. }
        ) | (
            SOUND_CARD_STOP_ALL_FILES_COMMAND_NODE_TYPE,
            SoundCardCommandRequest::StopAllFiles
        ) | (
            SOUND_CARD_SET_MASTER_VOLUME_COMMAND_NODE_TYPE,
            SoundCardCommandRequest::SetMasterVolume { .. }
        ) | (
            SOUND_CARD_SET_CHANNEL_VOLUME_COMMAND_NODE_TYPE,
            SoundCardCommandRequest::SetChannelVolume { .. }
        )
    );
    matches.then_some(()).ok_or_else(|| {
        command_error(format!(
            "Sound Card command payload does not match node type '{command_type}'"
        ))
    })
}

fn resolve_virtual_output(
    snapshot: &ProcessTreeSnapshot,
    module: NodeId,
    reference: &NodeReference,
) -> Result<golden_audio::AudioChannelId, AudioError> {
    let target = reference
        .cached_id()
        .filter(|target| {
            snapshot
                .node(*target)
                .is_some_and(|node| node.uuid == reference.uuid())
        })
        .or_else(|| snapshot.node_id_by_uuid(reference.uuid()))
        .ok_or_else(|| {
            command_error(format!(
                "Sound Card virtual output {} is missing",
                reference.uuid().0
            ))
        })?;
    let target_state = snapshot
        .node(target)
        .ok_or_else(|| command_error("Sound Card virtual output disappeared"))?;
    if target_state.node_type != SoundCardVirtualOutput::NODE_TYPE {
        return Err(command_error(
            "Sound Card channel volume target is not a virtual output",
        ));
    }
    if crate::app::module::resolve_enclosing_module_root(snapshot, target)
        != Some(module)
    {
        return Err(command_error(
            "Sound Card channel volume target belongs to another module",
        ));
    }
    Ok(golden_audio::AudioChannelId::from_uuid(
        target_state.uuid.0,
    ))
}

fn command_error(message: impl Into<String>) -> AudioError {
    AudioError::new(AudioErrorCategory::InvalidConfiguration, message)
}

fn sync_device_enum_with_state(
    ctx: &mut ProcessCtx,
    parameter_id: NodeId,
    default_value: &'static str,
    default_label: &'static str,
    state: &golden_audio::AudioDeviceInspectorState,
    direction: golden_audio::AudioDirection,
) {
    let mut options = device_options(default_value, default_label);
    let available_backends = state
        .backends
        .iter()
        .filter(|backend| backend.state == golden_audio::AudioBackendState::Available);
    for backend in available_backends {
        let target = golden_audio::AudioDeviceTargetId::SystemDefault {
            backend: backend.backend.clone(),
        };
        options.push(enum_option(
            runtime::device_target_value(&target).as_str(),
            format!("{} — System Default", backend.label).as_str(),
            10,
        ));
    }
    for (index, device) in state
        .devices
        .iter()
        .filter(|device| device.supports(direction))
        .enumerate()
    {
        let backend_label = state
            .backends
            .iter()
            .find(|backend| backend.backend == *device.target.backend())
            .map(|backend| backend.label.as_str())
            .unwrap_or_else(|| device.target.backend().as_str());
        options.push(enum_option(
            runtime::device_target_value(&device.target).as_str(),
            format!("{backend_label} — {}", device.label).as_str(),
            100 + i32::try_from(index).unwrap_or(i32::MAX),
        ));
    }
    options.sort_by(|left, right| {
        left.ordering
            .cmp(&right.ordering)
            .then_with(|| left.label.cmp(&right.label))
    });
    options.dedup_by(|left, right| left.variant_id == right.variant_id);

    ctx.call_node_mutation_without_snapshot(parameter_id, move |node, inner_ctx| {
        let Some(parameter) = node.as_any_mut().downcast_mut::<Parameter>() else {
            return Err("Sound Card device selector is not a parameter".to_string());
        };
        let current = parameter
            .value
            .as_enum()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| default_value.to_string());
        if !options.iter().any(|option| option.variant_id == current) {
            let mut missing = enum_option(
                current.as_str(),
                format!("Missing: {current}").as_str(),
                i32::MAX,
            );
            missing.tags.push("missing".to_string());
            options.push(missing);
        }
        if parameter.constraints.enum_options == options {
            return Ok(());
        }
        let mut constraints = parameter.constraints.clone();
        constraints.enum_options = options;
        inner_ctx.edits.push(Edit::SetParamConstraints {
            node: parameter_id,
            constraints,
        });
        Ok(())
    });
}

fn direction_ready(status: &golden_audio::AudioStreamStatus) -> bool {
    status.enabled && status.readiness == golden_audio::AudioDeviceReadiness::Ready
}
