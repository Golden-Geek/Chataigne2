use std::{path::PathBuf, time::Duration};

use golden_core::{
    events::{CustomEvent, Event, EventKind},
    node,
    node::{Node, NodeId, NodeReference},
    parameter::{ParamValue, ReferenceTargetKind},
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

use crate::app::module_modules_audio_sound_card_schema::SOUND_CARD_OUTPUT_GAIN_FILTER_KEY;

pub(crate) const SOUND_CARD_PLAY_FILE_COMMAND_NODE_TYPE: &str = "sound_card_play_file_command";
pub(crate) const SOUND_CARD_STOP_FILE_COMMAND_NODE_TYPE: &str = "sound_card_stop_file_command";
pub(crate) const SOUND_CARD_STOP_ALL_FILES_COMMAND_NODE_TYPE: &str = "sound_card_stop_all_files_command";
pub(crate) const SOUND_CARD_SET_MASTER_VOLUME_COMMAND_NODE_TYPE: &str = "sound_card_set_master_volume_command";
pub(crate) const SOUND_CARD_SET_CHANNEL_VOLUME_COMMAND_NODE_TYPE: &str = "sound_card_set_channel_volume_command";

pub(crate) const SOUND_CARD_COMMAND_TYPES: &[&str] = &[
    SOUND_CARD_PLAY_FILE_COMMAND_NODE_TYPE,
    SOUND_CARD_STOP_FILE_COMMAND_NODE_TYPE,
    SOUND_CARD_STOP_ALL_FILES_COMMAND_NODE_TYPE,
    SOUND_CARD_SET_MASTER_VOLUME_COMMAND_NODE_TYPE,
    SOUND_CARD_SET_CHANNEL_VOLUME_COMMAND_NODE_TYPE,
];

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum SoundCardCommandRequest {
    PlayFile {
        path: PathBuf,
        playback_id: golden_audio::PlaybackId,
        start_offset: Duration,
        force_restart: bool,
    },
    StopFile {
        playback_id: golden_audio::PlaybackId,
    },
    StopAllFiles,
    SetMasterVolume {
        gain: golden_audio::GainDb,
    },
    SetChannelVolume {
        output_channel: NodeReference,
        gain: golden_audio::GainDb,
    },
}

mod preparation;
pub(crate) use preparation::prepare_command;

fn trigger_command<T: Node>(command: &T, ctx: &mut ProcessCtx, changed_param: NodeId) {
    let Some(snapshot) = ctx.tree_snapshot_arc() else {
        return;
    };
    if crate::app::module_command::module_command_triggered(&snapshot, command.id(), changed_param) {
        dispatch(command, ctx, None);
    }
}

fn execute_command_event<T: Node>(command: &T, ctx: &mut ProcessCtx, event: &CustomEvent) {
    if event.topic == crate::app::module_command::MODULE_COMMAND_EXECUTE_TOPIC
        && event.payload.get("command_id").and_then(serde_json::Value::as_u64) == Some(command.id().0)
    {
        dispatch(command, ctx, Some(event.clone()));
    }
}

fn dispatch<T: Node>(command: &T, ctx: &mut ProcessCtx, event: Option<CustomEvent>) {
    let Some(snapshot) = ctx.tree_snapshot_arc() else {
        return;
    };
    let result =
        crate::app::module_command::CapturedInvocation::capture(&snapshot, command.id(), event, &["sound_card_module"])
            .and_then(|invocation| crate::app::SoundCardModule::dispatch_command(ctx, invocation));
    if let Err(error) = result {
        golden_core::logerror!(origin = command.id(); format!("Sound Card command was not admitted: {error}"));
    }
}

pub(crate) fn playback_start_offset(seconds: f64) -> Result<Duration, String> {
    Duration::try_from_secs_f64(seconds)
        .map_err(|_| "Sound Card playback start offset must be a finite non-negative number of seconds".to_string())
}

#[node("sound_card_play_file_command", label = "Play Audio File")]
#[children(
    audio_file: golden_core::parameter::File = golden_core::parameter::File::default() (
        label = "Audio File",
        description = "Audio asset prepared asynchronously for playback.",
        file_allowed_extensions = golden_audio::supported_audio_extensions()
            .iter()
            .map(|extension| (*extension).to_string())
            .collect::<Vec<_>>()
    );
    playback_id: String = "playback".to_string() (
        label = "Playback ID",
        description = "Stable non-empty lane identifier used for replacement and stopping."
    );
    start_offset: f64 = 0.0 [0.0..] (
        label = "Start Offset",
        description = "Time in the audio file to begin playback from.",
        widget = "time"
    );
    force_restart: bool = true (
        label = "Force Restart",
        description = "Restart this playback ID when it is already loading or playing."
    );
)]
pub struct SoundCardPlayFileCommand {
    base: crate::app::ModuleCommandBase,
}

impl SoundCardPlayFileCommand {
    pub fn create() -> Self {
        Self::new(crate::app::ModuleCommandBase::new())
    }
}

