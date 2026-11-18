use super::*;
use crate::{Extent2D, TextureDescriptor};

fn pixel(rgba: [u8; 4]) -> ImageData {
    ImageData::new(
        TextureDescriptor {
            extent: Extent2D::new(1, 1).unwrap(),
            format: PixelFormat::Rgba8Unorm,
            color_encoding: ColorEncoding::Srgb,
            alpha_mode: AlphaMode::Straight,
        },
        rgba.to_vec(),
    )
    .unwrap()
}

#[test]
fn srgba8_crossfade_interpolates_light_instead_of_byte_values() {
    let black = pixel([0, 0, 0, 255]);
    let white = pixel([255, 255, 255, 255]);
    let budget = ImageStorageBudget::new(4).unwrap();
    let middle = crossfade_srgba8(&black, &white, Opacity::new(0.5).unwrap(), &budget).unwrap();
    assert_eq!(middle.descriptor(), black.descriptor());
    assert_eq!(middle.pixels(), &[188, 188, 188, 255]);
    assert_eq!(budget.snapshot().retained_bytes, 4);
    assert!(matches!(
        crossfade_srgba8(&black, &white, Opacity::new(0.5).unwrap(), &budget),
        Err(ImageError::Capacity)
    ));
    drop(middle);
    assert_eq!(budget.snapshot().retained_bytes, 0);
}

#[test]
fn translucent_inputs_mix_premultiplied_color_without_leaking_hidden_rgb() {
    let red = pixel([255, 0, 0, 128]);
    let blue = pixel([0, 0, 255, 255]);
    let hidden_green = pixel([0, 255, 0, 0]);
    let budget = ImageStorageBudget::new(4).unwrap();
    let half = Opacity::new(0.5).unwrap();
    assert_eq!(
        crossfade_srgba8(&red, &blue, half, &budget).unwrap().pixels(),
        &[156, 0, 213, 192]
    );
    assert_eq!(
        crossfade_srgba8(&red, &hidden_green, half, &budget).unwrap().pixels(),
        &[255, 0, 0, 64]
    );
    assert_eq!(
        crossfade_srgba8(&hidden_green, &hidden_green, half, &budget)
            .unwrap()
            .pixels(),
        &[0; 4]
    );
}

#[test]
fn endpoints_preserve_source_bytes_and_descriptor_mismatch_does_not_allocate() {
    let tail = pixel([255, 0, 0, 0]);
    let head = pixel([0, 0, 255, 0]);
    let budget = ImageStorageBudget::new(4).unwrap();
    for (weight, expected) in [(Opacity::TRANSPARENT, &tail), (Opacity::OPAQUE, &head)] {
        let result = crossfade_srgba8(&tail, &head, weight, &budget).unwrap();
        assert_eq!(result.descriptor(), expected.descriptor());
        assert_eq!(result.pixels(), expected.pixels());
    }
    let mut incompatible = head.descriptor();
    incompatible.alpha_mode = AlphaMode::Opaque;
    let incompatible = ImageData::new(incompatible, vec![0; 4]).unwrap();
    assert!(matches!(
        crossfade_srgba8(&tail, &incompatible, Opacity::TRANSPARENT, &budget),
        Err(ImageError::CrossfadeDescriptor)
    ));
    assert_eq!(budget.snapshot().retained_bytes, 0);
}
