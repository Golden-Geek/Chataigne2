use std::{collections::BTreeMap, mem::size_of};

use super::{AudioEvent, PlaybackCommandIgnored};
use crate::{AudioDeviceTargetId, AudioError};

const TEXT_BYTES: usize = 4096;
const CONTEXT_ITEMS: usize = 16;
const KEY_BYTES: usize = 256;
const CONTEXT_VALUE_BYTES: usize = 1024;
const TRUNCATED: &str = " [truncated]";

impl AudioEvent {
    /// Inline event and owned heap storage, including spare string/path capacity.
    /// Map accounting conservatively charges one whole B-tree node per entry plus
    /// an empty root. This excludes allocator metadata and independent copies.
    #[must_use]
    pub fn retained_bytes(&self) -> usize {
        size_of::<Self>().saturating_add(match self {
            Self::ConfigurationRejected { error, .. } => error_bytes(error),
            Self::BackendStatusChanged(status) => {
                status.backend.retained_heap_bytes() + status.label.capacity() + optional_bytes(&status.detail)
            }
            Self::DeviceStatusChanged(status) => {
                status.selected_target.as_ref().map_or(0, target_bytes)
                    + optional_bytes(&status.selected_label)
                    + status.profile_key.as_ref().map_or(0, |id| id.retained_heap_bytes())
                    + status.active_target.as_ref().map_or(0, target_bytes)
                    + status.error.as_ref().map_or(0, |error| {
                        error.message.capacity() + optional_bytes(&error.technical_detail)
                    })
            }
            Self::PlaybackStarted(info) | Self::PlaybackFinished(info) => {
                info.playback_id.retained_heap_bytes() + info.path.capacity()
            }
            Self::PlaybackStopped(info) => info.playback_id.retained_heap_bytes(),
            Self::PlaybackFailed(failure) => {
                failure.playback_id.retained_heap_bytes() + failure.path.capacity() + error_bytes(&failure.error)
            }
            Self::PlaybackCommandIgnored(ignored) => match ignored {
                PlaybackCommandIgnored::PlayFileAlreadyActive { playback_id, path, .. } => {
                    playback_id.retained_heap_bytes() + path.capacity()
                }
                PlaybackCommandIgnored::StopFileNotFound { playback_id, .. } => playback_id.retained_heap_bytes(),
                PlaybackCommandIgnored::StopAllFilesEmpty { .. } => 0,
            },
            Self::Diagnostic(diagnostic) => {
                diagnostic.code.capacity() + diagnostic.message.capacity() + context_bytes(&diagnostic.context)
            }
            Self::ConfigurationApplied { .. }
            | Self::DeviceInventoryChanged { .. }
            | Self::QueuePressure(_)
            | Self::ShutdownComplete => 0,
        })
    }

    /// Only diagnostic text is shortened. Accepted playback identities and paths
    /// are preserved verbatim; their admission bounds precede command acceptance.
    pub(in super::super) fn prepare_delivery(&mut self) {
        match self {
            Self::ConfigurationRejected { error, .. } => compact_error(error),
            Self::BackendStatusChanged(status) => {
                compact_text(&mut status.label, TEXT_BYTES);
                compact_optional(&mut status.detail);
            }
            Self::DeviceStatusChanged(status) => {
                compact_optional(&mut status.selected_label);
                if let Some(error) = &mut status.error {
                    compact_text(&mut error.message, TEXT_BYTES);
                    compact_optional(&mut error.technical_detail);
                }
            }
            Self::PlaybackFailed(failure) => compact_error(&mut failure.error),
            Self::Diagnostic(diagnostic) => {
                compact_text(&mut diagnostic.code, KEY_BYTES);
                compact_text(&mut diagnostic.message, TEXT_BYTES);
                compact_context(&mut diagnostic.context);
            }
            _ => {}
        }
    }
}

fn target_bytes(target: &AudioDeviceTargetId) -> usize {
    match target {
        AudioDeviceTargetId::SystemDefault { backend } => backend.retained_heap_bytes(),
        AudioDeviceTargetId::Device { backend, device } => backend.retained_heap_bytes() + device.retained_heap_bytes(),
    }
}
fn optional_bytes(value: &Option<String>) -> usize {
    value.as_ref().map_or(0, String::capacity)
}
fn context_bytes(context: &BTreeMap<String, String>) -> usize {
    context
        .iter()
        .fold((context.len() + 1).saturating_mul(1024), |bytes, (key, value)| {
            bytes.saturating_add(key.capacity()).saturating_add(value.capacity())
        })
}
fn error_bytes(error: &AudioError) -> usize {
    error.message.capacity().saturating_add(context_bytes(&error.context))
}
fn compact_error(error: &mut AudioError) {
    compact_text(&mut error.message, TEXT_BYTES);
    compact_context(&mut error.context);
}
fn compact_optional(value: &mut Option<String>) {
    if let Some(value) = value {
        compact_text(value, TEXT_BYTES);
    }
}
fn compact_text(value: &mut String, maximum: usize) -> bool {
    let truncated = value.len() > maximum;
    if truncated {
        let mut end = maximum - TRUNCATED.len();
        while !value.is_char_boundary(end) {
            end -= 1;
        }
        value.truncate(end);
        value.push_str(TRUNCATED);
    }
    *value = std::mem::take(value).into_boxed_str().into_string();
    truncated
}
fn compact_context(context: &mut BTreeMap<String, String>) {
    let mut truncated = context.len() > CONTEXT_ITEMS;
    let old = std::mem::take(context);
    for (mut key, mut value) in old.into_iter().take(CONTEXT_ITEMS) {
        truncated |= compact_text(&mut key, KEY_BYTES);
        truncated |= compact_text(&mut value, CONTEXT_VALUE_BYTES);
        context.insert(key, value);
    }
    if truncated {
        context.insert("truncated".into(), "true".into());
    }
}

#[cfg(test)]
mod tests;
