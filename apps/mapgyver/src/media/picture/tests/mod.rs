use std::borrow::Cow;
use std::num::NonZeroUsize;
use std::time::{Duration, Instant};

use golden_assets::{
    AssetBinding, AssetId, AssetLocator, AssetPreparer, AssetRecord, AssetScope, PreparationFailure, PreparationLimits,
};
use golden_video::{AlphaMode, ColorEncoding, PixelFormat, TextureDescriptor};
use image::ImageEncoder;

use super::PictureDecoder;

const WAIT: Duration = Duration::from_secs(5);
const BUDGET: usize = 8 * 1024 * 1024;

struct Snapshot {
    descriptor: TextureDescriptor,
    pixels: Vec<u8>,
}

fn prepare(bytes: &[u8], budget: usize) -> Result<Snapshot, PreparationFailure> {
    let project = tempfile::tempdir().unwrap();
    std::fs::write(project.path().join("picture"), bytes).unwrap();
    let scope = AssetScope::new(project.path()).unwrap();
    let binding = AssetBinding::new(AssetRecord {
        id: AssetId::new(),
        locator: AssetLocator::ProjectRelative {
            path: "picture".to_string().try_into().unwrap(),
        },
    });
    let limits = PreparationLimits {
        max_decode_bytes: NonZeroUsize::new(budget).unwrap(),
        max_source_bytes: 1024 * 1024,
        max_prepared_bytes: NonZeroUsize::new(32 * 1024 * 1024).unwrap(),
        ..Default::default()
    };
    let mut worker = AssetPreparer::new(PictureDecoder, limits).unwrap();
    worker.submit(scope.capture(&binding)).unwrap();
    let mut outcomes = Vec::new();
    let deadline = Instant::now() + WAIT;
    while outcomes.is_empty() {
        worker.drain(&mut outcomes, NonZeroUsize::MIN);
        assert!(Instant::now() < deadline, "Picture worker produced no result");
        std::thread::sleep(Duration::from_millis(2));
    }
    let result = outcomes.pop().unwrap().result.map(|prepared| {
        assert!(prepared.is_current());
        Snapshot {
            descriptor: prepared.value().descriptor(),
            pixels: prepared.value().pixels().to_vec(),
        }
    });
    let retirement = worker.retirement();
    drop(worker);
    assert!(retirement.wait(WAIT));
    result
}

fn png(info: png::Info<'_>, pixels: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    let encoder = png::Encoder::with_info(&mut bytes, info).unwrap();
    let mut writer = encoder.write_header().unwrap();
    writer.write_image_data(pixels).unwrap();
    writer.finish().unwrap();
    bytes
}

fn info(width: u32, height: u32, color: png::ColorType) -> png::Info<'static> {
    let mut info = png::Info::with_size(width, height);
    info.color_type = color;
    info.bit_depth = png::BitDepth::Eight;
    info
}

#[test]
fn png_preserves_straight_alpha_and_explicit_srgb_metadata() {
    let mut source = info(2, 1, png::ColorType::Rgba);
    source.srgb = Some(png::SrgbRenderingIntent::Perceptual);
    let pixels = [255, 0, 0, 128, 0, 0, 255, 0];
    let decoded = prepare(&png(source, &pixels), BUDGET).unwrap();
    assert_eq!(decoded.pixels, pixels);
    assert_eq!(
        (decoded.descriptor.extent.width(), decoded.descriptor.extent.height()),
        (2, 1)
    );
    assert_eq!(decoded.descriptor.format, PixelFormat::Rgba8Unorm);
    assert_eq!(decoded.descriptor.color_encoding, ColorEncoding::Srgb);
    assert_eq!(decoded.descriptor.alpha_mode, AlphaMode::Straight);
}

#[test]
fn untagged_grayscale_and_palette_transparency_expand_to_rgba() {
    let decoded = prepare(
        &png(info(2, 1, png::ColorType::GrayscaleAlpha), &[37, 0, 181, 128]),
        BUDGET,
    )
    .unwrap();
    assert_eq!(decoded.pixels, [37, 37, 37, 0, 181, 181, 181, 128]);
    let mut indexed = info(2, 1, png::ColorType::Indexed);
    indexed.palette = Some(Cow::Owned(vec![255, 0, 0, 0, 255, 0]));
    indexed.trns = Some(Cow::Owned(vec![32, 255]));
    let decoded = prepare(&png(indexed, &[0, 1]), BUDGET).unwrap();
    assert_eq!(decoded.pixels, [255, 0, 0, 32, 0, 255, 0, 255]);
}

