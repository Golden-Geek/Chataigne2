use super::*;

#[test]
fn non_finite_numbers_are_rejected_at_the_value_boundary() {
    assert_eq!(FiniteF64::new(f64::NAN), Err(ValueError::NonFiniteNumber));
    assert_eq!(FiniteF64::new(f64::INFINITY), Err(ValueError::NonFiniteNumber));
}

#[test]
fn numeric_conversion_is_explicit_and_lossless() {
    let integer = Value::Integer(42);
    assert_eq!(
        integer.convert_to("float"),
        Ok(Value::Float(FiniteF64::new(42.0).unwrap()))
    );

    let fractional = Value::Float(FiniteF64::new(4.5).unwrap());
    assert_eq!(
        fractional.convert_to("integer"),
        Err(ValueError::LossyConversion {
            from: "float",
            to: "integer",
        })
    );
}

#[test]
fn value_sets_have_stable_lane_order() {
    let mut values = ValueSet::new();
    values.insert(LaneKey("second".into()), Value::Integer(2));
    values.insert(LaneKey("first".into()), Value::Integer(1));
    assert_eq!(
        values.keys().map(|key| key.0.as_str()).collect::<Vec<_>>(),
        ["first", "second"]
    );
}
