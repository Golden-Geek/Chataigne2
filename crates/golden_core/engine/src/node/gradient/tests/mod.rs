use super::model::{Gradient, GradientInterpolation, GradientStop};
use crate::color::Color;

fn black() -> Color {
    Color::new(0.0, 0.0, 0.0, 1.0)
}

fn white() -> Color {
    Color::new(1.0, 1.0, 1.0, 1.0)
}

#[test]
fn linear_interpolation_blends_midpoint() {
    let gradient = Gradient::new(vec![
        GradientStop::new(0.0, black(), GradientInterpolation::Linear),
        GradientStop::new(1.0, white(), GradientInterpolation::Linear),
    ]);
    let mid = gradient.sample(0.5).expect("gradient has stops");
    assert!((mid.r() - 0.5).abs() < 1e-9);
    assert!((mid.g() - 0.5).abs() < 1e-9);
    assert!((mid.b() - 0.5).abs() < 1e-9);
}

#[test]
fn none_interpolation_holds_left_color() {
    let gradient = Gradient::new(vec![
        GradientStop::new(0.0, Color::new(0.2, 0.2, 0.2, 1.0), GradientInterpolation::None),
        GradientStop::new(1.0, Color::new(0.8, 0.8, 0.8, 1.0), GradientInterpolation::Linear),
    ]);
    let mid = gradient.sample(0.5).expect("gradient has stops");
    assert!((mid.r() - 0.2).abs() < 1e-9);
}

#[test]
fn samples_clamp_outside_range() {
    let gradient = Gradient::new(vec![
        GradientStop::new(0.25, Color::new(1.0, 0.0, 0.0, 1.0), GradientInterpolation::Linear),
        GradientStop::new(0.75, Color::new(0.0, 0.0, 1.0, 1.0), GradientInterpolation::Linear),
    ]);
    assert_eq!(gradient.sample(0.0), Some(Color::new(1.0, 0.0, 0.0, 1.0)));
    assert_eq!(gradient.sample(1.0), Some(Color::new(0.0, 0.0, 1.0, 1.0)));
}

#[test]
fn unsorted_stops_are_ordered_by_position() {
    let gradient = Gradient::new(vec![
        GradientStop::new(1.0, white(), GradientInterpolation::Linear),
        GradientStop::new(0.0, black(), GradientInterpolation::Linear),
    ]);
    let stops = gradient.stops();
    assert_eq!(stops[0].position, 0.0);
    assert_eq!(stops[1].position, 1.0);
}

#[test]
fn empty_gradient_has_no_sample() {
    let gradient = Gradient::new(Vec::new());
    assert_eq!(gradient.sample(0.5), None);
    assert_eq!(gradient.sample_or_black(0.5), black());
}
