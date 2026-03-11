use super::*;

#[test]
fn parameter_nodes_are_not_disableable_by_default() {
    let parameter = Parameter::new("Amount", ParamValue::Float(0.5), ParameterChangeCheck::ValueChange);

    assert!(!parameter.node_data().meta.can_be_disabled);
}

fn approx_eq(left: f64, right: f64) {
    assert!((left - right).abs() < 1e-9, "expected {left} ~= {right}");
}

#[test]
fn file_constraints_accept_matching_extension() {
    let constraints = ParameterConstraints {
        file: FileConstraints {
            allowed_types: vec![FileTypeGroup::Audio],
            allowed_extensions: vec![".WAV".to_string()],
        },
        ..Default::default()
    };

    let normalized = constraints
        .normalize(ParamValue::File("C:/tmp/kick.wav".to_string()))
        .expect("wav should pass file constraints");
    assert_eq!(normalized, ParamValue::File("C:/tmp/kick.wav".to_string()));
}

#[test]
fn file_constraints_reject_non_matching_extension() {
    let constraints = ParameterConstraints {
        file: FileConstraints {
            allowed_types: vec![FileTypeGroup::Audio],
            allowed_extensions: vec!["wav".to_string(), "flac".to_string()],
        },
        ..Default::default()
    };

    let error = constraints
        .normalize(ParamValue::File("C:/tmp/clip.mp4".to_string()))
        .expect_err("mp4 should fail audio constraints");
    assert!(error.contains("not allowed"));
}

#[test]
fn template_text_mode_is_only_supported_for_string_parameters() {
    let string_value = ParamValue::Str("demo".to_string());
    let int_value = ParamValue::Int(42);

    assert!(control_mode_supported_for_value(
        ParameterControlMode::TemplateText,
        &string_value
    ));
    assert!(!control_mode_supported_for_value(
        ParameterControlMode::TemplateText,
        &int_value
    ));

    let string_modes = available_control_modes_for_value(&string_value);
    assert!(string_modes.contains(&ParameterControlMode::TemplateText));

    let int_modes = available_control_modes_for_value(&int_value);
    assert!(!int_modes.contains(&ParameterControlMode::TemplateText));
}

#[test]
fn projection_supports_float_expansions() {
    let source = ParamValue::Float(2.5);

    assert_eq!(
        project_param_value(&source, ParamValueProjection::FloatToVec2X0),
        Some(ParamValue::Vec2(2.5, 0.0))
    );
    assert_eq!(
        project_param_value(&source, ParamValueProjection::FloatToVec20Y),
        Some(ParamValue::Vec2(0.0, 2.5))
    );
    assert_eq!(
        project_param_value(&source, ParamValueProjection::FloatToVec2XX),
        Some(ParamValue::Vec2(2.5, 2.5))
    );
    assert_eq!(
        project_param_value(&source, ParamValueProjection::FloatToVec3X00),
        Some(ParamValue::Vec3(2.5, 0.0, 0.0))
    );
    assert_eq!(
        project_param_value(&source, ParamValueProjection::FloatToVec30Y0),
        Some(ParamValue::Vec3(0.0, 2.5, 0.0))
    );
    assert_eq!(
        project_param_value(&source, ParamValueProjection::FloatToVec300Z),
        Some(ParamValue::Vec3(0.0, 0.0, 2.5))
    );
    assert_eq!(
        project_param_value(&source, ParamValueProjection::FloatToVec3XXX),
        Some(ParamValue::Vec3(2.5, 2.5, 2.5))
    );
}

