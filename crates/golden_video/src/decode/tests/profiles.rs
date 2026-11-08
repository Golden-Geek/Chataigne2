use super::*;
use crate::decode::SoftwareImageConverter;

fn linear(encoded: f64) -> f64 {
    let encoded = encoded.clamp(0.0, 1.0);
    if encoded < 0.081 {
        encoded / 4.5
    } else {
        ((encoded + 0.099) / 1.099).powf(1.0 / 0.45)
    }
}
fn srgb_linear(byte: u8) -> f64 {
    let encoded = f64::from(byte) / 255.0;
    if encoded <= 0.04045 {
        encoded / 12.92
    } else {
        ((encoded + 0.055) / 1.055).powf(2.4)
    }
}

#[test]
fn chromatic_high_main10_and_prores4444_preserve_color_alpha_and_exact_timestamps() {
    for (name, pixel, rate, alpha) in [
        ("h264-color.mp4", ffmpeg::format::Pixel::YUV420P, (24, 1), false),
        (
            "hevc-main10.mp4",
            ffmpeg::format::Pixel::YUV420P10LE,
            (30000, 1001),
            false,
        ),
        ("prores-alpha.mov", ffmpeg::format::Pixel::YUVA444P12LE, (25, 1), true),
    ] {
        let mut decoder = Decoder::open(&fixture(name), limits()).unwrap();
        let info = decoder.info().clone();
        let mut converter = SoftwareImageConverter::default();
        let mut count = 0;
        while let Some(frame) = decoder.next_frame().unwrap() {
            assert!(count < 6);
            assert_eq!(frame.software_image().unwrap().format(), pixel, "{name}");
            assert_eq!(
                i128::from(frame.timestamp()) * i128::from(info.time_base.numerator) * rate.0,
                count as i128 * i128::from(info.time_base.denominator) * rate.1
            );
            let image = converter.convert(&frame).unwrap();
            let expected_alpha = if alpha {
                (count as f64 * 256.0 / 1023.0).min(1.0)
            } else {
                1.0
            };
            let y = (16.0 + count as f64 * 24.0) / 219.0;
            let cb = -32.0 / 224.0;
            let cr = 32.0 / 224.0;
            let expected = [y + 1.5748 * cr, y - 0.187324 * cb - 0.468124 * cr, y + 1.8556 * cb].map(linear);
            for rgba in image.pixels().chunks_exact(4) {
                for component in 0..3 {
                    assert!(
                        (srgb_linear(rgba[component]) - expected[component]).abs() <= 4.0 / 255.0,
                        "{name}, frame{count}, component{component}: {rgba:?}, {expected:?}"
                    );
                }
                assert!(
                    (f64::from(rgba[3]) / 255.0 - expected_alpha).abs() <= 1.0 / 255.0,
                    "{name}, frame{count}: alpha{} expected{expected_alpha}",
                    rgba[3]
                );
            }
            count += 1;
        }
        assert_eq!(count, 6);
    }
}
