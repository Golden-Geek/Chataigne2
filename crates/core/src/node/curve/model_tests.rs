use super::model::*;

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= 1e-6,
        "expected {expected}, got {actual} (delta={})",
        (actual - expected).abs()
    );
}

#[test]
fn linear_curve_samples_expected_values() {
    let curve = Curve::new(vec![
        CurveKey::new(0.0, 0.0, CurveEasing::Linear),
        CurveKey::new(10.0, 10.0, CurveEasing::Linear),
    ]);

    assert_close(curve.sample(-1.0).expect("sample should exist"), 0.0);
    assert_close(curve.sample(5.0).expect("sample should exist"), 5.0);
    assert_close(curve.sample(15.0).expect("sample should exist"), 10.0);
}

#[test]
fn value_range_constraint_clamps_sampled_output() {
    let mut curve = Curve::new(vec![
        CurveKey::new(0.0, -10.0, CurveEasing::Linear),
        CurveKey::new(1.0, 10.0, CurveEasing::Linear),
    ]);
    curve.set_value_range_constraint(Some(-2.0), Some(3.0));

    assert_close(curve.sample(0.0).expect("sample should exist"), -2.0);
    assert_close(curve.sample(1.0).expect("sample should exist"), 3.0);
    assert_close(curve.sample(0.5).expect("sample should exist"), 0.0);
}

#[test]
fn hold_easing_keeps_start_value_until_boundary() {
    let curve = Curve::new(vec![
        CurveKey::new(0.0, 3.0, CurveEasing::Hold),
        CurveKey::new(2.0, 9.0, CurveEasing::Linear),
    ]);

    assert_close(curve.sample(0.5).expect("sample should exist"), 3.0);
    assert_close(curve.sample(1.9).expect("sample should exist"), 3.0);
    assert_close(curve.sample(2.0).expect("sample should exist"), 9.0);
}

#[test]
fn steps_mode_num_steps_quantizes_progress() {
    let curve = Curve::new(vec![
        CurveKey::new(
            0.0,
            0.0,
            CurveEasing::Steps {
                step_mode: CurveStepMode::NumSteps,
                step_size: 0.1,
                num_steps: 4,
            },
        ),
        CurveKey::new(4.0, 8.0, CurveEasing::Linear),
    ]);

    assert_close(curve.sample(0.9).expect("sample should exist"), 0.0);
    assert_close(curve.sample(1.1).expect("sample should exist"), 2.0);
    assert_close(curve.sample(2.2).expect("sample should exist"), 4.0);
    assert_close(curve.sample(3.8).expect("sample should exist"), 6.0);
}

#[test]
fn bezier_easing_stays_on_key_boundaries() {
    let curve = Curve::new(vec![
        CurveKey::new(
            0.0,
            0.0,
            CurveEasing::Bezier {
                out_handle: CurveHandle::new(0.2, 0.0),
                in_handle: CurveHandle::new(-0.2, -0.2),
            },
        ),
        CurveKey::new(1.0, 1.0, CurveEasing::Linear),
    ]);

    assert_close(curve.sample(0.0).expect("sample should exist"), 0.0);
    assert_close(curve.sample(1.0).expect("sample should exist"), 1.0);
}

#[test]
fn bezier_crossed_handle_x_stays_non_linear() {
    let curve = Curve::new(vec![
        CurveKey::new(
            0.0,
            0.0,
            CurveEasing::Bezier {
                out_handle: CurveHandle::new(1.0, 0.0),
                in_handle: CurveHandle::new(-1.0, 0.0),
            },
        ),
        CurveKey::new(1.0, 1.0, CurveEasing::Linear),
    ]);

    let quarter = curve.sample(0.25).expect("sample should exist");
    let half = curve.sample(0.5).expect("sample should exist");
    let three_quarter = curve.sample(0.75).expect("sample should exist");

    assert!(quarter < 0.2, "expected strong ease-in shape, got {quarter}");
    assert!(
        (half - 0.5).abs() < 0.01,
        "expected midpoint to stay near 0.5, got {half}"
    );
    assert!(
        three_quarter > 0.8,
        "expected strong ease-out shape, got {three_quarter}"
    );
}

