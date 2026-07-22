use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    path::Path,
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, MutexGuard,
    },
    thread::{self, JoinHandle},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
use std::ptr;

use sysinfo::{Process, ProcessesToUpdate, System};

#[cfg(windows)]
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, RECT},
    UI::WindowsAndMessaging::{
        EnumWindows, GetWindowRect, GetWindowTextLengthW, GetWindowTextW,
        GetWindowThreadProcessId, IsWindowVisible, SetWindowPos, ShowWindow,
        HWND_NOTOPMOST, HWND_TOPMOST, SW_HIDE, SW_MAXIMIZE, SW_MINIMIZE, SW_RESTORE,
        SW_SHOW, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER,
    },
};
use crate::app::module::common::app_control::{
    CommandTargetSource, KillProcessRequest, LaunchMode, LaunchProcessRequest,
    ProcessMatchMode, WindowAction, WindowControlRequest,
};
use crate::app::module::common::system_metrics;

const WATCHED_APP_WORKER_INTERVAL: Duration = Duration::from_millis(500);

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LaunchExecution {
    pub effective_program: String,
}

#[derive(Clone, Debug, PartialEq)]
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
    fn idle(target_path: &str) -> Self {
        let trimmed_path = target_path.trim();
        let path = Path::new(trimmed_path);

        Self {
            target_path: trimmed_path.to_string(),
            target_name: path
                .file_stem()
                .or_else(|| path.file_name())
                .map(|value| value.to_string_lossy().to_string())
                .unwrap_or_default(),
            exists: !trimmed_path.is_empty() && path.exists(),
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

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct FileStamp {
    is_dir: bool,
    len: u64,
    modified_ms: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct FolderTreeSnapshot {
    exists: bool,
    entries: BTreeMap<String, FileStamp>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct StoredFolderSnapshot {
    path: String,
    snapshot: FolderTreeSnapshot,
    last_change_ms: u64,
}

struct WatchedAppWorkerState {
    target_paths: Mutex<Vec<String>>,
    metrics_by_target: Mutex<HashMap<String, WatchedAppMetrics>>,
    stop_requested: AtomicBool,
}

struct WatchedAppMetricsWorker {
    state: Arc<WatchedAppWorkerState>,
    worker: Option<JoinHandle<()>>,
}

impl WatchedAppMetricsWorker {
    fn create() -> Self {
        let state = Arc::new(WatchedAppWorkerState {
            target_paths: Mutex::new(Vec::new()),
            metrics_by_target: Mutex::new(HashMap::new()),
            stop_requested: AtomicBool::new(false),
        });

        Self {
            state,
            worker: None,
        }
    }

    fn ensure_started(&mut self) {
        if self.worker.is_some() {
            return;
        }

        self.state.stop_requested.store(false, Ordering::Relaxed);
        let worker_state = Arc::clone(&self.state);
        self.worker = thread::Builder::new()
            .name("app-control-metrics".to_string())
            .spawn(move || watched_app_worker_loop(worker_state))
            .ok();
    }

    fn sync_targets<'a, I>(&mut self, target_paths: I)
    where
        I: IntoIterator<Item = &'a str>,
    {
        let target_paths = dedupe_watched_app_paths(target_paths);
        let has_targets = !target_paths.is_empty();
        let allowed_keys = target_paths
            .iter()
            .map(|path| watched_app_target_key(path.as_str()))
            .collect::<HashSet<_>>();

        {
            let mut metrics_by_target = lock_unpoisoned(&self.state.metrics_by_target);
            metrics_by_target.retain(|key, _| allowed_keys.contains(key));
            for target_path in &target_paths {
                metrics_by_target
                    .entry(watched_app_target_key(target_path.as_str()))
                    .or_insert_with(|| WatchedAppMetrics::idle(target_path.as_str()));
            }
        }

        *lock_unpoisoned(&self.state.target_paths) = target_paths;
        if has_targets {
            self.ensure_started();
        }
        if self.worker.is_some() {
            self.request_refresh();
        }
    }

    fn watched_app_metrics(&self, target_path: &str) -> WatchedAppMetrics {
        lock_unpoisoned(&self.state.metrics_by_target)
            .get(&watched_app_target_key(target_path))
            .cloned()
            .unwrap_or_else(|| WatchedAppMetrics::idle(target_path))
    }

    fn request_refresh(&self) {
        if let Some(worker) = &self.worker {
            worker.thread().unpark();
        }
    }

    fn reset(&mut self) {
        self.sync_targets(std::iter::empty::<&str>());
    }

    fn stop(&mut self) {
        self.state.stop_requested.store(true, Ordering::Relaxed);
        self.request_refresh();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Drop for WatchedAppMetricsWorker {
    fn drop(&mut self) {
        self.stop();
    }
}

pub(crate) struct AppControlRuntime {
    system: System,
    watched_app_worker: WatchedAppMetricsWorker,
    folder_snapshots: HashMap<String, StoredFolderSnapshot>,
}

impl AppControlRuntime {
    pub(crate) fn create() -> Self {
        Self {
            system: System::new(),
            watched_app_worker: WatchedAppMetricsWorker::create(),
            folder_snapshots: HashMap::new(),
        }
    }

    pub(crate) fn reset(&mut self) {
        self.watched_app_worker.reset();
        self.folder_snapshots.clear();
    }

    pub(crate) fn sync_watched_app_targets<'a, I>(&mut self, target_paths: I)
    where
        I: IntoIterator<Item = &'a str>,
    {
        self.watched_app_worker.sync_targets(target_paths);
    }

    pub(crate) fn sync_folder_keys<'a, I>(&mut self, keys: I)
    where
        I: IntoIterator<Item = &'a str>,
    {
        let allowed = keys
            .into_iter()
            .map(str::to_string)
            .collect::<HashSet<_>>();
        self.folder_snapshots
            .retain(|key, _| allowed.contains(key.as_str()));
    }

    pub(crate) fn refresh_processes(&mut self) {
        self.system.refresh_cpu_usage();
        self.system.refresh_memory();
        self.system.refresh_processes(ProcessesToUpdate::All, true);
    }

    pub(crate) fn watched_app_metrics(&self, target_path: &str) -> WatchedAppMetrics {
        self.watched_app_worker.watched_app_metrics(target_path)
    }

    pub(crate) fn poll_folder(&mut self, key: &str, path: &str) -> FolderWatchUpdate {
        let normalized_path = path.trim().to_string();
        let current = capture_folder_snapshot(normalized_path.as_str());
        let timestamp_ms = unix_time_ms();
        let previous = self.folder_snapshots.get(key).cloned();
        let mut update = FolderWatchUpdate {
            path: normalized_path.clone(),
            exists: current.exists,
            entry_count: saturating_i32(current.entries.len()),
            created: Vec::new(),
            modified: Vec::new(),
            removed: Vec::new(),
            timestamp_ms,
            last_change_ms: previous.as_ref().map(|snapshot| snapshot.last_change_ms).unwrap_or(0),
        };

        let Some(previous) = previous else {
            self.folder_snapshots.insert(
                key.to_string(),
                StoredFolderSnapshot {
                    path: normalized_path,
                    snapshot: current,
                    last_change_ms: update.last_change_ms,
                },
            );
            return update;
        };
        if previous.path != normalized_path {
            self.folder_snapshots.insert(
                key.to_string(),
                StoredFolderSnapshot {
                    path: normalized_path,
                    snapshot: current,
                    last_change_ms: 0,
                },
            );
            return update;
        }

        for (entry_path, stamp) in &current.entries {
            match previous.snapshot.entries.get(entry_path) {
                None => update.created.push(entry_path.clone()),
                Some(previous_stamp) if previous_stamp != stamp => {
                    update.modified.push(entry_path.clone())
                }
                Some(_) => {}
            }
        }

        for entry_path in previous.snapshot.entries.keys() {
            if !current.entries.contains_key(entry_path) {
                update.removed.push(entry_path.clone());
            }
        }

        if update.has_changes() {
            update.last_change_ms = timestamp_ms;
        }

        self.folder_snapshots.insert(
            key.to_string(),
            StoredFolderSnapshot {
                path: normalized_path,
                snapshot: current,
                last_change_ms: update.last_change_ms,
            },
        );

        update
    }

    pub(crate) fn execute_launch(
        &mut self,
        request: &LaunchProcessRequest,
        watched_target: Option<&str>,
    ) -> Result<LaunchExecution, String> {
        match request.mode {
            LaunchMode::WatchedApp => {
                let program = watched_target
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| "watched app launch target is empty".to_string())?;
                spawn_executable(program, request.arguments.as_str(), optional_non_empty(request.working_directory.as_str()))?;
                Ok(LaunchExecution {
                    effective_program: program.to_string(),
                })
            }
            LaunchMode::Executable => {
                let program = request.executable_path.trim();
                if program.is_empty() {
                    return Err("application launch path cannot be empty".to_string());
                }
                spawn_executable(program, request.arguments.as_str(), optional_non_empty(request.working_directory.as_str()))?;
                Ok(LaunchExecution {
                    effective_program: program.to_string(),
                })
            }
            LaunchMode::CommandLine => {
                let command_line = request.command_line.trim();
                if command_line.is_empty() {
                    return Err("command line cannot be empty".to_string());
                }
                spawn_command_line(command_line, optional_non_empty(request.working_directory.as_str()))?;
                Ok(LaunchExecution {
                    effective_program: command_line.to_string(),
                })
            }
        }
    }

    pub(crate) fn execute_kill(
        &mut self,
        request: &KillProcessRequest,
        watched_target: Option<&str>,
    ) -> Result<usize, String> {
        self.refresh_processes();

        let pids = resolve_target_pids(
            &self.system,
            request.target_source,
            request.target.as_str(),
            request.match_mode,
            watched_target,
        )?;

        for pid in &pids {
            terminate_pid(*pid, request.hard_kill)?;
        }

        Ok(pids.len())
    }

    pub(crate) fn execute_window_control(
        &mut self,
        request: &WindowControlRequest,
        watched_target: Option<&str>,
    ) -> Result<usize, String> {
        self.refresh_processes();

        let pids = resolve_target_pids(
            &self.system,
            request.target_source,
            request.target.as_str(),
            request.match_mode,
            watched_target,
        )?;
        if pids.is_empty() {
            return Err("no matching process is running".to_string());
        }

        let affected = control_windows_for_pids(
            &pids,
            request.action,
            request.x,
            request.y,
            request.width,
            request.height,
            request.always_on_top,
        )?;
        if affected == 0 {
            return Err("no application windows matched the requested target".to_string());
        }

        Ok(affected)
    }
}

fn watched_app_worker_loop(state: Arc<WatchedAppWorkerState>) {
    let mut system = System::new_all();

    loop {
        if state.stop_requested.load(Ordering::Relaxed) {
            break;
        }

        let target_paths = lock_unpoisoned(&state.target_paths).clone();
        if target_paths.is_empty() {
            lock_unpoisoned(&state.metrics_by_target).clear();
        } else {
            let metrics_by_target = collect_watched_app_metrics(&mut system, target_paths.as_slice());
            *lock_unpoisoned(&state.metrics_by_target) = metrics_by_target;
        }

        if state.stop_requested.load(Ordering::Relaxed) {
            break;
        }

        thread::park_timeout(WATCHED_APP_WORKER_INTERVAL);
    }
}

fn collect_watched_app_metrics(
    system: &mut System,
    target_paths: &[String],
) -> HashMap<String, WatchedAppMetrics> {
    if target_paths.is_empty() {
        return HashMap::new();
    }

    system.refresh_cpu_usage();
    system.refresh_memory();
    system.refresh_processes(ProcessesToUpdate::All, true);

    let window_counts_by_pid = visible_window_counts_by_pid();
    target_paths
        .iter()
        .map(|target_path| {
            (
                watched_app_target_key(target_path.as_str()),
                watched_app_metrics_from_system(system, target_path.as_str(), &window_counts_by_pid),
            )
        })
        .collect()
}

fn watched_app_metrics_from_system(
    system: &System,
    target_path: &str,
    window_counts_by_pid: &HashMap<u32, usize>,
) -> WatchedAppMetrics {
    let trimmed_path = target_path.trim();
    let path = Path::new(trimmed_path);
    let processes = watched_processes(system, trimmed_path);
    let cpu_ratio = system_metrics::process_cpu_percent_to_ratio(
        processes.iter().map(|process| process.cpu_usage() as f64).sum(),
        system.cpus().len(),
    );
    let memory_mb = system_metrics::bytes_f64_to_mb(
        processes.iter().map(|process| process.memory() as f64).sum(),
    );
    let virtual_memory_mb = system_metrics::bytes_f64_to_mb(
        processes
            .iter()
            .map(|process| process.virtual_memory() as f64)
            .sum(),
    );
    let memory_max_mb = system_metrics::bytes_to_mb(system.total_memory());
    let pids = processes
        .iter()
        .map(|process| process.pid().as_u32())
        .collect::<HashSet<_>>();
    let uptime_seconds = processes
        .iter()
        .map(|process| system_metrics::uptime_seconds_from_unix_start(process.start_time()))
        .max_by(f64::total_cmp)
        .unwrap_or(0.0);
    let window_count = window_count_for_pids(&pids, window_counts_by_pid);
    let main_pid = processes
        .iter()
        .map(|process| process.pid().as_u32())
        .min()
        .map(|pid| pid.min(i32::MAX as u32) as i32)
        .unwrap_or(0);

    WatchedAppMetrics {
        target_path: trimmed_path.to_string(),
        target_name: path
            .file_stem()
            .or_else(|| path.file_name())
            .map(|value| value.to_string_lossy().to_string())
            .unwrap_or_default(),
        exists: !trimmed_path.is_empty() && path.exists(),
        running: !processes.is_empty(),
        uptime_seconds,
        process_count: saturating_i32(processes.len()),
        main_pid,
        window_opened: window_count > 0,
        window_count,
        cpu_ratio,
        memory_mb,
        memory_max_mb,
        virtual_memory_mb,
    }
}

fn resolve_target_pids(
    system: &System,
    target_source: CommandTargetSource,
    target: &str,
    match_mode: ProcessMatchMode,
    watched_target: Option<&str>,
) -> Result<Vec<u32>, String> {
    let pids = match target_source {
        CommandTargetSource::WatchedApp => watched_processes(
            system,
            watched_target.ok_or_else(|| "missing watched app executable path".to_string())?,
        )
        .into_iter()
        .map(|process| process.pid().as_u32())
        .collect::<Vec<_>>(),
        CommandTargetSource::FreeProcess => free_processes(system, target, match_mode)
            .into_iter()
            .map(|process| process.pid().as_u32())
            .collect::<Vec<_>>(),
    };

    if pids.is_empty() {
        return Err(match target_source {
            CommandTargetSource::WatchedApp => "watched app is not currently running".to_string(),
            CommandTargetSource::FreeProcess => {
                format!("no process matches '{}' using {}", target, match_mode.as_str())
            }
        });
    }

    let mut unique = pids.into_iter().collect::<HashSet<_>>().into_iter().collect::<Vec<_>>();
    unique.sort_unstable();
    Ok(unique)
}

fn watched_processes<'a>(system: &'a System, target_path: &str) -> Vec<&'a Process> {
    let normalized_target_path = normalize_path_text(target_path);
    let normalized_target_name = Path::new(target_path)
        .file_name()
        .map(|value| value.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    if normalized_target_path.is_empty() && normalized_target_name.is_empty() {
        return Vec::new();
    }

    system
        .processes()
        .values()
        .filter(|process| {
            let process_name = process.name().to_string_lossy().to_ascii_lowercase();
            let process_exe = process
                .exe()
                .map(normalize_path)
                .unwrap_or_default();
            let process_file_name = process
                .exe()
                .and_then(|path| path.file_name())
                .map(|value| value.to_string_lossy().to_ascii_lowercase())
                .unwrap_or_default();

            (!normalized_target_path.is_empty() && process_exe == normalized_target_path)
                || (!normalized_target_name.is_empty()
                    && (process_file_name == normalized_target_name
                        || process_name == normalized_target_name))
        })
        .collect()
}

fn dedupe_watched_app_paths<'a, I>(target_paths: I) -> Vec<String>
where
    I: IntoIterator<Item = &'a str>,
{
    let mut seen = HashSet::new();
    let mut deduped = Vec::new();
    for target_path in target_paths {
        let trimmed = target_path.trim();
        if trimmed.is_empty() {
            continue;
        }

        let key = watched_app_target_key(trimmed);
        if seen.insert(key) {
            deduped.push(trimmed.to_string());
        }
    }
    deduped
}

fn watched_app_target_key(target_path: &str) -> String {
    normalize_path_text(target_path)
}

fn free_processes<'a>(
    system: &'a System,
    target: &str,
    match_mode: ProcessMatchMode,
) -> Vec<&'a Process> {
    let target = target.trim();
    if target.is_empty() {
        return Vec::new();
    }

    system
        .processes()
        .values()
        .filter(|process| {
            let process_name = process.name().to_string_lossy().to_string();
            let process_exe = process
                .exe()
                .map(|path| path.to_string_lossy().to_string())
                .unwrap_or_default();
            match_mode.matches(process_name.as_str(), target)
                || match_mode.matches(process_exe.as_str(), target)
        })
        .collect()
}

fn optional_non_empty(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then_some(trimmed)
}

fn spawn_executable(
    program: &str,
    arguments: &str,
    working_directory: Option<&str>,
) -> Result<(), String> {
    let mut command = Command::new(program);
    if let Some(working_directory) = working_directory {
        command.current_dir(working_directory);
    }
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    #[cfg(windows)]
    {
        if let Some(arguments) = optional_non_empty(arguments) {
            command.raw_arg(arguments);
        }
    }

    #[cfg(not(windows))]
    {
        for argument in arguments.split_whitespace() {
            command.arg(argument);
        }
    }

    command
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("failed to launch '{program}': {error}"))
}

fn spawn_command_line(command_line: &str, working_directory: Option<&str>) -> Result<(), String> {
    #[cfg(windows)]
    let mut command = {
        let mut command = Command::new("cmd");
        command.args(["/C", command_line]);
        command
    };

    #[cfg(not(windows))]
    let mut command = {
        let mut command = Command::new("sh");
        command.args(["-c", command_line]);
        command
    };

    if let Some(working_directory) = working_directory {
        command.current_dir(working_directory);
    }
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("failed to launch command line '{command_line}': {error}"))
}

