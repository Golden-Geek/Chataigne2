use super::*;
use crate::{AlphaMode, ColorEncoding, Extent2D, PixelFormat};

#[test]
fn native_video_admission_includes_raw_images_decoded_slots_and_uniform() {
    let descriptor = TextureDescriptor {
        extent: Extent2D::new(2, 2).unwrap(),
        format: PixelFormat::Rgba32Float,
        color_encoding: ColorEncoding::Linear,
        alpha_mode: AlphaMode::Straight,
    };
    for (format, raw, total) in [
        (Format::Ayuv, 16, 240),
        (Format::Y410, 16, 240),
        (Format::Y416, 32, 272),
        (Format::Nv12, 6, 220),
        (Format::P010, 12, 232),
        (Format::P016, 12, 232),
        (Format::Yuy2, 8, 224),
        (Format::Y210, 16, 240),
        (Format::Y216, 16, 240),
    ] {
        assert_eq!(format.image_bytes(descriptor).unwrap(), raw);
        assert_eq!(format.source_bytes(descriptor).unwrap(), total);
        assert_eq!(
            crate::native::InputLimits::source_storage_bytes_for_native(descriptor, format.native()).unwrap(),
            total
        );
        assert!(
            format
                .source_bytes(TextureDescriptor {
                    extent: Extent2D::new(u32::MAX, u32::MAX).unwrap(),
                    ..descriptor
                })
                .is_err()
        );
    }
}
