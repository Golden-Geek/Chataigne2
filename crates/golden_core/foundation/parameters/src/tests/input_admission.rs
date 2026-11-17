use super::*;

fn bounds() -> ParameterConstraints {
    ParameterConstraints {
        range: RangeConstraint::uniform(Some(0.), Some(1.)),
        policy: ParameterConstraintPolicy::Reject,
        ..Default::default()
    }
}

#[test]
fn all_numeric_inputs_adapt_to_declared_ranges_without_weakening_authored_validation() {
    let constraints = bounds();
    for (input, target, expected) in [
        (ParamValue::Float(-1.), ParamValue::Float(0.5), ParamValue::Float(0.)),
        (ParamValue::Float(2.), ParamValue::Float(0.5), ParamValue::Float(1.)),
        (ParamValue::Float(0.), ParamValue::Float(0.5), ParamValue::Float(0.)),
        (ParamValue::Float(1.), ParamValue::Float(0.5), ParamValue::Float(1.)),
        (ParamValue::Float(1e200), ParamValue::Int(0), ParamValue::Int(1)),
        (ParamValue::Str("-1e200".into()), ParamValue::Int(1), ParamValue::Int(0)),
        (
            ParamValue::Vec2(-4., 5.),
            ParamValue::Vec2(0., 0.),
            ParamValue::Vec2(0., 1.),
        ),
        (
            ParamValue::Vec3(-4., 0.5, 5.),
            ParamValue::Vec3(0., 0., 0.),
            ParamValue::Vec3(0., 0.5, 1.),
        ),
        (
            ParamValue::Color(-4., 0.5, 5., 2.),
            ParamValue::Color(0., 0., 0., 1.),
            ParamValue::Color(0., 0.5, 1., 1.),
        ),
    ] {
        assert_eq!(
            prepare_param_value_for_target_owned(input, &target, &constraints).unwrap(),
            expected
        );
    }
    assert!(
        constraints.normalize(ParamValue::Float(-1.)).is_err(),
        "authored Reject validation remains strict"
    );
    assert_eq!(constraints.policy, ParameterConstraintPolicy::Reject);
}

#[test]
fn css_input_keeps_its_units_and_legal_stepped_values_remain_exact() {
    let value = ParamValue::CssValue(CssValue::new(-4., CssUnit::Em));
    let target = ParamValue::CssValue(CssValue::new(0.5, CssUnit::Em));
    assert_eq!(
        prepare_param_value_for_target_owned(value, &target, &bounds()).unwrap(),
        ParamValue::CssValue(CssValue::new(0., CssUnit::Em))
    );
    let constraints = ParameterConstraints {
        range: RangeConstraint::uniform(Some(-16.), Some(16.)),
        step: Some(0.01),
        ..bounds()
    };
    let value = ParamValue::Float(0.11);
    assert_eq!(
        prepare_param_value_for_target_owned(value.clone(), &value, &constraints).unwrap(),
        value
    );
}

#[test]
fn current_components_steps_and_projection_apply_before_bounded_conversion() {
    let constraints = ParameterConstraints {
        range: RangeConstraint::components(Some(vec![0., -2.]), Some(vec![1., 2.])),
        ..bounds()
    };
    assert_eq!(
        prepare_param_value_for_target_owned(ParamValue::Vec2(-4., 8.), &ParamValue::Vec2(0., 0.), &constraints)
            .unwrap(),
        ParamValue::Vec2(0., 2.)
    );
    let constraints = ParameterConstraints {
        step: Some(0.01),
        ..bounds()
    };
    assert_eq!(
        prepare_param_value_for_target_owned(
            ParamValue::Float(0.10661115427525991),
            &ParamValue::Float(0.),
            &constraints
        )
        .unwrap(),
        ParamValue::Float(0.11)
    );
    assert_eq!(
        prepare_param_value_for_target(
            &ParamValue::Vec2(1e200, 0.),
            &ParamValue::Int(0),
            &bounds(),
            Some(ParamValueProjection::Vec2X)
        )
        .unwrap(),
        ParamValue::Int(1)
    );
}

#[test]
fn nonfinite_inputs_and_invalid_constraints_are_rejected_before_integer_casts() {
    for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        for target in [ParamValue::Int(0), ParamValue::Float(0.)] {
            assert!(prepare_param_value_for_target_owned(ParamValue::Float(invalid), &target, &bounds()).is_err());
            assert!(
                prepare_param_value_for_target_owned(ParamValue::Str(invalid.to_string()), &target, &bounds()).is_err()
            );
        }
    }
    for constraints in [
        ParameterConstraints {
            step: Some(f64::NAN),
            ..bounds()
        },
        ParameterConstraints {
            range: RangeConstraint::uniform(Some(f64::NEG_INFINITY), Some(1.)),
            ..bounds()
        },
        ParameterConstraints {
            range: RangeConstraint::uniform(Some(2.), Some(1.)),
            ..bounds()
        },
    ] {
        assert!(
            prepare_param_value_for_target_owned(ParamValue::Float(0.5), &ParamValue::Float(0.), &constraints).is_err()
        );
    }
}
