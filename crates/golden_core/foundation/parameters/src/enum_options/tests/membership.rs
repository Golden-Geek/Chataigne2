use super::*;
use crate::{CssUnit, CssValue, NodeReference, ParamValueProjection};
use golden_model::{NodeId, NodeUuid};

fn domain(values: impl IntoIterator<Item = ParamValue>) -> ParameterEnumOptions {
    values
        .into_iter()
        .enumerate()
        .map(|(index, value)| ParameterEnumOption {
            variant_id: format!("variant-{}", index % 7),
            value,
            label: format!("entry-{index}"),
            tags: vec![],
            ordering: None,
        })
        .collect()
}

fn mixed_values() -> Vec<ParamValue> {
    let mut values = vec![
        ParamValue::Trigger(),
        ParamValue::Int(i32::MIN),
        ParamValue::Int(0),
        ParamValue::Int(i32::MAX),
        ParamValue::Bool(false),
        ParamValue::Bool(true),
        ParamValue::Str(String::new()),
        ParamValue::Str("same".into()),
        ParamValue::Enum("same".into()),
        ParamValue::Enum("é\0雪".into()),
        ParamValue::File("same".into()),
        ParamValue::File("C:/somewhere/file.wav".into()),
    ];
    for value in [
        f64::NEG_INFINITY,
        -1.0,
        -0.0,
        0.0,
        f64::from_bits(1),
        1.0,
        f64::INFINITY,
        f64::NAN,
        f64::from_bits(u64::MAX),
    ] {
        values.extend([
            ParamValue::Float(value),
            ParamValue::Vec2(value, 0.0),
            ParamValue::Vec3(0.0, value, 1.0),
            ParamValue::Color(0.0, 1.0, value, 1.0),
        ]);
        for unit in [
            CssUnit::Px,
            CssUnit::Rem,
            CssUnit::Em,
            CssUnit::Percent,
            CssUnit::Vw,
            CssUnit::Vh,
        ] {
            values.push(ParamValue::CssValue(CssValue::new(value, unit)));
        }
    }
    for projection in [
        None,
        Some(ParamValueProjection::Vec2X),
        Some(ParamValueProjection::ColorToVec2Hs),
    ] {
        let mut reference = NodeReference::new(NodeUuid::nil());
        reference.projection = projection;
        values.push(ParamValue::Reference(reference.clone()));
        reference.cached_id = Some(NodeId(123));
        reference.cached_name = Some("hint".into());
        reference.relative_path_from_root = vec!["old".into(), "path".into()];
        values.push(ParamValue::Reference(reference));
    }
    values
}

