use std::cell::Cell;
use std::collections::VecDeque;
use std::sync::{LazyLock, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::node::NodeId;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Topic emitted through UI custom events when a new logger record arrives.
pub const UI_LOG_RECORD_TOPIC: &str = "__logger.record";
/// Topic emitted through UI custom events when logger records are cleared.
pub const UI_LOG_CLEARED_TOPIC: &str = "__logger.cleared";
/// Topic emitted through UI custom events when logger capacity changes.
pub const UI_LOG_MAX_ENTRIES_TOPIC: &str = "__logger.max_entries";

const DEFAULT_LOG_MAX_ENTRIES: usize = 1024;

fn is_default_repeat_count(value: &u32) -> bool {
    *value <= 1
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
    fn label(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Success => "success",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }
}

/// One logger entry stored and streamed to the UI.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct LogRecord {
    /// Monotonic record id.
    pub id: u64,
    /// Wall-clock timestamp in unix milliseconds.
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

fn default_repeat_count() -> u32 {
    1
}

#[derive(Default)]
struct LoggerState {
    next_id: u64,
    max_entries: usize,
    retained: VecDeque<LogRecord>,
    pending: VecDeque<LogRecord>,
}

impl LoggerState {
    fn with_defaults() -> Self {
        Self {
            next_id: 1,
            max_entries: DEFAULT_LOG_MAX_ENTRIES,
            retained: VecDeque::new(),
            pending: VecDeque::new(),
        }
    }

    fn trim_to_capacity(&mut self) {
        while self.retained.len() > self.max_entries {
            let Some(removed) = self.retained.pop_front() else {
                break;
            };

            while self.pending.front().is_some_and(|pending| pending.id <= removed.id) {
                self.pending.pop_front();
            }
        }
    }
}

static LOGGER_STATE: LazyLock<Mutex<LoggerState>> = LazyLock::new(|| Mutex::new(LoggerState::with_defaults()));

thread_local! {
    static CURRENT_NODE_ORIGIN: Cell<Option<NodeId>> = const { Cell::new(None) };
}

/// Runs `callback` while setting the implicit logger node origin for this thread.
pub fn with_node_origin<R>(origin: NodeId, callback: impl FnOnce() -> R) -> R {
    CURRENT_NODE_ORIGIN.with(|slot| {
        let previous = slot.replace(Some(origin));
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(callback));
        slot.set(previous);
        match result {
            Ok(value) => value,
            Err(panic) => std::panic::resume_unwind(panic),
        }
    })
}

/// Returns the implicit logger node origin for this thread, when set.
pub fn current_node_origin() -> Option<NodeId> {
    CURRENT_NODE_ORIGIN.with(Cell::get)
}

/// Returns the current logger max-entry capacity.
pub fn max_entries() -> usize {
    let state = lock_logger_state();
    state.max_entries
}

/// Sets logger max-entry capacity and returns the applied value.
pub fn set_max_entries(max_entries: usize) -> usize {
    let mut state = lock_logger_state();
    state.max_entries = max_entries.max(1);
    state.trim_to_capacity();
    state.max_entries
}

/// Clears all retained and pending logger entries.
pub fn clear() {
    let mut state = lock_logger_state();
    state.retained.clear();
    state.pending.clear();
}

/// Returns a snapshot clone of currently retained logger records.
pub fn records() -> Vec<LogRecord> {
    let state = lock_logger_state();
    state.retained.iter().cloned().collect()
}

/// Returns retained logger records newer than the provided sync cursor.
///
/// When the latest retained record was collapsed in place, the same `id` may be
/// returned again with a higher `repeat_count`.
pub fn records_since_cursor(last_id: u64, last_repeat_count: u32) -> Vec<LogRecord> {
    let state = lock_logger_state();
    state
        .retained
        .iter()
        .filter(|record| record.id > last_id || (record.id == last_id && record.repeat_count > last_repeat_count))
        .cloned()
        .collect()
}

/// Drains pending records that have not yet been streamed to the UI.
pub fn drain_pending() -> Vec<LogRecord> {
    let mut state = lock_logger_state();
    state.pending.drain(..).collect()
}

/// Pushes one logger message assembled from already rendered parts.
pub fn log_parts(level: LogLevel, tag: String, origin: Option<NodeId>, parts: Vec<String>) -> LogRecord {
    let mut message = String::new();
    for (index, part) in parts.into_iter().enumerate() {
        if index > 0 {
            message.push('\n');
        }
        message.push_str(&part);
    }

    log_message(level, tag, origin, message)
}

/// Pushes one logger message.
pub fn log_message(level: LogLevel, tag: String, origin: Option<NodeId>, message: String) -> LogRecord {
    let resolved_origin = origin.or_else(current_node_origin);
    print_process_output(level, &tag, resolved_origin, &message);
    let timestamp_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0);

    let mut state = lock_logger_state();
    if let Some(last) = state.retained.back_mut() {
        if last.level == level && last.tag == tag && last.message == message && last.origin == resolved_origin {
            last.timestamp_ms = timestamp_ms;
            last.repeat_count = last.repeat_count.saturating_add(1);
            let updated = last.clone();

            if let Some(pending_tail) = state.pending.back_mut() {
                if pending_tail.id == updated.id {
                    *pending_tail = updated.clone();
                } else {
                    state.pending.push_back(updated.clone());
                }
            } else {
                state.pending.push_back(updated.clone());
            }

            return updated;
        }
    }

    let record = LogRecord {
        id: state.next_id,
        timestamp_ms,
        level,
        tag,
        message,
        repeat_count: 1,
        origin: resolved_origin,
    };
    state.next_id = state.next_id.saturating_add(1);
    state.retained.push_back(record.clone());
    state.pending.push_back(record.clone());
    state.trim_to_capacity();

    record
}

