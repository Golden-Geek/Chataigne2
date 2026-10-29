use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use ts_rs::TS;

/// Maximum encoded UTF-8 bytes in an authored asset path.
pub const MAX_ASSET_PATH_BYTES: usize = 32_768;

/// An invalid authored locator. Validation performs no filesystem access.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum LocatorError {
    /// An asset path must name a source.
    #[error("asset path must not be empty")]
    Empty,
    /// A source path exceeded the fixed serialization/admission bound.
    #[error("asset path exceeds the maximum UTF-8 byte length")]
    TooLong,
    /// Native paths and serialized strings cannot contain a NUL byte.
    #[error("asset path must not contain NUL")]
    Nul,
    /// Project-relative paths use portable, nonempty forward-slash segments.
    #[error(
        "project-relative asset path must use nonempty forward-slash segments without dot components, backslash or colon"
    )]
    InvalidRelative,
    /// Absolute locators must follow the current target's native absolute-path rules.
    #[error("asset path must be absolute on this platform")]
    NotAbsolute,
}

fn validate_length(path: &str) -> Result<(), LocatorError> {
    if path.is_empty() {
        return Err(LocatorError::Empty);
    }
    if path.len() > MAX_ASSET_PATH_BYTES {
        return Err(LocatorError::TooLong);
    }
    if path.contains('\0') {
        return Err(LocatorError::Nul);
    }
    Ok(())
}

/// Portable project-relative source path. It does not establish filesystem containment:
/// symlinks, mount points and concurrent filesystem changes are resolved only by the worker.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, TS)]
#[ts(type = "string")]
pub struct RelativeAssetPath(Box<str>);

impl<'de> Deserialize<'de> for RelativeAssetPath {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer)?
            .try_into()
            .map_err(serde::de::Error::custom)
    }
}

impl RelativeAssetPath {
    /// Returns the validated portable UTF-8 path.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for RelativeAssetPath {
    type Error = LocatorError;

    fn try_from(path: String) -> Result<Self, Self::Error> {
        validate_length(&path)?;
        if path.contains(['\\', ':'])
            || path
                .split('/')
                .any(|segment| segment.is_empty() || segment == "." || segment == "..")
        {
            return Err(LocatorError::InvalidRelative);
        }
        Ok(Self(path.into_boxed_str()))
    }
}

impl From<RelativeAssetPath> for String {
    fn from(path: RelativeAssetPath) -> Self {
        path.0.into_string()
    }
}

/// Machine-specific source path using the current target's native absolute-path rules.
/// Persist project-relative paths when sources should follow project relocation.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, TS)]
#[ts(type = "string")]
pub struct AbsoluteAssetPath(Box<str>);

impl<'de> Deserialize<'de> for AbsoluteAssetPath {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer)?
            .try_into()
            .map_err(serde::de::Error::custom)
    }
}

impl AbsoluteAssetPath {
    /// Returns the validated native absolute UTF-8 path.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for AbsoluteAssetPath {
    type Error = LocatorError;

    fn try_from(path: String) -> Result<Self, Self::Error> {
        validate_length(&path)?;
        if !Path::new(&path).is_absolute() {
            return Err(LocatorError::NotAbsolute);
        }
        Ok(Self(path.into_boxed_str()))
    }
}

impl From<AbsoluteAssetPath> for String {
    fn from(path: AbsoluteAssetPath) -> Self {
        path.0.into_string()
    }
}

/// Authored source location, separate from canonical paths and disposable content fingerprints.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AssetLocator {
    /// Portable path resolved against an explicit project directory.
    ProjectRelative {
        /// Validated path without traversal components.
        path: RelativeAssetPath,
    },
    /// Machine-specific native absolute path.
    Absolute {
        /// Validated native absolute path.
        path: AbsoluteAssetPath,
    },
}

impl AssetLocator {
    /// Resolves lexically without IO, canonicalization or checking source availability.
    /// The caller supplies the project directory and performs filesystem work on its worker.
    /// This operation is not a filesystem sandbox or a containment/security guarantee.
    pub fn resolve(&self, project_directory: &Path) -> PathBuf {
        match self {
            Self::ProjectRelative { path } => project_directory.join(path.as_str()),
            Self::Absolute { path } => PathBuf::from(path.as_str()),
        }
    }
}