#[golden_core::item(
    "module_command",
    node = "sound_card_play_file_command",
    via = base,
    from_struct
)]
impl Node for SoundCardPlayFileCommand {
    fn child_event_interest_depth(&self, event: &Event) -> u32 {
        matches!(event.kind, EventKind::ParamChanged { .. })
            .then_some(u32::MAX)
            .unwrap_or(0)
    }

    fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, _old_value: ParamValue) {
        trigger_command(self, ctx, param);
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: CustomEvent) {
        execute_command_event(self, ctx, &event);
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::create)
    }
}

#[node("sound_card_stop_file_command", label = "Stop Audio File")]
#[children(
    playback_id: String = "playback".to_string() (
        label = "Playback ID",
        description = "Active or loading playback lane to stop."
    );
)]
pub struct SoundCardStopFileCommand {
    base: crate::app::ModuleCommandBase,
}

impl SoundCardStopFileCommand {
    pub fn create() -> Self {
        Self::new(crate::app::ModuleCommandBase::new())
    }
}

#[golden_core::item(
    "module_command",
    node = "sound_card_stop_file_command",
    via = base,
    from_struct
)]
impl Node for SoundCardStopFileCommand {
    fn child_event_interest_depth(&self, event: &Event) -> u32 {
        matches!(event.kind, EventKind::ParamChanged { .. })
            .then_some(u32::MAX)
            .unwrap_or(0)
    }

    fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, _old_value: ParamValue) {
        trigger_command(self, ctx, param);
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: CustomEvent) {
        execute_command_event(self, ctx, &event);
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::create)
    }
}

#[node("sound_card_stop_all_files_command", label = "Stop All Audio Files")]
pub struct SoundCardStopAllFilesCommand {
    base: crate::app::ModuleCommandBase,
}

impl SoundCardStopAllFilesCommand {
    pub fn create() -> Self {
        Self::new(crate::app::ModuleCommandBase::new())
    }
}

#[golden_core::item(
    "module_command",
    node = "sound_card_stop_all_files_command",
    via = base,
    from_struct
)]
impl Node for SoundCardStopAllFilesCommand {
    fn child_event_interest_depth(&self, event: &Event) -> u32 {
        matches!(event.kind, EventKind::ParamChanged { .. })
            .then_some(u32::MAX)
            .unwrap_or(0)
    }

    fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, _old_value: ParamValue) {
        trigger_command(self, ctx, param);
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: CustomEvent) {
        execute_command_event(self, ctx, &event);
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::create)
    }
}

#[node("sound_card_set_master_volume_command", label = "Set Audio Master Volume")]
#[children(
    volume_db: f64 = 0.0 [-120.0..24.0] (
        label = "Volume",
        description = "Master output target in decibels.",
        tags = vec![crate::app::module_command::COMMAND_PRIMARY_VALUE_TAG.to_owned()]
    );
)]
pub struct SoundCardSetMasterVolumeCommand {
    base: crate::app::ModuleCommandBase,
}

impl SoundCardSetMasterVolumeCommand {
    pub fn create() -> Self {
        Self::new(crate::app::ModuleCommandBase::new())
    }
}

#[golden_core::item(
    "module_command",
    node = "sound_card_set_master_volume_command",
    via = base,
    from_struct
)]
impl Node for SoundCardSetMasterVolumeCommand {
    fn child_event_interest_depth(&self, event: &Event) -> u32 {
        matches!(event.kind, EventKind::ParamChanged { .. })
            .then_some(u32::MAX)
            .unwrap_or(0)
    }

    fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, _old_value: ParamValue) {
        trigger_command(self, ctx, param);
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: CustomEvent) {
        execute_command_event(self, ctx, &event);
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::create)
    }
}

#[node("sound_card_set_channel_volume_command", label = "Set Audio Channel Volume")]
#[children(
    output_channel: NodeReference = NodeReference::default() (
        label = "Output Channel",
        description = "Stable output channel owned by the target Sound Card module.",
        reference_target_kind = ReferenceTargetKind::AnyNode,
        reference_allowed_node_types = vec!["float".to_string()],
        reference_custom_filter_key = Some(SOUND_CARD_OUTPUT_GAIN_FILTER_KEY.to_string())
    );
    volume_db: f64 = 0.0 [-120.0..24.0] (
        label = "Volume",
        description = "Output channel target in decibels."
    );
)]
pub struct SoundCardSetChannelVolumeCommand {
    base: crate::app::ModuleCommandBase,
}

impl SoundCardSetChannelVolumeCommand {
    pub fn create() -> Self {
        Self::new(crate::app::ModuleCommandBase::new())
    }
}

#[golden_core::item(
    "module_command",
    node = "sound_card_set_channel_volume_command",
    via = base,
    from_struct
)]
impl Node for SoundCardSetChannelVolumeCommand {
    fn child_event_interest_depth(&self, event: &Event) -> u32 {
        matches!(event.kind, EventKind::ParamChanged { .. })
            .then_some(u32::MAX)
            .unwrap_or(0)
    }

    fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, _old_value: ParamValue) {
        trigger_command(self, ctx, param);
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: CustomEvent) {
        execute_command_event(self, ctx, &event);
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::create)
    }
}

#[cfg(test)]
mod tests;
