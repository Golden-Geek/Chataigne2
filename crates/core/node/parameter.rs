use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::{
    node::{Node, NodeData, NodeReference, NodeUuid},
    process_ctx::ProcessCtx,
};

pub use crate::color::Color;

/// Runtime value variants used by parameter nodes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ParamValue {
    /// Trigger-like pulse with no payload.
    Trigger(),

    /// Signed integer value.
    Int(i32),
    /// Floating-point value.
    Float(f64),
    /// UTF-8 string value.
    Str(String),
    /// File path value.
    File(String),
    /// Enum variant identifier.
    Enum(String),
    /// Boolean value.
    Bool(bool),

    /// 2D vector value.
    Vec2(f64, f64),
    /// 3D vector value.
    Vec3(f64, f64, f64),
    /// RGBA color value.
    Color(f64, f64, f64, f64),

    /// Reference to another node.
    Reference(NodeReference),
}

/// Strongly-typed file path wrapper for parameter handles and params DSL.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

/// Strongly-typed 2D vector value for parameter handles and params DSL.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
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
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
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
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

//implement into for ParamValue
impl From<i32> for ParamValue {
    fn from(value: i32) -> Self {
        ParamValue::Int(value)
    }
}

impl From<f64> for ParamValue {
    fn from(value: f64) -> Self {
        ParamValue::Float(value)
    }
}

impl From<String> for ParamValue {
    fn from(value: String) -> Self {
        ParamValue::Str(value)
    }
}

impl From<&str> for ParamValue {
    fn from(value: &str) -> Self {
        ParamValue::Str(value.to_string())
    }
}

impl From<File> for ParamValue {
    fn from(value: File) -> Self {
        ParamValue::File(value.into_inner())
    }
}

impl From<std::path::PathBuf> for ParamValue {
    fn from(value: std::path::PathBuf) -> Self {
        ParamValue::File(value.to_string_lossy().to_string())
    }
}

impl From<bool> for ParamValue {
    fn from(value: bool) -> Self {
        ParamValue::Bool(value)
    }
}

impl From<(f64, f64)> for ParamValue {
    fn from(value: (f64, f64)) -> Self {
        ParamValue::Vec2(value.0, value.1)
    }
}

impl From<(f64, f64, f64)> for ParamValue {
    fn from(value: (f64, f64, f64)) -> Self {
        ParamValue::Vec3(value.0, value.1, value.2)
    }
}

impl From<(f64, f64, f64, f64)> for ParamValue {
    fn from(value: (f64, f64, f64, f64)) -> Self {
        ParamValue::Color(value.0, value.1, value.2, value.3)
    }
}

impl From<Vec2> for ParamValue {
    fn from(value: Vec2) -> Self {
        ParamValue::Vec2(value.x, value.y)
    }
}

impl From<Vec3> for ParamValue {
    fn from(value: Vec3) -> Self {
        ParamValue::Vec3(value.x, value.y, value.z)
    }
}

impl From<Color> for ParamValue {
    fn from(value: Color) -> Self {
        ParamValue::Color(value.r(), value.g(), value.b(), value.a())
    }
}

impl From<Enum> for ParamValue {
    fn from(value: Enum) -> Self {
        ParamValue::Enum(value.into_inner())
    }
}

impl From<NodeReference> for ParamValue {
    fn from(value: NodeReference) -> Self {
        ParamValue::Reference(value)
    }
}

impl From<NodeUuid> for ParamValue {
    fn from(value: NodeUuid) -> Self {
        ParamValue::Reference(NodeReference::new(value))
    }
}

//Implement value coercion
impl ParamValue {
    /// Coerces this value into an integer, when possible.
    pub fn as_int(&self) -> Option<i32> {
        match self {
            ParamValue::Int(i) => Some(*i),
            ParamValue::Float(f) => Some(*f as i32),
            ParamValue::Str(s) | ParamValue::Enum(s) => s.parse().ok(),
            ParamValue::Bool(b) => Some(if *b { 1 } else { 0 }),
            _ => None,
        }
    }

