use super::{WatchedAppEntry, WatchedFolderEntry};
use crate::app::module::{
    common::app_control::{
        CommandTargetSource, KillProcessRequest, LaunchMode, LaunchProcessRequest, ProcessMatchMode,
        WindowControlRequest,
    },
    script_api::NativeCallbackPayload,
};
use golden_io::{
    BoundedWorker, PayloadPool, RetainedPayload, RetirementPool, RevisionGuard, RevisionSource, WorkerLimits,
    WorkerNotifier,
};
use std::{
    num::NonZeroUsize,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, OnceLock,
    },
};
mod input;
mod native;
mod outcomes;
#[cfg(test)]
pub(super) mod tests;
mod worker;
pub(super) use input::{Input, Request, Source};
use outcomes::Work;
pub(super) use outcomes::{Outcome, Receipt};
const COMMAND_WEIGHT: usize = 64 * 1024;
const REPORT_WEIGHT: usize = 2 * 1024 * 1024;
const CONFIG_WEIGHT: usize = 1024 * 1024;

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct WatchedAppMetrics {
    pub target_path: String,
    pub target_name: String,
    pub exists: bool,
    pub running: bool,
    pub uptime_seconds: f64,
    pub process_count: i32,
    pub main_pid: i32,
    pub window_opened: bool,
    pub window_count: i32,
    pub cpu_ratio: f64,
    pub memory_mb: f64,
    pub memory_max_mb: f64,
    pub virtual_memory_mb: f64,
}

