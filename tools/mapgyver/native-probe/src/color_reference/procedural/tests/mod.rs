use super::*;

#[test]
fn decimal_digits_have_the_expected_segments_and_dark_centers() {
    let centers = [
        (0.5, 0.08),
        (0.87, 0.29),
        (0.87, 0.71),
        (0.5, 0.92),
        (0.13, 0.71),
        (0.13, 0.29),
        (0.5, 0.5),
    ];
    for (number, expected) in [
        "1111110", "0110000", "1101101", "1111001", "0110011", "1011011", "1011111", "1110000", "1111111", "1111011",
    ]
    .iter()
    .enumerate()
    {
        let actual = centers
            .iter()
            .map(|&(x, y)| if digit(x, y, number as u32) { '1' } else { '0' })
            .collect::<String>();
        assert_eq!(&actual, expected);
        assert!(!digit(0.4, 0.3, number as u32));
    }
}

#[test]
fn marker_cells_preserve_word_order_bgra_identity_and_all_stamp_bits() {
    for channel in 0..2 {
        let input = CardInput::fixed(channel).unwrap();
        let extent = [1920, 1080];
        for (cell, word) in input.marker_words().into_iter().enumerate() {
            let x = ((0.03 + cell as f64 * 0.02) * 1920.0) as u32;
            let bgra = pixel(extent, x, 286, input);
            assert_eq!(u32::from_le_bytes([bgra[2], bgra[1], bgra[0], bgra[3]]), word);
        }
        assert_eq!(input.marker_words()[1], 0xa671_c300 | channel);
        assert_eq!(
            input.marker_words()[2] as u64 | ((input.marker_words()[3] as u64) << 32),
            input.stamp
        );
    }
}

#[test]
fn opaque_color_grid_and_all_four_alpha_stripes_are_independently_known() {
    let input = CardInput::fixed(0).unwrap();
    assert_eq!(pixel([1920, 1080], 960, 540, input), [0, 64, 255, 255]);
    assert_eq!(pixel([1920, 1080], 0, 0, input), [255; 4]);
    assert_eq!(pixel([1920, 1080], 60, 350, input), [7, 36, 122, 255]);
    for (x, alpha) in [(240, 64), (720, 128), (1200, 191), (1680, 255)] {
        assert_eq!(pixel([1920, 1080], x, 1000, input), [0, 64, 255, alpha]);
    }
    assert!(CardInput::fixed(2).is_err());
}
