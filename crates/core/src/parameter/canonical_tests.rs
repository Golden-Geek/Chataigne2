use golden_values::{TriggerValue, Value, ValueTypeId};

use crate::node::{NodeReference, NodeUuid};

use super::{CanonicalValueError, CssUnit, CssValue, ParamValue};

#[test]
fn every_parameter_variant_round_trips_through_the_canonical_value() {
    let mut reference = NodeReference::new(NodeUuid::nil());
    reference.set_cached_name(Some("Dangling target".into()));
    reference.set_relative_path_from_root(vec!["module".into(), "value".into()]);

    let values = [
        ParamValue::Trigger(),
        ParamValue::Int(-12),
        ParamValue::Float(3.5),
        ParamValue::Str("hello".into()),
        ParamValue::File("fixtures/example.noisette".into()),
        ParamValue::Enum("multiply".into()),
        ParamValue::Bool(true),
        ParamValue::CssValue(CssValue::new(1.25, CssUnit::Rem)),
        ParamValue::Vec2(1.0, 2.0),
        ParamValue::Vec3(1.0, 2.0, 3.0),
        ParamValue::Color(0.1, 0.2, 0.3, 0.4),
        ParamValue::Reference(reference),
    ];

    for expected in values {
        let canonical = Value::try_from(&expected).unwrap();
        let actual = ParamValue::try_from(&canonical).unwrap();
        assert_eq!(actual, expected);
    }
}

#[test]
fn unsupported_or_lossy_reverse_conversions_are_explicit() {
    assert_eq!(
        ParamValue::try_from(&Value::Int(i64::MAX)),
        Err(CanonicalValueError::IntegerOutOfRange(i64::MAX))
    );
    assert_eq!(
        ParamValue::try_from(&Value::Trigger(TriggerValue::default())),
        Err(CanonicalValueError::UnfiredTrigger)
    );
    assert_eq!(
        ParamValue::try_from(&Value::Unit),
        Err(CanonicalValueError::UnsupportedValueType(ValueTypeId::new("unit")))
    );
}