#[test]
fn indexed_membership_matches_linear_value_equality_across_all_kinds_and_duplicates() {
    let values = mixed_values();
    // Alternating source positions leave both hits and misses in every family.
    for parity in [0, 1] {
        let options = domain(
            values
                .iter()
                .enumerate()
                .filter(|(index, _)| index % 2 == parity)
                .map(|(_, value)| value.clone())
                .rev(),
        );
        for value in &values {
            assert_eq!(
                options.contains_value(value),
                options.iter().any(|option| option.value == *value),
                "{value:?}"
            );
        }
        for variant in ["variant-0", "variant-6", "variant-7", "", "é\0雪"] {
            assert_eq!(
                options.contains_variant(variant),
                options.iter().any(|option| option.variant_id == variant)
            );
        }
        assert_eq!(
            options.iter().map(|option| option.label.clone()).collect::<Vec<_>>(),
            (0..options.len())
                .map(|index| format!("entry-{index}"))
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn zero_sign_and_reference_hints_do_not_change_indexed_equality() {
    for (left, right) in [
        (ParamValue::Float(-0.0), ParamValue::Float(0.0)),
        (ParamValue::Vec2(-0.0, 0.0), ParamValue::Vec2(0.0, -0.0)),
        (ParamValue::Vec3(-0.0, 1.0, 0.0), ParamValue::Vec3(0.0, 1.0, -0.0)),
        (
            ParamValue::Color(-0.0, 0.0, 1.0, 1.0),
            ParamValue::Color(0.0, -0.0, 1.0, 1.0),
        ),
        (
            ParamValue::CssValue(CssValue::new(-0.0, CssUnit::Rem)),
            ParamValue::CssValue(CssValue::new(0.0, CssUnit::Rem)),
        ),
    ] {
        assert!(domain([left.clone()]).contains_value(&right));
        assert!(domain([right]).contains_value(&left));
    }
    let original = NodeReference::empty();
    let mut hinted = original.clone();
    hinted.cached_name = Some("display name".into());
    hinted.cached_id = Some(NodeId(7));
    hinted.relative_path_from_root = vec!["somewhere".into()];
    let options = domain([ParamValue::Reference(original)]);
    assert!(options.contains_value(&ParamValue::Reference(hinted.clone())));
    hinted.projection = Some(ParamValueProjection::Vec2X);
    assert!(!options.contains_value(&ParamValue::Reference(hinted)));
    assert!(!domain([ParamValue::Float(f64::NAN)]).contains_value(&ParamValue::Float(f64::NAN)));
}

#[test]
fn normalization_preserves_variant_aliases_and_exact_value_membership() {
    let options: ParameterEnumOptions = [
        ("id", ParamValue::Int(1)),
        ("id", ParamValue::Float(2.0)),
        ("other", ParamValue::Str("payload".into())),
    ]
    .into_iter()
    .map(|(id, value)| ParameterEnumOption {
        variant_id: id.into(),
        value,
        label: id.into(),
        tags: vec![],
        ordering: None,
    })
    .collect();
    let constraints = ParameterConstraints {
        enum_options: options,
        ..Default::default()
    };
    for value in [
        ParamValue::Int(1),
        ParamValue::Float(2.0),
        ParamValue::Str("payload".into()),
        ParamValue::Enum("id".into()),
    ] {
        assert_eq!(constraints.normalize(value.clone()).unwrap(), value);
    }
    assert_eq!(
        constraints.normalize(ParamValue::Str("id".into())).unwrap(),
        ParamValue::Enum("id".into())
    );
    assert!(constraints.normalize(ParamValue::Enum("payload".into())).is_err());
    assert!(
        constraints.normalize(ParamValue::Float(1.0)).is_err(),
        "int and float are distinct exact values"
    );
}

#[test]
fn large_domains_keep_authored_order_and_rejection_diagnostics_bounded() {
    let start = std::time::Instant::now();
    let options: ParameterEnumOptions = (0..32_768).rev().map(option).collect();
    let build = start.elapsed();
    assert_eq!(options.first().unwrap().variant_id, "variant-32767");
    assert_eq!(options.last().unwrap().variant_id, "variant-0");
    let start = std::time::Instant::now();
    for index in 0..1024 {
        assert!(options.contains_variant(&format!("variant-{}", index * 31)));
        assert!(options.contains_value(&ParamValue::Enum(format!("variant-{}", index * 31))));
        assert!(!options.contains_variant(&format!("absent-{index}")));
    }
    let lookup = start.elapsed();
    let constraints = ParameterConstraints {
        enum_options: options,
        ..Default::default()
    };
    let error = constraints
        .normalize_ref(&ParamValue::Str("x".repeat(1024 * 1024)))
        .unwrap_err();
    assert!(error.len() < 128);
    assert!(error.contains("32768"));
    eprintln!("32768-entry domain: freeze {build:?}; 3072 membership lookups {lookup:?}");
}

#[test]
fn huge_variant_ids_are_not_copied_into_rejection_messages() {
    let mut entry = option(0);
    entry.variant_id = "雪".repeat(400_000);
    let constraints = ParameterConstraints {
        enum_options: vec![entry].into(),
        ..Default::default()
    };
    let error = constraints
        .normalize_ref(&ParamValue::Str("invalid".into()))
        .unwrap_err();
    assert!(error.len() < 128);
}
