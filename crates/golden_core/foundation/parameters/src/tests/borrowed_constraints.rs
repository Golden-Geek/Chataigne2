use super::*;
use std::borrow::Cow;

#[test]
fn borrowed_validation_preserves_original_storage_until_an_accepted_value_needs_ownership() {
    let constraints = ParameterConstraints::default();
    for value in [
        ParamValue::Str("a".repeat(4096)),
        ParamValue::File("file.wav".into()),
        ParamValue::Reference(NodeReference::default()),
    ] {
        assert!(
            matches!(constraints.normalize_ref(&value).unwrap(), Cow::Borrowed(borrowed) if std::ptr::eq(borrowed, &value))
        );
    }
    let value = ParamValue::Str("large".repeat(4096));
    let constraints = ParameterConstraints {
        enum_options: (vec![ParameterEnumOption {
            variant_id: "allowed".into(),
            label: "Allowed".into(),
            value: ParamValue::Int(1),
            tags: Vec::new(),
            ordering: None,
        }])
        .into(),
        ..Default::default()
    };
    assert!(constraints.normalize_ref(&value).is_err());
    assert_eq!(
        constraints
            .normalize_ref(&ParamValue::Str("allowed".into()))
            .unwrap()
            .as_ref(),
        &ParamValue::Enum("allowed".into())
    );
    let constraints = ParameterConstraints {
        range: RangeConstraint::uniform(Some(0.0), Some(1.0)),
        ..Default::default()
    };
    assert_eq!(
        constraints.normalize_ref(&ParamValue::Float(2.0)).unwrap().as_ref(),
        &ParamValue::Float(1.0)
    );
}
