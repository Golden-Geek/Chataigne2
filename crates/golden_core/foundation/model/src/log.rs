use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::NodeId;

fn is_default_repeat_count(value: &u32) -> bool {
    *value <= 1
}

fn default_repeat_count() -> u32 {
    1
}

/// Severity level for a logger record.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    /// Informational message.
    Info,
    /// Success message.
    Success,
    /// Warning message.
    Warning,
    /// Error message.
    Error,
}

impl LogLevel {
    /// Stable lowercase label used by text sinks.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Success => "success",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }
}

/// One logger entry stored and streamed to clients.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct LogRecord {
    /// Monotonic record id.
    pub id: u64,
    /// Wall-clock timestamp in Unix milliseconds.
    pub timestamp_ms: u64,
    /// Severity level.
    pub level: LogLevel,
    /// Free-form log tag.
    pub tag: String,
    /// Final rendered message.
    pub message: String,
    /// Number of consecutive identical messages represented by this record.
    #[serde(default = "default_repeat_count", skip_serializing_if = "is_default_repeat_count")]
    pub repeat_count: u32,
    /// Optional node origin.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<NodeId>,
}