#[cfg(windows)]
fn terminate_pid(pid: u32, hard_kill: bool) -> Result<(), String> {
    let pid_string = pid.to_string();
    let mut command = Command::new("taskkill");
    command.arg("/PID").arg(pid_string.as_str());
    if hard_kill {
        command.arg("/F");
    }

    let status = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|error| format!("failed to terminate process {pid}: {error}"))?;

    if !status.success() {
        return Err(format!(
            "taskkill returned exit code {:?} for process {pid}",
            status.code(),
        ));
    }

    Ok(())
}

#[cfg(not(windows))]
fn terminate_pid(pid: u32, hard_kill: bool) -> Result<(), String> {
    let signal = if hard_kill { "-9" } else { "-TERM" };
    let status = Command::new("kill")
        .arg(signal)
        .arg(pid.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|error| format!("failed to terminate process {pid}: {error}"))?;

    if !status.success() {
        return Err(format!(
            "kill returned exit code {:?} for process {pid}",
            status.code(),
        ));
    }

    Ok(())
}

fn capture_folder_snapshot(path: &str) -> FolderTreeSnapshot {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return FolderTreeSnapshot::default();
    }

    let root = Path::new(trimmed);
    if !root.is_dir() {
        return FolderTreeSnapshot {
            exists: root.exists(),
            entries: BTreeMap::new(),
        };
    }

    let mut snapshot = FolderTreeSnapshot {
        exists: true,
        entries: BTreeMap::new(),
    };
    let _ = collect_folder_entries(root, root, &mut snapshot.entries);
    snapshot
}

