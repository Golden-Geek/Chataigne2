use super::*;
use crate::image::ImageStream;
use std::sync::Arc;

fn scratch_bytes(converter: &SoftwareImageConverter) -> u64 {
    let output = &converter.conversion.as_ref().unwrap().output;
    // SAFETY: immutable allocation metadata on the owned, populated scratch frame.
    unsafe { (*(*output.as_ptr()).buf[0]).size as u64 }
}
fn measured_storage(width: u32, height: u32) -> (u64, u64) {
    let mut converter = SoftwareImageConverter::new(128 * 1024 * 1024).unwrap();
    let image = converter
        .convert(&input(ffmpeg::format::Pixel::RGBA, width, height, 1))
        .unwrap();
    let bytes = (scratch_bytes(&converter), image.pixels().len() as u64);
    assert_eq!(converter.storage_budget().snapshot().retained_bytes, bytes.0 + bytes.1);
    bytes
}

#[test]
fn retained_stream_images_keep_storage_charged_through_external_retain_and_converter_drop() {
    let (scratch, tight) = measured_storage(65, 4);
    let mut converter = SoftwareImageConverter::new(scratch + 2 * tight).unwrap();
    let budget = converter.storage_budget();
    let first = converter
        .convert(&input(ffmpeg::format::Pixel::RGBA, 65, 4, 1))
        .unwrap();
    let stream = ImageStream::new(first.descriptor());
    let source_owner = Arc::new(());
    stream.publish(Some(first.retain(source_owner.clone()))).unwrap();
    let retained = stream.sample().unwrap().image.unwrap();
    stream.publish(None).unwrap();
    let second = converter
        .convert(&input(ffmpeg::format::Pixel::RGBA, 65, 4, 2))
        .unwrap();
    let full = budget.snapshot();
    assert_eq!(full.retained_bytes, full.limit_bytes);
    assert_eq!(full.allocations, 3);
    assert!(matches!(
        converter.convert(&input(ffmpeg::format::Pixel::RGBA, 65, 4, 3)),
        Err(ConversionError::Capacity)
    ));
    assert_eq!(
        budget.snapshot(),
        full,
        "rejected output cannot allocate or uncharge old readers"
    );
    check(&retained, 65, 4, 1);
    drop(second);
    let replacement = converter
        .convert(&input(ffmpeg::format::Pixel::RGBA, 65, 4, 3))
        .unwrap();
    check(&replacement, 65, 4, 3);
    assert_eq!(budget.snapshot().retained_bytes, full.limit_bytes);
    drop(converter);
    assert_eq!(budget.snapshot().retained_bytes, 2 * tight);
    check(&retained, 65, 4, 1);
    drop(retained);
    assert_eq!(Arc::strong_count(&source_owner), 1);
    assert_eq!(budget.snapshot().retained_bytes, tight);
    drop(replacement);
    assert_eq!(budget.snapshot().retained_bytes, 0);
    assert_eq!(budget.snapshot().allocations, 0);
}

#[test]
fn capacity_failure_before_scratch_is_atomic_and_a_smaller_layout_can_recover() {
    let (scratch, tight) = measured_storage(65, 4);
    let frame = input(ffmpeg::format::Pixel::RGBA, 65, 4, 7);
    assert!(matches!(SoftwareImageConverter::new(0), Err(ConversionError::Capacity)));
    let mut no_image = SoftwareImageConverter::new(tight - 1).unwrap();
    assert!(matches!(no_image.convert(&frame), Err(ConversionError::Capacity)));
    assert_eq!(no_image.storage_budget().snapshot().peak_bytes, 0);
    let mut no_scratch = SoftwareImageConverter::new(scratch + tight - 1).unwrap();
    let budget = no_scratch.storage_budget();
    assert!(matches!(no_scratch.convert(&frame), Err(ConversionError::Capacity)));
    assert!(no_scratch.scratch_layout().is_none());
    assert_eq!(budget.snapshot().retained_bytes, 0);
    assert_eq!(budget.snapshot().allocations, 0);
    let recovered = no_scratch
        .convert(&input(ffmpeg::format::Pixel::RGBA, 1, 1, 9))
        .unwrap();
    check(&recovered, 1, 1, 9);
    drop(no_scratch);
    assert_eq!(budget.snapshot().retained_bytes, 4);
    drop(recovered);
    assert_eq!(budget.snapshot().retained_bytes, 0);
}

#[test]
fn layout_replacement_releases_obsolete_scratch_before_admission_without_changing_old_pixels() {
    let (scratch, tight) = measured_storage(65, 4);
    let mut converter = SoftwareImageConverter::new(scratch + tight).unwrap();
    let budget = converter.storage_budget();
    let first = converter
        .convert(&input(ffmpeg::format::Pixel::RGBA, 65, 4, 3))
        .unwrap();
    let replacement = converter
        .convert(&input(ffmpeg::format::Pixel::BGRA, 33, 3, 7))
        .unwrap();
    check(&first, 65, 4, 3);
    check(&replacement, 33, 3, 7);
    assert_eq!(
        budget.snapshot().retained_bytes,
        tight + replacement.pixels().len() as u64 + scratch_bytes(&converter)
    );
    assert!(budget.snapshot().retained_bytes <= budget.snapshot().limit_bytes);
    drop(converter);
    assert_eq!(
        budget.snapshot().retained_bytes,
        first.pixels().len() as u64 + replacement.pixels().len() as u64
    );
    drop(first);
    drop(replacement);
    assert_eq!(budget.snapshot().retained_bytes, 0);
}
