use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::Path;
use ts_rs::TS;

use crate::node::NodeUuid;

use super::control::{is_default_parameter_control_state, is_true};
use super::{CssValue, ParamValue, ParameterChangeCheck, ParameterControlState, ParameterEventBehaviour};

/// Data-level enum option descriptor used by validation and UI rendering.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default, TS)]
pub enum ParameterConstraintPolicy {
    /// Clamp to min/max and snap to step when relevant.
    #[default]
    ClampAdapt,
    /// Reject values that violate constraints.
    Reject,
}

/// Root scope used to validate and recover reference parameters.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default, TS)]
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default, TS)]
pub enum ReferenceTargetKind {
    /// Any node type can be targeted.
    #[default]
    AnyNode,
    /// Only parameter nodes can be targeted.
    ParameterOnly,
}

/// Additional constraints specific to `ParamValue::Reference`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
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
    /// Whether projection-based compatibility is accepted for typed references.
    ///
    /// When `false`, only direct type compatibility is accepted.
    #[serde(
        default = "reference_constraints_default_true",
        skip_serializing_if = "is_reference_constraints_true"
    )]
    pub allow_projections: bool,
    /// Optional app-defined runtime filter key looked up in the engine registry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_filter_key: Option<String>,
    /// Optional UI default search filter suggested by the engine/app.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_search_filter: Option<String>,
}

impl Default for ReferenceConstraints {
    fn default() -> Self {
        Self {
            root: ReferenceRoot::default(),
            target_kind: ReferenceTargetKind::default(),
            allowed_node_types: Vec::new(),
            allowed_parameter_types: Vec::new(),
            allow_projections: true,
            custom_filter_key: None,
            default_search_filter: None,
        }
    }
}

fn reference_constraints_default_true() -> bool {
    true
}

fn is_reference_constraints_true(value: &bool) -> bool {
    *value
}

fn is_default_reference_constraints(value: &ReferenceConstraints) -> bool {
    *value == ReferenceConstraints::default()
}

/// Named groups of allowed file extensions for `ParamValue::File`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default, TS)]
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
            Self::Script => matches!(extension.as_str(), "js" | "mjs" | "cjs"),
        }
    }
}

/// Additional constraints specific to `ParamValue::File`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default, TS)]
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

        let group_match = self.allowed_types.is_empty()
            || self
                .allowed_types
                .iter()
                .any(|group| group.matches_extension(&extension));
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
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
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
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
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

impl fmt::Display for ParameterConstraints {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut sections = Vec::new();

        if let Some(range) = &self.range {
            match range {
                RangeConstraint::Uniform { min, max } => {
                    let min_text = min.map(|value| value.to_string()).unwrap_or_else(|| "-inf".to_string());
                    let max_text = max.map(|value| value.to_string()).unwrap_or_else(|| "+inf".to_string());
                    sections.push(format!("range={min_text}..{max_text}"));
                }
                RangeConstraint::Components { min, max } => {
                    let min_text = min
                        .as_ref()
                        .map(|values| {
                            values
                                .iter()
                                .map(|value| value.to_string())
                                .collect::<Vec<_>>()
                                .join(",")
                        })
                        .map(|value| format!("[{value}]"))
                        .unwrap_or_else(|| "[]".to_string());
                    let max_text = max
                        .as_ref()
                        .map(|values| {
                            values
                                .iter()
                                .map(|value| value.to_string())
                                .collect::<Vec<_>>()
                                .join(",")
                        })
                        .map(|value| format!("[{value}]"))
                        .unwrap_or_else(|| "[]".to_string());
                    sections.push(format!("components={min_text}..{max_text}"));
                }
            }
        }

        if let Some(step) = self.step {
            sections.push(format!("step={step}"));
        }
        if let Some(step_base) = self.step_base {
            sections.push(format!("stepBase={step_base}"));
        }
        if !self.enum_options.is_empty() {
            sections.push(format!("enumOptions={}", self.enum_options.len()));
        }
        if !self.file.allowed_types.is_empty() || !self.file.allowed_extensions.is_empty() {
            sections.push("fileConstraints".to_string());
        }

        if sections.is_empty() {
            write!(f, "no constraints")
        } else {
            write!(f, "{}", sections.join(", "))
        }
    }
}

impl ParameterConstraints {
    /// Normalizes or validates an incoming value according to constraint policy.
    pub fn normalize(&self, incoming: ParamValue) -> Result<ParamValue, String> {
        let mut normalized = match incoming {
            ParamValue::Int(value) => self.normalize_int(value)?,
            ParamValue::Float(value) => self.normalize_float(value)?,
            ParamValue::CssValue(value) => self.normalize_css_value(value)?,
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
                let allowed: Vec<String> = self
                    .enum_options
                    .iter()
                    .map(|option| option.variant_id.clone())
                    .collect();
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

    fn normalize_css_value(&self, value: CssValue) -> Result<ParamValue, String> {
        Ok(ParamValue::CssValue(CssValue::new(
            self.normalize_numeric(value.value)?,
            value.unit,
        )))
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
                        return Err(format!(
                            "value {value} does not align with step {step} from base {base}"
                        ));
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
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
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
    /// Parameter control-plane state.
    #[serde(default, skip_serializing_if = "is_default_parameter_control_state")]
    pub control: ParameterControlState,
    /// Whether control modes other than `manual` are available for this parameter.
    #[serde(default = "default_control_modes_enabled", skip_serializing_if = "is_true")]
    pub control_modes_enabled: bool,
}

pub(crate) fn default_control_modes_enabled() -> bool {
    true
}