fn collect_folder_entries(
    root: &Path,
    current: &Path,
    entries: &mut BTreeMap<String, FileStamp>,
) -> Result<(), std::io::Error> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let entry_path = entry.path();
        let file_type = entry.file_type()?;
        let metadata = entry.metadata()?;
        let relative_path = entry_path
            .strip_prefix(root)
            .unwrap_or(entry_path.as_path())
            .to_string_lossy()
            .replace('\\', "/");
        entries.insert(
            relative_path,
            FileStamp {
                is_dir: file_type.is_dir(),
                len: metadata.len(),
                modified_ms: metadata
                    .modified()
                    .ok()
                    .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
                    .map(|duration| duration.as_millis().min(u64::MAX as u128) as u64)
                    .unwrap_or(0),
            },
        );

        if file_type.is_dir() && !file_type.is_symlink() {
            collect_folder_entries(root, entry_path.as_path(), entries)?;
        }
    }

    Ok(())
}

#[cfg(windows)]
#[derive(Clone, Debug)]
struct WindowInfo {
    hwnd: HWND,
    pid: u32,
    visible: bool,
}

#[cfg(windows)]
fn visible_window_counts_by_pid() -> HashMap<u32, usize> {
    let mut counts = HashMap::new();
    for window in enumerate_windows() {
        if window.visible {
            *counts.entry(window.pid).or_default() += 1;
        }
    }
    counts
}

