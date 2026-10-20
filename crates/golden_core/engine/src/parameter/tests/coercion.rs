use super::*;

#[test]
fn parameter_preparation_moves_enum_and_reference_storage() {
    let parameter = Parameter::new(
        "Choice",
        ParamValue::Enum("initial".into()),
        ParameterChangeCheck::ValueChange,
    );
    let text = "selected".repeat(32_768);
    let pointer = text.as_ptr();
    let prepared = parameter.engine_prepare_param_value(ParamValue::Str(text)).unwrap();
    let ParamValue::Enum(text) = prepared else {
        panic!("enum coercion must retain its target kind")
    };
    assert_eq!(text.as_ptr(), pointer);

    let mut reference = crate::node::NodeReference::empty();
    reference.cached_name = Some("recovery".repeat(32_768));
    let pointer = reference.cached_name.as_ref().unwrap().as_ptr();
    let parameter = Parameter::new(
        "Reference",
        ParamValue::Reference(crate::node::NodeReference::empty()),
        ParameterChangeCheck::ValueChange,
    );
    let prepared = parameter
        .engine_prepare_param_value(ParamValue::Reference(reference))
        .unwrap();
    let ParamValue::Reference(reference) = prepared else {
        panic!("reference coercion must preserve the complete reference")
    };
    assert_eq!(reference.cached_name.as_ref().unwrap().as_ptr(), pointer);
}

#[test]
fn constraint_revalidation_preserves_unchanged_storage_and_failed_updates_are_atomic() {
    let text = "selected".repeat(32_768);
    let pointer = text.as_ptr();
    let options: ParameterEnumOptions = vec![ParameterEnumOption {
        variant_id: text.clone(),
        value: ParamValue::Enum(text.clone()),
        label: "Selected".into(),
        tags: vec![],
        ordering: None,
    }]
    .into();
    let mut parameter = Parameter::new("Choice", ParamValue::Enum(text), ParameterChangeCheck::ValueChange);
    for _ in 0..8 {
        parameter
            .engine_set_param_constraints(ParameterConstraints {
                enum_options: options.clone(),
                ..Default::default()
            })
            .unwrap();
        let ParamValue::Enum(value) = &parameter.value else {
            unreachable!()
        };
        assert_eq!(value.as_ptr(), pointer);
        assert!(parameter.constraints.enum_options.same_storage(&options));
    }
    let rejected = ParameterConstraints {
        enum_options: vec![ParameterEnumOption {
            variant_id: "other".into(),
            value: ParamValue::Enum("other".into()),
            label: "Other".into(),
            tags: vec![],
            ordering: None,
        }]
        .into(),
        ..Default::default()
    };
    assert!(parameter.engine_set_param_constraints(rejected).is_err());
    let ParamValue::Enum(value) = &parameter.value else {
        unreachable!()
    };
    assert_eq!(value.as_ptr(), pointer);
    assert!(parameter.constraints.enum_options.same_storage(&options));
}

#[test]
fn typed_history_restore_moves_text_and_preserves_normalization() {
    let text = "restored".repeat(32_768);
    let pointer = text.as_ptr();
    let mut parameter = Parameter::new(
        "Choice",
        ParamValue::Enum("initial".into()),
        ParameterChangeCheck::ValueChange,
    );
    parameter
        .engine_restore_param_state(ParamValue::Str(text), ParameterConstraints::default())
        .unwrap();
    let ParamValue::Enum(value) = &parameter.value else {
        unreachable!()
    };
    assert_eq!(value.as_ptr(), pointer);

    let mut parameter = Parameter::new("Number", ParamValue::Float(0.0), ParameterChangeCheck::ValueChange);
    let constraints = ParameterConstraints {
        range: RangeConstraint::uniform(Some(1.0), Some(2.0)),
        ..Default::default()
    };
    parameter
        .engine_restore_param_state(ParamValue::Int(7), constraints)
        .unwrap();
    assert_eq!(parameter.value, ParamValue::Float(2.0));
    let before = parameter.snapshot();
    assert!(
        parameter
            .engine_restore_param_state(ParamValue::File("invalid".into()), Default::default())
            .is_err()
    );
    assert_eq!(parameter.snapshot(), before);
}
