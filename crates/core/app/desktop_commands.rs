use std::collections::BTreeSet;
use std::path::PathBuf;

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

fn apply_extension_filter(
    mut dialog: rfd::FileDialog,
    normalized_extensions: &[String],
) -> rfd::FileDialog {
    if normalized_extensions.is_empty() {
        return dialog;
    }

    let extension_refs = normalized_extensions
        .iter()
        .map(String::as_str)
        .collect::<Vec<&str>>();
    dialog = dialog.add_filter("Allowed files", &extension_refs);
    dialog
}

fn apply_suggested_path(dialog: rfd::FileDialog, suggested_path: Option<String>) -> rfd::FileDialog {
    let Some(path_text) = suggested_path else {
        return dialog;
    };

    let trimmed = path_text.trim();
    if trimmed.is_empty() {
        return dialog;
    }

    let path = PathBuf::from(trimmed);
    let dialog = if let Some(parent) = path.parent() {
        dialog.set_directory(parent)
    } else {
        dialog
    };

    if let Some(file_name) = path.file_name().and_then(|value| value.to_str()) {
        if !file_name.trim().is_empty() {
            return dialog.set_file_name(file_name);
        }
    }

    dialog
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
pub fn window_is_maximized<R: tauri::Runtime>(window: tauri::Window<R>) -> Result<bool, String> {
    window.is_maximized().map_err(|err| err.to_string())
}

#[tauri::command]
pub fn start_drag<R: tauri::Runtime>(window: tauri::Window<R>) -> Result<(), String> {
    println!("Start drag");
    window.start_dragging().map_err(|err| err.to_string())
}

#[tauri::command]
pub fn open_file_dialog(allowed_extensions: Option<Vec<String>>) -> Result<Option<String>, String> {
    let normalized_extensions = normalize_extensions(allowed_extensions);
    let dialog = apply_extension_filter(rfd::FileDialog::new(), &normalized_extensions);

    Ok(dialog.pick_file().map(|path| path.to_string_lossy().to_string()))
}

#[tauri::command]
pub fn save_file_dialog(
    suggested_path: Option<String>,
    allowed_extensions: Option<Vec<String>>,
) -> Result<Option<String>, String> {
    let normalized_extensions = normalize_extensions(allowed_extensions);
    let dialog = apply_extension_filter(rfd::FileDialog::new(), &normalized_extensions);
    let dialog = apply_suggested_path(dialog, suggested_path);

    Ok(dialog.save_file().map(|path| path.to_string_lossy().to_string()))
}