    /// Coerces this value into a floating-point value, when possible.
    pub fn as_float(&self) -> Option<f64> {
        match self {
            ParamValue::Int(i) => Some(*i as f64),
            ParamValue::Float(f) => Some(*f),
            ParamValue::Str(s) | ParamValue::Enum(s) => s.parse().ok(),
            ParamValue::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
            _ => None,
        }
    }

    /// Coerces this value into a string, when possible.
    pub fn as_str(&self) -> Option<String> {
        match self {
            ParamValue::Int(i) => Some(i.to_string()),
            ParamValue::Float(f) => Some(f.to_string()),
            ParamValue::Str(s) | ParamValue::File(s) | ParamValue::Enum(s) => Some(s.clone()),
            ParamValue::Bool(b) => Some(b.to_string()),
            _ => None,
        }
    }

    /// Coerces this value into an enum variant id, when possible.
    pub fn as_enum(&self) -> Option<String> {
        match self {
            ParamValue::Enum(variant_id) => Some(variant_id.clone()),
            ParamValue::Str(s) => Some(s.clone()),
            _ => None,
        }
    }

    /// Coerces this value into a boolean, when possible.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            ParamValue::Int(i) => Some(*i != 0),
            ParamValue::Float(f) => Some(*f != 0.0),
            ParamValue::Str(s) | ParamValue::Enum(s) => s.parse().ok(),
            ParamValue::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Coerces this value into a 2D vector, when possible.
    pub fn as_vec2(&self) -> Option<(f64, f64)> {
        match self {
            ParamValue::Int(i) => Some((*i as f64, *i as f64)),
            ParamValue::Float(f) => Some((*f, *f)),
            ParamValue::Str(s) | ParamValue::Enum(s) => {
                let parts: Vec<&str> = s.split(',').collect();
                if parts.len() == 2 {
                    if let (Ok(x), Ok(y)) = (parts[0].trim().parse(), parts[1].trim().parse()) {
                        return Some((x, y));
                    }
                }
                None
            }
            ParamValue::Vec2(x, y) => Some((*x, *y)),
            _ => None,
        }
    }

    /// Coerces this value into a 3D vector, when possible.
    pub fn as_vec3(&self) -> Option<(f64, f64, f64)> {
        match self {
            ParamValue::Int(i) => Some((*i as f64, *i as f64, *i as f64)),
            ParamValue::Float(f) => Some((*f, *f, *f)),
            ParamValue::Str(s) | ParamValue::Enum(s) => {
                let parts: Vec<&str> = s.split(',').collect();
                if parts.len() == 3 {
                    if let (Ok(x), Ok(y), Ok(z)) = (parts[0].trim().parse(), parts[1].trim().parse(), parts[2].trim().parse()) {
                        return Some((x, y, z));
                    }
                }
                None
            }
            ParamValue::Vec3(x, y, z) => Some((*x, *y, *z)),
            ParamValue::Color(r, g, b, _) => Some((*r, *g, *b)),
            _ => None,
        }
    }

    /// Coerces this value into an RGBA color, when possible.
    pub fn as_color(&self) -> Option<(f64, f64, f64, f64)> {
        match self {
            ParamValue::Int(i) => Some(((*i as f64 / 255.0), (*i as f64 / 255.0), (*i as f64 / 255.0), 1.0)),
            ParamValue::Float(f) => Some((*f, *f, *f, 1.0)),
            ParamValue::Str(s) | ParamValue::Enum(s) => {
                let parts: Vec<&str> = s.split(',').collect();
                if parts.len() == 4 {
                    if let (Ok(r), Ok(g), Ok(b), Ok(a)) = (parts[0].trim().parse(), parts[1].trim().parse(), parts[2].trim().parse(), parts[3].trim().parse()) {
                        return Some((r, g, b, a));
                    }
                }
                None
            }
            ParamValue::Color(r, g, b, a) => Some((*r, *g, *b, *a)),
            _ => None,
        }
    }
}

