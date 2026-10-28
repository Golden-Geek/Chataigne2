use crate::{ColorError, LinearColor, Opacity};

#[test]
fn straight_colors_are_converted_to_linear_premultiplied_coverage() {
    for (straight, premultiplied) in [
        ([1.0, 0.5, 0.25, 0.0], [0.0, 0.0, 0.0, 0.0]),
        ([1.0, 0.5, 0.25, 1.0], [1.0, 0.5, 0.25, 1.0]),
        ([1.0, 0.5, 0.25, 0.25], [0.25, 0.125, 0.0625, 0.25]),
    ] {
        let [red, green, blue, alpha] = straight;
        let color = LinearColor::from_straight(red, green, blue, alpha).unwrap();
        assert_eq!(color.components(), premultiplied);
        assert_eq!(LinearColor::premultiplied(premultiplied), Ok(color));
    }
    assert_eq!(LinearColor::TRANSPARENT.components(), [0.0; 4]);
}

#[test]
fn premultiplied_colors_accept_coverage_boundaries_without_a_second_multiplication() {
    for components in [[0.0; 4], [1.0; 4], [0.5, 0.25, 0.0, 0.5], [0.5; 4]] {
        assert_eq!(LinearColor::premultiplied(components).unwrap().components(), components);
    }
}

#[test]
fn premultiplied_rgb_cannot_exceed_alpha_even_by_one_representable_step() {
    let above_alpha = f32::from_bits(0.5_f32.to_bits() + 1);
    for channel in 0..3 {
        let mut color = [0.0, 0.0, 0.0, 0.5];
        color[channel] = above_alpha;
        assert_eq!(
            LinearColor::premultiplied(color),
            Err(ColorError::NotPremultiplied),
            "RGB channel {channel} exceeded coverage"
        );
        color[channel] = f32::MIN_POSITIVE;
        color[3] = 0.0;
        assert_eq!(LinearColor::premultiplied(color), Err(ColorError::NotPremultiplied));
    }
}

#[test]
fn nonfinite_components_are_rejected_in_every_position_for_both_color_constructors() {
    for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        for channel in 0..4 {
            let mut color = [0.25, 0.25, 0.25, 0.5];
            color[channel] = invalid;
            assert_eq!(
                LinearColor::premultiplied(color),
                Err(ColorError::NonFinite),
                "premultiplied component {channel} accepted {invalid}"
            );
            let [red, green, blue, alpha] = color;
            assert_eq!(
                LinearColor::from_straight(red, green, blue, alpha),
                Err(ColorError::NonFinite),
                "straight component {channel} accepted {invalid}"
            );
        }
    }
}

#[test]
fn out_of_range_components_are_rejected_instead_of_clamped() {
    let below_zero = -f32::from_bits(1);
    let above_one = f32::from_bits(1.0_f32.to_bits() + 1);
    for invalid in [below_zero, above_one] {
        for channel in 0..4 {
            let mut color = [0.25, 0.25, 0.25, 0.5];
            color[channel] = invalid;
            assert_eq!(LinearColor::premultiplied(color), Err(ColorError::OutOfRange));
            let [red, green, blue, alpha] = color;
            assert_eq!(
                LinearColor::from_straight(red, green, blue, alpha),
                Err(ColorError::OutOfRange)
            );
        }
    }
}

#[test]
fn opacity_retains_valid_coverage_including_both_closed_boundaries() {
    for value in [0.0, f32::MIN_POSITIVE, 0.25, 0.5, 1.0] {
        assert_eq!(Opacity::new(value).unwrap().get(), value);
    }
    assert_eq!(Opacity::TRANSPARENT, Opacity::new(0.0).unwrap());
    assert_eq!(Opacity::OPAQUE, Opacity::new(1.0).unwrap());
}

#[test]
fn invalid_opacity_is_rejected_without_sanitizing_authored_intent() {
    for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert_eq!(Opacity::new(invalid), Err(ColorError::NonFinite));
    }
    for invalid in [-f32::from_bits(1), f32::from_bits(1.0_f32.to_bits() + 1)] {
        assert_eq!(Opacity::new(invalid), Err(ColorError::OutOfRange));
    }
}
