use std::path::PathBuf;

use golden_core::{
    events::{CustomEvent, Event, EventKind},
    node,
    node::{Node, NodeId, NodeReference},
    parameter::{ParamValue, ReferenceTargetKind},
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};
use serde::{Deserialize, Serialize};

use crate::app::module_modules_audio_sound_card_schema::{
    SoundCardVirtualOutput, SOUND_CARD_VIRTUAL_OUTPUT_FILTER_KEY,
};

pub(crate) const SOUND_CARD_PLAY_FILE_COMMAND_NODE_TYPE: &str =
    "sound_card_play_file_command";
pub(crate) const SOUND_CARD_STOP_FILE_COMMAND_NODE_TYPE: &str =
    "sound_card_stop_file_command";
pub(crate) const SOUND_CARD_STOP_ALL_FILES_COMMAND_NODE_TYPE: &str =
    "sound_card_stop_all_files_command";
pub(crate) const SOUND_CARD_SET_MASTER_VOLUME_COMMAND_NODE_TYPE: &str =
    "sound_card_set_master_volume_command";
pub(crate) const SOUND_CARD_SET_CHANNEL_VOLUME_COMMAND_NODE_TYPE: &str =
    "sound_card_set_channel_volume_command";

pub(crate) const SOUND_CARD_COMMAND_TYPES: &[&str] = &[
    SOUND_CARD_PLAY_FILE_COMMAND_NODE_TYPE,
    SOUND_CARD_STOP_FILE_COMMAND_NODE_TYPE,
    SOUND_CARD_STOP_ALL_FILES_COMMAND_NODE_TYPE,
    SOUND_CARD_SET_MASTER_VOLUME_COMMAND_NODE_TYPE,
    SOUND_CARD_SET_CHANNEL_VOLUME_COMMAND_NODE_TYPE,
];

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub(crate) enum SoundCardCommandRequest {
    PlayFile {
        path: PathBuf,
        playback_id: golden_audio::PlaybackId,
    },
    StopFile {
        playback_id: golden_audio::PlaybackId,
    },
    StopAllFiles,
    SetMasterVolume {
        gain: golden_audio::GainDb,
    },
    SetChannelVolume {
        virtual_output: NodeReference,
        gain: golden_audio::GainDb,
    },
}

trait SoundCardCommand: Node {
    fn request(&self, snapshot: &ProcessTreeSnapshot) -> Result<SoundCardCommandRequest, String>;
}

fn trigger_command<T: SoundCardCommand>(
    command: &T,
    ctx: &mut ProcessCtx,
    changed_param: NodeId,
) {
    let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
        return;
    };
    let snapshot = snapshot_arc.as_ref();
    if !crate::app::module_command::module_command_triggered(
        snapshot,
        command.id(),
        changed_param,
    ) {
        return;
    }
    execute_command(command, ctx, snapshot);
}

fn execute_command_event<T: SoundCardCommand>(
    command: &T,
    ctx: &mut ProcessCtx,
    event: &CustomEvent,
) {
    if !crate::app::module_command::is_command_execute_request(event, command.id()) {
        return;
    }
    let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
        return;
    };
    let snapshot = crate::app::module_command::command_execute_snapshot(
        event,
        snapshot_arc.as_ref(),
        command.id(),
    );
    execute_command(command, ctx, snapshot.as_ref());
}

fn execute_command<T: SoundCardCommand>(
    command: &T,
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
) {
    if let Err(error) = command.request(snapshot).and_then(|request| {
        crate::app::module_command::emit_module_command_request(
            ctx,
            snapshot,
            command.id(),
            command.get_type(),
            &request,
        )
    }) {
        golden_core::logerror!(origin = command.id(); format!(
            "Failed to execute Sound Card command: {error}"
        ));
    }
}

fn command_param<'a>(
    snapshot: &'a ProcessTreeSnapshot,
    command: NodeId,
    path: &str,
) -> Option<&'a ParamValue> {
    crate::app::module_command::resolve_module_command_child(snapshot, command, path)
        .and_then(|param| snapshot.node(param))
        .and_then(|node| node.param_value.as_ref())
}

fn playback_id(
    snapshot: &ProcessTreeSnapshot,
    command: NodeId,
) -> Result<golden_audio::PlaybackId, String> {
    let value = command_param(snapshot, command, "playback_id")
        .and_then(ParamValue::as_str)
        .unwrap_or_default();
    golden_audio::PlaybackId::new(value)
        .map_err(|error| format!("invalid Sound Card playback ID: {error}"))
}

fn gain(
    snapshot: &ProcessTreeSnapshot,
    command: NodeId,
) -> Result<golden_audio::GainDb, String> {
    let value = command_param(snapshot, command, "volume_db")
        .and_then(ParamValue::as_float)
        .ok_or_else(|| "Sound Card volume must be numeric".to_string())?;
    golden_audio::GainDb::new(value as f32)
        .map_err(|error| format!("invalid Sound Card volume: {error}"))
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
)]
pub struct SoundCardPlayFileCommand {
    base: crate::app::ModuleCommandBase,
}