/// Strategy used to decide whether a `set` call should enqueue an edit.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Default, Deserialize)]
pub enum ParameterChangeCheck {
    /// Emit only when the value differs.
    #[default]
    ValueChange,
    /// Always emit, even if unchanged.
    None,
}

/// Strategy for handling multiple parameter changes within the same process tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Default, Deserialize)]
pub enum ParameterEventBehaviour {
    /// Keep only the latest pending set for this parameter within a queue drain.
    #[default]
    Coalesce,
    /// Keep every pending set for this parameter within a queue drain.
    Append,
}

/// Data-level enum option descriptor used by validation and UI rendering.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterEnumOption {
    /// Stable enum variant id.
    pub variant_id: String,
    /// Value represented by this variant.
    pub value: ParamValue,
    /// Display label for this variant.
    pub label: String,
    /// Optional tags used for filtering/grouping.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Optional explicit ordering key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ordering: Option<i32>,
}

/// Policy used when incoming values do not match constraints.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ParameterConstraintPolicy {
    /// Clamp to min/max and snap to step when relevant.
    #[default]
    ClampAdapt,
    /// Reject values that violate constraints.
    Reject,
}

/// Root scope used to validate and recover reference parameters.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ReferenceRoot {
    /// Use engine root as reference root.
    #[default]
    EngineRoot,
    /// Resolve an explicit root by persistent UUID.
    Uuid(NodeUuid),
    /// Resolve root from the parameter owner using a relative decl-id path.
    RelativeToOwner {
        /// Child decl-id path under the owner node (parameter parent).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        path: Vec<String>,
    },
}

/// Target family accepted by a reference parameter.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ReferenceTargetKind {
    /// Any node type can be targeted.
    #[default]
    AnyNode,
    /// Only parameter nodes can be targeted.
    ParameterOnly,
}

/// Additional constraints specific to `ParamValue::Reference`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ReferenceConstraints {
    /// Root scope used by target validation and relative recovery.
    #[serde(default)]
    pub root: ReferenceRoot,
    /// High-level target family.
    #[serde(default)]
    pub target_kind: ReferenceTargetKind,
    /// Optional allowed runtime node types.
    ///
    /// Empty means all node types are accepted.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_node_types: Vec<String>,
    /// Optional allowed parameter value kinds (`int`, `float`, `str`, ...).
    ///
    /// Empty means all parameter kinds are accepted.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_parameter_types: Vec<String>,
    /// Optional app-defined runtime filter key looked up in the engine registry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_filter_key: Option<String>,
    /// Optional UI default search filter suggested by the engine/app.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_search_filter: Option<String>,
}

fn is_default_reference_constraints(value: &ReferenceConstraints) -> bool {
    *value == ReferenceConstraints::default()
}

/// Named groups of allowed file extensions for `ParamValue::File`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum FileTypeGroup {
    /// Common audio formats.
    #[default]
    Audio,
    /// Common video formats.
    Video,
    /// Script source formats.
    Script,
}

impl FileTypeGroup {
    /// Parses a group label used by runtime manifests and UI payloads.
    pub fn from_label(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "audio" => Some(Self::Audio),
            "video" => Some(Self::Video),
            "script" => Some(Self::Script),
            _ => None,
        }
    }

    /// Returns true when `extension` belongs to this group.
    pub fn matches_extension(self, extension: &str) -> bool {
        let extension = extension.trim().to_ascii_lowercase();
        match self {
            Self::Audio => matches!(
                extension.as_str(),
                "wav" | "wave" | "aif" | "aiff" | "flac" | "mp3" | "ogg" | "opus" | "m4a" | "aac" | "wma"
            ),
            Self::Video => matches!(
                extension.as_str(),
                "mp4" | "m4v" | "mov" | "avi" | "mkv" | "webm" | "mpg" | "mpeg" | "ts" | "m2ts" | "flv"
            ),
            Self::Script => matches!(extension.as_str(), "lua" | "luau" | "js" | "mjs" | "cjs"),
        }
    }
}

