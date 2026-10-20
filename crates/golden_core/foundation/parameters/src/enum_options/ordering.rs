use std::cmp::Ordering;

use crate::ParamValue;

// A total lookup order whose equality classes agree with ParamValue equality
// for comparable values. The final membership check still rejects NaN values.
pub(super) fn compare(left: &ParamValue, right: &ParamValue) -> Ordering {
    let kind = rank(left).cmp(&rank(right));
    if kind != Ordering::Equal {
        return kind;
    }
    match (left, right) {
        (ParamValue::Trigger(), ParamValue::Trigger()) => Ordering::Equal,
        (ParamValue::Int(left), ParamValue::Int(right)) => left.cmp(right),
        (ParamValue::Float(left), ParamValue::Float(right)) => number(*left, *right),
        (ParamValue::Str(left), ParamValue::Str(right))
        | (ParamValue::Enum(left), ParamValue::Enum(right))
        | (ParamValue::File(left), ParamValue::File(right)) => left.cmp(right),
        (ParamValue::Bool(left), ParamValue::Bool(right)) => left.cmp(right),
        (ParamValue::CssValue(left), ParamValue::CssValue(right)) => (left.unit as usize)
            .cmp(&(right.unit as usize))
            .then_with(|| number(left.value, right.value)),
        (ParamValue::Vec2(a, b), ParamValue::Vec2(x, y)) => number(*a, *x).then_with(|| number(*b, *y)),
        (ParamValue::Vec3(a, b, c), ParamValue::Vec3(x, y, z)) => {
            number(*a, *x).then_with(|| number(*b, *y)).then_with(|| number(*c, *z))
        }
        (ParamValue::Color(a, b, c, d), ParamValue::Color(w, x, y, z)) => number(*a, *w)
            .then_with(|| number(*b, *x))
            .then_with(|| number(*c, *y))
            .then_with(|| number(*d, *z)),
        (ParamValue::Reference(left), ParamValue::Reference(right)) => {
            left.uuid.0.as_bytes().cmp(right.uuid.0.as_bytes()).then_with(|| {
                left.projection
                    .map(|value| value as usize)
                    .cmp(&right.projection.map(|value| value as usize))
            })
        }
        _ => unreachable!("distinct parameter kinds have distinct ranks"),
    }
}

fn number(left: f64, right: f64) -> Ordering {
    // f64::total_cmp distinguishes signed zero, while ParamValue equality does not.
    if left == right {
        Ordering::Equal
    } else {
        left.total_cmp(&right)
    }
}

fn rank(value: &ParamValue) -> u8 {
    match value {
        ParamValue::Trigger() => 0,
        ParamValue::Int(_) => 1,
        ParamValue::Float(_) => 2,
        ParamValue::Str(_) => 3,
        ParamValue::File(_) => 4,
        ParamValue::Enum(_) => 5,
        ParamValue::Bool(_) => 6,
        ParamValue::CssValue(_) => 7,
        ParamValue::Vec2(..) => 8,
        ParamValue::Vec3(..) => 9,
        ParamValue::Color(..) => 10,
        ParamValue::Reference(_) => 11,
    }
}
