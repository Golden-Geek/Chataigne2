use super::normalized::Case;
use crate::PixelFormat;
use windows::Win32::Graphics::Dxgi::Common::*;

pub(super) fn cases() -> Vec<Case> {
    vec![
        (
            PixelFormat::Bgra4444Unorm,
            DXGI_FORMAT_B4G4R4A4_UNORM,
            [0.125, 0.5, 0.75, 0.5],
            vec![],
        ),
        (
            PixelFormat::Rgb10XrA2Unorm,
            DXGI_FORMAT_R10G10B10_XR_BIAS_A2_UNORM,
            [129. / 1023., 639. / 1023., 1., 2. / 3.],
            (129_u32 | (639 << 10) | (1023 << 20) | (2 << 30))
                .to_le_bytes()
                .to_vec(),
        ),
        (
            PixelFormat::Rgb10XrA2Unorm,
            DXGI_FORMAT_R10G10B10_XR_BIAS_A2_UNORM,
            [384. / 1023., 894. / 1023., 0., 1.],
            (384_u32 | (894 << 10) | (3 << 30)).to_le_bytes().to_vec(),
        ),
    ]
}

pub(super) fn expected(format: PixelFormat, bytes: &[u8]) -> Option<[f32; 4]> {
    Some(match format {
        PixelFormat::Bgra4444Unorm => {
            let bits = u16::from_le_bytes(bytes.try_into().unwrap());
            [
                ((bits >> 8) & 15) as f32 / 15.,
                ((bits >> 4) & 15) as f32 / 15.,
                (bits & 15) as f32 / 15.,
                (bits >> 12) as f32 / 15.,
            ]
        }
        PixelFormat::Rgb10XrA2Unorm => {
            let bits = u32::from_le_bytes(bytes.try_into().unwrap());
            let channel = |shift: u32| (((bits >> shift) & 1023_u32) as i32 - 384) as f32 / 510.;
            [channel(0), channel(10), channel(20), (bits >> 30) as f32 / 3.]
        }
        _ => return None,
    })
}

#[test]
fn xr_reference_matches_documented_bias_points_and_leaves_alpha_unbiased() {
    for (code, expected_value) in [
        (0_u32, -384. / 510.),
        (129, -0.5),
        (384, 0.),
        (639, 0.5),
        (894, 1.),
        (1023, 639. / 510.),
    ] {
        let bits = code | (code << 10) | (code << 20) | (2 << 30);
        let value = expected(PixelFormat::Rgb10XrA2Unorm, &bits.to_le_bytes()).unwrap();
        assert_eq!(value, [expected_value, expected_value, expected_value, 2. / 3.]);
    }
}
