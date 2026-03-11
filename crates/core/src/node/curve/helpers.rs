use super::prelude::*;

pub(super) const CURVE_RANGE_EPSILON: f64 = 1e-9;
pub(super) const KEY_ORDER_POSITION_EPSILON: f64 = 1e-10;

pub(super) fn make_float_parameter(label: &str, decl_id: &str, default_value: f64) -> Parameter {
    let mut parameter = Parameter::new(
        label,
        ParamValue::Float(default_value),
        ParameterChangeCheck::ValueChange,
    );
    parameter.node_data_mut().meta.decl_id = DeclId(decl_id.to_string());
    parameter.node_data_mut().meta.can_be_disabled = false;
    parameter
}

pub(super) fn make_vec2_parameter(label: &str, decl_id: &str, x: f64, y: f64) -> Parameter {
    let mut parameter = Parameter::new(label, ParamValue::Vec2(x, y), ParameterChangeCheck::ValueChange);
    parameter.node_data_mut().meta.decl_id = DeclId(decl_id.to_string());
    parameter.node_data_mut().meta.can_be_disabled = false;
    parameter
}

pub(super) fn default_curve_easing() -> CurveEasing {
    CurveEasing::Bezier {
        out_handle: CurveHandle::new(1.0 / 3.0, 0.0),
        in_handle: CurveHandle::new(-1.0 / 3.0, 0.0),
    }
}

pub(super) fn curve_easing_kind_id(easing: &CurveEasing) -> &'static str {
    match easing {
        CurveEasing::Linear => "linear",
        CurveEasing::Bezier { .. } => "bezier",
        CurveEasing::Hold => "hold",
        CurveEasing::Steps { .. } => "steps",
        CurveEasing::Shape { .. } => "shape",
        CurveEasing::PerlinNoise { .. } => "perlinNoise",
        CurveEasing::Random { .. } => "random",
        CurveEasing::Script { .. } => "script",
    }
}

pub(super) fn curve_step_mode_variant_id(mode: CurveStepMode) -> &'static str {
    match mode {
        CurveStepMode::StepSize => "stepSize",
        CurveStepMode::NumSteps => "numSteps",
    }
}

pub(super) fn curve_shape_variant_id(shape: CurveShape) -> &'static str {
    match shape {
        CurveShape::Sine => "sine",
        CurveShape::Triangle => "triangle",
        CurveShape::Saw => "saw",
        CurveShape::ReverseSaw => "reverseSaw",
        CurveShape::Square => "square",
    }
}

pub(super) fn curve_phase_mode_variant_id(mode: CurvePhaseMode) -> &'static str {
    match mode {
        CurvePhaseMode::Frequency => "frequency",
        CurvePhaseMode::NumPhases => "numPhases",
    }
}

pub(super) fn read_child_param_value<'a>(
    snapshot: &'a ProcessTreeSnapshot,
    parent: NodeId,
    decl_id: &str,
) -> Option<&'a ParamValue> {
    let child = snapshot.find_child(parent, decl_id)?;
    snapshot.node(child)?.param_value.as_ref()
}

pub(super) fn read_child_param_f64(
    snapshot: &ProcessTreeSnapshot,
    parent: NodeId,
    decl_id: &str,
    default_value: f64,
) -> f64 {
    read_child_param_value(snapshot, parent, decl_id)
        .and_then(|value| {
            value
                .as_float()
                .or_else(|| value.as_int().map(|int_value| int_value as f64))
        })
        .unwrap_or(default_value)
}

pub(super) fn read_child_param_u32(
    snapshot: &ProcessTreeSnapshot,
    parent: NodeId,
    decl_id: &str,
    default_value: u32,
) -> u32 {
    read_child_param_value(snapshot, parent, decl_id)
        .and_then(|value| {
            value
                .as_int()
                .or_else(|| value.as_float().map(|float_value| float_value.round() as i32))
        })
        .map(|value| value.max(0) as u32)
        .unwrap_or(default_value)
}