impl WatchedAppMetrics {
    pub(super) fn idle(target_path: &str) -> Self {
        let trimmed_path = target_path.trim();
        let path = Path::new(trimmed_path);

        Self {
            target_path: trimmed_path.to_string(),
            target_name: path
                .file_stem()
                .or_else(|| path.file_name())
                .map(|value| value.to_string_lossy().to_string())
                .unwrap_or_default(),
            exists: false,
            running: false,
            uptime_seconds: 0.0,
            process_count: 0,
            main_pid: 0,
            window_opened: false,
            window_count: 0,
            cpu_ratio: 0.0,
            memory_mb: 0.0,
            memory_max_mb: 0.0,
            virtual_memory_mb: 0.0,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct FolderWatchUpdate {
    pub path: String,
    pub exists: bool,
    pub entry_count: i32,
    pub created: Vec<String>,
    pub modified: Vec<String>,
    pub removed: Vec<String>,
    pub timestamp_ms: u64,
    pub last_change_ms: u64,
}

impl FolderWatchUpdate {
    pub(crate) fn has_changes(&self) -> bool {
        !self.created.is_empty() || !self.modified.is_empty() || !self.removed.is_empty()
    }

    pub(crate) fn last_event_kind(&self) -> String {
        if !self.created.is_empty() {
            return "created".to_string();
        }
        if !self.modified.is_empty() {
            return "modified".to_string();
        }
        if !self.removed.is_empty() {
            return "removed".to_string();
        }
        String::new()
    }

    pub(crate) fn last_event_path(&self) -> String {
        self.created
            .first()
            .or_else(|| self.modified.first())
            .or_else(|| self.removed.first())
            .cloned()
            .unwrap_or_default()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PathKind {
    Empty,
    Missing,
    File,
    Directory,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Config {
    pub(super) apps: Vec<WatchedAppEntry>,
    pub(super) folders: Vec<WatchedFolderEntry>,
}
pub(super) struct AppObservation {
    pub(super) metrics: WatchedAppMetrics,
    pub(super) kind: PathKind,
}
pub(super) struct FolderObservation {
    pub(super) update: FolderWatchUpdate,
    pub(super) kind: PathKind,
}
pub(super) struct Observation {
    pub(super) apps: Vec<AppObservation>,
    pub(super) folders: Vec<FolderObservation>,
}
pub(super) struct Report {
    pub(super) config: RetainedPayload<Config>,
    pub(super) rows: Observation,
    pub(super) callbacks: Vec<Option<NativeCallbackPayload>>,
    pub(super) fence: Fence,
}
#[derive(Clone)]
pub(super) struct Fence {
    actor: RevisionGuard,
    native: RevisionGuard,
    config: RevisionGuard,
}
impl Fence {
    pub(super) fn current(&self) -> bool {
        self.actor.is_current() && self.native.is_current() && self.config.is_current()
    }
    pub(super) fn guards(&self) -> Vec<RevisionGuard> {
        vec![self.actor.clone(), self.native.clone(), self.config.clone()]
    }
}
pub(super) struct Execution {
    pub(super) attempted: bool,
    pub(super) method: &'static str,
    pub(super) details: serde_json::Value,
}
pub(super) trait Provider: Send + 'static {
    fn sample(&mut self, config: &Config) -> Result<Observation, String>;
    fn execute(&mut self, request: &Request) -> Result<Execution, String>;
}
pub(super) enum Event {
    Ready,
    Warning(String),
    Error(String),
    Report(RetainedPayload<Report>),
}
#[derive(Clone)]
struct Configuration {
    value: RetainedPayload<Config>,
    fence: Fence,
}
struct Shared {
    configuration: Mutex<Option<Configuration>>,
    running: AtomicBool,
    wake: WorkerNotifier,
}
pub(super) struct AppControlRuntime {
    worker: BoundedWorker<Work, Event>,
    shared: Arc<Shared>,
    actor: Option<RevisionSource>,
    generation: RevisionSource,
    native: RevisionGuard,
    configuration: Option<RetainedPayload<Config>>,
    commands: PayloadPool,
}
fn slots() -> &'static RetirementPool {
    static POOL: OnceLock<RetirementPool> = OnceLock::new();
    POOL.get_or_init(|| RetirementPool::new(NonZeroUsize::new(16).unwrap()))
}
fn pool(which: usize) -> Result<PayloadPool, String> {
    static POOLS: [OnceLock<Mutex<Option<PayloadPool>>>; 3] = [const { OnceLock::new() }; 3];
    let mut cell = POOLS[which]
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    if cell.is_none() {
        *cell = Some(
            PayloadPool::new(
                ["app-control-config", "app-control-reports", "app-control-commands"][which],
                [64, 32, 512][which],
                [64, 64, 32][which] * 1024 * 1024,
            )
            .map_err(|error| error.to_string())?,
        );
    }
    Ok(cell.as_ref().unwrap().clone())
}
impl AppControlRuntime {
    pub(super) fn spawn(wake: WorkerNotifier) -> Result<Self, String> {
        Self::spawn_using(wake, slots(), pool(1)?, pool(2)?, native::Native::open)
    }
    pub(super) fn spawn_using<P: Provider>(
        wake: WorkerNotifier,
        slots: &RetirementPool,
        reports: PayloadPool,
        commands: PayloadPool,
        open: impl FnOnce() -> Result<P, String> + Send + 'static,
    ) -> Result<Self, String> {
        let shared = Arc::new(Shared {
            configuration: Mutex::new(None),
            running: AtomicBool::new(true),
            wake: wake.clone(),
        });
        let actor = RevisionSource::default();
        let native = RevisionSource::default();
        let generation = RevisionSource::default();
        let native_guard = native.reader().guard_at(0);
        let state = shared.clone();
        let output = commands.clone();
        let worker = BoundedWorker::spawn_notified(
            "app-control",
            slots,
            WorkerLimits {
                command_items: 64,
                command_weight: 4 * 1024 * 1024,
                event_items: 8,
                event_weight: 8 * 1024 * 1024,
            },
            wake,
            move |ctx| worker::run(ctx, state, native, reports, output, open),
        )
        .map_err(|error| error.to_string())?;
        Ok(Self {
            worker,
            shared,
            actor: Some(actor),
            generation,
            native: native_guard,
            configuration: None,
            commands,
        })
    }
    pub(super) fn configure(&mut self, config: Config) -> Result<RetainedPayload<Config>, String> {
        if let Some(current) = &self.configuration {
            if **current == config {
                return Ok(current.clone());
            }
        }
        input::validate_config(&config)?;
        let value = pool(0)?
            .try_retain(config, CONFIG_WEIGHT)
            .map_err(|_| "App Control configuration capacity exhausted")?;
        let version = self.generation.advance();
        let fence = Fence {
            actor: self
                .actor
                .as_ref()
                .ok_or("App Control runtime retired")?
                .reader()
                .guard_at(0),
            native: self.native.clone(),
            config: self.generation.reader().guard_at(version),
        };
        *self
            .shared
            .configuration
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some(Configuration {
            value: value.clone(),
            fence,
        });
        self.configuration = Some(value.clone());
        Ok(value)
    }
    pub(super) fn capture(
        &self,
        input: Input,
        producer: golden_core::engine::NodeWaker,
        time: golden_core::engine::EngineTime,
        permit: golden_io::RetirementPermit,
    ) -> Result<RetainedPayload<Source>, String> {
        let actor = self.actor.as_ref().ok_or("App Control runtime retired")?;
        let source = Source {
            input,
            producer,
            time,
            fence: Fence {
                actor: actor.reader().guard_at(0),
                native: self.native.clone(),
                config: self.generation.reader().guard_at(self.generation.reader().current()),
            },
            _permit: permit,
        };
        let weight = source.weight();
        if weight > 4 * 1024 * 1024 {
            return Err("App Control source exceeds native mailbox byte capacity; request was not admitted".into());
        }
        let reservation = self
            .commands
            .try_reserve(weight)
            .map_err(|_| "App Control source capacity exhausted")?;
        reservation
            .retain(source, weight)
            .map_err(|_| "App Control source reservation failed".into())
    }
    pub(super) fn submit(&self, source: RetainedPayload<Source>) -> Result<Receipt, RetainedPayload<Source>> {
        let weight = source.weight();
        let (work, receipt) = Work::new(source.clone(), self.shared.wake.clone());
        self.worker.send_weighted(work, weight).map_err(|error| {
            let (golden_io::PendingSendError::Full(work) | golden_io::PendingSendError::Disconnected(work)) = error;
            work.unadmitted();
            source
        })?;
        Ok(receipt)
    }
    pub(super) fn running(&self) -> bool {
        self.shared.running.load(Ordering::Acquire)
    }
    pub(super) fn pending(&self) -> bool {
        self.worker.has_pending() || !self.running()
    }
    pub(super) fn event(&self) -> Option<Event> {
        let mut values = Vec::with_capacity(1);
        self.worker.drain_events(&mut values, NonZeroUsize::MIN);
        values.pop()
    }
    pub(super) fn stop(&mut self) {
        self.actor = None;
        self.worker.cancel();
    }
}
impl Drop for AppControlRuntime {
    fn drop(&mut self) {
        self.stop();
    }
}
fn diagnostic(value: String) -> String {
    let mut end = value.len().min(1024);
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end].to_owned()
}
