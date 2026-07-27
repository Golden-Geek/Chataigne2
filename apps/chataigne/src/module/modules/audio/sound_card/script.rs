use std::path::PathBuf;

use golden_audio::AudioEvent;
use golden_core::{
    node::{Node, NodeReference, NodeScriptDescriptor, NodeUuid},
    parameter::ParamValue,
    process_ctx::ProcessCtx,
};

use super::SoundCardModule;
use crate::app::module_modules_audio_sound_card_commands::SoundCardCommandRequest;

pub(super) const SOUND_CARD_SCRIPT_METHODS: &[&str] = &[
    "playFile",
    "stopFile",
    "stopAllFiles",
    "setMasterVolume",
    "setChannelVolume",
];

pub(super) const PLAYBACK_STARTED_CALLBACK: &str = "playbackStarted";
pub(super) const PLAYBACK_FINISHED_CALLBACK: &str = "playbackFinished";
pub(super) const PLAYBACK_STOPPED_CALLBACK: &str = "playbackStopped";
pub(super) const PLAYBACK_FAILED_CALLBACK: &str = "playbackFailed";
pub(super) const AUDIO_DEVICE_STATUS_CHANGED_CALLBACK: &str = "audioDeviceStatusChanged";
pub(super) const AUDIO_BACKEND_STATUS_CHANGED_CALLBACK: &str = "audioBackendStatusChanged";

impl SoundCardModule {
    pub(super) fn sound_card_script_descriptor(&self) -> NodeScriptDescriptor {
        crate::app::module::script_api::descriptor_for_node(
            self.node_data(),
            self.get_type(),
            SOUND_CARD_SCRIPT_METHODS,
        )
    }

    pub(super) fn call_sound_card_script_method(
        &mut self,
        ctx: &mut ProcessCtx,
        method: &str,
        args: &[ParamValue],
    ) -> Option<Result<(), String>> {
        let request = script_request(method, args)?;
        Some(request.and_then(|request| {
            let snapshot = ctx
                .tree_snapshot_arc()
                .ok_or_else(|| format!("{method} requires a tree snapshot"))?;
            self.admit_and_apply_request(ctx, snapshot.as_ref(), request)
                .map(|_| ())
                .map_err(|error| format!("Sound Card {method} was not admitted: {error}"))
        }))
    }

    pub(super) fn emit_audio_event_callback(&self, ctx: &mut ProcessCtx, event: &AudioEvent) {
        use crate::app::module::script_api::{emit_script_callback, emit_transient_script_callback};

        match event {
            AudioEvent::PlaybackStarted(info) => emit_transient_script_callback(
                ctx,
                self.id(),
                PLAYBACK_STARTED_CALLBACK,
                vec![
                    serde_json::json!(info.playback_id.as_str()),
                    serde_json::json!(info.path.to_string_lossy()),
                    json_value(info),
                ],
            ),
            AudioEvent::PlaybackFinished(info) => emit_transient_script_callback(
                ctx,
                self.id(),
                PLAYBACK_FINISHED_CALLBACK,
                vec![serde_json::json!(info.playback_id.as_str()), json_value(info)],
            ),
            AudioEvent::PlaybackStopped(info) => emit_transient_script_callback(
                ctx,
                self.id(),
                PLAYBACK_STOPPED_CALLBACK,
                vec![
                    serde_json::json!(info.playback_id.as_str()),
                    json_value(&info.reason),
                    json_value(info),
                ],
            ),
            AudioEvent::PlaybackFailed(failure) => emit_transient_script_callback(
                ctx,
                self.id(),
                PLAYBACK_FAILED_CALLBACK,
                vec![
                    serde_json::json!(failure.playback_id.as_str()),
                    serde_json::json!(failure.path.to_string_lossy()),
                    json_value(&failure.error),
                ],
            ),
            AudioEvent::DeviceStatusChanged(status) => emit_script_callback(
                ctx,
                self.id(),
                AUDIO_DEVICE_STATUS_CHANGED_CALLBACK,
                vec![json_value(&status.direction), json_value(status)],
            ),
            AudioEvent::BackendStatusChanged(status) => emit_script_callback(
                ctx,
                self.id(),
                AUDIO_BACKEND_STATUS_CHANGED_CALLBACK,
                vec![serde_json::json!(status.backend.as_str()), json_value(status)],
            ),
            _ => {}
        }
    }
}