fn process_output_prefix(level: LogLevel, tag: &str, origin: Option<NodeId>) -> String {
    match origin {
        Some(node) => format!("[golden][{}][{}][node={}]", level.label(), tag, node.0),
        None => format!("[golden][{}][{}]", level.label(), tag),
    }
}

fn print_process_output(level: LogLevel, tag: &str, origin: Option<NodeId>, message: &str) {
    let prefix = process_output_prefix(level, tag, origin);
    let mut emitted = false;
    for line in message.lines() {
        println!("{prefix} {line}");
        emitted = true;
    }

    if !emitted {
        println!("{prefix}");
    }
}

fn lock_logger_state() -> std::sync::MutexGuard<'static, LoggerState> {
    match LOGGER_STATE.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

#[doc(hidden)]
#[macro_export]
macro_rules! __golden_log_level {
    (info) => {
        $crate::logger::LogLevel::Info
    };
    (success) => {
        $crate::logger::LogLevel::Success
    };
    (warning) => {
        $crate::logger::LogLevel::Warning
    };
    (error) => {
        $crate::logger::LogLevel::Error
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __golden_log_emit {
    ($level:expr, $tag:expr, $origin:expr; $($msg:expr),+ $(,)?) => {{
        $crate::logger::log_parts(
            $level,
            ($tag).to_string(),
            $origin,
            vec![$(format!("{}", $msg)),+],
        )
    }};
}

/// Logs one record with optional `level`, `tag`, and `origin` options.
///
/// # Examples
/// ```rust
/// use golden_engine::log;
///
/// log!("hello", 123);
/// log!(tag = "transport", level = warning; "socket", "late");
/// ```
#[macro_export]
macro_rules! log {
    ($($msg:expr),+ $(,)?) => {{
        $crate::__golden_log_emit!(
            $crate::logger::LogLevel::Info,
            "general",
            None::<$crate::node::NodeId>;
            $($msg),+
        )
    }};
    (level = $level:ident ; $($msg:expr),+ $(,)?) => {{
        $crate::__golden_log_emit!(
            $crate::__golden_log_level!($level),
            "general",
            None::<$crate::node::NodeId>;
            $($msg),+
        )
    }};
    (tag = $tag:expr ; $($msg:expr),+ $(,)?) => {{
        $crate::__golden_log_emit!(
            $crate::logger::LogLevel::Info,
            $tag,
            None::<$crate::node::NodeId>;
            $($msg),+
        )
    }};
    (origin = $origin:expr ; $($msg:expr),+ $(,)?) => {{
        $crate::__golden_log_emit!(
            $crate::logger::LogLevel::Info,
            "general",
            Some($origin);
            $($msg),+
        )
    }};
    (level = $level:ident, tag = $tag:expr ; $($msg:expr),+ $(,)?) => {{
        $crate::__golden_log_emit!($crate::__golden_log_level!($level), $tag, None::<$crate::node::NodeId>; $($msg),+)
    }};
    (tag = $tag:expr, level = $level:ident ; $($msg:expr),+ $(,)?) => {{
        $crate::__golden_log_emit!($crate::__golden_log_level!($level), $tag, None::<$crate::node::NodeId>; $($msg),+)
    }};
    (level = $level:ident, origin = $origin:expr ; $($msg:expr),+ $(,)?) => {{
        $crate::__golden_log_emit!($crate::__golden_log_level!($level), "general", Some($origin); $($msg),+)
    }};
    (origin = $origin:expr, level = $level:ident ; $($msg:expr),+ $(,)?) => {{
        $crate::__golden_log_emit!($crate::__golden_log_level!($level), "general", Some($origin); $($msg),+)
    }};
    (tag = $tag:expr, origin = $origin:expr ; $($msg:expr),+ $(,)?) => {{
        $crate::__golden_log_emit!($crate::logger::LogLevel::Info, $tag, Some($origin); $($msg),+)
    }};
    (origin = $origin:expr, tag = $tag:expr ; $($msg:expr),+ $(,)?) => {{
        $crate::__golden_log_emit!($crate::logger::LogLevel::Info, $tag, Some($origin); $($msg),+)
    }};
    (level = $level:ident, tag = $tag:expr, origin = $origin:expr ; $($msg:expr),+ $(,)?) => {{
        $crate::__golden_log_emit!($crate::__golden_log_level!($level), $tag, Some($origin); $($msg),+)
    }};
    (level = $level:ident, origin = $origin:expr, tag = $tag:expr ; $($msg:expr),+ $(,)?) => {{
        $crate::__golden_log_emit!($crate::__golden_log_level!($level), $tag, Some($origin); $($msg),+)
    }};
    (tag = $tag:expr, level = $level:ident, origin = $origin:expr ; $($msg:expr),+ $(,)?) => {{
        $crate::__golden_log_emit!($crate::__golden_log_level!($level), $tag, Some($origin); $($msg),+)
    }};
    (tag = $tag:expr, origin = $origin:expr, level = $level:ident ; $($msg:expr),+ $(,)?) => {{
        $crate::__golden_log_emit!($crate::__golden_log_level!($level), $tag, Some($origin); $($msg),+)
    }};
    (origin = $origin:expr, level = $level:ident, tag = $tag:expr ; $($msg:expr),+ $(,)?) => {{
        $crate::__golden_log_emit!($crate::__golden_log_level!($level), $tag, Some($origin); $($msg),+)
    }};
    (origin = $origin:expr, tag = $tag:expr, level = $level:ident ; $($msg:expr),+ $(,)?) => {{
        $crate::__golden_log_emit!($crate::__golden_log_level!($level), $tag, Some($origin); $($msg),+)
    }};
}

#[cfg(test)]
mod tests;