/// Additional constraints specific to `ParamValue::File`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct FileConstraints {
    /// Optional extension groups accepted by this file parameter.
    ///
    /// Empty means all groups are accepted.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_types: Vec<FileTypeGroup>,
    /// Optional explicit extension allow-list (`wav`, `.mp3`, ...).
    ///
    /// Empty means all extensions are accepted.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_extensions: Vec<String>,
}

impl FileConstraints {
    /// Normalizes one extension label (`.WAV` -> `wav`).
    pub fn normalize_extension_label(value: &str) -> Option<String> {
        let trimmed = value.trim().trim_start_matches('.');
        if trimmed.is_empty() {
            return None;
        }
        Some(trimmed.to_ascii_lowercase())
    }

    fn normalized_allowed_extensions(&self) -> Vec<String> {
        let mut normalized = Vec::new();
        for ext in &self.allowed_extensions {
            if let Some(ext) = Self::normalize_extension_label(ext) {
                normalized.push(ext);
            }
        }
        normalized
    }

    fn accepts_extension(&self, extension: &str) -> bool {
        let extension = extension.to_ascii_lowercase();

        let group_match = self.allowed_types.is_empty() || self.allowed_types.iter().any(|group| group.matches_extension(&extension));
        if !group_match {
            return false;
        }

        let allowed_extensions = self.normalized_allowed_extensions();
        allowed_extensions.is_empty() || allowed_extensions.iter().any(|allowed| allowed == &extension)
    }
}

fn is_default_file_constraints(value: &FileConstraints) -> bool {
    *value == FileConstraints::default()
}

/// Numeric range constraints for scalar and vector-like parameter values.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum RangeConstraint {
    /// One min/max pair applied uniformly.
    Uniform {
        /// Optional minimum bound.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        min: Option<f64>,
        /// Optional maximum bound.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max: Option<f64>,
    },
    /// Component-wise bounds for vector-like values.
    Components {
        /// Optional per-component minimum bounds.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        min: Option<Vec<f64>>,
        /// Optional per-component maximum bounds.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max: Option<Vec<f64>>,
    },
}

impl RangeConstraint {
    /// Builds a uniform range constraint when at least one bound is provided.
    pub fn uniform(min: Option<f64>, max: Option<f64>) -> Option<Self> {
        if min.is_none() && max.is_none() {
            None
        } else {
            Some(Self::Uniform { min, max })
        }
    }

    /// Builds a component-wise range constraint when at least one bound list is provided.
    pub fn components(min: Option<Vec<f64>>, max: Option<Vec<f64>>) -> Option<Self> {
        let min = min.filter(|values| !values.is_empty());
        let max = max.filter(|values| !values.is_empty());
        if min.is_none() && max.is_none() {
            None
        } else {
            Some(Self::Components { min, max })
        }
    }
}

/// Runtime data constraints for parameter values.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ParameterConstraints {
    /// Optional numeric range constraints.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range: Option<RangeConstraint>,
    /// Optional numeric step increment.
    ///
    /// Applies to scalar numeric values and each component of vector values.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step: Option<f64>,
    /// Optional base used for step snapping/validation.
    ///
    /// Applies to scalar numeric values and each component of vector values.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step_base: Option<f64>,
    /// Optional enum-domain constraints.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub enum_options: Vec<ParameterEnumOption>,
    /// Enforcement strategy for invalid incoming values.
    #[serde(default)]
    pub policy: ParameterConstraintPolicy,
    /// Reference-specific filtering and recovery constraints.
    #[serde(default, skip_serializing_if = "is_default_reference_constraints")]
    pub reference: ReferenceConstraints,
    /// File-specific extension constraints.
    #[serde(default, skip_serializing_if = "is_default_file_constraints")]
    pub file: FileConstraints,
}

