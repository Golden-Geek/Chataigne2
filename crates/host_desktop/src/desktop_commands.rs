use std::collections::BTreeSet;
use std::path::PathBuf;

/// Writes `contents` to `file_name` inside a subdirectory of the OS
/// shared application-data directory, creating it if needed. `subdir_segments`
/// is joined onto the app-data root (e.g. `["MyApp", "presets"]`); the app
/// decides its own convention, this command just resolves the OS-specific
/// root and performs the write. Returns the absolute path written.
#[tauri::command]
pub fn write_app_data_file(
    subdir_segments: Vec<String>,
    file_name: String,
    contents: String,
) -> Result<String, String> {
    let mut dir = dirs::data_dir()
        .ok_or_else(|| "could not resolve the app data directory on this system".to_string())?;
    for segment in &subdir_segments {
        dir.push(segment);
    }
    std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    let path = dir.join(file_name);
    std::fs::write(&path, contents).map_err(|error| error.to_string())?;
    Ok(path.to_string_lossy().to_string())
}

fn normalize_extensions(allowed_extensions: Option<Vec<String>>) -> Vec<String> {
    allowed_extensions
        .unwrap_or_default()
        .into_iter()
        .map(|value| value.trim().trim_start_matches('.').to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .collect::<BTreeSet<String>>()
        .into_iter()
        .collect()
}

fn normalize_filter_label(filter_label: Option<String>) -> String {
    let normalized = filter_label.unwrap_or_default();
    let trimmed = normalized.trim();
    if trimmed.is_empty() {
        return "File".to_string();
    }
    trimmed.to_string()
}

fn normalize_dialog_title(title: Option<String>) -> Option<String> {
    let normalized = title.unwrap_or_default();
    let trimmed = normalized.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.to_string())
}

fn apply_extension_filter(
    mut dialog: rfd::FileDialog,
    filter_label: Option<String>,
    normalized_extensions: &[String],
) -> rfd::FileDialog {
    if normalized_extensions.is_empty() {
        return dialog;
    }

    let filter_label = normalize_filter_label(filter_label);
    let extension_refs = normalized_extensions.iter().map(String::as_str).collect::<Vec<&str>>();
    dialog = dialog.add_filter(filter_label.as_str(), &extension_refs);
    dialog
}

fn apply_dialog_title(dialog: rfd::FileDialog, title: Option<String>) -> rfd::FileDialog {
    let Some(title) = normalize_dialog_title(title) else {
        return dialog;
    };

    dialog.set_title(title)
}

fn apply_suggested_path(dialog: rfd::FileDialog, suggested_path: Option<String>) -> rfd::FileDialog {
    let (directory, file_name) = resolve_suggested_path_parts(suggested_path);

    let dialog = if let Some(directory) = directory {
        dialog.set_directory(directory)
    } else {
        dialog
    };

    let Some(file_name) = file_name else {
        return dialog;
    };

    dialog.set_file_name(file_name)
}

fn resolve_suggested_path_parts(suggested_path: Option<String>) -> (Option<PathBuf>, Option<String>) {
    let Some(path_text) = suggested_path else {
        return (None, None);
    };

    let trimmed = path_text.trim();
    if trimmed.is_empty() {
        return (None, None);
    }

    let path = PathBuf::from(trimmed);
    let directory = path.parent().map(PathBuf::from);
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    (directory, file_name)
}

#[tauri::command]
pub fn window_minimize<R: tauri::Runtime>(window: tauri::Window<R>) -> Result<(), String> {
    window.minimize().map_err(|err| err.to_string())
}

#[tauri::command]
pub fn window_toggle_maximize<R: tauri::Runtime>(window: tauri::Window<R>) -> Result<(), String> {
    let is_maximized = window.is_maximized().map_err(|err| err.to_string())?;
    if is_maximized {
        window.unmaximize().map_err(|err| err.to_string())
    } else {
        window.maximize().map_err(|err| err.to_string())
    }
}

#[tauri::command]
pub fn window_close<R: tauri::Runtime>(window: tauri::Window<R>) -> Result<(), String> {
    window.close().map_err(|err| err.to_string())
}

#[tauri::command]
pub fn window_destroy<R: tauri::Runtime>(window: tauri::Window<R>) -> Result<(), String> {
    window.destroy().map_err(|err| err.to_string())
}

#[tauri::command]
pub fn window_is_maximized<R: tauri::Runtime>(window: tauri::Window<R>) -> Result<bool, String> {
    window.is_maximized().map_err(|err| err.to_string())
}

#[tauri::command]
pub fn start_drag<R: tauri::Runtime>(window: tauri::Window<R>) -> Result<(), String> {
    println!("Start drag");
    window.start_dragging().map_err(|err| err.to_string())
}

#[tauri::command]
pub fn open_file_dialog(
    allowed_extensions: Option<Vec<String>>,
    filter_label: Option<String>,
    title: Option<String>,
) -> Result<Option<String>, String> {
    let normalized_extensions = normalize_extensions(allowed_extensions);
    let dialog = apply_dialog_title(rfd::FileDialog::new(), title);
    let dialog = apply_extension_filter(dialog, filter_label, &normalized_extensions);
    Ok(dialog.pick_file().map(|path| path.to_string_lossy().to_string()))
}

#[tauri::command]
pub fn save_file_dialog(
    suggested_path: Option<String>,
    allowed_extensions: Option<Vec<String>>,
    filter_label: Option<String>,
    title: Option<String>,
) -> Result<Option<String>, String> {
    let normalized_extensions = normalize_extensions(allowed_extensions);
    let dialog = apply_dialog_title(rfd::FileDialog::new(), title);
    let dialog = apply_extension_filter(dialog, filter_label, &normalized_extensions);
    let dialog = apply_suggested_path(dialog, suggested_path);
    Ok(dialog.save_file().map(|path| path.to_string_lossy().to_string()))
}
