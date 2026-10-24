use super::ParameterAuthoredState;
use crate::parameter::{ParamValue, ParameterConstraints, ParameterEnumOptions, RangeConstraint};

impl PartialEq for ParameterAuthoredState {
    fn eq(&self, other: &Self) -> bool {
        let Self {
            value,
            default_value,
            change_check,
            event_behaviour,
            read_only,
            persist_read_only_value,
            constraints,
            ui_hints,
            control,
            control_modes_enabled,
        } = self;
        value_eq(value, &other.value)
            && value_eq(default_value, &other.default_value)
            && change_check == &other.change_check
            && event_behaviour == &other.event_behaviour
            && read_only == &other.read_only
            && persist_read_only_value == &other.persist_read_only_value
            && constraints_eq(constraints, &other.constraints)
            && ui_hints == &other.ui_hints
            && control == &other.control
            && control_modes_enabled == &other.control_modes_enabled
    }
}

// Native snapshots retain exact values without JSON's nonfinite-number loss.
// Equal NaN bits are stable for a no-op checkpoint; ordinary value validation
// still rejects nonfinite writes. Signed zeros keep normal value equality.
fn number_eq(left: f64, right: f64) -> bool {
    left == right || left.to_bits() == right.to_bits()
}

fn value_eq(left: &ParamValue, right: &ParamValue) -> bool {
    match left {
        ParamValue::Float(left) => matches!(right, ParamValue::Float(right) if number_eq(*left, *right)),
        ParamValue::CssValue(left) => {
            matches!(right, ParamValue::CssValue(right) if left.unit == right.unit && number_eq(left.value, right.value))
        }
        ParamValue::Vec2(a, b) => matches!(right, ParamValue::Vec2(x, y) if number_eq(*a, *x) && number_eq(*b, *y)),
        ParamValue::Vec3(a, b, c) => {
            matches!(right, ParamValue::Vec3(x, y, z) if number_eq(*a, *x) && number_eq(*b, *y) && number_eq(*c, *z))
        }
        ParamValue::Color(a, b, c, d) => {
            matches!(right, ParamValue::Color(w, x, y, z) if number_eq(*a, *w) && number_eq(*b, *x) && number_eq(*c, *y) && number_eq(*d, *z))
        }
        // Persisted display/recovery hints participate; runtime cached ids do not.
        ParamValue::Reference(left) => matches!(right, ParamValue::Reference(right) if left == right
                && left.cached_name == right.cached_name && left.relative_path_from_root == right.relative_path_from_root),
        ParamValue::Trigger()
        | ParamValue::Int(_)
        | ParamValue::Str(_)
        | ParamValue::Bytes(_)
        | ParamValue::File(_)
        | ParamValue::Enum(_)
        | ParamValue::Bool(_) => left == right,
    }
}

fn domain_eq(left: &ParameterEnumOptions, right: &ParameterEnumOptions) -> bool {
    left.same_storage(right)
        || (left.len() == right.len()
            && left.iter().zip(right.iter()).all(|(left, right)| {
                let crate::parameter::ParameterEnumOption {
                    variant_id,
                    value,
                    label,
                    tags,
                    ordering,
                } = left;
                variant_id == &right.variant_id
                    && value_eq(value, &right.value)
                    && label == &right.label
                    && tags == &right.tags
                    && ordering == &right.ordering
            }))
}

fn optional_number_eq(left: Option<f64>, right: Option<f64>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => number_eq(left, right),
        (None, None) => true,
        _ => false,
    }
}

fn optional_numbers_eq(left: Option<&[f64]>, right: Option<&[f64]>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => {
            left.len() == right.len() && left.iter().zip(right).all(|(left, right)| number_eq(*left, *right))
        }
        (None, None) => true,
        _ => false,
    }
}

fn range_eq(left: &Option<RangeConstraint>, right: &Option<RangeConstraint>) -> bool {
    match (left, right) {
        (None, None) => true,
        (Some(RangeConstraint::Uniform { min: a, max: b }), Some(RangeConstraint::Uniform { min: x, max: y })) => {
            optional_number_eq(*a, *x) && optional_number_eq(*b, *y)
        }
        (
            Some(RangeConstraint::Components { min: a, max: b }),
            Some(RangeConstraint::Components { min: x, max: y }),
        ) => optional_numbers_eq(a.as_deref(), x.as_deref()) && optional_numbers_eq(b.as_deref(), y.as_deref()),
        _ => false,
    }
}

fn constraints_eq(left: &ParameterConstraints, right: &ParameterConstraints) -> bool {
    let ParameterConstraints {
        range,
        step,
        step_base,
        enum_options,
        policy,
        reference,
        file,
    } = left;
    range_eq(range, &right.range)
        && optional_number_eq(*step, right.step)
        && optional_number_eq(*step_base, right.step_base)
        && domain_eq(enum_options, &right.enum_options)
        && policy == &right.policy
        && reference == &right.reference
        && file == &right.file
}