impl ParameterConstraints {
    /// Normalizes or validates an incoming value according to constraint policy.
    pub fn normalize(&self, incoming: ParamValue) -> Result<ParamValue, String> {
        let mut normalized = match incoming {
            ParamValue::Int(value) => self.normalize_int(value)?,
            ParamValue::Float(value) => self.normalize_float(value)?,
            ParamValue::Vec2(x, y) => self.normalize_vec2(x, y)?,
            ParamValue::Vec3(x, y, z) => self.normalize_vec3(x, y, z)?,
            ParamValue::File(path) => self.normalize_file(path)?,
            other => other,
        };

        if !self.enum_options.is_empty() {
            let matches_value = self.enum_options.iter().any(|option| option.value == normalized);
            let matches_variant_id = self.enum_options.iter().any(|option| match &normalized {
                ParamValue::Enum(variant_id) => option.variant_id == *variant_id,
                ParamValue::Str(variant_id) => option.variant_id == *variant_id,
                _ => false,
            });

            if !matches_value && !matches_variant_id {
                let allowed: Vec<String> = self.enum_options.iter().map(|option| option.variant_id.clone()).collect();
                return Err(format!("value is not in enum options: allowed variants {:?}", allowed));
            }

            if let ParamValue::Str(variant_id) = &normalized {
                if self.enum_options.iter().any(|option| option.variant_id == *variant_id) {
                    normalized = ParamValue::Enum(variant_id.clone());
                }
            }
        }

        Ok(normalized)
    }

    fn normalize_int(&self, value: i32) -> Result<ParamValue, String> {
        let normalized = self.normalize_numeric(value as f64)?;
        let rounded = normalized.round();

        if self.policy == ParameterConstraintPolicy::Reject && (normalized - rounded).abs() > 1e-9 {
            return Err(format!("value {normalized} is not an integer"));
        }

        if rounded < i32::MIN as f64 || rounded > i32::MAX as f64 {
            return Err(format!("value {rounded} is outside i32 range"));
        }

        Ok(ParamValue::Int(rounded as i32))
    }

    fn normalize_float(&self, value: f64) -> Result<ParamValue, String> {
        Ok(ParamValue::Float(self.normalize_numeric(value)?))
    }

    fn normalize_vec2(&self, x: f64, y: f64) -> Result<ParamValue, String> {
        let bounds = self.vector_component_bounds(2, "vec2")?;
        let x = self
            .normalize_numeric_with_bounds(x, bounds[0].0, bounds[0].1)
            .map_err(|message| format!("vec2.x: {message}"))?;
        let y = self
            .normalize_numeric_with_bounds(y, bounds[1].0, bounds[1].1)
            .map_err(|message| format!("vec2.y: {message}"))?;
        Ok(ParamValue::Vec2(x, y))
    }

    fn normalize_vec3(&self, x: f64, y: f64, z: f64) -> Result<ParamValue, String> {
        let bounds = self.vector_component_bounds(3, "vec3")?;
        let x = self
            .normalize_numeric_with_bounds(x, bounds[0].0, bounds[0].1)
            .map_err(|message| format!("vec3.x: {message}"))?;
        let y = self
            .normalize_numeric_with_bounds(y, bounds[1].0, bounds[1].1)
            .map_err(|message| format!("vec3.y: {message}"))?;
        let z = self
            .normalize_numeric_with_bounds(z, bounds[2].0, bounds[2].1)
            .map_err(|message| format!("vec3.z: {message}"))?;
        Ok(ParamValue::Vec3(x, y, z))
    }

    fn normalize_numeric(&self, value: f64) -> Result<f64, String> {
        let (min, max) = self.scalar_bounds()?;
        self.normalize_numeric_with_bounds(value, min, max)
    }

