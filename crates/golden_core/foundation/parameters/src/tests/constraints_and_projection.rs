use super::{
    CssUnit, CssValue, ParamValue, ParamValueProjection, ParameterConstraintPolicy, ParameterConstraints,
    RangeConstraint, coerce_param_value_for_target,
};

#[test]
fn extracted_constraints_preserve_normalization_behavior() {
    let constraints = ParameterConstraints {
        range: RangeConstraint::uniform(Some(0.0), Some(1.0)),
        policy: ParameterConstraintPolicy::Reject,
        ..ParameterConstraints::default()
    };
    assert_eq!(
        constraints.normalize(ParamValue::Float(0.5)).unwrap(),
        ParamValue::Float(0.5)
    );
    assert!(constraints.normalize(ParamValue::Float(2.0)).is_err());
}

#[test]
fn extracted_projection_preserves_vector_component_behavior() {
    assert_eq!(
        coerce_param_value_for_target(
            &ParamValue::Vec2(3.0, 4.0),
            &ParamValue::Float(0.0),
            Some(ParamValueProjection::Vec2X),
        ),
        Some(ParamValue::Float(3.0))
    );
    assert_eq!(
        ParamValue::CssValue(CssValue::new(2.0, CssUnit::Rem)).as_float(),
        Some(2.0)
    );
}
