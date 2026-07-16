use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use golden_engine::app::{
    ProjectFileSpec, ProjectLifecycle, configure_loaded_engine, create_new_project_engine, ensure_preferences_tree,
    insert_sparse_preferences_json, load_sparse_project_file_with_ui_state,
    load_sparse_project_file_with_ui_state_recovering,
};
use golden_engine::application::{ProductionRuntime, ProjectReplacement, ProjectSaveRequest};
use golden_engine::engine::{Engine, ProjectLoadRecoveryReport, ProjectPersistenceError};
use golden_engine::logger::{self, LogLevel};

use crate::ui_server::UiPreferencesConfig;

pub(crate) struct ProjectLoadResult {
    pub(crate) path: String,
    pub(crate) ui_state: Option<serde_json::Value>,
    pub(crate) recovery: ProjectLoadRecoveryReport,
}

#[derive(Debug, Clone)]
pub(crate) struct ProjectLoadError {
    pub(crate) message: String,
    pub(crate) recovery: Option<ProjectLoadRecoveryReport>,
}

impl ProjectLoadError {
    fn from_persistence(error: ProjectPersistenceError) -> Self {
        let recovery = match &error {
            ProjectPersistenceError::Engine(engine_error) => {
                Some(ProjectLoadRecoveryReport::from_engine_rebuild_error(engine_error))
            }
            _ => None,
        };
        Self {
            message: error.to_string(),
            recovery,
        }
    }
}

impl std::fmt::Display for ProjectLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

fn normalize_project_path(raw_path: &str) -> Option<String> {
    let path = raw_path.trim();
    if path.is_empty() {
        return None;
    }
    Some(path.to_string())
}

fn normalize_project_save_path(raw_path: &str, file_spec: &ProjectFileSpec) -> Option<String> {
    let normalized_path = normalize_project_path(raw_path)?;
    let normalized_extension = file_spec.normalized_extension();
    let mut path = PathBuf::from(normalized_path);

    let already_matches = path
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case(&normalized_extension));
    if already_matches {
        return Some(path.to_string_lossy().to_string());
    }

    path.set_extension(&normalized_extension);
    Some(path.to_string_lossy().to_string())
}

fn browser_project_directory<T: ProjectLifecycle>() -> Result<PathBuf, String> {
    let home_dir = env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .or_else(|| {
            let home_drive = env::var_os("HOMEDRIVE")?;
            let home_path = env::var_os("HOMEPATH")?;
            let mut path = PathBuf::from(home_drive);
            path.push(home_path);
            Some(path)
        })
        .ok_or_else(|| "unable to resolve a home directory for browser project uploads".to_string())?;

    let mut directory = home_dir;
    directory.push("Documents");
    directory.push(T::app_data_directory_name());
    Ok(directory)
}

fn sanitize_browser_upload_file_name(raw_file_name: &str, file_spec: &ProjectFileSpec) -> String {
    let normalized_extension = file_spec.normalized_extension();
    let candidate = Path::new(raw_file_name.trim())
        .file_name()
        .and_then(|value| Path::new(value).file_stem().or_else(|| Path::new(value).file_name()))
        .and_then(|value| value.to_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("project");

    let mut sanitized = candidate
        .chars()
        .map(|character| match character {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '.' | '-' | '_' => character,
            _ => '_',
        })
        .collect::<String>();

    if sanitized.trim_matches(['_', '.']).is_empty() {
        sanitized = "project".to_string();
    }

    format!("{sanitized}.{normalized_extension}")
}

fn replace_live_engine<T: ProjectLifecycle>(
    runtime: &ProductionRuntime<T>,
    next_engine: Engine<T>,
    reason: &str,
    recover: bool,
) -> Result<ProjectLoadRecoveryReport, String> {
    let project_file = runtime.read_model().current_snapshot().project_file.clone();
    let result = runtime.replace_project(ProjectReplacement {
        engine: next_engine,
        project_file,
        reason: reason.to_string(),
        recover,
    })?;
    eprintln!(
        "[project-host] replace_engine reason={reason} nodes={} shutdown_ms={} drop_ms={} prepare_ms={} total_ms={}",
        result.node_count,
        result.shutdown.as_millis(),
        result.drop_previous.as_millis(),
        result.prepare.as_millis(),
        result.total.as_millis()
    );
    Ok(result.recovery)
}

pub(crate) fn load_preferences_into_engine<T: ProjectLifecycle>(
    engine: &mut Engine<T>,
    preferences: Option<&UiPreferencesConfig>,
) -> Result<(), String> {
    let default_data_folder = preferences
        .map(|config| config.default_data_folder.clone())
        .unwrap_or_default();

    if let Some(preferences) = preferences {
        match fs::read_to_string(&preferences.file_path) {
            Ok(contents) if !contents.trim().is_empty() => {
                insert_sparse_preferences_json(engine, &contents).map_err(|err| err.to_string())?;
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "failed to read preferences file {}: {error}",
                    preferences.file_path.display()
                ));
            }
        }
    }

    ensure_preferences_tree(engine, default_data_folder);
    Ok(())
}

