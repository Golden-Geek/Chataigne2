use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use golden_engine::app::{
    ProjectFileSpec, ProjectLifecycle, configure_loaded_engine, create_new_project_engine, prepare_engine_for_runtime,
};
use golden_engine::engine::Engine;

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
    mut next_engine: Engine<T>,
    reason: &str,
) -> Result<(), String> {
    prepare_engine_for_runtime(&mut next_engine).map_err(|err| err.to_string())?;
    next_engine.clear_ui_event_log();
    next_engine.push_ui_custom_event(
        "__transport.resync_required",
        None,
        serde_json::json!({ "reason": reason }),
    );

    let mut guard = match engine.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    *guard = next_engine;
    Ok(())
}

pub(crate) fn create_new_project<T: ProjectLifecycle>(engine: &Arc<Mutex<Engine<T>>>) -> Result<(), String> {
    let next_engine = create_new_project_engine::<T>()?;
    replace_live_engine(engine, next_engine, "project_new")
}

pub(crate) fn save_project<T: ProjectLifecycle>(engine: &Arc<Mutex<Engine<T>>>, raw_path: &str) -> Result<(), String> {
    let file_spec = T::project_file_spec();
    let path = normalize_project_save_path(raw_path, &file_spec)
        .ok_or_else(|| "project-save path cannot be empty".to_string())?;

    let guard = match engine.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    guard
        .save_project_file_with(path.as_str(), |node| node.project_encode_data())
        .map_err(|err| err.to_string())
}

pub(crate) fn load_project<T: ProjectLifecycle>(engine: &Arc<Mutex<Engine<T>>>, raw_path: &str) -> Result<(), String> {
    let path = normalize_project_path(raw_path).ok_or_else(|| "project-load path cannot be empty".to_string())?;

    let mut next_engine =
        Engine::<T>::load_project_file_with(path.as_str(), T::project_decode_node).map_err(|err| err.to_string())?;
    configure_loaded_engine(&mut next_engine)?;
    replace_live_engine(engine, next_engine, "project_loaded")
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
    load_project(engine, &normalized_path)?;
    Ok(normalized_path)
}

#[cfg(test)]
mod tests {
    use super::{normalize_project_save_path, sanitize_browser_upload_file_name};
    use golden_engine::app::ProjectFileSpec;

    #[test]
    fn normalize_project_save_path_replaces_non_matching_extension() {
        let spec = ProjectFileSpec::new("Noisette files", "noisette");
        let normalized = normalize_project_save_path("D:/tmp/demo.json", &spec).expect("path should normalize");
        assert!(
            normalized.ends_with("demo.noisette"),
            "expected save path to end with .noisette, got {normalized}"
        );
    }

    #[test]
    fn normalize_project_save_path_appends_missing_extension() {
        let spec = ProjectFileSpec::new("Noisette files", ".noisette");
        let normalized = normalize_project_save_path("D:/tmp/demo", &spec).expect("path should normalize");
        assert!(
            normalized.ends_with("demo.noisette"),
            "expected save path to append .noisette, got {normalized}"
        );
    }

    #[test]
    fn sanitize_browser_upload_file_name_uses_app_extension() {
        let spec = ProjectFileSpec::new("Noisette files", "noisette");
        assert_eq!(sanitize_browser_upload_file_name("show.json", &spec), "show.noisette");
        assert_eq!(sanitize_browser_upload_file_name("", &spec), "project.noisette");
    }
}
