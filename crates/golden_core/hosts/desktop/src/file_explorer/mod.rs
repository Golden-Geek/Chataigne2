//! Bounded local directory and preview reads for the trusted desktop workbench.
use image::{ImageFormat, ImageReader, Limits};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
    sync::Mutex,
};
use ts_rs::{Config, TS};

const PAGE_SIZE: usize = 96;
const MAX_SUFFIXES: usize = 32;
const THUMBNAIL_SOURCE_BYTES: u64 = 64 * 1024 * 1024;
static THUMBNAIL_WORKER: Mutex<()> = Mutex::new(());

#[derive(Serialize, TS)]
pub(crate) struct FileExplorerEntry {
    name: String,
    path: String,
    directory: bool,
}

#[derive(Deserialize, Serialize, TS)]
pub(crate) struct FileExplorerCursor {
    name: String,
    directory: bool,
}

#[derive(Serialize, TS)]
pub(crate) struct FileExplorerPage {
    root: String,
    directory: String,
    entries: Vec<FileExplorerEntry>,
    next: Option<FileExplorerCursor>,
}

/// Exports the desktop-only File Explorer contract consumed by Golden UI.
pub fn export_file_explorer_typescript(directory: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    FileExplorerPage::export_all(&Config::new().with_out_dir(directory))?;
    Ok(())
}

fn root_path(folder: Option<&str>, project: Option<&str>) -> Option<PathBuf> {
    folder
        .filter(|folder| !folder.is_empty())
        .map(PathBuf::from)
        .or_else(|| project.and_then(|file| Path::new(file).parent().map(Path::to_path_buf)))
}

fn within(root: &Path, path: &Path) -> io::Result<()> {
    if !root.is_absolute() || !path.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Choose an absolute project folder",
        ));
    }
    let root = root.canonicalize()?;
    if !path.canonicalize()?.starts_with(root) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Path is outside the project folder",
        ));
    }
    Ok(())
}

fn key(name: &str, directory: bool) -> (bool, String, String) {
    (!directory, name.to_lowercase(), name.into())
}

fn browse(
    root: &Path,
    directory: &Path,
    after: Option<FileExplorerCursor>,
    suffixes: &[String],
) -> io::Result<FileExplorerPage> {
    within(root, directory)?;
    let after = after.map(|cursor| key(&cursor.name, cursor.directory));
    let mut page = BTreeMap::new();
    for entry in fs::read_dir(directory)? {
        let Ok(entry) = entry else { continue };
        let Ok(kind) = entry.file_type() else { continue };
        if !kind.is_dir() && !kind.is_file() {
            continue;
        }
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        let order = key(&name, kind.is_dir());
        if !kind.is_dir() && !suffixes.is_empty() && !suffixes.iter().any(|suffix| order.1.ends_with(suffix)) {
            continue;
        }
        if after.as_ref().is_some_and(|after| order <= *after) {
            continue;
        }
        let Ok(path) = entry.path().into_os_string().into_string() else {
            continue;
        };
        page.insert(
            order,
            FileExplorerEntry {
                name,
                path,
                directory: kind.is_dir(),
            },
        );
        if page.len() > PAGE_SIZE + 1 {
            page.pop_last();
        }
    }
    let mut entries = page.into_values().collect::<Vec<_>>();
    let next = if entries.len() > PAGE_SIZE {
        entries.pop();
        entries.last().map(|entry| FileExplorerCursor {
            name: entry.name.clone(),
            directory: entry.directory,
        })
    } else {
        None
    };
    Ok(FileExplorerPage {
        root: root.to_string_lossy().into_owned(),
        directory: directory.to_string_lossy().into_owned(),
        entries,
        next,
    })
}

fn file_suffixes(mut suffixes: Vec<String>) -> Result<Vec<String>, String> {
    if suffixes.len() > MAX_SUFFIXES
        || suffixes.iter().any(|suffix| {
            suffix.len() > 32
                || suffix.strip_prefix('.').is_none_or(|rest| {
                    rest.split('.').any(|part| {
                        part.is_empty()
                            || !part
                                .bytes()
                                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
                    })
                })
        })
    {
        return Err("Invalid File Explorer suffix filter".into());
    }
    for suffix in &mut suffixes {
        suffix.make_ascii_lowercase();
    }
    Ok(suffixes)
}

fn thumbnail(root: &Path, path: &Path) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    within(root, path)?;
    let metadata = path.metadata()?;
    if !metadata.is_file() || metadata.len() > THUMBNAIL_SOURCE_BYTES {
        return Err("File exceeds the thumbnail budget".into());
    }
    let _worker = THUMBNAIL_WORKER.lock().unwrap_or_else(|error| error.into_inner());
    let mut reader = ImageReader::open(path)?.with_guessed_format()?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(16_384);
    limits.max_image_height = Some(16_384);
    limits.max_alloc = Some(96 * 1024 * 1024);
    reader.limits(limits);
    let image = reader.decode()?.thumbnail(144, 108);
    let mut output = io::Cursor::new(Vec::new());
    image.write_to(&mut output, ImageFormat::Png)?;
    Ok(output.into_inner())
}

#[tauri::command(rename_all = "snake_case")]
pub(crate) async fn file_explorer_browse(
    folder: Option<String>,
    project: Option<String>,
    directory: Option<String>,
    after: Option<FileExplorerCursor>,
    suffixes: Option<Vec<String>>,
) -> Result<Option<FileExplorerPage>, String> {
    let Some(root) = root_path(folder.as_deref(), project.as_deref()) else {
        return Ok(None);
    };
    let directory = directory.map_or_else(|| root.clone(), PathBuf::from);
    let suffixes = file_suffixes(suffixes.unwrap_or_default())?;
    tauri::async_runtime::spawn_blocking(move || {
        browse(&root, &directory, after, &suffixes)
            .map(Some)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command(rename_all = "snake_case")]
pub(crate) async fn file_explorer_thumbnail(root: String, path: String) -> Result<tauri::ipc::Response, String> {
    tauri::async_runtime::spawn_blocking(move || {
        thumbnail(Path::new(&root), Path::new(&path))
            .map(tauri::ipc::Response::new)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[cfg(test)]
mod tests;