    fn scalar_bounds(&self) -> Result<(Option<f64>, Option<f64>), String> {
        match &self.range {
            None => Ok((None, None)),
            Some(RangeConstraint::Uniform { min, max }) => {
                if let (Some(min), Some(max)) = (*min, *max) {
                    if min > max {
                        return Err(format!("invalid range: min {min} is greater than max {max}"));
                    }
                }
                Ok((*min, *max))
            }
            Some(RangeConstraint::Components { .. }) => {
                Err("component range constraints cannot be applied to scalar values".to_string())
            }
        }
    }

    fn vector_component_bounds(
        &self,
        dimensions: usize,
        value_kind: &str,
    ) -> Result<Vec<(Option<f64>, Option<f64>)>, String> {
        match &self.range {
            None => Ok(vec![(None, None); dimensions]),
            Some(RangeConstraint::Uniform { min, max }) => {
                if let (Some(min), Some(max)) = (*min, *max) {
                    if min > max {
                        return Err(format!("invalid range: min {min} is greater than max {max}"));
                    }
                }
                Ok(vec![(*min, *max); dimensions])
            }
            Some(RangeConstraint::Components { min, max }) => {
                if let Some(min_values) = min {
                    if min_values.len() != dimensions {
                        return Err(format!(
                            "invalid range: min has {} components but {} expects {}",
                            min_values.len(),
                            value_kind,
                            dimensions
                        ));
                    }
                }

                if let Some(max_values) = max {
                    if max_values.len() != dimensions {
                        return Err(format!(
                            "invalid range: max has {} components but {} expects {}",
                            max_values.len(),
                            value_kind,
                            dimensions
                        ));
                    }
                }

                let mut out = Vec::with_capacity(dimensions);
                for index in 0..dimensions {
                    let min_value = min.as_ref().and_then(|values| values.get(index)).copied();
                    let max_value = max.as_ref().and_then(|values| values.get(index)).copied();
                    if let (Some(min_value), Some(max_value)) = (min_value, max_value) {
                        if min_value > max_value {
                            return Err(format!(
                                "invalid range: {value_kind}[{index}] min {min_value} is greater than max {max_value}"
                            ));
                        }
                    }
                    out.push((min_value, max_value));
                }
                Ok(out)
            }
        }
    }

    fn normalize_numeric_with_bounds(&self, mut value: f64, min: Option<f64>, max: Option<f64>) -> Result<f64, String> {
        if let (Some(min), Some(max)) = (min, max) {
            if min > max {
                return Err(format!("invalid constraints: min {min} is greater than max {max}"));
            }
        }

        if let Some(min) = min {
            if value < min {
                match self.policy {
                    ParameterConstraintPolicy::ClampAdapt => value = min,
                    ParameterConstraintPolicy::Reject => return Err(format!("value {value} is lower than min {min}")),
                }
            }
        }

        if let Some(max) = max {
            if value > max {
                match self.policy {
                    ParameterConstraintPolicy::ClampAdapt => value = max,
                    ParameterConstraintPolicy::Reject => return Err(format!("value {value} is higher than max {max}")),
                }
            }
        }

        if let Some(step) = self.step {
            if step <= 0.0 {
                return Err(format!("invalid step {step}: expected positive value"));
            }

            let base = self.step_base.or(min).unwrap_or(0.0);
            let scaled = (value - base) / step;
            let nearest = scaled.round();

            match self.policy {
                ParameterConstraintPolicy::ClampAdapt => {
                    value = base + nearest * step;
                }
                ParameterConstraintPolicy::Reject => {
                    if (scaled - nearest).abs() > 1e-9 {
                        return Err(format!("value {value} does not align with step {step} from base {base}"));
                    }
                }
            }
        }

        if self.policy == ParameterConstraintPolicy::ClampAdapt {
            if let Some(min) = min {
                value = value.max(min);
            }
            if let Some(max) = max {
                value = value.min(max);
            }
        }

        Ok(value)
    }

