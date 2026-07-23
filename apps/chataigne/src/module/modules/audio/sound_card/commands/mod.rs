use golden_core::{
    node,
    node::{Node, NodeReference},
    parameter::ReferenceTargetKind,
};

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

#[golden_core::item(
    "module_command",
    node = "sound_card_play_file_command",
    via = base,
    from_struct
)]
impl Node for SoundCardPlayFileCommand {
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

#[golden_core::item(
    "module_command",
    node = "sound_card_set_master_volume_command",
    via = base,
    from_struct
)]
impl Node for SoundCardSetMasterVolumeCommand {
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

#[golden_core::item(
    "module_command",
    node = "sound_card_set_channel_volume_command",
    via = base,
    from_struct
)]
impl Node for SoundCardSetChannelVolumeCommand {
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::create)
    }
}