#[test]
fn jpeg_decodes_to_opaque_srgb_rgba() {
    let pixels = [30, 100, 220].repeat(4);
    let mut encoded = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut encoded, 100)
        .write_image(&pixels, 2, 2, image::ExtendedColorType::Rgb8)
        .unwrap();
    let decoded = prepare(&encoded, BUDGET).unwrap();
    assert_eq!(
        (decoded.descriptor.extent.width(), decoded.descriptor.extent.height()),
        (2, 2)
    );
    for pixel in decoded.pixels.chunks_exact(4) {
        assert_eq!(pixel[3], 255);
        for (actual, reference) in pixel[..3].iter().zip([30_u8, 100, 220]) {
            assert!(
                actual.abs_diff(reference) <= 4,
                "JPEG component {actual} differs from {reference}"
            );
        }
    }
    assert_eq!(decoded.descriptor.color_encoding, ColorEncoding::Srgb);
}

#[test]
fn unsupported_explicit_gamma_chromaticities_and_sixteen_bit_png_fail() {
    let mut linear = info(1, 1, png::ColorType::Rgba);
    linear.source_gamma = Some(png::ScaledFloat::new(1.0));
    assert!(matches!(
        prepare(&png(linear, &[1, 2, 3, 255]), BUDGET),
        Err(PreparationFailure::Decode)
    ));
    let mut wide = info(1, 1, png::ColorType::Rgb);
    wide.source_chromaticities = Some(png::SourceChromaticities::new(
        (0.3127, 0.3290),
        (0.68, 0.32),
        (0.265, 0.69),
        (0.15, 0.06),
    ));
    assert!(matches!(
        prepare(&png(wide, &[1, 2, 3]), BUDGET),
        Err(PreparationFailure::Decode)
    ));
    let mut high_depth = info(1, 1, png::ColorType::Rgba);
    high_depth.bit_depth = png::BitDepth::Sixteen;
    assert!(matches!(
        prepare(&png(high_depth, &[0; 8]), BUDGET),
        Err(PreparationFailure::Decode)
    ));
}

#[test]
fn dimensions_and_working_allocation_are_admitted_before_pixel_decode() {
    let overwide = png(info(16_385, 1, png::ColorType::Rgba), &vec![0; 16_385 * 4]);
    assert!(matches!(prepare(&overwide, BUDGET), Err(PreparationFailure::Decode)));
    let small = png(info(64, 64, png::ColorType::Rgba), &vec![0; 64 * 64 * 4]);
    assert!(matches!(
        prepare(&small, 128 * 1024),
        Err(PreparationFailure::ResourcePressure)
    ));
    let mut jpeg = Vec::new();
    image::codecs::jpeg::JpegEncoder::new(&mut jpeg)
        .write_image(&vec![0; 32 * 32 * 3], 32, 32, image::ExtendedColorType::Rgb8)
        .unwrap();
    assert!(matches!(
        prepare(&jpeg, 128 * 1024),
        Err(PreparationFailure::ResourcePressure)
    ));
}

#[test]
fn malformed_truncated_and_unsupported_formats_fail_without_prepared_pixels() {
    assert!(matches!(
        prepare(b"not an image", BUDGET),
        Err(PreparationFailure::Decode)
    ));
    let valid = png(info(2, 1, png::ColorType::Rgba), &[0; 8]);
    assert!(matches!(
        prepare(&valid[..valid.len() / 2], BUDGET),
        Err(PreparationFailure::Decode)
    ));
    assert!(matches!(
        prepare(b"GIF89a0000000000", BUDGET),
        Err(PreparationFailure::Decode)
    ));
}

#[test]
fn png_exif_orientation_rotates_pixels_and_resolved_extent() {
    let mut source = info(2, 1, png::ColorType::Rgba);
    // Minimal little-endian TIFF IFD0, Orientation=6 (90 degrees clockwise).
    source.exif_metadata = Some(Cow::Owned(vec![
        73, 73, 42, 0, 8, 0, 0, 0, 1, 0, 18, 1, 3, 0, 1, 0, 0, 0, 6, 0, 0, 0, 0, 0, 0, 0,
    ]));
    let decoded = prepare(&png(source, &[255, 0, 0, 255, 0, 0, 255, 255]), BUDGET).unwrap();
    assert_eq!(
        (decoded.descriptor.extent.width(), decoded.descriptor.extent.height()),
        (1, 2)
    );
    assert_eq!(decoded.pixels, [255, 0, 0, 255, 0, 0, 255, 255]);
}
