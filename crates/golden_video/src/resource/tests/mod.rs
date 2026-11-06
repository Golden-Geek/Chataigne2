use crate::{AlphaMode, ColorEncoding, Extent2D, PixelFormat, TextureDescriptor, TextureError};

const FORMATS: [PixelFormat; 5] = [
    PixelFormat::Rgba8Unorm,
    PixelFormat::Bgra8Unorm,
    PixelFormat::Bgrx8Unorm,
    PixelFormat::Rgba8Snorm,
    PixelFormat::Rgb10A2Unorm,
];

#[test]
fn normalized_sixteen_bit_storage_is_not_charged_as_byte_storage() {
    for format in [PixelFormat::Rgba16Unorm, PixelFormat::Rgba16Snorm] {
        assert_eq!(descriptor(3840, 2160, format).byte_len(), Ok(66_355_200));
        assert!(matches!(
            descriptor(u32::MAX, (1 << 29) + 1, format).byte_len(),
            Err(TextureError::ByteSizeOverflow { .. })
        ));
    }
}

#[test]
fn floating_compositing_storage_is_charged_at_eight_bytes_without_overflow() {
    let format = PixelFormat::Rgba16Float;
    assert_eq!(descriptor(1, 1, format).byte_len(), Ok(8));
    assert_eq!(descriptor(1920, 1080, format).byte_len(), Ok(16_588_800));
    assert_eq!(descriptor(3840, 2160, format).byte_len(), Ok(66_355_200));
    let largest_height = 1_u32 << 29;
    assert_eq!(
        descriptor(u32::MAX, largest_height, format).byte_len(),
        Ok(u64::MAX - u64::from(u32::MAX))
    );
    assert!(matches!(
        descriptor(u32::MAX, largest_height + 1, format).byte_len(),
        Err(TextureError::ByteSizeOverflow { .. })
    ));
}

#[test]
fn binary32_storage_preserves_its_sixteen_byte_charge_and_checked_overflow() {
    let format = PixelFormat::Rgba32Float;
    assert_eq!(descriptor(1, 1, format).byte_len(), Ok(16));
    assert_eq!(descriptor(3840, 2160, format).byte_len(), Ok(132_710_400));
    assert!(descriptor(u32::MAX, 1 << 28, format).byte_len().is_ok());
    assert!(matches!(
        descriptor(u32::MAX, (1 << 28) + 1, format).byte_len(),
        Err(TextureError::ByteSizeOverflow { .. })
    ));
}

fn descriptor(width: u32, height: u32, format: PixelFormat) -> TextureDescriptor {
    TextureDescriptor {
        extent: Extent2D::new(width, height).unwrap(),
        format,
        color_encoding: ColorEncoding::Linear,
        alpha_mode: AlphaMode::Premultiplied,
    }
}

#[test]
fn unresolved_or_empty_sources_cannot_be_represented_as_texture_extents() {
    for (width, height) in [(0, 0), (0, 1), (1, 0), (0, u32::MAX), (u32::MAX, 0)] {
        assert_eq!(
            Extent2D::new(width, height),
            Err(TextureError::EmptyExtent { width, height })
        );
    }
}

#[test]
fn nonempty_extents_preserve_texel_dimensions_without_an_implicit_device_limit() {
    for (width, height) in [(1, 1), (1_920, 1_080), (3_840, 2_160), (u32::MAX, u32::MAX)] {
        let extent = Extent2D::new(width, height).unwrap();
        assert_eq!(extent.width(), width);
        assert_eq!(extent.height(), height);
    }
}

#[test]
fn supported_channel_orders_have_exact_1080p_and_4k_logical_storage() {
    for format in FORMATS {
        assert_eq!(descriptor(1_920, 1_080, format).byte_len(), Ok(8_294_400));
        assert_eq!(descriptor(3_840, 2_160, format).byte_len(), Ok(33_177_600));
        assert_eq!(descriptor(1, 1, format).byte_len(), Ok(4));
    }
}

#[test]
fn admission_size_does_not_wrap_at_the_representable_boundary() {
    // Four bytes per texel makes this the last representable height at maximum width.
    let largest_height = 1_u32 << 30;
    for format in FORMATS {
        assert_eq!(
            descriptor(u32::MAX, largest_height, format).byte_len(),
            Ok(u64::MAX - u64::from(u32::MAX))
        );
        for height in [largest_height + 1, u32::MAX] {
            assert_eq!(
                descriptor(u32::MAX, height, format).byte_len(),
                Err(TextureError::ByteSizeOverflow {
                    width: u32::MAX,
                    height,
                })
            );
        }
    }
}

#[test]
fn channel_order_does_not_choose_source_encoding_or_alpha_convention() {
    let source_conventions = [
        (ColorEncoding::Linear, AlphaMode::Straight),
        (ColorEncoding::Linear, AlphaMode::Opaque),
        (ColorEncoding::Srgb, AlphaMode::Straight),
        (ColorEncoding::Srgb, AlphaMode::Premultiplied),
        (ColorEncoding::Srgb, AlphaMode::Opaque),
    ];
    for format in FORMATS {
        assert!(descriptor(3_840, 2_160, format).is_compositing_target());
        for (color_encoding, alpha_mode) in source_conventions {
            let source = TextureDescriptor {
                color_encoding,
                alpha_mode,
                ..descriptor(3_840, 2_160, format)
            };
            assert!(
                !source.is_compositing_target(),
                "{format:?} must not silently normalize {color_encoding:?}/{alpha_mode:?}"
            );
            assert_eq!(source.byte_len(), Ok(33_177_600));
        }
    }
}
