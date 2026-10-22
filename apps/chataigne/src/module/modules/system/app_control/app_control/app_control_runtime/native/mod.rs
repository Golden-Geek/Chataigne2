use super::*;
use crate::app::module::common::system_metrics;
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    path::Path,
    process::{Command, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};
use sysinfo::{Process, ProcessesToUpdate, System};
mod folders;
mod windows;
use windows::{control_windows_for_pids, visible_window_counts_by_pid, window_count_for_pids};

pub(super) struct Native {
    system: System,
    folders: HashMap<golden_core::node::NodeId, folders::Stored>,
    children: Vec<std::process::Child>,
}
impl Native {
    pub(super) fn open() -> Result<Self, String> {
        Ok(Self {
            system: System::new(),
            folders: HashMap::new(),
            children: Vec::new(),
        })
    }
    fn refresh(&mut self) -> Result<(), String> {
        self.system.refresh_cpu_usage();
        self.system.refresh_memory();
        self.system.refresh_processes(ProcessesToUpdate::All, true);
        if self.system.processes().len() > 16384 || self.system.cpus().len() > 4096 {
            return Err("App Control process/CPU inventory exceeds 16384 / 4096 entries".into());
        }
        self.children
            .retain_mut(|child| !matches!(child.try_wait(), Ok(Some(_))));
        Ok(())
    }
    fn launch(&mut self, request: &LaunchProcessRequest, target: Option<&str>) -> Result<Execution, String> {
        self.children
            .retain_mut(|child| !matches!(child.try_wait(), Ok(Some(_))));
        if self.children.len() >= 64 {
            return Err("App Control owns 64 running child processes; launch was not admitted".into());
        }
        let program = match request.mode {
            LaunchMode::WatchedApp => target.ok_or("missing watched application")?,
            LaunchMode::Executable => request.executable_path.as_str(),
            LaunchMode::CommandLine => request.command_line.as_str(),
        }
        .trim();
        if program.is_empty() {
            return Err("App Control launch target is empty".into());
        }
        let child = if request.mode == LaunchMode::CommandLine {
            spawn_command_line(program, optional_non_empty(&request.working_directory))?
        } else {
            spawn_executable(
                program,
                &request.arguments,
                optional_non_empty(&request.working_directory),
            )?
        };
        self.children.push(child);
        Ok(Execution {
            attempted: true,
            method: "launch",
            details: serde_json::json!({
                "mode":request.mode.as_str(),
                "watchedTarget":request.watched_app,
                "effectiveProgram":program,
                "arguments":request.arguments,
                "workingDirectory":request.working_directory,
                "commandLine":request.command_line,
            }),
        })
    }
}
impl Provider for Native {
    fn sample(&mut self, config: &Config) -> Result<Observation, String> {
        if config.apps.iter().all(|entry| entry.target_path.is_empty())
            && config.folders.iter().all(|entry| entry.target_path.is_empty())
        {
            return Ok(Observation {
                apps: config
                    .apps
                    .iter()
                    .map(|entry| AppObservation {
                        metrics: WatchedAppMetrics::idle(&entry.target_path),
                        kind: PathKind::Empty,
                    })
                    .collect(),
                folders: config
                    .folders
                    .iter()
                    .map(|entry| FolderObservation {
                        update: FolderWatchUpdate {
                            path: entry.target_path.clone(),
                            ..Default::default()
                        },
                        kind: PathKind::Empty,
                    })
                    .collect(),
            });
        }
        self.refresh()?;
        let metrics = collect_watched_app_metrics(
            &mut self.system,
            &config
                .apps
                .iter()
                .map(|entry| entry.target_path.clone())
                .collect::<Vec<_>>(),
        )?;
        let apps = config
            .apps
            .iter()
            .map(|entry| AppObservation {
                metrics: metrics
                    .get(&watched_app_target_key(&entry.target_path))
                    .cloned()
                    .unwrap_or_default(),
                kind: kind(&entry.target_path),
            })
            .collect();
        let mut budget = folders::Budget::default();
        let mut rows = Vec::with_capacity(config.folders.len());
        let mut staged = self.folders.clone();
        staged.retain(|id, _| config.folders.iter().any(|entry| entry.item_id == *id));
        for entry in &config.folders {
            rows.push(folders::sample(&mut staged, entry, &mut budget)?);
        }
        self.folders = staged;
        Ok(Observation { apps, folders: rows })
    }
    fn execute(&mut self, request: &Request) -> Result<Execution, String> {
        if cfg!(test) {
            return Err("Native App Control actions are disabled in tests; use a controlled provider".into());
        }
        match request {
            Request::Running(path, desired) => {
                self.refresh()?;
                let running = !watched_processes(&self.system, path).is_empty();
                if running == *desired {
                    return Ok(Execution {
                        attempted: false,
                        method: "setRunning",
                        details: serde_json::json!({}),
                    });
                }
                if *desired {
                    self.launch(
                        &LaunchProcessRequest {
                            mode: LaunchMode::WatchedApp,
                            watched_app: path.clone(),
                            executable_path: String::new(),
                            arguments: String::new(),
                            working_directory: String::new(),
                            command_line: String::new(),
                        },
                        Some(path),
                    )
                } else {
                    self.execute(&Request::Kill(
                        KillProcessRequest {
                            target_source: CommandTargetSource::WatchedApp,
                            target: path.clone(),
                            match_mode: ProcessMatchMode::Exact,
                            hard_kill: false,
                        },
                        Some(path.clone()),
                    ))
                }
            }
            Request::Launch(request, target) => self.launch(request, target.as_deref()),
            Request::Kill(request, target) => {
                self.refresh()?;
                let pids = resolve_target_pids(
                    &self.system,
                    request.target_source,
                    &request.target,
                    request.match_mode,
                    target.as_deref(),
                )?;
                if pids.len() > 64 {
                    return Err("App Control refuses a kill batch above 64 processes".into());
                }
                for pid in &pids {
                    terminate_pid(*pid, request.hard_kill)?;
                }
                Ok(Execution {
                    attempted: true,
                    method: "kill",
                    details: serde_json::json!({
                        "target":request.target,
                        "targetSource":request.target_source.as_str(),
                        "matchMode":request.match_mode.as_str(),
                        "hardKill":request.hard_kill,
                        "processCount":pids.len(),
                    }),
                })
            }
            Request::Window(request, target) => {
                self.refresh()?;
                let pids = resolve_target_pids(
                    &self.system,
                    request.target_source,
                    &request.target,
                    request.match_mode,
                    target.as_deref(),
                )?;
                if pids.len() > 64 {
                    return Err("App Control refuses a window batch above 64 processes".into());
                }
                let count = control_windows_for_pids(
                    &pids,
                    request.action,
                    request.x,
                    request.y,
                    request.width,
                    request.height,
                    request.always_on_top,
                )?;
                if count == 0 {
                    return Err("no application windows matched the requested target".into());
                }
                Ok(Execution {
                    attempted: true,
                    method: "controlWindow",
                    details: serde_json::json!({
                        "target":request.target,
                        "targetSource":request.target_source.as_str(),
                        "matchMode":request.match_mode.as_str(),
                        "action":request.action.as_str(),
                        "windowCount":count,
                        "x":request.x,
                        "y":request.y,
                        "width":request.width,
                        "height":request.height,
                        "alwaysOnTop":request.always_on_top,
                    }),
                })
            }
        }
    }
}
impl Drop for Native {
    fn drop(&mut self) {
        for child in &mut self.children {
            let _ = child.wait();
        }
    }
}
fn kind(path: &str) -> PathKind {
    if path.trim().is_empty() {
        PathKind::Empty
    } else {
        match fs::metadata(path) {
            Ok(value) if value.is_dir() => PathKind::Directory,
            Ok(_) => PathKind::File,
            Err(_) => PathKind::Missing,
        }
    }
}