#[test]
fn projection_supports_vec_reshapes() {
    let vec3 = ParamValue::Vec3(1.0, 2.0, 3.0);
    let vec2 = ParamValue::Vec2(4.0, 5.0);

    assert_eq!(
        project_param_value(&vec3, ParamValueProjection::Vec3ToVec2XY),
        Some(ParamValue::Vec2(1.0, 2.0))
    );
    assert_eq!(
        project_param_value(&vec3, ParamValueProjection::Vec3ToVec2XZ),
        Some(ParamValue::Vec2(1.0, 3.0))
    );
    assert_eq!(
        project_param_value(&vec3, ParamValueProjection::Vec3ToVec2YZ),
        Some(ParamValue::Vec2(2.0, 3.0))
    );
    assert_eq!(
        project_param_value(&vec2, ParamValueProjection::Vec2ToVec3XY0),
        Some(ParamValue::Vec3(4.0, 5.0, 0.0))
    );
    assert_eq!(
        project_param_value(&vec2, ParamValueProjection::Vec2ToVec3X0Y),
        Some(ParamValue::Vec3(4.0, 0.0, 5.0))
    );
}

#[test]
fn projection_supports_color_rgb_hsv_mappings() {
    let color = ParamValue::Color(1.0, 0.0, 0.0, 0.75);
    let vec_hs = ParamValue::Vec2(0.0, 1.0);
    let vec_hsv = ParamValue::Vec3(1.0 / 3.0, 1.0, 1.0);
    let vec_rgb = ParamValue::Vec3(0.1, 0.2, 0.3);

    assert_eq!(
        project_param_value(&color, ParamValueProjection::ColorToVec3Rgb),
        Some(ParamValue::Vec3(1.0, 0.0, 0.0))
    );
    let hsv = project_param_value(&color, ParamValueProjection::ColorToVec3Hsv)
        .expect("color->hsv projection should succeed");
    let ParamValue::Vec3(h, s, v) = hsv else {
        panic!("expected vec3 hsv result");
    };
    approx_eq(h, 0.0);
    approx_eq(s, 1.0);
    approx_eq(v, 1.0);

    let hs =
        project_param_value(&color, ParamValueProjection::ColorToVec2Hs).expect("color->hs projection should succeed");
    let ParamValue::Vec2(h, s) = hs else {
        panic!("expected vec2 hs result");
    };
    approx_eq(h, 0.0);
    approx_eq(s, 1.0);

    let color_from_hs = project_param_value(&vec_hs, ParamValueProjection::Vec2ToColorHs)
        .expect("vec2 hs->color projection should succeed");
    let ParamValue::Color(r, g, b, a) = color_from_hs else {
        panic!("expected color result");
    };
    approx_eq(r, 1.0);
    approx_eq(g, 0.0);
    approx_eq(b, 0.0);
    approx_eq(a, 1.0);

    let color_from_hsv = project_param_value(&vec_hsv, ParamValueProjection::Vec3ToColorHsv)
        .expect("vec3 hsv->color projection should succeed");
    let ParamValue::Color(r, g, b, a) = color_from_hsv else {
        panic!("expected color result");
    };
    approx_eq(r, 0.0);
    approx_eq(g, 1.0);
    approx_eq(b, 0.0);
    approx_eq(a, 1.0);

    assert_eq!(
        project_param_value(&vec_rgb, ParamValueProjection::Vec3ToColorRgb),
        Some(ParamValue::Color(0.1, 0.2, 0.3, 1.0))
    );
}

#[test]
fn compatibility_includes_float_to_vec2_projections() {
    let compatibility = compatibility_for_values(&ParamValue::Float(3.0), &ParamValue::Vec2(0.0, 0.0));
    assert!(compatibility.direct, "float->vec2 keeps direct coercion");
    assert!(compatibility.projections.contains(&ParamValueProjection::FloatToVec2X0));
    assert!(compatibility.projections.contains(&ParamValueProjection::FloatToVec20Y));
    assert!(compatibility.projections.contains(&ParamValueProjection::FloatToVec2XX));
}

