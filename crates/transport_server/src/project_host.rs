use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use golden_engine::app::{
    ProjectFileSpec, ProjectLifecycle, configure_loaded_engine, create_new_project_engine, load_sparse_project_file,
    prepare_engine_for_runtime, shutdown_engine_for_runtime, to_sparse_project_json_pretty,
};
use golden_engine::engine::Engine;
use golden_engine::logger::{self, LogLevel};

const BROWSER_PROJECT_DIRECTORY_SEGMENTS: &[&str] = &["Documents", "Chataigne"];

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

fn browser_project_directory() -> Result<PathBuf, String> {
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
    for segment in BROWSER_PROJECT_DIRECTORY_SEGMENTS {
        directory.push(segment);
    }
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
    engine: &Arc<Mutex<Engine<T>>>,
    next_engine: Engine<T>,
    reason: &str,
) -> Result<(), String> {
    let started = Instant::now();
    let next_node_count = next_engine.nodes.iter().count();
    let mut guard = match engine.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let shutdown_started = Instant::now();
    shutdown_engine_for_runtime(&mut *guard);
    let shutdown_elapsed = shutdown_started.elapsed();

    let drop_started = Instant::now();
    let previous_engine = std::mem::replace(&mut *guard, next_engine);
    drop(previous_engine);
    let drop_elapsed = drop_started.elapsed();

    // apply_edits runs node-ready callbacks, which can bind transports.
    // Fully unload and drop the previous engine before the replacement starts runtime work.
    let prepare_started = Instant::now();
    prepare_engine_for_runtime(&mut *guard).map_err(|err| err.to_string())?;
    let prepare_elapsed = prepare_started.elapsed();
    guard.clear_ui_event_log();
    guard.push_ui_custom_event(
        "__transport.resync_required",
        None,
        serde_json::json!({ "reason": reason }),
    );
    eprintln!(
        "[project-host] replace_engine reason={reason} nodes={} shutdown_ms={} drop_ms={} prepare_ms={} total_ms={}",
        next_node_count,
        shutdown_elapsed.as_millis(),
        drop_elapsed.as_millis(),
        prepare_elapsed.as_millis(),
        started.elapsed().as_millis()
    );
    Ok(())
}

pub(crate) fn create_new_project<T: ProjectLifecycle>(engine: &Arc<Mutex<Engine<T>>>) -> Result<(), String> {
    let next_engine = create_new_project_engine::<T>()?;
    replace_live_engine(engine, next_engine, "project_new")
}

pub(crate) fn save_project<T: ProjectLifecycle>(
    engine: &Arc<Mutex<Engine<T>>>,
    raw_path: &str,
) -> Result<String, String> {
    let file_spec = T::project_file_spec();
    let path = normalize_project_save_path(raw_path, &file_spec)
        .ok_or_else(|| "project-save path cannot be empty".to_string())?;

    let started = Instant::now();

    let lock_started = Instant::now();
    let guard = match engine.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let lock_wait_elapsed = lock_started.elapsed();
    let node_count = guard.nodes.iter().count();

    let clone_or_snapshot_ms = 0;

    let serialize_started = Instant::now();
    let json = to_sparse_project_json_pretty(&guard).map_err(|err| err.to_string())?;
    let serialize_elapsed = serialize_started.elapsed();
    drop(guard);

    let write_started = Instant::now();
    fs::write(path.as_str(), json.as_bytes()).map_err(|err| err.to_string())?;
    let write_elapsed = write_started.elapsed();
    eprintln!(
        "[project-host] save_project path='{}' nodes={} bytes={} lock_wait_ms={} clone_or_snapshot_ms={} serialize_ms={} write_ms={} total_ms={}",
        path,
        node_count,
        json.len(),
        lock_wait_elapsed.as_millis(),
        clone_or_snapshot_ms,
        serialize_elapsed.as_millis(),
        write_elapsed.as_millis(),
        started.elapsed().as_millis()
    );
    let _ = logger::log_message(
        LogLevel::Success,
        "project".to_string(),
        None,
        format!(
            "Saved project: {path} (nodes={node_count} bytes={} lock_wait_ms={} serialize_ms={} write_ms={} total_ms={})",
            json.len(),
            lock_wait_elapsed.as_millis(),
            serialize_elapsed.as_millis(),
            write_elapsed.as_millis(),
            started.elapsed().as_millis()
        ),
    );
    Ok(path)
}

pub(crate) fn load_project<T: ProjectLifecycle>(
    engine: &Arc<Mutex<Engine<T>>>,
    raw_path: &str,
) -> Result<String, String> {
    let path = normalize_project_path(raw_path).ok_or_else(|| "project-load path cannot be empty".to_string())?;

    let started = Instant::now();
    let load_started = Instant::now();
    let mut next_engine = load_sparse_project_file::<T, _>(path.as_str()).map_err(|err| err.to_string())?;
    let load_elapsed = load_started.elapsed();
    let node_count = next_engine.nodes.iter().count();

    let configure_started = Instant::now();
    configure_loaded_engine(&mut next_engine)?;
    let configure_elapsed = configure_started.elapsed();

    let replace_started = Instant::now();
    replace_live_engine(engine, next_engine, "project_loaded")?;
    let replace_elapsed = replace_started.elapsed();

    eprintln!(
        "[project-host] load_project path='{}' nodes={} rebuild_ms={} configure_ms={} replace_ms={} total_ms={}",
        path,
        node_count,
        load_elapsed.as_millis(),
        configure_elapsed.as_millis(),
        replace_elapsed.as_millis(),
        started.elapsed().as_millis()
    );
    let _ = logger::log_message(
        LogLevel::Success,
        "project".to_string(),
        None,
        format!(
            "Loaded project: {path} (nodes={node_count} rebuild_ms={} configure_ms={} replace_ms={} total_ms={})",
            load_elapsed.as_millis(),
            configure_elapsed.as_millis(),
            replace_elapsed.as_millis(),
            started.elapsed().as_millis()
        ),
    );
    Ok(path)
}

pub(crate) fn upload_project_and_load<T: ProjectLifecycle>(
    engine: &Arc<Mutex<Engine<T>>>,
    raw_file_name: &str,
    contents: &str,
) -> Result<String, String> {
    if contents.trim().is_empty() {
        return Err("project upload contents cannot be empty".to_string());
    }

    let directory = browser_project_directory()?;
    fs::create_dir_all(&directory).map_err(|err| {
        format!(
            "failed to create browser project upload directory {}: {err}",
            directory.display()
        )
    })?;

    let file_spec = T::project_file_spec();
    let file_name = sanitize_browser_upload_file_name(raw_file_name, &file_spec);
    let path = directory.join(file_name);
    fs::write(&path, contents)
        .map_err(|err| format!("failed to write uploaded project file {}: {err}", path.display()))?;

    let normalized_path = path.to_string_lossy().to_string();
    load_project(engine, &normalized_path)
}

#[cfg(test)]
mod project_host_tests;
