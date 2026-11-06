use super::normalized::Case;
use crate::{AlphaMode, ColorEncoding, Extent2D, PixelFormat, TextureDescriptor};
use windows::Win32::Graphics::Dxgi::Common::*;

pub(super) fn cases() -> Vec<Case> {
    vec![
        (
            PixelFormat::Rg32Float,
            DXGI_FORMAT_R32G32_FLOAT,
            [0.1234567, -1.25, 0.75, 0.5],
            [0.1234567_f32, -1.25].into_iter().flat_map(f32::to_le_bytes).collect(),
        ),
        (
            PixelFormat::Bgr565Unorm,
            DXGI_FORMAT_B5G6R5_UNORM,
            [0.125, 0.5, 0.75, 0.5],
            vec![],
        ),
        (
            PixelFormat::Bgra5551Unorm,
            DXGI_FORMAT_B5G5R5A1_UNORM,
            [0.125, 0.5, 0.75, 1.],
            vec![],
        ),
        (
            PixelFormat::Bgra5551Unorm,
            DXGI_FORMAT_B5G5R5A1_UNORM,
            [0.125, 0.5, 0.75, 0.],
            vec![],
        ),
    ]
}

pub(super) fn expected(format: PixelFormat, bytes: &[u8]) -> Option<[f32; 4]> {
    Some(match format {
        PixelFormat::Rg32Float => [
            f32::from_le_bytes(bytes[..4].try_into().unwrap()),
            f32::from_le_bytes(bytes[4..8].try_into().unwrap()),
            0.,
            1.,
        ],
        PixelFormat::Bgr565Unorm => {
            let bits = u16::from_le_bytes(bytes.try_into().unwrap());
            [
                ((bits >> 11) & 31) as f32 / 31.,
                ((bits >> 5) & 63) as f32 / 63.,
                (bits & 31) as f32 / 31.,
                1.,
            ]
        }
        PixelFormat::Bgra5551Unorm => {
            let bits = u16::from_le_bytes(bytes.try_into().unwrap());
            [
                ((bits >> 10) & 31) as f32 / 31.,
                ((bits >> 5) & 31) as f32 / 31.,
                (bits & 31) as f32 / 31.,
                (bits >> 15) as f32,
            ]
        }
        _ => return None,
    })
}

#[test]
fn conversion_admission_counts_raw_alias_capture_and_three_float_bridge_images() {
    for (format, per_pixel) in [
        (PixelFormat::Rg32Float, 64),
        (PixelFormat::Bgr565Unorm, 52),
        (PixelFormat::Bgra5551Unorm, 52),
        (PixelFormat::Bgra4444Unorm, 52),
        (PixelFormat::Rgb10XrA2Unorm, 16),
        (PixelFormat::Rgba32Float, 64),
        (PixelFormat::Rgba8Unorm, 16),
    ] {
        let descriptor = TextureDescriptor {
            extent: Extent2D::new(7, 3).unwrap(),
            format,
            color_encoding: ColorEncoding::Linear,
            alpha_mode: AlphaMode::Straight,
        };
        assert_eq!(
            crate::native::InputLimits::source_storage_bytes(descriptor).unwrap(),
            21 * per_pixel
        );
        assert!(
            crate::native::InputLimits::source_storage_bytes(TextureDescriptor {
                extent: Extent2D::new(u32::MAX, u32::MAX).unwrap(),
                ..descriptor
            })
            .is_err()
        );
    }
}