fn script_request(method: &str, args: &[ParamValue]) -> Option<Result<SoundCardCommandRequest, String>> {
    let result = match method {
        "playFile" => exact_args(method, args, 2).and_then(|()| {
            let path = string_arg(method, args, 0, "path")?;
            if path.trim().is_empty() {
                return Err("playFile path cannot be empty".to_string());
            }
            Ok(SoundCardCommandRequest::PlayFile {
                path: PathBuf::from(path),
                playback_id: playback_id_arg(method, args, 1)?,
            })
        }),
        "stopFile" => exact_args(method, args, 1).and_then(|()| {
            Ok(SoundCardCommandRequest::StopFile {
                playback_id: playback_id_arg(method, args, 0)?,
            })
        }),
        "stopAllFiles" => exact_args(method, args, 0).map(|()| SoundCardCommandRequest::StopAllFiles),
        "setMasterVolume" => exact_args(method, args, 1).and_then(|()| {
            Ok(SoundCardCommandRequest::SetMasterVolume {
                gain: gain_arg(method, args, 0)?,
            })
        }),
        "setChannelVolume" => exact_args(method, args, 2).and_then(|()| {
            Ok(SoundCardCommandRequest::SetChannelVolume {
                output_channel: channel_reference_arg(method, args, 0)?,
                gain: gain_arg(method, args, 1)?,
            })
        }),
        _ => return None,
    };
    Some(result)
}

fn exact_args(method: &str, args: &[ParamValue], expected: usize) -> Result<(), String> {
    if args.len() == expected {
        Ok(())
    } else {
        Err(format!(
            "{method} expects {expected} argument(s), received {}",
            args.len()
        ))
    }
}

fn string_arg(method: &str, args: &[ParamValue], index: usize, name: &str) -> Result<String, String> {
    args.get(index)
        .and_then(ParamValue::as_str)
        .ok_or_else(|| format!("{method} expects {name} to be a string"))
}

fn playback_id_arg(method: &str, args: &[ParamValue], index: usize) -> Result<golden_audio::PlaybackId, String> {
    golden_audio::PlaybackId::new(string_arg(method, args, index, "playbackId")?)
        .map_err(|error| format!("{method} received an invalid playbackId: {error}"))
}

fn gain_arg(method: &str, args: &[ParamValue], index: usize) -> Result<golden_audio::GainDb, String> {
    let value = args
        .get(index)
        .and_then(ParamValue::as_float)
        .ok_or_else(|| format!("{method} expects volumeDb to be numeric"))?;
    golden_audio::GainDb::new(value as f32).map_err(|error| format!("{method} received an invalid volumeDb: {error}"))
}

fn channel_reference_arg(method: &str, args: &[ParamValue], index: usize) -> Result<NodeReference, String> {
    let Some(value) = args.get(index) else {
        return Err(format!("{method} expects an output-channel node handle or UUID token"));
    };
    if let ParamValue::Reference(reference) = value {
        if reference.is_empty() {
            return Err(format!("{method} received an empty channel reference"));
        }
        return Ok(reference.clone());
    }
    let Some(token) = value.as_str() else {
        return Err(format!("{method} expects an output-channel node handle or UUID token"));
    };
    let uuid = uuid::Uuid::parse_str(token.as_str())
        .map_err(|_| format!("{method} received an invalid channel UUID token"))?;
    Ok(NodeReference::new(NodeUuid(uuid)))
}

fn json_value(value: &impl serde::Serialize) -> serde_json::Value {
    serde_json::to_value(value).unwrap_or_else(|error| {
        serde_json::json!({
            "serializationError": error.to_string(),
        })
    })
}