fn collect_watched_app_metrics(
    system: &mut System,
    target_paths: &[String],
) -> Result<HashMap<String, WatchedAppMetrics>, String> {
    if target_paths.is_empty() {
        return Ok(HashMap::new());
    }

    let window_counts_by_pid = visible_window_counts_by_pid()?;
    Ok(target_paths
        .iter()
        .map(|target_path| {
            (
                watched_app_target_key(target_path.as_str()),
                watched_app_metrics_from_system(system, target_path.as_str(), &window_counts_by_pid),
            )
        })
        .collect())
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
    let memory_mb = system_metrics::bytes_f64_to_mb(processes.iter().map(|process| process.memory() as f64).sum());
    let virtual_memory_mb =
        system_metrics::bytes_f64_to_mb(processes.iter().map(|process| process.virtual_memory() as f64).sum());
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
            let process_exe = process.exe().map(normalize_path).unwrap_or_default();
            let process_file_name = process
                .exe()
                .and_then(|path| path.file_name())
                .map(|value| value.to_string_lossy().to_ascii_lowercase())
                .unwrap_or_default();

            (!normalized_target_path.is_empty() && process_exe == normalized_target_path)
                || (!normalized_target_name.is_empty()
                    && (process_file_name == normalized_target_name || process_name == normalized_target_name))
        })
        .collect()
}

fn free_processes<'a>(system: &'a System, target: &str, match_mode: ProcessMatchMode) -> Vec<&'a Process> {
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
            match_mode.matches(process_name.as_str(), target) || match_mode.matches(process_exe.as_str(), target)
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
) -> Result<std::process::Child, String> {
    let mut command = Command::new(program);
    if let Some(working_directory) = working_directory {
        command.current_dir(working_directory);
    }
    command.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());

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
        .map_err(|error| format!("failed to launch '{program}': {error}"))
}

fn spawn_command_line(command_line: &str, working_directory: Option<&str>) -> Result<std::process::Child, String> {
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
        return Err(format!("kill returned exit code {:?} for process {pid}", status.code(),));
    }

    Ok(())
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/").to_ascii_lowercase()
}

fn watched_app_target_key(path: &str) -> String {
    normalize_path_text(path.trim())
}

fn normalize_path_text(path: &str) -> String {
    normalize_path(Path::new(path))
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
