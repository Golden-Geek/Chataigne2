use super::OscValuePayload;
use golden_core::{node::NodeId, parameter::ParamValue, process_ctx::ProcessTreeSnapshot};

pub(super) fn format_osc_payload(payload: &OscValuePayload) -> String {
    match payload {
        OscValuePayload::Single(value) => format_param_value(value),
        OscValuePayload::Multi(values) => format!(
            "[{}]",
            values.iter().map(format_param_value).collect::<Vec<_>>().join(", ")
        ),
        OscValuePayload::Arguments(values) => format!(
            "[{}]",
            values.iter().map(format_param_value).collect::<Vec<_>>().join(", ")
        ),
    }
}

pub(super) fn format_param_value(value: &ParamValue) -> String {
    match value {
        ParamValue::Trigger() => "trigger".to_string(),
        ParamValue::Int(value) => value.to_string(),
        ParamValue::Float(value) => value.to_string(),
        ParamValue::Str(value) | ParamValue::File(value) | ParamValue::Enum(value) => value.clone(),
        ParamValue::Bool(value) => value.to_string(),
        ParamValue::Vec2(x, y) => format!("({}, {})", x, y),
        ParamValue::Vec3(x, y, z) => format!("({}, {}, {})", x, y, z),
        ParamValue::Color(r, g, b, a) => format!("rgba({}, {}, {}, {})", r, g, b, a),
        ParamValue::CssValue(value) => format!("{value:?}"),
        ParamValue::Reference(reference) => format!("{reference:?}"),
    }
}

pub(super) fn is_descendant_of_node(
    snapshot: &ProcessTreeSnapshot,
    start: NodeId,
    ancestor: NodeId,
) -> Result<bool, &'static str> {
    let mut current = Some(start);
    for _ in 0..=64 {
        let Some(node) = current else { return Ok(false) };
        if node == ancestor {
            return Ok(true);
        }
        current = snapshot.node(node).and_then(|node| node.parent);
    }
    Err("OSC feedback path exceeds 64 levels; value was not admitted")
}
