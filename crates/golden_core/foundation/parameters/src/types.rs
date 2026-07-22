//! Strongly typed parameter value wrappers.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::{Path, PathBuf};
use ts_rs::TS;

/// Strongly-typed file path wrapper for parameter handles and params DSL.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
pub struct File(pub String);

impl File {
    /// Creates a new file value from a path-like string.
    pub fn new(path: impl Into<String>) -> Self {
        Self(path.into())
    }

    /// Returns the wrapped path string.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns this file value as a [`Path`].
    pub fn as_path(&self) -> &Path {
        Path::new(&self.0)
    }

    /// Consumes this wrapper and returns the inner path string.
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl From<String> for File {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for File {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl From<PathBuf> for File {
    fn from(value: PathBuf) -> Self {
        Self(value.to_string_lossy().to_string())
    }
}

impl From<File> for String {
    fn from(value: File) -> Self {
        value.0
    }
}

impl From<File> for PathBuf {
    fn from(value: File) -> Self {
        PathBuf::from(value.0)
    }
}

/// CSS unit used by [`CssValue`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum CssUnit {
    /// CSS pixels.
    Px,
    /// Root-em units.
    #[default]
    Rem,
    /// Element-em units.
    Em,
    /// Percentage values.
    Percent,
    /// Viewport width percentage.
    Vw,
    /// Viewport height percentage.
    Vh,
}

impl CssUnit {
    /// Returns the CSS suffix used when formatting this unit.
    pub fn suffix(self) -> &'static str {
        match self {
            Self::Px => "px",
            Self::Rem => "rem",
            Self::Em => "em",
            Self::Percent => "%",
            Self::Vw => "vw",
            Self::Vh => "vh",
        }
    }
}

impl fmt::Display for CssUnit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.suffix())
    }
}

/// CSS scalar value with an explicit unit.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
pub struct CssValue {
    /// Numeric component.
    pub value: f64,
    /// CSS unit.
    pub unit: CssUnit,
}

impl CssValue {
    /// Creates a new CSS value.
    pub fn new(value: f64, unit: CssUnit) -> Self {
        Self { value, unit }
    }

    /// Parses a CSS value string such as `12rem` or `50%`.
    pub fn parse(value: &str) -> Option<Self> {
        Self::parse_with_default_unit(value, None)
    }

    /// Parses a CSS value string, accepting raw numbers via `default_unit`.
    pub fn parse_with_default_unit(value: &str, default_unit: Option<CssUnit>) -> Option<Self> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return None;
        }

        let lowercase = trimmed.to_ascii_lowercase();
        for (suffix, unit) in [
            ("rem", CssUnit::Rem),
            ("px", CssUnit::Px),
            ("em", CssUnit::Em),
            ("vw", CssUnit::Vw),
            ("vh", CssUnit::Vh),
            ("%", CssUnit::Percent),
        ] {
            if lowercase.ends_with(suffix) {
                let number_text = trimmed[..trimmed.len() - suffix.len()].trim();
                let parsed = number_text.parse::<f64>().ok()?;
                return Some(Self::new(parsed, unit));
            }
        }

        let unit = default_unit?;
        trimmed.parse::<f64>().ok().map(|parsed| Self::new(parsed, unit))
    }

    /// Formats this CSS value as a CSS string.
    pub fn to_css_string(self) -> String {
        format!("{}{}", self.value, self.unit.suffix())
    }
}

impl fmt::Display for CssValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_css_string())
    }
}

impl From<(f64, CssUnit)> for CssValue {
    fn from(value: (f64, CssUnit)) -> Self {
        Self::new(value.0, value.1)
    }
}

impl From<CssValue> for (f64, CssUnit) {
    fn from(value: CssValue) -> Self {
        (value.value, value.unit)
    }
}

/// Strongly-typed 2D vector value for parameter handles and params DSL.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
pub struct Vec2 {
    /// X component.
    pub x: f64,
    /// Y component.
    pub y: f64,
}

impl Vec2 {
    /// Creates a new 2D vector.
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

impl From<(f64, f64)> for Vec2 {
    fn from(value: (f64, f64)) -> Self {
        Self::new(value.0, value.1)
    }
}

impl From<Vec2> for (f64, f64) {
    fn from(value: Vec2) -> Self {
        (value.x, value.y)
    }
}

/// Strongly-typed 3D vector value for parameter handles and params DSL.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
pub struct Vec3 {
    /// X component.
    pub x: f64,
    /// Y component.
    pub y: f64,
    /// Z component.
    pub z: f64,
}

impl Vec3 {
    /// Creates a new 3D vector.
    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }
}

impl From<(f64, f64, f64)> for Vec3 {
    fn from(value: (f64, f64, f64)) -> Self {
        Self::new(value.0, value.1, value.2)
    }
}

impl From<Vec3> for (f64, f64, f64) {
    fn from(value: Vec3) -> Self {
        (value.x, value.y, value.z)
    }
}

/// Strongly-typed enum variant wrapper for parameter handles and params DSL.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
pub struct Enum(pub String);

impl Enum {
    /// Creates a new enum value from a variant id.
    pub fn new(variant_id: impl Into<String>) -> Self {
        Self(variant_id.into())
    }

    /// Returns the wrapped variant id.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consumes the wrapper and returns the inner variant id.
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl From<String> for Enum {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for Enum {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl From<Enum> for String {
    fn from(value: Enum) -> Self {
        value.0
    }
}