#[cfg(not(windows))]
fn visible_window_counts_by_pid() -> HashMap<u32, usize> {
    HashMap::new()
}

fn window_count_for_pids(pids: &HashSet<u32>, counts_by_pid: &HashMap<u32, usize>) -> i32 {
    saturating_i32(
        pids.iter()
            .map(|pid| counts_by_pid.get(pid).copied().unwrap_or(0))
            .sum(),
    )
}

#[cfg(windows)]
fn control_windows_for_pids(
    pids: &[u32],
    action: WindowAction,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    always_on_top: bool,
) -> Result<usize, String> {
    let wanted = pids.iter().copied().collect::<HashSet<_>>();
    let windows = enumerate_windows()
        .into_iter()
        .filter(|window| wanted.contains(&window.pid))
        .collect::<Vec<_>>();

    if windows.is_empty() {
        return Ok(0);
    }

    for window in &windows {
        unsafe {
            match action {
                WindowAction::Move => {
                    SetWindowPos(
                        window.hwnd,
                        ptr::null_mut(),
                        x,
                        y,
                        0,
                        0,
                        SWP_NOACTIVATE | SWP_NOSIZE | SWP_NOZORDER,
                    );
                }
                WindowAction::Resize => {
                    SetWindowPos(
                        window.hwnd,
                        ptr::null_mut(),
                        0,
                        0,
                        width.max(1),
                        height.max(1),
                        SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOZORDER,
                    );
                }
                WindowAction::Bounds => {
                    SetWindowPos(
                        window.hwnd,
                        ptr::null_mut(),
                        x,
                        y,
                        width.max(1),
                        height.max(1),
                        SWP_NOACTIVATE | SWP_NOZORDER,
                    );
                }
                WindowAction::Minimize => {
                    ShowWindow(window.hwnd, SW_MINIMIZE);
                }
                WindowAction::Maximize => {
                    ShowWindow(window.hwnd, SW_MAXIMIZE);
                }
                WindowAction::Restore => {
                    ShowWindow(window.hwnd, SW_RESTORE);
                }
                WindowAction::Tray => {
                    ShowWindow(window.hwnd, SW_HIDE);
                }
                WindowAction::Show => {
                    ShowWindow(window.hwnd, SW_SHOW);
                }
                WindowAction::AlwaysOnTop => {
                    SetWindowPos(
                        window.hwnd,
                        if always_on_top {
                            HWND_TOPMOST
                        } else {
                            HWND_NOTOPMOST
                        },
                        0,
                        0,
                        0,
                        0,
                        SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE,
                    );
                }
            }
        }
    }

    Ok(windows.len())
}

