use super::choose;
use crate::d3d::OutputInfo;

const UHD: [u32; 2] = [3840, 2160];

fn output(name: &str, attached: bool, desktop_rect: [i32; 4]) -> OutputInfo {
    OutputInfo {
        device_name: name.into(),
        attached,
        desktop_rect,
        coordinate_context: "physical",
    }
}

#[test]
fn exact_fit_wins_over_larger_displays_independently_of_inventory_order() {
    let mut outputs = [
        output("small", true, [0, 0, 2560, 1600]),
        output("larger", true, [0, 0, 7680, 4320]),
        output("exact", true, [2560, 0, 6400, 2160]),
    ];
    for _ in 0..outputs.len() {
        assert_eq!(choose(&outputs, UHD).unwrap().device_name, "exact");
        outputs.rotate_left(1);
    }
}

#[test]
fn an_adapter_without_a_fitting_display_cannot_place_a_uhd_fixture() {
    let outputs = [
        output("primary", true, [0, 0, 2560, 1600]),
        output("secondary", true, [2560, 0, 4480, 1080]),
        output("detached-uhd", false, [0, 0, 3840, 2160]),
    ];
    assert!(choose(&outputs, UHD).is_err());
    assert!(choose(&[], UHD).is_err());
}

#[test]
fn a_single_larger_fitting_display_is_accepted_without_an_exact_match() {
    let outputs = [
        output("small", true, [0, 0, 1920, 1080]),
        output("larger", true, [1920, 0, 7040, 2880]),
    ];
    let selected = choose(&outputs, UHD).unwrap();
    assert!(std::ptr::eq(selected, &outputs[1]));
}

#[test]
fn detached_empty_and_inverted_rectangles_do_not_become_candidates() {
    let outputs = [
        output("detached-exact", false, [0, 0, 3840, 2160]),
        output("empty-width", true, [0, 0, 0, 2160]),
        output("empty-height", true, [0, 0, 3840, 0]),
        output("inverted-width", true, [3840, 0, 0, 2160]),
        output("inverted-height", true, [0, 2160, 3840, 0]),
        output("valid", true, [0, 0, 5120, 2880]),
    ];
    assert_eq!(choose(&outputs, UHD).unwrap().device_name, "valid");
    assert!(choose(&outputs[..5], UHD).is_err());
}

#[test]
fn multiple_larger_fitting_displays_require_an_unambiguous_selection() {
    let mut outputs = [
        output("larger-a", true, [0, 0, 5120, 2880]),
        output("larger-b", true, [5120, 0, 12_800, 4320]),
    ];
    assert!(choose(&outputs, UHD).is_err());
    outputs.reverse();
    assert!(choose(&outputs, UHD).is_err());
}

#[test]
fn multiple_exact_displays_remain_ambiguous_even_with_a_larger_candidate() {
    let mut outputs = [
        output("exact-a", true, [0, 0, 3840, 2160]),
        output("exact-b", true, [3840, 0, 7680, 2160]),
        output("larger", true, [0, 2160, 7680, 6480]),
    ];
    for _ in 0..outputs.len() {
        assert!(choose(&outputs, UHD).is_err());
        outputs.rotate_left(1);
    }
}

#[test]
fn negative_desktop_origins_preserve_the_complete_display_extent() {
    let outputs = [output("negative-origin", true, [-3840, -2160, 0, 0])];
    let selected = choose(&outputs, UHD).unwrap();
    assert_eq!(selected.device_name, "negative-origin");
    assert_eq!(selected.desktop_rect, [-3840, -2160, 0, 0]);
}

#[test]
fn coordinate_subtraction_uses_the_full_range_without_signed_overflow() {
    let outputs = [output(
        "wide-coordinate-range",
        true,
        [i32::MIN, i32::MIN, i32::MAX, i32::MAX],
    )];
    assert_eq!(
        choose(&outputs, [u32::MAX, u32::MAX]).unwrap().device_name,
        "wide-coordinate-range"
    );
    assert!(choose(&outputs, UHD).is_ok());
    let inverted = [output("inverted-range", true, [i32::MAX, i32::MAX, i32::MIN, i32::MIN])];
    assert!(choose(&inverted, [1, 1]).is_err());
}

#[test]
fn zero_requested_dimensions_are_rejected_before_selection() {
    let outputs = [output("attached", true, [0, 0, 3840, 2160])];
    for extent in [[0, 0], [0, 2160], [3840, 0]] {
        assert!(choose(&outputs, extent).is_err(), "{extent:?}");
    }
}
