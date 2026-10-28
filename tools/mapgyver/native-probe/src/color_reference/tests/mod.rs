use super::*;

#[test]
fn texel_centers_clamp_and_bilinear_weights_follow_the_authored_image() {
    let image = Image {
        extent: [2, 2],
        pixels: vec![[0; 4], [100; 4], [200; 4], [240; 4]],
    };
    assert_eq!(image.sample(0, 0, [2, 2]), [0; 4]);
    assert_eq!(image.sample(1, 1, [2, 2]), [240; 4]);
    assert_eq!(image.sample(0, 0, [1, 1]), [135; 4]);
    assert_eq!(image.sample(0, 1, [4, 4]), [50; 4]);
    assert_eq!(image.sample(1, 2, [4, 4]), [164; 4]);
    assert_eq!(image.sample(0, 0, [8, 8]), [0; 4]);
    assert_eq!(image.sample(7, 7, [8, 8]), [240; 4]);
}

#[test]
fn every_pixel_and_alpha_are_compared_including_edges_and_the_channel_seam() {
    let images = [Image::card([48, 32], 0).unwrap(), Image::card([32, 24], 1).unwrap()];
    let extent = [64, 48];
    assert_eq!(expected(&images, extent, 0, 0), [255, 0, 0, 255]);
    assert_eq!(expected(&images, extent, 32, 0), [255, 255, 0, 255]);
    assert_eq!(expected(&images, extent, 31, 47), [0, 255, 0, 64]);
    assert_eq!(expected(&images, extent, 63, 47), [0, 255, 255, 64]);
    let mut actual = (0..48)
        .flat_map(|y| (0..64).map(move |x| (x, y)))
        .map(|(x, y)| expected(&images, extent, x, y))
        .collect::<Vec<_>>();
    assert_eq!(compare(&images, extent, &actual).unwrap().mismatched_pixels, 0);
    for (x, y, component) in [(0, 0, 0), (31, 17, 1), (32, 23, 2), (63, 47, 3)] {
        actual[y * 64 + x][component] ^= 128;
    }
    let result = compare(&images, extent, &actual).unwrap();
    assert_eq!(result.compared_pixels, 64 * 48);
    assert_eq!(result.mismatched_pixels, 4);
    assert_eq!(result.differing_components_bgra, [1; 4]);
    assert!(compare(&images, extent, &actual[..actual.len() - 1]).is_err());
}

#[test]
fn flipped_swapped_and_premultiplied_images_cannot_pass() {
    let images = [Image::card([32, 24], 0).unwrap(), Image::card([32, 24], 1).unwrap()];
    for mode in 0..5 {
        let actual = (0..24)
            .flat_map(|y| (0..64).map(move |x| (x, y)))
            .map(|(x, y)| {
                let source_x = match mode {
                    0 => 63 - x,
                    4 => (x + 32) % 64,
                    _ => x,
                };
                let source_y = if mode == 1 { 23 - y } else { y };
                let mut pixel = expected(&images, [64, 24], source_x, source_y);
                if mode == 2 {
                    pixel.swap(0, 2);
                }
                if mode == 3 {
                    let alpha = u16::from(pixel[3]);
                    for component in pixel.iter_mut().take(3) {
                        *component = (u16::from(*component) * alpha / 255) as u8;
                    }
                }
                pixel
            })
            .collect::<Vec<_>>();
        assert!(compare(&images, [64, 24], &actual).unwrap().mismatched_pixels > 100);
    }
}

#[test]
fn tolerance_is_inclusive_for_each_component_and_larger_errors_are_rejected() {
    let images = [0, 1].map(|_| Image {
        extent: [16, 16],
        pixels: vec![[100; 4]; 16 * 16],
    });
    let actual = vec![[102; 4]; 32 * 16];
    let result = compare(&images, [32, 16], &actual).unwrap();
    assert_eq!(result.tolerance_per_component, 2);
    assert_eq!(result.mismatched_pixels, 0);
    assert_eq!(result.maximum_error_bgra, [2; 4]);
    for component in 0..4 {
        let mut corrupted = actual.clone();
        corrupted.last_mut().unwrap()[component] = 103;
        let result = compare(&images, [32, 16], &corrupted).unwrap();
        assert_eq!(result.mismatched_pixels, 1);
        assert_eq!(result.differing_components_bgra[component], 1);
        assert_eq!(result.differing_components_bgra.iter().sum::<u64>(), 1);
        assert_eq!(result.first_differences[0].position, [31, 15]);
        assert_eq!(result.maximum_error_bgra[component], 3);
    }
}

#[test]
fn mismatch_diagnostics_are_bounded_without_truncating_the_compared_image() {
    let images = [0, 1].map(|_| Image {
        extent: [16, 16],
        pixels: vec![[100; 4]; 16 * 16],
    });
    let actual = [[0; 4]; 32 * 16];
    let result = compare(&images, [32, 16], &actual).unwrap();
    assert_eq!(result.compared_pixels, 512);
    assert_eq!(result.mismatched_pixels, 512);
    assert_eq!(result.differing_components_bgra, [512; 4]);
    assert_eq!(result.first_differences.len(), 16);
    assert_eq!(result.first_differences.last().unwrap().position, [15, 0]);
}

#[test]
fn public_construction_and_comparison_reject_invalid_extents() {
    for extent in [[0, 16], [16, 0], [15, 16], [16, 15], [u32::MAX, u32::MAX]] {
        assert!(Image::card(extent, 0).is_err());
    }
    assert!(Image::card([16, 16], 2).is_err());
    let images = [Image::card([16, 16], 0).unwrap(), Image::card([16, 16], 1).unwrap()];
    assert_eq!(images[0].extent(), [16, 16]);
    assert_eq!(images[0].pixels().len(), 16 * 16);
    for extent in [[0, 16], [1, 16], [31, 16], [32, 0], [u32::MAX - 1, u32::MAX]] {
        assert!(compare(&images, extent, &[]).is_err());
    }
}
