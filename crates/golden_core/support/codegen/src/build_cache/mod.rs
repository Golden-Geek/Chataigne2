//! Content-based caching for external build steps. Stamps are disposable build
//! artifacts, scoped by the caller to its Cargo output directory.

use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io;
use std::path::{Path, PathBuf};

/// Runs `build` when inputs, context, or output contents differ from the last
/// successful invocation. Returns whether the build ran. The caller supplies
/// every input file and a context identifying its toolchain and environment.
pub fn build_if_stale(
    stamp: &Path,
    output: &Path,
    inputs: &[PathBuf],
    context: &str,
    force: bool,
    build: impl FnOnce() -> io::Result<()>,
) -> io::Result<bool> {
    let input_hash = fingerprint_files(inputs, context)?;
    if !force
        && let Ok(previous) = fs::read_to_string(stamp)
        && let Some(output_hash) = fingerprint_output(output)?
        && previous == format!("v1:{input_hash}:{output_hash}")
    {
        return Ok(false);
    }

    // A failed or interrupted build must never leave a valid cache entry.
    match fs::remove_file(stamp) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    build()?;
    let output_hash =
        fingerprint_output(output)?.ok_or_else(|| io::Error::other("external build produced no output files"))?;
    // Inputs may change while the external process is building them. Let the
    // next invocation rebuild rather than bless a potentially mixed bundle.
    if input_hash == fingerprint_files(inputs, context)? {
        fs::write(stamp, format!("v1:{input_hash}:{output_hash}"))?;
    }
    Ok(true)
}

fn fingerprint_files(files: &[PathBuf], context: &str) -> io::Result<String> {
    let mut hasher = DefaultHasher::new();
    context.hash(&mut hasher);
    let mut files = files.to_vec();
    files.sort();
    files.dedup();
    for path in files {
        path.hash(&mut hasher);
        fs::read(path)?.hash(&mut hasher);
    }
    Ok(format!("{:016x}", hasher.finish()))
}

fn fingerprint_output(root: &Path) -> io::Result<Option<String>> {
    let mut files = Vec::new();
    match collect_files(root, &mut files) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    }
    if files.is_empty() {
        return Ok(None);
    }
    fingerprint_files(&files, "").map(Some)
}

fn collect_files(root: &Path, files: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            collect_files(&path, files)?;
        } else {
            files.push(path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
