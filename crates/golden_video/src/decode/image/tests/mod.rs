use super::*;
use crate::decode::FrameStorage;

fn input(format: ffmpeg::format::Pixel, width: u32, height: u32, value: u8) -> DecodedFrame {
    let mut image = ffmpeg::frame::Video::new(format, width, height);
    let stride = image.stride(0);
    image.data_mut(0).fill(0xed);
    for y in 0..height as usize {
        for x in 0..width as usize {
            let offset = y * stride + x * 4;
            let rgba = [value, x as u8, y as u8, value.wrapping_mul(37)];
            let bytes = if format == ffmpeg::format::Pixel::BGRA {
                [rgba[2], rgba[1], rgba[0], rgba[3]]
            } else {
                rgba
            };
            image.data_mut(0)[offset..offset + 4].copy_from_slice(&bytes);
        }
    }
    // SAFETY: uniquely owned allocated software frame; no data pointers are changed.
    unsafe {
        (*image.as_mut_ptr()).colorspace = Space::RGB.into();
        (*image.as_mut_ptr()).color_range = Range::JPEG.into();
        (*image.as_mut_ptr()).color_trc = Transfer::IEC61966_2_1.into();
    }
    DecodedFrame {
        timestamp: 0,
        storage: FrameStorage::Software(image),
    }
}

fn check(image: &ImageData, width: u32, height: u32, value: u8) {
    assert_eq!(image.descriptor().extent, Extent2D::new(width, height).unwrap());
    assert_eq!(image.pixels().len(), (width * height * 4) as usize);
    for y in 0..height as usize {
        for x in 0..width as usize {
            let offset = (y * width as usize + x) * 4;
            assert_eq!(
                &image.pixels()[offset..offset + 4],
                &[value, x as u8, y as u8, value.wrapping_mul(37)]
            );
        }
    }
}

#[test]
fn scratch_reuses_actual_storage_and_retained_images_survive_format_and_extent_changes() {
    use ffmpeg::format::Pixel;
    let mut converter = SoftwareImageConverter::default();
    let mut retained = Vec::new();
    for (format, width, height) in [
        (Pixel::RGBA, 65, 4),
        (Pixel::BGRA, 65, 4),
        (Pixel::BGRA, 33, 3),
        (Pixel::RGBA, 65, 4),
    ] {
        let mut storage = None;
        for value in 1..=6 {
            let image = converter.convert(&input(format, width, height, value)).unwrap();
            check(&image, width, height, value);
            let scratch = &converter.conversion.as_ref().unwrap().output;
            assert!(scratch.stride(0) > width as usize * 4, "exercise FFmpeg padding");
            let current = scratch.data(0).as_ptr();
            if let Some(previous) = storage {
                assert_eq!(
                    current, previous,
                    "same contract must keep the actual scratch allocation"
                );
            }
            storage = Some(current);
            retained.push((image, width, height, value));
        }
    }
    drop(converter);
    for (image, width, height, value) in retained {
        check(&image, width, height, value);
    }
}