#[test]
fn shape_relative_mode_modulates_linear_segment() {
    let curve = Curve::new(vec![
        CurveKey::new(
            0.0,
            0.0,
            CurveEasing::Shape {
                shape: CurveShape::Sine,
                amplitude: 0.5,
                phase_mode: CurvePhaseMode::NumPhases,
                frequency: 1.0,
                num_phases: 1.0,
                fade_in: 0.0,
                fade_out: 0.0,
            },
        ),
        CurveKey::new(1.0, 1.0, CurveEasing::Linear),
    ]);

    assert_close(curve.sample(0.25).expect("sample should exist"), 0.75);
}

#[test]
fn perlin_noise_is_deterministic() {
    let curve = Curve::new(vec![
        CurveKey::new(
            0.0,
            0.0,
            CurveEasing::PerlinNoise {
                frequency: 2.0,
                amplitude: 1.0,
                octaves: 5,
                fade_in: 0.0,
                fade_out: 0.0,
                phase: 0.3,
            },
        ),
        CurveKey::new(2.0, 1.0, CurveEasing::Linear),
    ]);

    let a = curve.sample(0.77).expect("sample should exist");
    let b = curve.sample(0.77).expect("sample should exist");
    assert_close(a, b);
}

#[test]
fn random_mode_uses_seed() {
    let curve_a = Curve::new(vec![
        CurveKey::new(
            0.0,
            0.0,
            CurveEasing::Random {
                frequency: 10.0,
                fade_in: 0.0,
                fade_out: 0.0,
                seed: 12,
            },
        ),
        CurveKey::new(1.0, 1.0, CurveEasing::Linear),
    ]);
    let curve_b = Curve::new(vec![
        CurveKey::new(
            0.0,
            0.0,
            CurveEasing::Random {
                frequency: 10.0,
                fade_in: 0.0,
                fade_out: 0.0,
                seed: 12,
            },
        ),
        CurveKey::new(1.0, 1.0, CurveEasing::Linear),
    ]);
    let curve_c = Curve::new(vec![
        CurveKey::new(
            0.0,
            0.0,
            CurveEasing::Random {
                frequency: 10.0,
                fade_in: 0.0,
                fade_out: 0.0,
                seed: 42,
            },
        ),
        CurveKey::new(1.0, 1.0, CurveEasing::Linear),
    ]);

    let a = curve_a.sample(0.43).expect("sample should exist");
    let b = curve_b.sample(0.43).expect("sample should exist");
    let c = curve_c.sample(0.43).expect("sample should exist");
    assert_close(a, b);
    assert!((a - c).abs() > 1e-6, "different seeds should produce different values");
}

#[test]
fn sample_range_matches_single_point_sampling() {
    let curve = Curve::new(vec![
        CurveKey::new(0.0, 0.0, CurveEasing::Linear),
        CurveKey::new(1.0, 1.0, CurveEasing::Hold),
        CurveKey::new(2.0, 0.0, CurveEasing::Linear),
    ]);

    let mut sampled = vec![0.0; 257];
    let written = curve.sample_range(0.0, 2.0, sampled.as_mut_slice());
    assert_eq!(written, sampled.len());

    let step = 2.0 / ((sampled.len() - 1) as f64);
    for (index, value) in sampled.iter().enumerate() {
        let position = if index + 1 == sampled.len() {
            2.0
        } else {
            step * (index as f64)
        };
        let expected = curve.sample(position).expect("sample should exist");
        assert_close(*value, expected);
    }
}

#[test]
fn cursor_sampling_matches_regular_sampling_in_both_directions() {
    let curve = Curve::new(vec![
        CurveKey::new(0.0, 1.0, CurveEasing::Linear),
        CurveKey::new(1.0, 5.0, CurveEasing::Linear),
        CurveKey::new(2.0, 2.0, CurveEasing::Linear),
    ]);

    let mut cursor = CurveCursor::new();
    for index in 0..200 {
        let position = index as f64 / 100.0;
        let with_cursor = curve
            .sample_with_cursor(position, &mut cursor)
            .expect("sample should exist");
        let without_cursor = curve.sample(position).expect("sample should exist");
        assert_close(with_cursor, without_cursor);
    }

    for index in (0..200).rev() {
        let position = index as f64 / 100.0;
        let with_cursor = curve
            .sample_with_cursor(position, &mut cursor)
            .expect("sample should exist");
        let without_cursor = curve.sample(position).expect("sample should exist");
        assert_close(with_cursor, without_cursor);
    }
}

