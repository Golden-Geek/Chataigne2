use std::cell::Cell;
use std::collections::VecDeque;
use std::io::{self, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError, TrySendError, sync_channel};
use std::sync::{Arc, LazyLock, Mutex};
use std::thread;
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
const PROCESS_OUTPUT_QUEUE_CAPACITY: usize = 4_096;
const PROCESS_OUTPUT_WRITE_BATCH_SIZE: usize = 256;

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

/// One already-rendered message to append through [`log_messages`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogMessage {
    /// Severity level.
    pub level: LogLevel,
    /// Free-form log tag.
    pub tag: String,
    /// Optional node origin.
    pub origin: Option<NodeId>,
    /// Final rendered message.
    pub message: String,
}

impl LogMessage {
    /// Creates one message for a batched logger append.
    pub fn new(level: LogLevel, tag: String, origin: Option<NodeId>, message: String) -> Self {
        Self {
            level,
            tag,
            origin,
            message,
        }
    }
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

    fn push_message(
        &mut self,
        timestamp_ms: u64,
        level: LogLevel,
        tag: String,
        origin: Option<NodeId>,
        message: String,
    ) -> LogRecord {
        if let Some(last) = self.retained.back_mut() {
            if last.level == level && last.tag == tag && last.message == message && last.origin == origin {
                last.timestamp_ms = timestamp_ms;
                last.repeat_count = last.repeat_count.saturating_add(1);
                let updated = last.clone();

                if let Some(pending_tail) = self.pending.back_mut() {
                    if pending_tail.id == updated.id {
                        *pending_tail = updated.clone();
                    } else {
                        self.pending.push_back(updated.clone());
                    }
                } else {
                    self.pending.push_back(updated.clone());
                }

                return updated;
            }
        }

        let record = LogRecord {
            id: self.next_id,
            timestamp_ms,
            level,
            tag,
            message,
            repeat_count: 1,
            origin,
        };
        self.next_id = self.next_id.saturating_add(1);
        self.retained.push_back(record.clone());
        self.pending.push_back(record.clone());
        self.trim_to_capacity();

        record
    }
}

static LOGGER_STATE: LazyLock<Mutex<LoggerState>> = LazyLock::new(|| Mutex::new(LoggerState::with_defaults()));
static PROCESS_OUTPUT_SINK: LazyLock<ProcessOutputSink> = LazyLock::new(ProcessOutputSink::spawn);

#[cfg(test)]
static LOGGER_TEST_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

#[cfg(test)]
pub(crate) fn test_lock() -> std::sync::MutexGuard<'static, ()> {
    match LOGGER_TEST_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

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
///
/// Retained state and pending UI delivery are updated synchronously. Process
/// stdout is best-effort through a bounded background queue so a slow output
/// consumer cannot stall the caller or grow memory without bound.
pub fn log_message(level: LogLevel, tag: String, origin: Option<NodeId>, message: String) -> LogRecord {
    let resolved_origin = origin.or_else(current_node_origin);
    let timestamp_ms = unix_timestamp_ms();

    let record = lock_logger_state().push_message(timestamp_ms, level, tag, resolved_origin, message);
    PROCESS_OUTPUT_SINK.enqueue(ProcessOutputMessage::from_record(&record));
    record
}

/// Pushes a batch of already-rendered messages.
///
/// The logger resolves the implicit node origin and timestamp once for the
/// complete batch, while preserving input order, duplicate collapsing, pending
/// UI updates, and retained-log capacity semantics. As with [`log_message`],
/// process stdout is best-effort and never blocks the caller.
pub fn log_messages(messages: impl IntoIterator<Item = LogMessage>) -> Vec<LogRecord> {
    let messages = messages.into_iter().collect::<Vec<_>>();
    if messages.is_empty() {
        return Vec::new();
    }

    let resolved_origin = current_node_origin();
    let timestamp_ms = unix_timestamp_ms();
    let mut state = lock_logger_state();
    let records = messages
        .into_iter()
        .map(|message| {
            state.push_message(
                timestamp_ms,
                message.level,
                message.tag,
                message.origin.or(resolved_origin),
                message.message,
            )
        })
        .collect::<Vec<_>>();
    drop(state);

    for record in &records {
        PROCESS_OUTPUT_SINK.enqueue(ProcessOutputMessage::from_record(record));
    }
    records
}

fn unix_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

#[derive(Debug)]
struct ProcessOutputMessage {
    level: LogLevel,
    tag: String,
    origin: Option<NodeId>,
    message: String,
}

impl ProcessOutputMessage {
    fn from_record(record: &LogRecord) -> Self {
        Self {
            level: record.level,
            tag: record.tag.clone(),
            origin: record.origin,
            message: record.message.clone(),
        }
    }
}

struct ProcessOutputSink {
    sender: Option<SyncSender<ProcessOutputMessage>>,
    dropped: Arc<AtomicU64>,
}