pub(super) fn read_child_param_u64(
    snapshot: &ProcessTreeSnapshot,
    parent: NodeId,
    decl_id: &str,
    default_value: u64,
) -> u64 {
    read_child_param_value(snapshot, parent, decl_id)
        .and_then(|value| {
            value
                .as_int()
                .or_else(|| value.as_float().map(|float_value| float_value.round() as i32))
        })
        .map(|value| value as i64 as u64)
        .unwrap_or(default_value)
}

pub(super) fn read_child_param_enum(
    snapshot: &ProcessTreeSnapshot,
    parent: NodeId,
    decl_id: &str,
    default_value: &str,
) -> String {
    read_child_param_value(snapshot, parent, decl_id)
        .and_then(ParamValue::as_enum)
        .unwrap_or_else(|| default_value.to_string())
}

pub(super) fn read_child_param_string(
    snapshot: &ProcessTreeSnapshot,
    parent: NodeId,
    decl_id: &str,
    default_value: &str,
) -> String {
    read_child_param_value(snapshot, parent, decl_id)
        .and_then(ParamValue::as_str)
        .unwrap_or_else(|| default_value.to_string())
}

pub(super) fn parse_step_mode(value: &str) -> CurveStepMode {
    match value.trim().to_ascii_lowercase().as_str() {
        "stepsize" => CurveStepMode::StepSize,
        _ => CurveStepMode::NumSteps,
    }
}

pub(super) fn parse_shape(value: &str) -> CurveShape {
    match value.trim().to_ascii_lowercase().as_str() {
        "triangle" => CurveShape::Triangle,
        "saw" => CurveShape::Saw,
        "reversesaw" => CurveShape::ReverseSaw,
        "square" => CurveShape::Square,
        _ => CurveShape::Sine,
    }
}

pub(super) fn parse_phase_mode(value: &str) -> CurvePhaseMode {
    match value.trim().to_ascii_lowercase().as_str() {
        "numphases" => CurvePhaseMode::NumPhases,
        _ => CurvePhaseMode::Frequency,
    }
}

pub(super) fn clamp_f64(value: f64, min: f64, max: f64) -> f64 {
    value.max(min).min(max)
}

pub(super) fn key_secant_slope(
    start_position: f64,
    start_value: f64,
    end_position: f64,
    end_value: f64,
) -> Option<f64> {
    let span = end_position - start_position;
    if !span.is_finite() || span.abs() <= CURVE_RANGE_EPSILON {
        return None;
    }
    let slope = (end_value - start_value) / span;
    slope.is_finite().then_some(slope)
}

pub(super) fn outgoing_segment_slope(start: &CurveKey, end: &CurveKey) -> Option<f64> {
    let secant = key_secant_slope(start.position, start.value, end.position, end.value);
    match &start.easing {
        CurveEasing::Bezier { out_handle, .. } => {
            let span = end.position - start.position;
            let delta_x = out_handle.position * span;
            if !delta_x.is_finite() || delta_x.abs() <= CURVE_RANGE_EPSILON {
                return secant;
            }
            let slope = out_handle.value / delta_x;
            if slope.is_finite() { Some(slope) } else { secant }
        }
        CurveEasing::Hold => Some(0.0),
        _ => secant,
    }
}

pub(super) fn incoming_segment_slope(start: &CurveKey, end: &CurveKey) -> Option<f64> {
    let secant = key_secant_slope(start.position, start.value, end.position, end.value);
    match &start.easing {
        CurveEasing::Bezier { in_handle, .. } => {
            let span = end.position - start.position;
            let delta_x = in_handle.position * span;
            if !delta_x.is_finite() || delta_x.abs() <= CURVE_RANGE_EPSILON {
                return secant;
            }
            let slope = in_handle.value / delta_x;
            if slope.is_finite() { Some(slope) } else { secant }
        }
        CurveEasing::Hold => Some(0.0),
        _ => secant,
    }
}