pub(crate) fn save_preferences<T: ProjectLifecycle>(
    runtime: &ProductionRuntime<T>,
    preferences: &UiPreferencesConfig,
) -> Result<(), String> {
    let Some(json) = runtime.encode_preferences()? else {
        return Ok(());
    };

    if let Some(parent) = preferences.file_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create preferences directory {}: {err}", parent.display()))?;
    }
    golden_persistence::write_file_atomically_with_recovery(&preferences.file_path, json.as_bytes())
        .map(|_| ())
        .map_err(|err| {
            format!(
                "failed to write preferences file {}: {err}",
                preferences.file_path.display()
            )
        })
}

pub(crate) fn create_new_project<T: ProjectLifecycle>(
    runtime: &ProductionRuntime<T>,
    preferences: Option<&UiPreferencesConfig>,
) -> Result<(), String> {
    let mut next_engine = create_new_project_engine::<T>()?;
    load_preferences_into_engine(&mut next_engine, preferences)?;
    T::project_opened(&mut next_engine)?;
    replace_live_engine(runtime, next_engine, "project_new", false).map(|_| ())
}

pub(crate) fn save_project<T: ProjectLifecycle>(
    runtime: &ProductionRuntime<T>,
    raw_path: &str,
    ui_state: Option<serde_json::Value>,
) -> Result<String, String> {
    let file_spec = T::project_file_spec();
    let path = normalize_project_save_path(raw_path, &file_spec)
        .ok_or_else(|| "project-save path cannot be empty".to_string())?;

    let started = Instant::now();

    let encoded = runtime.encode_project(ProjectSaveRequest { ui_state })?;
    let clone_or_snapshot_ms = 0;

    let write_started = Instant::now();
    golden_persistence::write_file_atomically_with_recovery(path.as_str(), encoded.json.as_bytes())
        .map_err(|err| err.to_string())?;
    let write_elapsed = write_started.elapsed();
    eprintln!(
        "[project-host] save_project path='{}' nodes={} bytes={} lock_wait_ms={} clone_or_snapshot_ms={} serialize_ms={} write_ms={} total_ms={}",
        path,
        encoded.node_count,
        encoded.json.len(),
        encoded.lock_wait.as_millis(),
        clone_or_snapshot_ms,
        encoded.serialize.as_millis(),
        write_elapsed.as_millis(),
        started.elapsed().as_millis()
    );
    let _ = logger::log_message(
        LogLevel::Success,
        "project".to_string(),
        None,
        format!(
            "Saved project: {path} (nodes={} bytes={} lock_wait_ms={} serialize_ms={} write_ms={} total_ms={})",
            encoded.node_count,
            encoded.json.len(),
            encoded.lock_wait.as_millis(),
            encoded.serialize.as_millis(),
            write_elapsed.as_millis(),
            started.elapsed().as_millis()
        ),
    );
    Ok(path)
}

pub(crate) fn load_project<T: ProjectLifecycle>(
    runtime: &ProductionRuntime<T>,
    raw_path: &str,
    preferences: Option<&UiPreferencesConfig>,
) -> Result<ProjectLoadResult, ProjectLoadError> {
    load_project_with_options(runtime, raw_path, preferences, false)
}

pub(crate) fn load_project_recovering<T: ProjectLifecycle>(
    runtime: &ProductionRuntime<T>,
    raw_path: &str,
    preferences: Option<&UiPreferencesConfig>,
) -> Result<ProjectLoadResult, ProjectLoadError> {
    load_project_with_options(runtime, raw_path, preferences, true)
}

