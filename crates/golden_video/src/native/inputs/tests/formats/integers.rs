use super::normalized::Case;
use crate::PixelFormat;
use windows::Win32::Graphics::Dxgi::Common::*;

// Independent native layout table: no production format/view/shader-tag helper is used.
const FORMATS: &[(PixelFormat, DXGI_FORMAT, u32, usize, bool)] = &[
    (PixelFormat::Rgba8Uint, DXGI_FORMAT_R8G8B8A8_UINT, 8, 4, false),
    (PixelFormat::Rgba8Sint, DXGI_FORMAT_R8G8B8A8_SINT, 8, 4, true),
    (PixelFormat::Rgba16Uint, DXGI_FORMAT_R16G16B16A16_UINT, 16, 4, false),
    (PixelFormat::Rgba16Sint, DXGI_FORMAT_R16G16B16A16_SINT, 16, 4, true),
    (PixelFormat::Rgba32Uint, DXGI_FORMAT_R32G32B32A32_UINT, 32, 4, false),
    (PixelFormat::Rgba32Sint, DXGI_FORMAT_R32G32B32A32_SINT, 32, 4, true),
    (PixelFormat::Rg8Uint, DXGI_FORMAT_R8G8_UINT, 8, 2, false),
    (PixelFormat::Rg8Sint, DXGI_FORMAT_R8G8_SINT, 8, 2, true),
    (PixelFormat::Rg16Uint, DXGI_FORMAT_R16G16_UINT, 16, 2, false),
    (PixelFormat::Rg16Sint, DXGI_FORMAT_R16G16_SINT, 16, 2, true),
    (PixelFormat::Rg32Uint, DXGI_FORMAT_R32G32_UINT, 32, 2, false),
    (PixelFormat::Rg32Sint, DXGI_FORMAT_R32G32_SINT, 32, 2, true),
    (PixelFormat::R8Uint, DXGI_FORMAT_R8_UINT, 8, 1, false),
    (PixelFormat::R8Sint, DXGI_FORMAT_R8_SINT, 8, 1, true),
    (PixelFormat::R16Uint, DXGI_FORMAT_R16_UINT, 16, 1, false),
    (PixelFormat::R16Sint, DXGI_FORMAT_R16_SINT, 16, 1, true),
    (PixelFormat::R32Uint, DXGI_FORMAT_R32_UINT, 32, 1, false),
    (PixelFormat::R32Sint, DXGI_FORMAT_R32_SINT, 32, 1, true),
    (PixelFormat::Rgb10A2Uint, DXGI_FORMAT_R10G10B10A2_UINT, 10, 4, false),
];

pub(super) fn layout(format: PixelFormat) -> Option<(u32, usize, bool)> {
    FORMATS
        .iter()
        .find(|row| row.0 == format)
        .map(|row| (row.2, row.3, row.4))
}

pub(super) fn narrow_cases() -> Vec<Case> {
    cases(false)
}
pub(super) fn wide_cases() -> Vec<Case> {
    cases(true)
}

fn cases(wide: bool) -> Vec<Case> {
    let mut result = Vec::new();
    for &(format, native, bits, channels, signed) in FORMATS {
        if (bits == 32) != wide {
            continue;
        }
        let mut pixel = Vec::new();
        if bits == 10 {
            pixel.extend_from_slice(&(123_u32 | (456 << 10) | (789 << 20) | (2 << 30)).to_le_bytes());
        } else {
            let values: [i64; 4] = match (bits, signed) {
                (8, false) => [31, 129, 241, 193],
                (8, true) => [-31, 63, -128, 65],
                (16, false) => [12345, 54321, 65534, 32769],
                (16, true) => [-12345, 23456, -32768, 16385],
                (32, false) => [0x12345679, 0xff800001, 0x7fc00001, 0x80000001],
                (32, true) => [-123456789, 2147483647, -2147483648, 1073741823],
                _ => unreachable!(),
            };
            for value in values.into_iter().take(channels) {
                pixel.extend_from_slice(&value.to_le_bytes()[..bits as usize / 8]);
            }
        }
        result.push((format, native, [0.; 4], pixel));
    }
    if wide {
        for (format, native, values) in [
            (
                PixelFormat::Rgba32Uint,
                DXGI_FORMAT_R32G32B32A32_UINT,
                [0_u32, u32::MAX, 0x80000001, u32::MAX],
            ),
            (
                PixelFormat::Rgba32Sint,
                DXGI_FORMAT_R32G32B32A32_SINT,
                [0x87654321, 123456789, 2147483647, 0x80000000],
            ),
        ] {
            result.push((
                format,
                native,
                [0.; 4],
                values.into_iter().flat_map(u32::to_le_bytes).collect(),
            ));
        }
    }
    result
}

pub(super) fn expected(format: PixelFormat, bytes: &[u8]) -> Option<[f32; 4]> {
    let (bits, channels, signed) = layout(format)?;
    let mut rgba = [0., 0., 0., 1.];
    if bits == 10 {
        let code = u32::from_le_bytes(bytes.try_into().unwrap());
        return Some([
            (code & 1023) as f32 / 1023.,
            ((code >> 10) & 1023) as f32 / 1023.,
            ((code >> 20) & 1023) as f32 / 1023.,
            (code >> 30) as f32 / 3.,
        ]);
    }
    for (i, component) in rgba.iter_mut().enumerate().take(channels) {
        let start = i * bits as usize / 8;
        let raw = &bytes[start..start + bits as usize / 8];
        let value = match (bits, signed) {
            (8, false) => raw[0] as f64 / u8::MAX as f64,
            (8, true) => (raw[0] as i8 as f64 / i8::MAX as f64).max(-1.),
            (16, false) => u16::from_le_bytes(raw.try_into().unwrap()) as f64 / u16::MAX as f64,
            (16, true) => (i16::from_le_bytes(raw.try_into().unwrap()) as f64 / i16::MAX as f64).max(-1.),
            (32, false) => u32::from_le_bytes(raw.try_into().unwrap()) as f64 / u32::MAX as f64,
            (32, true) => (i32::from_le_bytes(raw.try_into().unwrap()) as f64 / i32::MAX as f64).max(-1.),
            _ => unreachable!(),
        };
        *component = if i == 3 {
            value.clamp(0., 1.) as f32
        } else {
            value as f32
        };
    }
    Some(rgba)
}

#[test]
fn integer_reference_preserves_range_implicit_components_and_coverage() {
    assert_eq!(expected(PixelFormat::R8Sint, &[128]), Some([-1., 0., 0., 1.]));
    assert_eq!(
        expected(PixelFormat::Rg16Uint, &[255, 255, 0, 0]),
        Some([1., 0., 0., 1.])
    );
    let codes = [i32::MIN, i32::MAX, 0, i32::MIN]
        .into_iter()
        .flat_map(i32::to_le_bytes)
        .collect::<Vec<_>>();
    assert_eq!(expected(PixelFormat::Rgba32Sint, &codes), Some([-1., 1., 0., 0.]));
}