#[test]
fn projected_expansions_preserve_target_components() {
    assert_eq!(
        coerce_param_value_for_target(
            &ParamValue::Float(2.5),
            &ParamValue::Vec2(9.0, 8.0),
            Some(ParamValueProjection::FloatToVec20Y),
        ),
        Some(ParamValue::Vec2(9.0, 2.5))
    );

    assert_eq!(
        coerce_param_value_for_target(
            &ParamValue::Float(2.5),
            &ParamValue::Vec3(9.0, 8.0, 7.0),
            Some(ParamValueProjection::FloatToVec3X00),
        ),
        Some(ParamValue::Vec3(2.5, 8.0, 7.0))
    );

    assert_eq!(
        coerce_param_value_for_target(
            &ParamValue::Vec2(4.0, 5.0),
            &ParamValue::Vec3(9.0, 8.0, 7.0),
            Some(ParamValueProjection::Vec2ToVec3X0Y),
        ),
        Some(ParamValue::Vec3(4.0, 8.0, 5.0))
    );
}

#[test]
fn projected_color_expansions_preserve_existing_channels() {
    assert_eq!(
        coerce_param_value_for_target(
            &ParamValue::Vec3(0.1, 0.2, 0.3),
            &ParamValue::Color(0.0, 0.0, 0.0, 0.7),
            Some(ParamValueProjection::Vec3ToColorRgb),
        ),
        Some(ParamValue::Color(0.1, 0.2, 0.3, 0.7))
    );

    let converted = coerce_param_value_for_target(
        &ParamValue::Vec2(0.0, 1.0),
        &ParamValue::Color(0.2, 0.4, 0.6, 0.25),
        Some(ParamValueProjection::Vec2ToColorHs),
    )
    .expect("vec2 hs projection should convert against color target");
    let ParamValue::Color(r, g, b, a) = converted else {
        panic!("expected color result");
    };
    approx_eq(r, 0.6);
    approx_eq(g, 0.0);
    approx_eq(b, 0.0);
    approx_eq(a, 0.25);
}

#[test]
fn reverse_projection_preserves_target_components() {
    assert_eq!(
        coerce_param_value_for_target_reverse(
            &ParamValue::Float(3.5),
            &ParamValue::Vec2(1.0, 2.0),
            Some(ParamValueProjection::Vec2X)
        ),
        Some(ParamValue::Vec2(3.5, 2.0))
    );
    assert_eq!(
        coerce_param_value_for_target_reverse(
            &ParamValue::Float(9.0),
            &ParamValue::Vec3(1.0, 2.0, 3.0),
            Some(ParamValueProjection::Vec3Y)
        ),
        Some(ParamValue::Vec3(1.0, 9.0, 3.0))
    );
    assert_eq!(
        coerce_param_value_for_target_reverse(
            &ParamValue::Float(0.6),
            &ParamValue::Color(0.1, 0.2, 0.3, 0.4),
            Some(ParamValueProjection::ColorB)
        ),
        Some(ParamValue::Color(0.1, 0.2, 0.6, 0.4))
    );
}

#[test]
fn binding_compatibility_requires_bidirectional_direct_conversion() {
    let compatibility =
        compatibility_for_binding_values(&ParamValue::Color(0.1, 0.2, 0.3, 1.0), &ParamValue::Vec3(0.0, 0.0, 0.0));
    assert!(!compatibility.direct, "color->vec3 is direct, but vec3->color is not");
    assert!(
        compatibility
            .projections
            .contains(&ParamValueProjection::ColorToVec3Rgb)
    );
}

#[test]
fn binding_projection_roundtrips_vec2_x_to_float() {
    let forward = coerce_param_value_for_target(
        &ParamValue::Vec2(8.0, 4.0),
        &ParamValue::Float(0.0),
        Some(ParamValueProjection::Vec2X),
    )
    .expect("vec2 x projection should produce float");
    assert_eq!(forward, ParamValue::Float(8.0));

    let reverse = coerce_param_value_for_target_reverse(
        &ParamValue::Float(6.0),
        &ParamValue::Vec2(8.0, 4.0),
        Some(ParamValueProjection::Vec2X),
    )
    .expect("reverse vec2 x projection should update vec2");
    assert_eq!(reverse, ParamValue::Vec2(6.0, 4.0));
}