impl ProcessOutputSink {
    fn spawn() -> Self {
        let (sender, receiver) = sync_channel(PROCESS_OUTPUT_QUEUE_CAPACITY);
        let dropped = Arc::new(AtomicU64::new(0));
        let worker_dropped = Arc::clone(&dropped);
        let sender = thread::Builder::new()
            .name("golden-log-output".to_string())
            .spawn(move || process_output_worker(receiver, worker_dropped))
            .ok()
            .map(|_| sender);

        Self { sender, dropped }
    }

    fn enqueue(&self, message: ProcessOutputMessage) {
        let Some(sender) = self.sender.as_ref() else {
            return;
        };

        match sender.try_send(message) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => {
                self.dropped.fetch_add(1, Ordering::Relaxed);
            }
            Err(TrySendError::Disconnected(_)) => {}
        }
    }
}

fn process_output_worker(receiver: Receiver<ProcessOutputMessage>, dropped: Arc<AtomicU64>) {
    while let Ok(first) = receiver.recv() {
        let mut batch = Vec::with_capacity(PROCESS_OUTPUT_WRITE_BATCH_SIZE);
        batch.push(first);
        while batch.len() < PROCESS_OUTPUT_WRITE_BATCH_SIZE {
            match receiver.try_recv() {
                Ok(message) => batch.push(message),
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
            }
        }

        let stdout = io::stdout();
        let mut output = stdout.lock();
        for message in batch {
            let _ = write_process_output(
                &mut output,
                message.level,
                &message.tag,
                message.origin,
                &message.message,
            );
        }
        let dropped_count = dropped.swap(0, Ordering::Relaxed);
        if dropped_count > 0 {
            let _ = writeln!(
                output,
                "[golden][warning][logger] omitted {dropped_count} process-output messages because stdout could not keep up"
            );
        }
        let _ = output.flush();
    }
}

fn process_output_prefix(level: LogLevel, tag: &str, origin: Option<NodeId>) -> String {
    match origin {
        Some(node) => format!("[golden][{}][{}][node={}]", level.label(), tag, node.0),
        None => format!("[golden][{}][{}]", level.label(), tag),
    }
}

fn write_process_output(
    output: &mut impl Write,
    level: LogLevel,
    tag: &str,
    origin: Option<NodeId>,
    message: &str,
) -> io::Result<()> {
    let prefix = process_output_prefix(level, tag, origin);
    let mut emitted = false;
    for line in message.lines() {
        writeln!(output, "{prefix} {line}")?;
        emitted = true;
    }

    if !emitted {
        writeln!(output, "{prefix}")?;
    }

    Ok(())
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

#[doc(hidden)]
#[macro_export]
macro_rules! __golden_log_with_fixed_level {
    ($level:ident; $($msg:expr),+ $(,)?) => {{
        $crate::log!(level = $level; $($msg),+)
    }};
    ($level:ident; tag = $tag:expr ; $($msg:expr),+ $(,)?) => {{
        $crate::log!(level = $level, tag = $tag; $($msg),+)
    }};
    ($level:ident; origin = $origin:expr ; $($msg:expr),+ $(,)?) => {{
        $crate::log!(level = $level, origin = $origin; $($msg),+)
    }};
    ($level:ident; tag = $tag:expr, origin = $origin:expr ; $($msg:expr),+ $(,)?) => {{
        $crate::log!(level = $level, tag = $tag, origin = $origin; $($msg),+)
    }};
    ($level:ident; origin = $origin:expr, tag = $tag:expr ; $($msg:expr),+ $(,)?) => {{
        $crate::log!(level = $level, origin = $origin, tag = $tag; $($msg),+)
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

/// Logs one warning record with optional `tag` and `origin` options.
///
/// # Examples
/// ```rust
/// use golden_engine::logwarning;
///
/// logwarning!("socket is slow");
/// logwarning!(tag = "transport", origin = golden_engine::node::NodeId(7); "socket", "late");
/// ```
#[macro_export]
macro_rules! logwarning {
    ($($tt:tt)*) => {{
        $crate::__golden_log_with_fixed_level!(warning; $($tt)*)
    }};
}

/// Logs one error record with optional `tag` and `origin` options.
///
/// # Examples
/// ```rust
/// use golden_engine::logerror;
///
/// logerror!("transport failed");
/// logerror!(tag = "transport"; "socket", "closed");
/// ```
#[macro_export]
macro_rules! logerror {
    ($($tt:tt)*) => {{
        $crate::__golden_log_with_fixed_level!(error; $($tt)*)
    }};
}

/// Logs one success record with optional `tag` and `origin` options.
///
/// # Examples
/// ```rust
/// use golden_engine::logsuccess;
///
/// logsuccess!("transport ready");
/// logsuccess!(origin = golden_engine::node::NodeId(7); "started");
/// ```
#[macro_export]
macro_rules! logsuccess {
    ($($tt:tt)*) => {{
        $crate::__golden_log_with_fixed_level!(success; $($tt)*)
    }};
}

#[cfg(test)]
mod tests;