impl SoundCardPlayFileCommand {
    pub fn create() -> Self {
        Self::new(crate::app::ModuleCommandBase::new())
    }
}

impl SoundCardCommand for SoundCardPlayFileCommand {
    fn request(
        &self,
        snapshot: &ProcessTreeSnapshot,
    ) -> Result<SoundCardCommandRequest, String> {
        let path = command_param(snapshot, self.id(), "audio_file")
            .and_then(ParamValue::as_str)
            .unwrap_or_default();
        if path.trim().is_empty() {
            return Err("Sound Card audio file path cannot be empty".to_string());
        }
        Ok(SoundCardCommandRequest::PlayFile {
            path: PathBuf::from(path),
            playback_id: playback_id(snapshot, self.id())?,
        })
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

    fn on_param_change(
        &mut self,
        ctx: &mut ProcessCtx,
        param: NodeId,
        _old_value: ParamValue,
    ) {
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

impl SoundCardCommand for SoundCardStopFileCommand {
    fn request(
        &self,
        snapshot: &ProcessTreeSnapshot,
    ) -> Result<SoundCardCommandRequest, String> {
        Ok(SoundCardCommandRequest::StopFile {
            playback_id: playback_id(snapshot, self.id())?,
        })
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

    fn on_param_change(
        &mut self,
        ctx: &mut ProcessCtx,
        param: NodeId,
        _old_value: ParamValue,
    ) {
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

impl SoundCardCommand for SoundCardStopAllFilesCommand {
    fn request(
        &self,
        _snapshot: &ProcessTreeSnapshot,
    ) -> Result<SoundCardCommandRequest, String> {
        Ok(SoundCardCommandRequest::StopAllFiles)
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

    fn on_param_change(
        &mut self,
        ctx: &mut ProcessCtx,
        param: NodeId,
        _old_value: ParamValue,
    ) {
        trigger_command(self, ctx, param);
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: CustomEvent) {
        execute_command_event(self, ctx, &event);
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::create)
    }
}

#[node(
    "sound_card_set_master_volume_command",
    label = "Set Audio Master Volume"
)]
#[children(
    volume_db: f64 = 0.0 [-120.0..24.0] (
        label = "Volume",
        description = "Master output target in decibels."
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

impl SoundCardCommand for SoundCardSetMasterVolumeCommand {
    fn request(
        &self,
        snapshot: &ProcessTreeSnapshot,
    ) -> Result<SoundCardCommandRequest, String> {
        Ok(SoundCardCommandRequest::SetMasterVolume {
            gain: gain(snapshot, self.id())?,
        })
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

    fn on_param_change(
        &mut self,
        ctx: &mut ProcessCtx,
        param: NodeId,
        _old_value: ParamValue,
    ) {
        trigger_command(self, ctx, param);
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: CustomEvent) {
        execute_command_event(self, ctx, &event);
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::create)
    }
}

#[node(
    "sound_card_set_channel_volume_command",
    label = "Set Audio Channel Volume"
)]
#[children(
    virtual_output: NodeReference = NodeReference::default() (
        label = "Virtual Output",
        description = "Stable virtual output owned by the target Sound Card module.",
        reference_target_kind = ReferenceTargetKind::AnyNode,
        reference_allowed_node_types = vec![SoundCardVirtualOutput::NODE_TYPE.to_string()],
        reference_custom_filter_key = Some(SOUND_CARD_VIRTUAL_OUTPUT_FILTER_KEY.to_string())
    );
    volume_db: f64 = 0.0 [-120.0..24.0] (
        label = "Volume",
        description = "Virtual output target in decibels."
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

impl SoundCardCommand for SoundCardSetChannelVolumeCommand {
    fn request(
        &self,
        snapshot: &ProcessTreeSnapshot,
    ) -> Result<SoundCardCommandRequest, String> {
        let Some(ParamValue::Reference(virtual_output)) =
            command_param(snapshot, self.id(), "virtual_output")
        else {
            return Err(
                "Sound Card channel volume requires a virtual-output reference"
                    .to_string(),
            );
        };
        if virtual_output.is_empty() {
            return Err(
                "Sound Card channel volume requires a virtual-output reference"
                    .to_string(),
            );
        }
        Ok(SoundCardCommandRequest::SetChannelVolume {
            virtual_output: virtual_output.clone(),
            gain: gain(snapshot, self.id())?,
        })
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

    fn on_param_change(
        &mut self,
        ctx: &mut ProcessCtx,
        param: NodeId,
        _old_value: ParamValue,
    ) {
        trigger_command(self, ctx, param);
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: CustomEvent) {
        execute_command_event(self, ctx, &event);
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::create)
    }
}