struct ScriptSampler {
    calls: usize,
}

impl CurveScriptSampler for ScriptSampler {
    fn sample(&mut self, source: &str, context: CurveSampleContext) -> Option<f64> {
        self.calls += 1;
        if source == "offset+2" {
            Some(context.linear_value + 2.0)
        } else {
            None
        }
    }
}

#[test]
fn script_easing_uses_host_callback() {
    let curve = Curve::new(vec![
        CurveKey::new(
            0.0,
            0.0,
            CurveEasing::Script {
                source: "offset+2".to_string(),
            },
        ),
        CurveKey::new(10.0, 10.0, CurveEasing::Linear),
    ]);

    let mut sampler = ScriptSampler { calls: 0 };
    let sampled = curve
        .sample_with_script(5.0, &mut sampler)
        .expect("sample should exist");
    assert_close(sampled, 7.0);
    assert_eq!(sampler.calls, 1);
}

#[test]
fn constructor_sorts_and_deduplicates_keys() {
    let curve = Curve::new(vec![
        CurveKey::new(2.0, 2.0, CurveEasing::Linear),
        CurveKey::new(0.0, 0.0, CurveEasing::Linear),
        CurveKey::new(1.0, 1.0, CurveEasing::Linear),
        CurveKey::new(1.0, 7.0, CurveEasing::Hold),
        CurveKey::new(f64::NAN, 9.0, CurveEasing::Linear),
    ]);

    assert_eq!(curve.key_count(), 3);
    assert_close(curve.keys()[0].position, 0.0);
    assert_close(curve.keys()[1].position, 1.0);
    assert_close(curve.keys()[2].position, 2.0);
    assert_close(curve.keys()[1].value, 7.0);
}

#[test]
fn fit_points_to_bezier_keys_preserves_straight_line_with_two_keys() {
    let keys = fit_points_to_bezier_keys(
        &[
            CurveFitPoint::new(0.0, 0.0),
            CurveFitPoint::new(0.5, 0.5),
            CurveFitPoint::new(1.0, 1.0),
        ],
        CurveBezierFitOptions {
            max_value_error: 1e-6,
            max_keys: 8,
        },
    );

    assert_eq!(keys.len(), 2, "straight line should fit into one segment");
    let curve = Curve::new(keys);
    assert_close(curve.sample(0.0).expect("sample should exist"), 0.0);
    assert_close(curve.sample(0.25).expect("sample should exist"), 0.25);
    assert_close(curve.sample(0.5).expect("sample should exist"), 0.5);
    assert_close(curve.sample(0.75).expect("sample should exist"), 0.75);
    assert_close(curve.sample(1.0).expect("sample should exist"), 1.0);
}

#[test]
fn fit_points_to_bezier_keys_keeps_last_duplicate_position_sample() {
    let keys = fit_points_to_bezier_keys(
        &[
            CurveFitPoint::new(0.0, 0.0),
            CurveFitPoint::new(0.5, 0.2),
            CurveFitPoint::new(0.5, 0.8),
            CurveFitPoint::new(1.0, 1.0),
        ],
        CurveBezierFitOptions {
            max_value_error: 0.05,
            max_keys: 8,
        },
    );

    let curve = Curve::new(keys);
    let sample = curve.sample(0.5).expect("sample should exist");
    assert!(
        (sample - 0.8).abs() < 0.12,
        "fit should honor the last sample at a duplicated position"
    );
}

#[test]
fn fit_points_to_bezier_keys_splits_curved_path_when_error_budget_is_tight() {
    let keys = fit_points_to_bezier_keys(
        &[
            CurveFitPoint::new(0.0, 0.0),
            CurveFitPoint::new(0.25, 1.0),
            CurveFitPoint::new(0.5, 0.0),
            CurveFitPoint::new(0.75, -1.0),
            CurveFitPoint::new(1.0, 0.0),
        ],
        CurveBezierFitOptions {
            max_value_error: 0.02,
            max_keys: 12,
        },
    );

    assert!(keys.len() > 2, "nonlinear path should split into multiple bezier keys");
}
