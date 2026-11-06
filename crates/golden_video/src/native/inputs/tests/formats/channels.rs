use super::normalized::Case;
use crate::PixelFormat;
use windows::Win32::Graphics::Dxgi::Common::*;

pub(super) fn cases() -> Vec<Case> {
    vec![
        (
            PixelFormat::R8Unorm,
            DXGI_FORMAT_R8_UNORM,
            [0.125, 0.5, 0.75, 0.5],
            vec![],
        ),
        (
            PixelFormat::R8Snorm,
            DXGI_FORMAT_R8_SNORM,
            [-0.25, 0.5, 0.75, 0.5],
            vec![],
        ),
        (
            PixelFormat::Rg8Unorm,
            DXGI_FORMAT_R8G8_UNORM,
            [0.125, 0.5, 0.75, 0.5],
            vec![],
        ),
        (
            PixelFormat::Rg8Snorm,
            DXGI_FORMAT_R8G8_SNORM,
            [-0.25, 0.5, 0.75, 0.5],
            vec![],
        ),
        (
            PixelFormat::R16Unorm,
            DXGI_FORMAT_R16_UNORM,
            [0.125, 0.5, 0.75, 0.5],
            vec![],
        ),
        (
            PixelFormat::R16Snorm,
            DXGI_FORMAT_R16_SNORM,
            [-0.25, 0.5, 0.75, 0.5],
            vec![],
        ),
        (
            PixelFormat::Rg16Unorm,
            DXGI_FORMAT_R16G16_UNORM,
            [0.125, 0.5, 0.75, 0.5],
            vec![],
        ),
        (
            PixelFormat::Rg16Snorm,
            DXGI_FORMAT_R16G16_SNORM,
            [-0.25, 0.5, 0.75, 0.5],
            vec![],
        ),
        (
            PixelFormat::R16Float,
            DXGI_FORMAT_R16_FLOAT,
            [1.25, -0.25, 0.75, 0.5],
            0x3d00_u16.to_le_bytes().to_vec(),
        ),
        (
            PixelFormat::Rg16Float,
            DXGI_FORMAT_R16G16_FLOAT,
            [1.25, -0.25, 0.75, 0.5],
            [0x3d00_u16, 0xb400].into_iter().flat_map(u16::to_le_bytes).collect(),
        ),
        (
            PixelFormat::R32Float,
            DXGI_FORMAT_R32_FLOAT,
            [0.1234567, 0.5, 0.75, 0.5],
            0.1234567_f32.to_le_bytes().to_vec(),
        ),
        (
            PixelFormat::Rg11B10Float,
            DXGI_FORMAT_R11G11B10_FLOAT,
            [0.125, 1.25, 0.75, 0.5],
            (0x300_u32 | (0x3d0 << 11) | (0x1d0 << 22)).to_le_bytes().to_vec(),
        ),
        (
            PixelFormat::A8Unorm,
            DXGI_FORMAT_A8_UNORM,
            [0.125, 0.5, 0.75, 0.5],
            vec![],
        ),
    ]
}

pub(super) fn opaque(format: PixelFormat) -> bool {
    matches!(
        format,
        PixelFormat::R8Unorm
            | PixelFormat::R8Snorm
            | PixelFormat::Rg8Unorm
            | PixelFormat::Rg8Snorm
            | PixelFormat::R16Unorm
            | PixelFormat::R16Snorm
            | PixelFormat::Rg16Unorm
            | PixelFormat::Rg16Snorm
            | PixelFormat::R16Float
            | PixelFormat::Rg16Float
            | PixelFormat::R32Float
            | PixelFormat::Rg11B10Float
    )
}

/// Independent storage decoding and D3D's implicit (zero color, one alpha) components.
pub(super) fn expected(format: PixelFormat, bytes: &[u8], color: [f32; 4]) -> Option<[f32; 4]> {
    Some(match format {
        PixelFormat::R8Unorm => [bytes[0] as f32 / 255., 0., 0., 1.],
        PixelFormat::R8Snorm => [(bytes[0] as i8 as f32 / 127.).max(-1.), 0., 0., 1.],
        PixelFormat::Rg8Unorm => [bytes[0] as f32 / 255., bytes[1] as f32 / 255., 0., 1.],
        PixelFormat::Rg8Snorm => [
            (bytes[0] as i8 as f32 / 127.).max(-1.),
            (bytes[1] as i8 as f32 / 127.).max(-1.),
            0.,
            1.,
        ],
        PixelFormat::R16Unorm => [unorm16(bytes), 0., 0., 1.],
        PixelFormat::Rg16Unorm => [unorm16(bytes), unorm16(&bytes[2..]), 0., 1.],
        PixelFormat::R16Snorm => [snorm16(bytes), 0., 0., 1.],
        PixelFormat::Rg16Snorm => [snorm16(bytes), snorm16(&bytes[2..]), 0., 1.],
        // Chosen half values are exactly representable; separate literal bit assertions
        // authenticate the independent producer before shader comparison.
        PixelFormat::R16Float | PixelFormat::R32Float => [color[0], 0., 0., 1.],
        PixelFormat::Rg16Float => [color[0], color[1], 0., 1.],
        PixelFormat::Rg11B10Float => {
            let value = u32::from_le_bytes(bytes.try_into().unwrap());
            [
                unsigned_float(value & 2047, 6),
                unsigned_float((value >> 11) & 2047, 6),
                unsigned_float(value >> 22, 5),
                1.,
            ]
        }
        PixelFormat::A8Unorm => [0., 0., 0., bytes[0] as f32 / 255.],
        _ => return None,
    })
}

fn unorm16(bytes: &[u8]) -> f32 {
    u16::from_le_bytes(bytes[..2].try_into().unwrap()) as f32 / 65535.
}
fn snorm16(bytes: &[u8]) -> f32 {
    (i16::from_le_bytes(bytes[..2].try_into().unwrap()) as f32 / 32767.).max(-1.)
}
fn unsigned_float(value: u32, fraction_bits: u32) -> f32 {
    let exponent = value >> fraction_bits;
    let fraction = (value & ((1 << fraction_bits) - 1)) as f32 / (1 << fraction_bits) as f32;
    assert!(exponent < 31, "finite source pattern");
    if exponent == 0 {
        fraction * 2_f32.powi(-14)
    } else {
        (1. + fraction) * 2_f32.powi(exponent as i32 - 15)
    }
}
