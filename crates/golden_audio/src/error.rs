use std::{collections::BTreeMap, fmt};

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "codegen", derive(ts_rs::TS))]
#[cfg_attr(feature = "codegen", ts(export))]
#[non_exhaustive]
pub enum AudioErrorCategory {
    InvalidConfiguration,
    UnsupportedFormat,
    BackendUnavailable,
    DeviceMissing,
    DeviceBusy,
    PermissionDenied,
    StreamNegotiationFailed,
    QueueFull,
    CapacityExceeded,
    DecodeFailed,
    PlaybackNotFound,
    ShuttingDown,
    InternalInvariant,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(feature = "codegen", derive(ts_rs::TS))]
#[cfg_attr(feature = "codegen", ts(export))]
pub struct AudioError {
    pub category: AudioErrorCategory,
    pub message: String,
    pub context: BTreeMap<String, String>,
}

impl AudioError {
    #[must_use]
    pub fn new(category: AudioErrorCategory, message: impl Into<String>) -> Self {
        Self {
            category,
            message: message.into(),
            context: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn with_context(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.context.insert(key.into(), value.into());
        self
    }

    #[must_use]
    pub fn invalid_configuration(message: impl Into<String>) -> Self {
        Self::new(AudioErrorCategory::InvalidConfiguration, message)
    }

    #[must_use]
    pub fn capacity_exceeded(message: impl Into<String>) -> Self {
        Self::new(AudioErrorCategory::CapacityExceeded, message)
    }

    #[must_use]
    pub fn queue_full(queue: &'static str) -> Self {
        Self::new(
            AudioErrorCategory::QueueFull,
            format!("{queue} queue is at its configured capacity"),
        )
        .with_context("queue", queue)
    }

    #[must_use]
    pub fn shutting_down() -> Self {
        Self::new(AudioErrorCategory::ShuttingDown, "the audio engine is shutting down")
    }
}

impl fmt::Display for AudioError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.message.as_str())
    }
}

impl std::error::Error for AudioError {}