    fn normalize_file(&self, path: String) -> Result<ParamValue, String> {
        if self.file.allowed_types.is_empty() && self.file.allowed_extensions.is_empty() {
            return Ok(ParamValue::File(path));
        }

        let extension = Path::new(&path)
            .extension()
            .and_then(|ext| ext.to_str())
            .and_then(FileConstraints::normalize_extension_label)
            .ok_or_else(|| "file extension is required by constraints".to_string())?;

        if !self.file.accepts_extension(&extension) {
            let allowed_types: Vec<&'static str> = self
                .file
                .allowed_types
                .iter()
                .map(|group| match group {
                    FileTypeGroup::Audio => "audio",
                    FileTypeGroup::Video => "video",
                    FileTypeGroup::Script => "script",
                })
                .collect();
            let allowed_extensions = self.file.normalized_allowed_extensions();
            return Err(format!(
                "file extension '.{extension}' is not allowed (allowed_types={allowed_types:?}, allowed_extensions={allowed_extensions:?})"
            ));
        }

        Ok(ParamValue::File(path))
    }
}

/// UI presentation hints for parameter editors.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ParameterUiHints {
    /// Preferred widget id (for example `slider`, `toggle`, `text`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub widget: Option<String>,
    /// Optional display unit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
}

/// Snapshot of parameter runtime state used for UI DTO projection.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterSnapshot {
    /// Current value.
    pub value: ParamValue,
    /// Declared default value.
    pub default_value: ParamValue,
    /// Change-check policy.
    pub change_check: ParameterChangeCheck,
    /// Event coalescing policy.
    pub event_behaviour: ParameterEventBehaviour,
    /// Read-only flag for editors.
    pub read_only: bool,
    /// Data constraints for this parameter value.
    pub constraints: ParameterConstraints,
    /// UI hints consumed by editor widgets.
    pub ui_hints: ParameterUiHints,
}

/// Built-in node type that stores a [`ParamValue`].
///
/// # Examples
/// ```rust
/// use golden_core::engine::EngineTime;
/// use golden_core::parameter::{ParamValue, Parameter, ParameterChangeCheck};
/// use golden_core::process_ctx::{ExecutionPhase, ProcessCtx};
///
/// let mut parameter = Parameter::new(
///     "gain",
///     ParamValue::Float(0.5),
///     ParameterChangeCheck::ValueChange,
/// );
/// let mut ctx = ProcessCtx::new(
///     ExecutionPhase::EngineTick,
///     EngineTime { tick: 0, micro: 0, seq: 0 },
/// );
///
/// parameter.set(&mut ctx, ParamValue::Float(0.75));
/// assert_eq!(ctx.edits.pending.len(), 1);
/// ```

pub struct Parameter {
    node_data: NodeData,
    /// Current parameter value.
    pub value: ParamValue,
    /// Declared default value.
    pub default_value: ParamValue,
    /// Change-detection policy for `set`.
    pub change_check: ParameterChangeCheck,

    /// Strategy for handling multiple parameter changes within the same process tick.
    pub event_behaviour: ParameterEventBehaviour,
    /// Whether this parameter is read-only for UI editing.
    pub read_only: bool,
    /// Data constraints used for clamping/validation/adaptation.
    pub constraints: ParameterConstraints,
    /// UI-facing editor hints.
    pub ui_hints: ParameterUiHints,
}

impl Parameter {
    /// Creates a new parameter node.
    pub fn new(label: &str, value: ParamValue, change_check: ParameterChangeCheck) -> Self {
        let mut node_data = NodeData::new(label.to_string());
        node_data.meta.can_be_disabled = false;
        let default_value = value.clone();

        Self {
            node_data,
            value,
            default_value,
            change_check,
            event_behaviour: ParameterEventBehaviour::Coalesce,
            read_only: false,
            constraints: ParameterConstraints::default(),
            ui_hints: ParameterUiHints::default(),
        }
    }

