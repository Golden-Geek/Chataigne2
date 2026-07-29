use golden_audio::{AudioEvent, PlaybackCommandIgnored, PlaybackStopReason};

pub(super) fn outgoing_audio_log_message(event: &AudioEvent) -> Option<String> {
    match event {
        AudioEvent::PlaybackStarted(info) => Some(format!(
            "Started playing audio file `{}` with playback ID `{}`.",
            info.path.display(),
            info.playback_id
        )),
        AudioEvent::PlaybackFinished(info) => Some(format!(
            "Finished playing audio file `{}` with playback ID `{}`.",
            info.path.display(),
            info.playback_id
        )),
        AudioEvent::PlaybackStopped(info) => Some(format!(
            "Stopped audio playback `{}` (reason: {}).",
            info.playback_id,
            playback_stop_reason_label(info.reason)
        )),
        AudioEvent::PlaybackFailed(failure) => Some(format!(
            "Audio playback `{}` for file `{}` failed: {}.",
            failure.playback_id,
            failure.path.display(),
            failure.error
        )),
        AudioEvent::PlaybackCommandIgnored(ignored) => {
            Some(playback_command_ignored_message(ignored))
        }
        _ => None,
    }
}

fn playback_command_ignored_message(ignored: &PlaybackCommandIgnored) -> String {
    match ignored {
        PlaybackCommandIgnored::PlayFileAlreadyActive {
            playback_id, path, ..
        } => format!(
            "Play audio file `{}` with playback ID `{}` had no effect because \
             Force Restart is disabled and that ID is already active or loading.",
            path.display(),
            playback_id
        ),
        PlaybackCommandIgnored::StopFileNotFound { playback_id, .. } => format!(
            "Stop audio playback `{}` had no effect because no active or loading \
             playback uses that ID.",
            playback_id
        ),
        PlaybackCommandIgnored::StopAllFilesEmpty { .. } => {
            "Stop all audio files had no effect because no audio files are active or loading."
                .to_owned()
        }
    }
}

const fn playback_stop_reason_label(reason: PlaybackStopReason) -> &'static str {
    match reason {
        PlaybackStopReason::Requested => "requested",
        PlaybackStopReason::Replaced => "replaced",
        PlaybackStopReason::StopAll => "stop all",
        PlaybackStopReason::ModuleDisabled => "module disabled",
        PlaybackStopReason::EndOfFile => "end of file",
        PlaybackStopReason::Failed => "failed",
    }
}
