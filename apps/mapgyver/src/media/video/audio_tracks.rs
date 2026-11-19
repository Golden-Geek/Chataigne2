//! Per-channel embedded audio choices and the read-only catalog for the current Video File.
use crate::audio::voice;
#[cfg(feature = "video-files")]
use golden_core::parameter::{ParamValue, ParameterEnumOption};
use golden_core::{engine::NodeWaker, parameter::ParameterEnumOptions};
use std::sync::Mutex;

pub(crate) const AUTOMATIC: &str = "Automatic";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum AudioTrack {
    #[default]
    Automatic,
    Embedded(u32),
}

impl AudioTrack {
    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        if value == AUTOMATIC {
            return Ok(Self::Automatic);
        }
        let index = value
            .strip_prefix("Embedded:")
            .and_then(|index| index.parse::<u32>().ok());
        index
            .filter(|index| format!("Embedded:{index}") == value)
            .map(Self::Embedded)
            .ok_or_else(|| "Invalid embedded audio track".into())
    }

    #[cfg(feature = "video-files")]
    pub(crate) fn index(self) -> Option<usize> {
        match self {
            Self::Automatic => None,
            Self::Embedded(index) => Some(index as usize),
        }
    }
}

pub(crate) fn default_options() -> ParameterEnumOptions {
    crate::parameters::enum_options(&[AUTOMATIC]).into()
}

#[cfg(feature = "video-files")]
pub(super) fn track_option(index: usize, label: String) -> ParameterEnumOption {
    let key = format!("Embedded:{index}");
    ParameterEnumOption {
        variant_id: key.clone(),
        value: ParamValue::Enum(key),
        label,
        tags: Vec::new(),
        ordering: None,
    }
}

struct Tracks {
    selection: AudioTrack,
    options: ParameterEnumOptions,
    waker: Option<NodeWaker>,
}

impl Default for Tracks {
    fn default() -> Self {
        Self {
            selection: AudioTrack::Automatic,
            options: default_options(),
            waker: None,
        }
    }
}

#[derive(Default)]
pub(crate) struct ChannelAudio {
    pub(crate) voice: voice::Binding,
    tracks: Mutex<Tracks>,
}

impl ChannelAudio {
    pub(crate) fn watch_options(&self, waker: Option<NodeWaker>) {
        self.voice.watch_options(waker.clone());
        self.tracks.lock().unwrap().waker = waker;
    }

    pub(crate) fn set_selection(&self, track: AudioTrack) {
        self.tracks.lock().unwrap().selection = track;
    }

    #[cfg(feature = "video-files")]
    pub(super) fn selection(&self) -> AudioTrack {
        self.tracks.lock().unwrap().selection
    }

    pub(crate) fn track_options(&self, authored: &str) -> ParameterEnumOptions {
        voice::option_set(&self.tracks.lock().unwrap().options, authored)
    }

    pub(super) fn set_options(&self, options: ParameterEnumOptions) {
        let mut tracks = self.tracks.lock().unwrap();
        if tracks.options == options {
            return;
        }
        tracks.options = options;
        let waker = tracks.waker.clone();
        drop(tracks);
        if let Some(waker) = waker {
            let _ = waker.wake();
        }
    }
}
