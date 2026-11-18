use base64::{Engine as _, engine::general_purpose::STANDARD};
use golden_engine::parameter::{ParamValue, RangeConstraint};
use golden_protocol::{UiParamDto, UiSchemaView};
use serde_json::{Map, Value, json};

pub(super) fn readable(value: &ParamValue) -> bool {
    !matches!(value, ParamValue::Trigger())
}

pub(super) fn type_tag(value: &ParamValue) -> &'static str {
    match value {
        ParamValue::Trigger() => "I",
        ParamValue::Int(_) | ParamValue::Bool(_) => "i",
        ParamValue::Float(_) => "f",
        ParamValue::Str(_)
        | ParamValue::File(_)
        | ParamValue::Enum(_)
        | ParamValue::Reference(_)
        | ParamValue::CssValue(_) => "s",
        ParamValue::Bytes(_) => "b",
        ParamValue::Vec2(..) => "ff",
        ParamValue::Vec3(..) => "fff",
        ParamValue::Color(..) => "ffff",
    }
}

pub(super) fn value(param: &UiParamDto) -> Option<Value> {
    Some(match &param.value {
        ParamValue::Trigger() => return None,
        ParamValue::Int(value) => json!([value]),
        ParamValue::Float(value) => json!([value]),
        ParamValue::Str(value) | ParamValue::File(value) | ParamValue::Enum(value) => json!([value]),
        ParamValue::Bool(value) => json!([i32::from(*value)]),
        ParamValue::Bytes(bytes) => json!([STANDARD.encode(bytes)]),
        ParamValue::Vec2(x, y) => json!([x, y]),
        ParamValue::Vec3(x, y, z) => json!([x, y, z]),
        ParamValue::Color(r, g, b, a) => json!([r, g, b, a]),
        ParamValue::CssValue(value) => json!([value.to_string()]),
        ParamValue::Reference(value) => json!([if value.is_empty() {
            String::new()
        } else {
            value.uuid.0.to_string()
        }]),
    })
}

pub(super) fn range(schema: &UiSchemaView, param: &UiParamDto) -> Option<Vec<Value>> {
    if matches!(&param.value, ParamValue::Enum(_)) {
        let values = param
            .enum_options_id
            .as_ref()
            .and_then(|id| {
                let choices = schema.enums.iter().find(|entry| &entry.enum_id == id)?;
                Some(
                    choices
                        .variants
                        .iter()
                        .map(|choice| choice.variant_id.as_str())
                        .collect::<Vec<_>>(),
                )
            })
            .unwrap_or_else(|| {
                param
                    .constraints
                    .enum_options
                    .as_slice()
                    .iter()
                    .map(|choice| choice.variant_id.as_str())
                    .collect()
            });
        return (!values.is_empty()).then(|| vec![json!({ "VALS": values })]);
    }
    let components = match &param.value {
        ParamValue::Int(_) | ParamValue::Float(_) => 1,
        ParamValue::Vec2(..) => 2,
        ParamValue::Vec3(..) => 3,
        ParamValue::Color(..) => 4,
        _ => return None,
    };
    let range = param.constraints.range.as_ref()?;
    Some(
        (0..components)
            .map(|index| {
                let (min, max) = match range {
                    RangeConstraint::Uniform { min, max } => (*min, *max),
                    RangeConstraint::Components { min, max } => (
                        min.as_ref().and_then(|values| values.get(index)).copied(),
                        max.as_ref().and_then(|values| values.get(index)).copied(),
                    ),
                };
                let mut entry = Map::new();
                if let Some(min) = min {
                    entry.insert("MIN".into(), json!(min));
                }
                if let Some(max) = max {
                    entry.insert("MAX".into(), json!(max));
                }
                Value::Object(entry)
            })
            .collect(),
    )
}