fn load_project_with_options<T: ProjectLifecycle>(
    runtime: &ProductionRuntime<T>,
    raw_path: &str,
    preferences: Option<&UiPreferencesConfig>,
    recover: bool,
) -> Result<ProjectLoadResult, ProjectLoadError> {
    let path = normalize_project_path(raw_path).ok_or_else(|| ProjectLoadError {
        message: "project-load path cannot be empty".to_string(),
        recovery: None,
    })?;

    let started = Instant::now();
    let load_started = Instant::now();
    let (mut next_engine, ui_state, mut recovery) = if recover {
        load_sparse_project_file_with_ui_state_recovering::<T, _>(path.as_str())
            .map_err(ProjectLoadError::from_persistence)?
    } else {
        let (next_engine, ui_state) = load_sparse_project_file_with_ui_state::<T, _>(path.as_str())
            .map_err(ProjectLoadError::from_persistence)?;
        (next_engine, ui_state, ProjectLoadRecoveryReport::default())
    };
    let load_elapsed = load_started.elapsed();
    let node_count = next_engine.nodes.iter().count();

    let configure_started = Instant::now();
    load_preferences_into_engine(&mut next_engine, preferences).map_err(|message| ProjectLoadError {
        message,
        recovery: None,
    })?;
    configure_loaded_engine(&mut next_engine).map_err(|message| ProjectLoadError {
        message,
        recovery: None,
    })?;
    let configure_elapsed = configure_started.elapsed();

    let replace_started = Instant::now();
    let runtime_recovery = replace_live_engine(runtime, next_engine, "project_loaded", recover).map_err(|message| {
        let recovery = recover.then(|| ProjectLoadRecoveryReport::from_runtime_startup_error(message.clone()));
        ProjectLoadError { message, recovery }
    })?;
    let replace_elapsed = replace_started.elapsed();
    recovery.problems.extend(runtime_recovery.problems);
    let problem_count = recovery.problems.len();

    eprintln!(
        "[project-host] load_project path='{}' nodes={} rebuild_ms={} configure_ms={} replace_ms={} total_ms={} recovery_problems={}",
        path,
        node_count,
        load_elapsed.as_millis(),
        configure_elapsed.as_millis(),
        replace_elapsed.as_millis(),
        started.elapsed().as_millis(),
        problem_count
    );
    let recovered = !recovery.is_empty();
    let _ = logger::log_message(
        if recovered {
            LogLevel::Warning
        } else {
            LogLevel::Success
        },
        "project".to_string(),
        None,
        if recovered {
            let first_problem = recovery
                .problems
                .first()
                .map(|problem| problem.message.as_str())
                .unwrap_or("unknown recoverable problem");
            format!(
                "Loaded project with recovery: {path} (nodes={node_count} problems={problem_count} first_problem={first_problem} rebuild_ms={} configure_ms={} replace_ms={} total_ms={})",
                load_elapsed.as_millis(),
                configure_elapsed.as_millis(),
                replace_elapsed.as_millis(),
                started.elapsed().as_millis()
            )
        } else {
            format!(
                "Loaded project: {path} (nodes={node_count} rebuild_ms={} configure_ms={} replace_ms={} total_ms={})",
                load_elapsed.as_millis(),
                configure_elapsed.as_millis(),
                replace_elapsed.as_millis(),
                started.elapsed().as_millis()
            )
        },
    );
    Ok(ProjectLoadResult {
        path,
        ui_state,
        recovery,
    })
}

pub(crate) fn upload_project_and_load<T: ProjectLifecycle>(
    runtime: &ProductionRuntime<T>,
    raw_file_name: &str,
    contents: &str,
    preferences: Option<&UiPreferencesConfig>,
) -> Result<ProjectLoadResult, ProjectLoadError> {
    upload_project_and_load_with_options(runtime, raw_file_name, contents, preferences, false)
}

pub(crate) fn upload_project_and_load_recovering<T: ProjectLifecycle>(
    runtime: &ProductionRuntime<T>,
    raw_file_name: &str,
    contents: &str,
    preferences: Option<&UiPreferencesConfig>,
) -> Result<ProjectLoadResult, ProjectLoadError> {
    upload_project_and_load_with_options(runtime, raw_file_name, contents, preferences, true)
}

fn upload_project_and_load_with_options<T: ProjectLifecycle>(
    runtime: &ProductionRuntime<T>,
    raw_file_name: &str,
    contents: &str,
    preferences: Option<&UiPreferencesConfig>,
    recover: bool,
) -> Result<ProjectLoadResult, ProjectLoadError> {
    if contents.trim().is_empty() {
        return Err(ProjectLoadError {
            message: "project upload contents cannot be empty".to_string(),
            recovery: None,
        });
    }

    let directory = browser_project_directory::<T>().map_err(|message| ProjectLoadError {
        message,
        recovery: None,
    })?;
    fs::create_dir_all(&directory).map_err(|err| ProjectLoadError {
        message: format!(
            "failed to create browser project upload directory {}: {err}",
            directory.display()
        ),
        recovery: None,
    })?;

    let file_spec = T::project_file_spec();
    let file_name = sanitize_browser_upload_file_name(raw_file_name, &file_spec);
    let path = directory.join(file_name);
    golden_persistence::write_file_atomically_with_recovery(&path, contents.as_bytes()).map_err(|err| {
        ProjectLoadError {
            message: format!("failed to write uploaded project file {}: {err}", path.display()),
            recovery: None,
        }
    })?;

    let normalized_path = path.to_string_lossy().to_string();
    load_project_with_options(runtime, &normalized_path, preferences, recover)
}

#[cfg(test)]
mod project_host_tests;
