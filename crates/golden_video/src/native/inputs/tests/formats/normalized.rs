use crate::PixelFormat;
use windows::Win32::Graphics::Dxgi::Common::*;

type Case = (PixelFormat, DXGI_FORMAT, [f32; 4], Vec<u8>);

pub(super) fn cases() -> Vec<Case> {
    vec![
        (
            PixelFormat::Rgba16Unorm,
            DXGI_FORMAT_R16G16B16A16_UNORM,
            [0.125, 0.5, 0.75, 0.5],
            vec![],
        ),
        (
            PixelFormat::Rgba16Snorm,
            DXGI_FORMAT_R16G16B16A16_SNORM,
            [-0.25, 0.5, 0.75, 0.5],
            vec![],
        ),
        (
            PixelFormat::Rgba8Snorm,
            DXGI_FORMAT_R8G8B8A8_SNORM,
            [-0.25, 0.5, 0.75, 0.5],
            vec![],
        ),
        (
            PixelFormat::Rgb10A2Unorm,
            DXGI_FORMAT_R10G10B10A2_UNORM,
            [0.125, 0.5, 0.75, 0.5],
            vec![],
        ),
        (
            PixelFormat::Bgrx8Unorm,
            DXGI_FORMAT_B8G8R8X8_UNORM,
            [0.125, 0.5, 0.75, 0.125],
            vec![],
        ),
    ]
}

/// Decode the independent producer's native bytes, without using Golden format/view mappings.
/// UNORM/SNORM Clear rounding may differ by device; transport must preserve the resulting bits.
pub(super) fn expected(format: PixelFormat, bytes: &[u8], color: [f32; 4]) -> [f32; 4] {
    let mut rgba: [f32; 4] = match format {
        PixelFormat::Rgba16Float | PixelFormat::Rgba32Float => color,
        PixelFormat::Rgba16Unorm => {
            std::array::from_fn(|i| u16::from_le_bytes(bytes[i * 2..i * 2 + 2].try_into().unwrap()) as f32 / 65535.)
        }
        PixelFormat::Rgba16Snorm => std::array::from_fn(|i| {
            (i16::from_le_bytes(bytes[i * 2..i * 2 + 2].try_into().unwrap()) as f32 / 32767.).max(-1.)
        }),
        PixelFormat::Rgba8Snorm => std::array::from_fn(|i| (bytes[i] as i8 as f32 / 127.).max(-1.)),
        PixelFormat::Rgb10A2Unorm => {
            let packed = u32::from_le_bytes(bytes.try_into().unwrap());
            [
                (packed & 1023) as f32 / 1023.,
                ((packed >> 10) & 1023) as f32 / 1023.,
                ((packed >> 20) & 1023) as f32 / 1023.,
                (packed >> 30) as f32 / 3.,
            ]
        }
        PixelFormat::Bgrx8Unorm => {
            let decode = |encoded: u8| {
                let value = encoded as f32 / 255.;
                if value <= 0.04045 {
                    value / 12.92
                } else {
                    ((value + 0.055) / 1.055).powf(2.4)
                }
            };
            [decode(bytes[2]), decode(bytes[1]), decode(bytes[0]), 1.]
        }
        _ => panic!("unexpected conformance format"),
    };
    assert!(
        rgba[..3].iter().any(|value| value.abs() > 0.01),
        "a black oracle is invalid"
    );
    for i in 0..3 {
        rgba[i] *= rgba[3];
    }
    rgba
}