#[cfg(not(windows))]
fn control_windows_for_pids(
    _pids: &[u32],
    _action: WindowAction,
    _x: i32,
    _y: i32,
    _width: i32,
    _height: i32,
    _always_on_top: bool,
) -> Result<usize, String> {
    Err("application window control is currently supported on Windows only".to_string())
}

#[cfg(windows)]
fn enumerate_windows() -> Vec<WindowInfo> {
    unsafe extern "system" fn collect_window(hwnd: HWND, lparam: LPARAM) -> i32 {
        let windows = &mut *(lparam as *mut Vec<WindowInfo>);
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid == 0 {
            return 1;
        }

        let visible = IsWindowVisible(hwnd) != 0;
        let title = read_window_title(hwnd);
        if !visible && title.trim().is_empty() {
            return 1;
        }

        let mut rect = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        let _ = GetWindowRect(hwnd, &mut rect);
        windows.push(WindowInfo { hwnd, pid, visible });
        1
    }

    let mut windows = Vec::new();
    unsafe {
        EnumWindows(Some(collect_window), &mut windows as *mut _ as LPARAM);
    }
    windows
}

#[cfg(windows)]
fn read_window_title(hwnd: HWND) -> String {
    unsafe {
        let length = GetWindowTextLengthW(hwnd);
        if length <= 0 {
            return String::new();
        }

        let mut buffer = vec![0u16; length as usize + 1];
        let copied = GetWindowTextW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32);
        if copied <= 0 {
            return String::new();
        }

        String::from_utf16_lossy(&buffer[..copied as usize])
    }
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/").to_ascii_lowercase()
}

fn normalize_path_text(path: &str) -> String {
    normalize_path(Path::new(path))
}

fn lock_unpoisoned<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn unix_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u64::MAX as u128) as u64
}

fn saturating_i32(value: usize) -> i32 {
    value.min(i32::MAX as usize) as i32
}
