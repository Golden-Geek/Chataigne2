use super::*;
use crate::app::module_command::{CapturedInvocation, ModuleCommandExecuteEvent};

pub(crate) const COMMAND_SOURCE_BYTES: usize = 256 * 1024;
pub(crate) const COMMAND_PATH_BYTES: usize = 64 * 1024;

struct View<'a> {
    snapshot: &'a ProcessTreeSnapshot,
    execution: Option<ModuleCommandExecuteEvent>,
}

impl View<'_> {
    fn param(&self, command: NodeId, path: &str) -> Option<&ParamValue> {
        let id = self
            .snapshot
            .find_child_by_decl_id(command, path)
            .or_else(|| self.snapshot.find_child(command, path))?;
        let original = self.snapshot.node(id)?.param_value.as_ref()?;
        Some(
            self.execution
                .as_ref()
                .and_then(|event| event.param_overrides.iter().rev().find(|value| value.param_id == id))
                .map(|value| &value.value)
                .unwrap_or(original),
        )
    }

    fn string(&self, command: NodeId, path: &str) -> Result<&str, String> {
        match self.param(command, path) {
            Some(ParamValue::Str(value) | ParamValue::File(value) | ParamValue::Enum(value)) => Ok(value),
            _ => Err("Sound Card command requires a string parameter".into()),
        }
    }
}

/// Decode the admitted override tree only on the preparation worker. Borrow parameter
/// values rather than cloning snapshot nodes and their unrelated metadata.
pub(crate) fn prepare_command(invocation: &CapturedInvocation) -> Result<SoundCardCommandRequest, String> {
    let execution = invocation
        .execution
        .as_ref()
        .map(|event| {
            serde_json::from_value::<ModuleCommandExecuteEvent>((**event).clone())
                .map_err(|_| "Invalid Sound Card command overrides".to_string())
        })
        .transpose()?;
    if execution
        .as_ref()
        .is_some_and(|event| event.command_id != invocation.command)
    {
        return Err("Sound Card invocation targets another command".into());
    }
    request(
        &View {
            snapshot: &invocation.snapshot,
            execution,
        },
        invocation.command,
    )
}

#[cfg(test)]
pub(super) fn request_from_snapshot(
    snapshot: &ProcessTreeSnapshot,
    command: NodeId,
) -> Result<SoundCardCommandRequest, String> {
    request(
        &View {
            snapshot,
            execution: None,
        },
        command,
    )
}

fn request(view: &View<'_>, command: NodeId) -> Result<SoundCardCommandRequest, String> {
    let mut bytes = 0usize;
    for path in [
        "audio_file",
        "playback_id",
        "start_offset",
        "force_restart",
        "volume_db",
        "output_channel",
    ] {
        bytes = bytes.saturating_add(view.param(command, path).map_or(0, ParamValue::retained_bytes));
        if bytes > COMMAND_SOURCE_BYTES {
            return Err("Sound Card command source exceeds 256 KiB".into());
        }
    }
    let playback_id = || {
        golden_audio::PlaybackId::new(view.string(command, "playback_id")?.to_owned())
            .map_err(|_| "Invalid Sound Card playback ID".to_string())
    };
    let gain = || {
        let value = view
            .param(command, "volume_db")
            .and_then(ParamValue::as_float)
            .ok_or("Sound Card volume must be numeric")?;
        golden_audio::GainDb::new(value as f32).map_err(|_| "Invalid Sound Card volume".to_string())
    };
    match view.snapshot.node(command).map(|node| node.node_type.as_str()) {
        Some(SOUND_CARD_PLAY_FILE_COMMAND_NODE_TYPE) => {
            let path = view.string(command, "audio_file")?;
            if path.is_empty() || path.len() > COMMAND_PATH_BYTES || path.trim().is_empty() {
                return Err("Sound Card audio path must contain 1 to 65536 bytes".into());
            }
            Ok(SoundCardCommandRequest::PlayFile {
                path: PathBuf::from(path),
                playback_id: playback_id()?,
                start_offset: playback_start_offset(
                    view.param(command, "start_offset")
                        .and_then(ParamValue::as_float)
                        .ok_or("Sound Card start offset must be numeric")?,
                )?,
                force_restart: view
                    .param(command, "force_restart")
                    .and_then(ParamValue::as_bool)
                    .ok_or("Sound Card Force Restart must be boolean")?,
            })
        }
        Some(SOUND_CARD_STOP_FILE_COMMAND_NODE_TYPE) => Ok(SoundCardCommandRequest::StopFile {
            playback_id: playback_id()?,
        }),
        Some(SOUND_CARD_STOP_ALL_FILES_COMMAND_NODE_TYPE) => Ok(SoundCardCommandRequest::StopAllFiles),
        Some(SOUND_CARD_SET_MASTER_VOLUME_COMMAND_NODE_TYPE) => {
            Ok(SoundCardCommandRequest::SetMasterVolume { gain: gain()? })
        }
        Some(SOUND_CARD_SET_CHANNEL_VOLUME_COMMAND_NODE_TYPE) => {
            let Some(ParamValue::Reference(reference)) = view.param(command, "output_channel") else {
                return Err("Sound Card channel volume requires an output-channel reference".into());
            };
            if reference.is_empty() {
                return Err("Sound Card output-channel reference is empty".into());
            }
            Ok(SoundCardCommandRequest::SetChannelVolume {
                output_channel: NodeReference::new(reference.uuid()),
                gain: gain()?,
            })
        }
        _ => Err("Unsupported Sound Card command".into()),
    }
}
