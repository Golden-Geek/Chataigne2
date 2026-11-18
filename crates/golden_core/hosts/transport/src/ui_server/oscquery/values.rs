use base64::{Engine as _, engine::general_purpose::STANDARD};
use golden_engine::{
    node::NodeUuid,
    parameter::{CssValue, NodeReference, ParamValue, RangeConstraint},
};
use golden_protocol::{UiParamDto, UiSchemaView};
use rosc::OscType;
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

pub(super) fn incoming(target: &ParamValue, args: &[OscType]) -> Option<ParamValue> {
    use OscType as Osc;
    use ParamValue as Param;
    Some(match (target, args) {
        (Param::Trigger(), [] | [Osc::Inf]) => Param::Trigger(),
        (Param::Int(_), [Osc::Int(value)]) => Param::Int(*value),
        (Param::Int(_), [Osc::Long(value)]) => Param::Int((*value).try_into().ok()?),
        (Param::Float(_), [value]) => Param::Float(number(value)?),
        (Param::Bool(_), [Osc::Bool(value)]) => Param::Bool(*value),
        (Param::Bool(_), [Osc::Int(value @ (0 | 1))]) => Param::Bool(*value == 1),
        (Param::Str(_), [Osc::String(value)]) => Param::Str(value.clone()),
        (Param::File(_), [Osc::String(value)]) => Param::File(value.clone()),
        (Param::Enum(_), [Osc::String(value)]) => Param::Enum(value.clone()),
        (Param::Bytes(_), [Osc::Blob(value)]) => Param::Bytes(value.clone()),
        (Param::CssValue(_), [Osc::String(value)]) => {
            let value = CssValue::parse(value)?;
            value.value.is_finite().then_some(Param::CssValue(value))?
        }
        (Param::Reference(_), [Osc::String(value)]) if value.is_empty() => Param::Reference(NodeReference::empty()),
        (Param::Reference(_), [Osc::String(value)]) => {
            Param::Reference(NodeReference::new(NodeUuid(value.parse().ok()?)))
        }
        (Param::Vec2(..), [x, y]) => Param::Vec2(number(x)?, number(y)?),
        (Param::Vec3(..), [x, y, z]) => Param::Vec3(number(x)?, number(y)?, number(z)?),
        (Param::Color(..), [r, g, b, a]) => Param::Color(number(r)?, number(g)?, number(b)?, number(a)?),
        (Param::Color(..), [Osc::Color(color)]) => {
            let unit = |value| f64::from(value) / 255.0;
            Param::Color(unit(color.red), unit(color.green), unit(color.blue), unit(color.alpha))
        }
        _ => return None,
    })
}

pub(super) fn outgoing(value: &ParamValue) -> Option<Vec<OscType>> {
    use OscType as Osc;
    use ParamValue as Param;
    let text = |value: &str| (!value.contains('\0')).then(|| vec![Osc::String(value.into())]);
    let floats = |values: &[f64]| {
        values
            .iter()
            .map(|value| {
                let value = *value as f32;
                value.is_finite().then_some(Osc::Float(value))
            })
            .collect::<Option<Vec<_>>>()
    };
    match value {
        Param::Trigger() => Some(vec![Osc::Inf]),
        Param::Int(value) => Some(vec![Osc::Int(*value)]),
        Param::Float(value) => floats(&[*value]),
        Param::Bool(value) => Some(vec![Osc::Int(i32::from(*value))]),
        Param::Str(value) | Param::File(value) | Param::Enum(value) => text(value),
        Param::Bytes(bytes) => Some(vec![Osc::Blob(bytes.clone())]),
        Param::Vec2(x, y) => floats(&[*x, *y]),
        Param::Vec3(x, y, z) => floats(&[*x, *y, *z]),
        Param::Color(r, g, b, a) => floats(&[*r, *g, *b, *a]),
        Param::CssValue(value) => text(&value.to_string()),
        Param::Reference(value) if value.is_empty() => text(""),
        Param::Reference(value) => text(&value.uuid.0.to_string()),
    }
}

fn number(value: &OscType) -> Option<f64> {
    let value = match value {
        OscType::Float(value) => f64::from(*value),
        OscType::Double(value) => *value,
        OscType::Int(value) => f64::from(*value),
        _ => return None,
    };
    value.is_finite().then_some(value)
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
