use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{ParamValue, ParamValueProjection};

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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Default, Deserialize, TS)]
pub enum ParameterEventBehaviour {
    /// Keep only the latest pending set for this parameter within a queue drain.
    #[default]
    Coalesce,
    /// Keep every pending set for this parameter within a queue drain.
    Append,
}

/// Runtime control mode used to drive one parameter value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default, TS)]
#[serde(rename_all = "camelCase")]
pub enum ParameterControlMode {
    /// Parameter uses its locally stored value.
    #[default]
    Manual,
    /// Parameter reads one lexical user-context symbol.
    ContextLink,
    /// Parameter string is produced from a text template with token interpolation.
    TemplateText,
    /// Parameter value is computed from an expression.
    Expression,
    /// Parameter reads from one referenced compatible parameter.
    Proxy,
    /// Parameter synchronizes bidirectionally with one referenced compatible parameter.
    Binding,
    /// Parameter is driven by a local animation function.
    Animation,
}

/// Returns whether one control mode is valid for a parameter value kind.
pub fn control_mode_supported_for_value(mode: ParameterControlMode, value: &ParamValue) -> bool {
    match mode {
        ParameterControlMode::TemplateText => matches!(value, ParamValue::Str(_)),
        _ => true,
    }
}

/// Returns the supported control modes for one parameter value kind.
pub fn available_control_modes_for_value(value: &ParamValue) -> Vec<ParameterControlMode> {
    [
        ParameterControlMode::Manual,
        ParameterControlMode::ContextLink,
        ParameterControlMode::TemplateText,
        ParameterControlMode::Expression,
        ParameterControlMode::Proxy,
        ParameterControlMode::Binding,
        ParameterControlMode::Animation,
    ]
    .into_iter()
    .filter(|mode| control_mode_supported_for_value(*mode, value))
    .collect()
}

/// Returns the supported control modes for one parameter, accounting for local policy.
pub fn available_control_modes_for_parameter(
    value: &ParamValue,
    control_modes_enabled: bool,
) -> Vec<ParameterControlMode> {
    if !control_modes_enabled {
        return vec![ParameterControlMode::Manual];
    }

    available_control_modes_for_value(value)
}

/// Animation waveform used by [`AnimationControlSpec`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default, TS)]
#[serde(rename_all = "camelCase")]
pub enum AnimationWaveform {
    /// Smooth sinus wave in range `[-1, 1]`.
    #[default]
    Sine,
    /// Triangle wave in range `[-1, 1]`.
    Triangle,
    /// Saw wave in range `[-1, 1]`.
    Saw,
    /// Square wave in range `[-1, 1]`.
    Square,
}

/// Animation driver configuration for `ParameterControlMode::Animation`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AnimationControlSpec {
    /// Oscillator waveform.
    #[serde(default)]
    pub waveform: AnimationWaveform,
    /// Oscillation frequency in Hertz.
    #[serde(default = "default_animation_frequency_hz")]
    pub frequency_hz: f64,
    /// Output amplitude (applied after waveform generation).
    #[serde(default = "default_animation_amplitude")]
    pub amplitude: f64,
    /// Constant output offset.
    #[serde(default)]
    pub offset: f64,
    /// Additional phase offset in cycles (`1.0 = full cycle`).
    #[serde(default)]
    pub phase: f64,
}

fn default_animation_frequency_hz() -> f64 {
    1.0
}

fn default_animation_amplitude() -> f64 {
    1.0
}

impl Default for AnimationControlSpec {
    fn default() -> Self {
        Self {
            waveform: AnimationWaveform::default(),
            frequency_hz: default_animation_frequency_hz(),
            amplitude: default_animation_amplitude(),
            offset: 0.0,
            phase: 0.0,
        }
    }
}

/// Persisted authoring intent for one parameter control mode.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "mode", rename_all = "camelCase")]
pub enum ParameterControlSpec {
    /// Manual value editing with no external source.
    Manual,
    /// One lexical context symbol lookup.
    ContextLink {
        /// Symbol to resolve from nearest visible `UserContext` scope.
        symbol: String,
        /// Optional projection applied before coercion.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        projection: Option<ParamValueProjection>,
    },
    /// Text template with `{token}` segments.
    TemplateText {
        /// Raw user-authored template string.
        template: String,
    },
    /// Expression mode driven by an internal control node.
    Expression,
    /// One-way reference-based parameter mode.
    Proxy,
    /// Two-way reference-based parameter mode.
    Binding,
    /// Local animation mode driven by an internal control node.
    Animation,
}

impl Default for ParameterControlSpec {
    fn default() -> Self {
        Self::Manual
    }
}

/// One parameter-control diagnostic message.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterControlDiagnostic {
    /// Stable diagnostic code.
    pub code: String,
    /// Human-readable summary.
    pub message: String,
    /// Optional detail payload.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl ParameterControlDiagnostic {
    /// Creates a new diagnostic with `code` and `message`.
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            detail: None,
        }
    }

    /// Sets diagnostic detail text.
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

/// Full control-plane state attached to one parameter.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterControlState {
    /// Active control mode.
    #[serde(default)]
    pub mode: ParameterControlMode,
    /// Persisted authoring intent for this mode.
    #[serde(default)]
    pub spec: ParameterControlSpec,
    /// Last known diagnostics for this control state.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<ParameterControlDiagnostic>,
}

impl Default for ParameterControlState {
    fn default() -> Self {
        Self::manual()
    }
}

impl ParameterControlState {
    /// Returns a manual/default control state.
    pub fn manual() -> Self {
        Self {
            mode: ParameterControlMode::Manual,
            spec: ParameterControlSpec::Manual,
            diagnostics: Vec::new(),
        }
    }

    /// Creates a state with explicit `mode` and `spec`.
    pub fn new(mode: ParameterControlMode, spec: ParameterControlSpec) -> Self {
        Self {
            mode,
            spec,
            diagnostics: Vec::new(),
        }
    }
}

pub(crate) fn is_default_parameter_control_state(value: &ParameterControlState) -> bool {
    *value == ParameterControlState::default()
}

pub(crate) fn is_true(value: &bool) -> bool {
    *value
}
