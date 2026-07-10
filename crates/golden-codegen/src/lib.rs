//! Deterministic generated-artifact writing and drift verification.

use std::{
    fs,
    path::{Component, Path, PathBuf},
};

use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodegenMode {
    Write,
    Check,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedArtifact {
    pub relative_path: PathBuf,
    pub contents: String,
}

pub fn apply_artifacts(
    root: &Path,
    artifacts: impl IntoIterator<Item = GeneratedArtifact>,
    mode: CodegenMode,
) -> Result<(), CodegenError> {
    for artifact in artifacts {
        validate_relative_path(&artifact.relative_path)?;
        let destination = root.join(&artifact.relative_path);
        let expected = normalize_newlines(&artifact.contents);
        match mode {
            CodegenMode::Write => {
                if let Some(parent) = destination.parent() {
                    fs::create_dir_all(parent).map_err(|source| CodegenError::Io {
                        path: parent.to_path_buf(),
                        source,
                    })?;
                }
                fs::write(&destination, expected).map_err(|source| CodegenError::Io {
                    path: destination,
                    source,
                })?;
            }
            CodegenMode::Check => {
                let actual = fs::read_to_string(&destination).map_err(|source| CodegenError::Io {
                    path: destination.clone(),
                    source,
                })?;
                if normalize_newlines(&actual) != expected {
                    return Err(CodegenError::Drift(destination));
                }
            }
        }
    }
    Ok(())
}

fn validate_relative_path(path: &Path) -> Result<(), CodegenError> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(CodegenError::UnsafePath(path.to_path_buf()));
    }
    Ok(())
}

fn normalize_newlines(contents: &str) -> String {
    contents.replace("\r\n", "\n")
}

#[derive(Debug, Error)]
pub enum CodegenError {
    #[error("generated artifact path is not workspace-relative: {0}")]
    UnsafePath(PathBuf),
    #[error("generated artifact differs from its source: {0}")]
    Drift(PathBuf),
    #[error("generated artifact IO failed for {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

#[cfg(test)]
mod tests;