    /// Requests a parameter update through the process context.
    pub fn set(&mut self, ctx: &mut ProcessCtx, new_value: ParamValue) {
        let normalized = match self.constraints.normalize(new_value) {
            Ok(value) => value,
            Err(message) => {
                eprintln!("Attempted to set invalid value for parameter '{}': {message}", self.node_data().meta.label);
                return;
            }
        };

        let is_trigger = matches!(&normalized, ParamValue::Trigger());
        let value_changed = self.value != normalized;
        if is_trigger || self.change_check == ParameterChangeCheck::None || value_changed {
            ctx.set_param_with_behaviour(self.node_data().id, normalized, self.event_behaviour);
        }
    }

    /// Convenience method to fire a trigger parameter.
    pub fn fire(&mut self, ctx: &mut ProcessCtx) {
        // verify that it's a trigger
        if !matches!(self.value, ParamValue::Trigger()) {
            eprintln!("Attempted to fire a non-trigger parameter '{}'", self.node_data().meta.label);
            return;
        }
        self.set(ctx, ParamValue::Trigger());
    }

    /// Returns the current parameter value.
    pub fn get(&self) -> &ParamValue {
        &self.value
    }

    /// Returns a UI snapshot view of this parameter.
    pub fn snapshot(&self) -> ParameterSnapshot {
        ParameterSnapshot {
            value: self.value.clone(),
            default_value: self.default_value.clone(),
            change_check: self.change_check.clone(),
            event_behaviour: self.event_behaviour,
            read_only: self.read_only,
            constraints: self.constraints.clone(),
            ui_hints: self.ui_hints.clone(),
        }
    }
}

impl Node for Parameter {
    fn node_data(&self) -> &crate::node::NodeData {
        &self.node_data
    }

    fn node_data_mut(&mut self) -> &mut crate::node::NodeData {
        &mut self.node_data
    }

    fn get_type(&self) -> &str {
        match self.value {
            ParamValue::Trigger() => "trigger",
            ParamValue::Int(_) => "int",
            ParamValue::Float(_) => "float",
            ParamValue::Str(_) => "str",
            ParamValue::File(_) => "file",
            ParamValue::Enum(_) => "enum",
            ParamValue::Bool(_) => "bool",
            ParamValue::Vec2(_, _) => "vec2",
            ParamValue::Vec3(_, _, _) => "vec3",
            ParamValue::Color(_, _, _, _) => "color",
            ParamValue::Reference(_) => "reference",
        }
    }

    fn engine_set_param_value(&mut self, value: ParamValue) -> Option<ParamValue> {
        let old = std::mem::replace(&mut self.value, value);
        Some(old)
    }

    fn engine_prepare_param_value(&self, value: ParamValue) -> Result<ParamValue, String> {
        self.constraints.normalize(value)
    }

    fn engine_param_snapshot(&self) -> Option<crate::parameter::ParameterSnapshot> {
        Some(self.snapshot())
    }

    fn engine_visit_references_mut(&mut self, visit: &mut dyn FnMut(&mut NodeReference)) {
        if let ParamValue::Reference(reference) = &mut self.value {
            visit(reference);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_constraints_accept_matching_extension() {
        let constraints = ParameterConstraints {
            file: FileConstraints {
                allowed_types: vec![FileTypeGroup::Audio],
                allowed_extensions: vec![".WAV".to_string()],
            },
            ..Default::default()
        };

        let normalized = constraints
            .normalize(ParamValue::File("C:/tmp/kick.wav".to_string()))
            .expect("wav should pass file constraints");
        assert_eq!(normalized, ParamValue::File("C:/tmp/kick.wav".to_string()));
    }

    #[test]
    fn file_constraints_reject_non_matching_extension() {
        let constraints = ParameterConstraints {
            file: FileConstraints {
                allowed_types: vec![FileTypeGroup::Audio],
                allowed_extensions: vec!["wav".to_string(), "flac".to_string()],
            },
            ..Default::default()
        };

        let error = constraints
            .normalize(ParamValue::File("C:/tmp/clip.mp4".to_string()))
            .expect_err("mp4 should fail audio constraints");
        assert!(error.contains("not allowed"));
    }
}
