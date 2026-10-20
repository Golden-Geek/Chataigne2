use super::*;
use std::borrow::Cow;

#[test]
fn unchanged_kinds_borrow_all_twelve_value_variants() {
    let reference = NodeReference {
        cached_name: Some("display".repeat(1024)),
        relative_path_from_root: vec!["recovery".into()],
        ..Default::default()
    };
    for value in [
        ParamValue::Trigger(),
        ParamValue::Int(-17),
        ParamValue::Float(-0.0),
        ParamValue::Str("text".repeat(1024)),
        ParamValue::File("audio.wav".into()),
        ParamValue::Enum("device".repeat(1024)),
        ParamValue::Bool(true),
        ParamValue::CssValue(CssValue::new(7.0, CssUnit::Percent)),
        ParamValue::Vec2(1.0, 2.0),
        ParamValue::Vec3(1.0, 2.0, 3.0),
        ParamValue::Color(0.1, 0.2, 0.3, 0.4),
        ParamValue::Reference(reference),
    ] {
        assert!(matches!(
            coerce_param_value_for_target_ref(&value, &value, None),
            Some(Cow::Borrowed(borrowed)) if std::ptr::eq(borrowed, &value)
        ));
        assert_eq!(coerce_param_value_for_target(&value, &value, None), Some(value.clone()));
        assert_eq!(coerce_param_value_for_target_reverse(&value, &value, None), Some(value));
    }
}

#[test]
fn textual_conversions_keep_their_allowed_directions_and_move_owned_storage() {
    let constructors: [fn(String) -> ParamValue; 3] = [ParamValue::Str, ParamValue::File, ParamValue::Enum];
    for (source_kind, source) in constructors.iter().enumerate() {
        for (target_kind, target) in constructors.iter().enumerate() {
            let text = "selected-device".repeat(4096);
            let pointer = text.as_ptr();
            let capacity = text.capacity();
            let target = target(String::new());
            let result = coerce_param_value_for_target_owned(source(text), &target, None);
            if source_kind == 1 && target_kind == 2 {
                assert!(result.is_none(), "file paths are not enum identifiers");
            } else {
                let result = result.unwrap();
                assert_eq!(std::mem::discriminant(&result), std::mem::discriminant(&target));
                let (ParamValue::Str(text) | ParamValue::File(text) | ParamValue::Enum(text)) = result else {
                    unreachable!()
                };
                assert_eq!(text.as_ptr(), pointer);
                assert_eq!(text.capacity(), capacity);
            }
        }
    }
}

#[test]
fn conversions_preserve_numeric_css_vector_and_reference_meaning() {
    let reference = NodeReference::default();
    for (source, target, expected) in [
        (
            ParamValue::Str("12".into()),
            ParamValue::Int(0),
            Some(ParamValue::Int(12)),
        ),
        (
            ParamValue::Enum("2.5".into()),
            ParamValue::Float(0.0),
            Some(ParamValue::Float(2.5)),
        ),
        (ParamValue::File("2.5".into()), ParamValue::Float(0.0), None),
        (ParamValue::Float(2.9), ParamValue::Int(0), Some(ParamValue::Int(2))),
        (
            ParamValue::Int(255),
            ParamValue::Color(0.0, 0.0, 0.0, 0.0),
            Some(ParamValue::Color(1.0, 1.0, 1.0, 1.0)),
        ),
        (
            ParamValue::Bool(true),
            ParamValue::Float(0.0),
            Some(ParamValue::Float(1.0)),
        ),
        (
            ParamValue::Str("true".into()),
            ParamValue::Bool(false),
            Some(ParamValue::Bool(true)),
        ),
        (ParamValue::File("true".into()), ParamValue::Bool(false), None),
        (
            ParamValue::Str("3,4".into()),
            ParamValue::Vec2(0.0, 0.0),
            Some(ParamValue::Vec2(3.0, 4.0)),
        ),
        (ParamValue::Vec3(3.0, 4.0, 5.0), ParamValue::Vec2(0.0, 0.0), None),
        (
            ParamValue::Color(0.1, 0.2, 0.3, 0.4),
            ParamValue::Vec3(0.0, 0.0, 0.0),
            Some(ParamValue::Vec3(0.1, 0.2, 0.3)),
        ),
        (
            ParamValue::Trigger(),
            ParamValue::Str(String::new()),
            Some(ParamValue::Str("trigger".into())),
        ),
        (ParamValue::Int(1), ParamValue::Trigger(), None),
        (ParamValue::Str("target".into()), ParamValue::Reference(reference), None),
        (
            ParamValue::Float(4.0),
            ParamValue::CssValue(CssValue::new(0.0, CssUnit::Em)),
            Some(ParamValue::CssValue(CssValue::new(4.0, CssUnit::Em))),
        ),
        (
            ParamValue::CssValue(CssValue::new(7.0, CssUnit::Percent)),
            ParamValue::CssValue(CssValue::new(0.0, CssUnit::Em)),
            Some(ParamValue::CssValue(CssValue::new(7.0, CssUnit::Percent))),
        ),
    ] {
        assert_eq!(coerce_param_value_for_target(&source, &target, None), expected);
        assert_eq!(
            coerce_param_value_for_target_ref(&source, &target, None).map(Cow::into_owned),
            expected
        );
        assert_eq!(coerce_param_value_for_target_owned(source, &target, None), expected);
    }
}

#[test]
fn explicit_projections_and_reverse_binding_keep_target_components() {
    let source = ParamValue::Float(9.0);
    let target = ParamValue::Vec2(3.0, 4.0);
    assert_eq!(
        coerce_param_value_for_target_owned(source.clone(), &target, Some(ParamValueProjection::FloatToVec2X0)),
        Some(ParamValue::Vec2(9.0, 4.0))
    );
    assert_eq!(
        coerce_param_value_for_target_ref(&source, &target, Some(ParamValueProjection::FloatToVec20Y))
            .unwrap()
            .as_ref(),
        &ParamValue::Vec2(3.0, 9.0)
    );
    assert_eq!(
        coerce_param_value_for_target_reverse(&source, &target, Some(ParamValueProjection::Vec2X)),
        Some(ParamValue::Vec2(9.0, 4.0))
    );
    assert!(!compatibility_for_values(&source, &target).direct);
    assert!(
        compatibility_for_values(&source, &target)
            .projections
            .contains(&ParamValueProjection::FloatToVec2X0)
    );
    let reference = ParamValue::Reference(NodeReference::default());
    assert!(compatibility_for_binding_values(&reference, &reference).direct);
}
