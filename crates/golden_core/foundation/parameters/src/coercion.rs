use std::borrow::Cow;

use super::{
    ParamValue, ParamValueProjection,
    projection::{project_param_value_for_target, project_param_value_for_target_reverse},
};

fn coerce_kind<'a>(source: Cow<'a, ParamValue>, target: &ParamValue) -> Option<Cow<'a, ParamValue>> {
    // Coercion to the same kind preserves the complete value, including CSS units
    // and reference recovery hints. Borrow or move its storage without copying it.
    if std::mem::discriminant(source.as_ref()) == std::mem::discriminant(target) {
        return Some(source);
    }

    let text_constructor: Option<fn(String) -> ParamValue> = match (source.as_ref(), target) {
        (ParamValue::Str(_) | ParamValue::File(_) | ParamValue::Enum(_), ParamValue::Str(_)) => Some(ParamValue::Str),
        (ParamValue::Str(_) | ParamValue::File(_) | ParamValue::Enum(_), ParamValue::File(_)) => Some(ParamValue::File),
        (ParamValue::Str(_) | ParamValue::Enum(_), ParamValue::Enum(_)) => Some(ParamValue::Enum),
        _ => None,
    };
    if let Some(constructor) = text_constructor {
        let (ParamValue::Str(text) | ParamValue::File(text) | ParamValue::Enum(text)) = source.into_owned() else {
            unreachable!("text coercion requires a textual source")
        };
        return Some(Cow::Owned(constructor(text)));
    }

    let source = source.as_ref();
    let converted = match target {
        ParamValue::Trigger() => return None,
        ParamValue::Bytes(_) => return None,
        ParamValue::Int(_) => source.as_int().map(ParamValue::Int),
        ParamValue::Float(_) => source.as_float().map(ParamValue::Float),
        ParamValue::Str(_) => source.as_str().map(ParamValue::Str),
        ParamValue::File(_) => source.as_str().map(ParamValue::File),
        ParamValue::Enum(_) => source.as_enum().map(ParamValue::Enum),
        ParamValue::Bool(_) => source.as_bool().map(ParamValue::Bool),
        ParamValue::CssValue(target_value) => source
            .as_css_value_with_unit(target_value.unit)
            .map(ParamValue::CssValue),
        ParamValue::Vec2(_, _) => source.as_vec2().map(|(x, y)| ParamValue::Vec2(x, y)),
        ParamValue::Vec3(_, _, _) => source.as_vec3().map(|(x, y, z)| ParamValue::Vec3(x, y, z)),
        ParamValue::Color(_, _, _, _) => source.as_color().map(|(r, g, b, a)| ParamValue::Color(r, g, b, a)),
        ParamValue::Reference(_) => return None,
    }?;
    Some(Cow::Owned(converted))
}

/// Coerces to the target kind, borrowing unchanged source storage. A projection
/// or kind conversion may produce an owned value; rejected input is never cloned.
pub fn coerce_param_value_for_target_ref<'a>(
    source: &'a ParamValue,
    target: &ParamValue,
    projection: Option<ParamValueProjection>,
) -> Option<Cow<'a, ParamValue>> {
    let source = match projection {
        Some(projection) => Cow::Owned(project_param_value_for_target(source, target, projection)?),
        None => Cow::Borrowed(source),
    };
    coerce_kind(source, target)
}

/// Coerces an owned input, moving its storage when the conversion permits it.
/// A projection is applied before kind conversion, as in the borrowed API.
pub fn coerce_param_value_for_target_owned(
    source: ParamValue,
    target: &ParamValue,
    projection: Option<ParamValueProjection>,
) -> Option<ParamValue> {
    let source = match projection {
        Some(projection) => project_param_value_for_target(&source, target, projection)?,
        None => source,
    };
    coerce_kind(Cow::Owned(source), target).map(Cow::into_owned)
}

/// Coerces `source` to the value kind represented by `target`, returning an owned
/// result. A projection is applied first; unchanged source data is copied once.
pub fn coerce_param_value_for_target(
    source: &ParamValue,
    target: &ParamValue,
    projection: Option<ParamValueProjection>,
) -> Option<ParamValue> {
    coerce_param_value_for_target_ref(source, target, projection).map(Cow::into_owned)
}

pub(crate) fn coerce_reverse_ref<'a>(
    source: &'a ParamValue,
    target: &ParamValue,
    projection: Option<ParamValueProjection>,
) -> Option<Cow<'a, ParamValue>> {
    let source = match projection {
        Some(projection) => Cow::Owned(project_param_value_for_target_reverse(source, target, projection)?),
        None => Cow::Borrowed(source),
    };
    coerce_kind(source, target)
}

/// Coerces using the reverse direction of a projection for bidirectional binding.
/// Unchanged source storage is copied only for the final owned result.
pub fn coerce_param_value_for_target_reverse(
    source: &ParamValue,
    target: &ParamValue,
    projection: Option<ParamValueProjection>,
) -> Option<ParamValue> {
    coerce_reverse_ref(source, target, projection).map(Cow::into_owned)
}
