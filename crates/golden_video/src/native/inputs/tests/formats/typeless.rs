use super::normalized::Case;
use crate::PixelFormat;
use windows::Win32::Graphics::Dxgi::Common::*;

// Independent producer/default table; do not use the production format/default helper.
const DEFAULTS: &[(DXGI_FORMAT, PixelFormat, DXGI_FORMAT)] = &[
    (
        DXGI_FORMAT_R32G32B32A32_TYPELESS,
        PixelFormat::Rgba32Float,
        DXGI_FORMAT_R32G32B32A32_FLOAT,
    ),
    (
        DXGI_FORMAT_R16G16B16A16_TYPELESS,
        PixelFormat::Rgba16Float,
        DXGI_FORMAT_R16G16B16A16_FLOAT,
    ),
    (
        DXGI_FORMAT_R32G32_TYPELESS,
        PixelFormat::Rg32Float,
        DXGI_FORMAT_R32G32_FLOAT,
    ),
    (
        DXGI_FORMAT_R10G10B10A2_TYPELESS,
        PixelFormat::Rgb10A2Unorm,
        DXGI_FORMAT_R10G10B10A2_UNORM,
    ),
    (
        DXGI_FORMAT_R8G8B8A8_TYPELESS,
        PixelFormat::Rgba8Unorm,
        DXGI_FORMAT_R8G8B8A8_UNORM,
    ),
    (
        DXGI_FORMAT_R16G16_TYPELESS,
        PixelFormat::Rg16Float,
        DXGI_FORMAT_R16G16_FLOAT,
    ),
    (DXGI_FORMAT_R32_TYPELESS, PixelFormat::R32Float, DXGI_FORMAT_R32_FLOAT),
    (DXGI_FORMAT_R8G8_TYPELESS, PixelFormat::Rg8Unorm, DXGI_FORMAT_R8G8_UNORM),
    (DXGI_FORMAT_R16_TYPELESS, PixelFormat::R16Float, DXGI_FORMAT_R16_FLOAT),
    (DXGI_FORMAT_R8_TYPELESS, PixelFormat::R8Unorm, DXGI_FORMAT_R8_UNORM),
    (
        DXGI_FORMAT_B8G8R8A8_TYPELESS,
        PixelFormat::Bgra8Unorm,
        DXGI_FORMAT_B8G8R8A8_UNORM,
    ),
    (
        DXGI_FORMAT_B8G8R8X8_TYPELESS,
        PixelFormat::Bgrx8Unorm,
        DXGI_FORMAT_B8G8R8X8_UNORM,
    ),
];

pub(super) fn default_format(native: DXGI_FORMAT) -> Option<PixelFormat> {
    DEFAULTS.iter().find(|row| row.0 == native).map(|row| row.1)
}
pub(super) fn view_format(native: DXGI_FORMAT) -> Option<DXGI_FORMAT> {
    DEFAULTS.iter().find(|row| row.0 == native).map(|row| row.2)
}

pub(super) fn default_cases() -> Vec<Case> {
    DEFAULTS
        .iter()
        .map(|&(native, format, _)| {
            let color = match format {
                PixelFormat::Rgba32Float | PixelFormat::R32Float => [0.1234567, 1.25, -0.25, 0.5],
                PixelFormat::Rgba16Float => [0.125, 1.25, -0.25, 0.5],
                PixelFormat::Rg32Float | PixelFormat::Rg16Float | PixelFormat::R16Float => [1.25, -0.25, 0.75, 0.5],
                _ => [0.125, 0.5, 0.75, 0.5],
            };
            let bits = match format {
                PixelFormat::Rgba32Float => color.into_iter().flat_map(f32::to_le_bytes).collect(),
                PixelFormat::Rg32Float => color[..2].iter().flat_map(|x| x.to_le_bytes()).collect(),
                PixelFormat::R32Float => color[0].to_le_bytes().to_vec(),
                PixelFormat::Rgba16Float => [0x3000_u16, 0x3d00, 0xb400, 0x3800]
                    .into_iter()
                    .flat_map(u16::to_le_bytes)
                    .collect(),
                PixelFormat::Rg16Float => [0x3d00_u16, 0xb400].into_iter().flat_map(u16::to_le_bytes).collect(),
                PixelFormat::R16Float => 0x3d00_u16.to_le_bytes().to_vec(),
                _ => vec![],
            };
            (format, native, color, bits)
        })
        .collect()
}

pub(super) fn declared_cases() -> Vec<Case> {
    super::integers::wide_cases()
        .into_iter()
        .filter_map(|(format, _, color, bytes)| {
            let native = match format {
                PixelFormat::Rgba32Uint | PixelFormat::Rgba32Sint => DXGI_FORMAT_R32G32B32A32_TYPELESS,
                PixelFormat::Rg32Uint | PixelFormat::Rg32Sint => DXGI_FORMAT_R32G32_TYPELESS,
                _ => return None,
            };
            Some((format, native, color, bytes))
        })
        .collect()
}
