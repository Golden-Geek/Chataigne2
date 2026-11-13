use super::*;
use crate::decode::{DecodedFrame, FrameStorage, SoftwareImageConverter};

#[test]
fn actual_odd_subsampled_ten_bit_alpha_and_packed_planes_report_row_padding() {
    use ffmpeg::format::Pixel;
    for (format, row_bytes, rows) in [
        (Pixel::YUV420P, vec![65, 33, 33], vec![5, 3, 3]),
        (Pixel::YUV420P10LE, vec![130, 66, 66], vec![5, 3, 3]),
        (Pixel::YUVA444P12LE, vec![130; 4], vec![5; 4]),
        (Pixel::RGBA, vec![260], vec![5]),
    ] {
        let image = ffmpeg::frame::Video::new(format, 65, 5);
        let layout = SoftwareFrameLayout::inspect(&image).unwrap();
        assert_eq!((layout.width, layout.height), (65, 5));
        assert_eq!(layout.planes.iter().flatten().count(), row_bytes.len());
        let mut total = 0;
        for (index, plane) in layout.planes.iter().flatten().enumerate() {
            assert_eq!(plane.row_bytes, row_bytes[index]);
            assert_eq!(plane.rows, rows[index]);
            assert_eq!(plane.stride as usize, image.stride(index));
            assert!(plane.stride as u32 > plane.row_bytes, "exercise actual padding");
            assert_eq!(plane.span_bytes, image.data(index).len() as u64);
            total += plane.span_bytes;
        }
        assert_eq!(layout.span_bytes, total);
    }
}

#[test]
fn bottom_up_stride_is_signed_metadata_without_constructing_a_pixel_slice() {
    let mut image = ffmpeg::frame::Video::new(ffmpeg::format::Pixel::RGBA, 5, 3);
    let original = SoftwareFrameLayout::inspect(&image).unwrap();
    let stride = image.stride(0);
    // SAFETY: uniquely owned allocated frame; move the origin to its final row and reverse
    // row traversal. AVBufferRef still owns the original allocation. No negative-stride
    // data slice is created by either this test or the layout inspector.
    unsafe {
        let frame = &mut *image.as_mut_ptr();
        frame.data[0] = frame.data[0].add(stride * 2);
        frame.linesize[0] = -frame.linesize[0];
    }
    let reversed = SoftwareFrameLayout::inspect(&image).unwrap();
    assert_eq!(reversed.span_bytes, original.span_bytes);
    assert_eq!(reversed.planes[0].unwrap().stride, -original.planes[0].unwrap().stride);
    assert_eq!(reversed.planes[0].unwrap().row_bytes, 20);
}

#[test]
fn empty_hardware_palette_and_invalid_stride_layouts_are_unknown_without_readback() {
    let mut empty = ffmpeg::frame::Video::empty();
    assert!(SoftwareFrameLayout::inspect(&empty).is_none());
    empty.set_format(ffmpeg::format::Pixel::D3D11);
    assert!(SoftwareFrameLayout::inspect(&empty).is_none());
    let palette = ffmpeg::frame::Video::new(ffmpeg::format::Pixel::PAL8, 5, 3);
    assert!(SoftwareFrameLayout::inspect(&palette).is_none());
    let mut image = ffmpeg::frame::Video::new(ffmpeg::format::Pixel::RGBA, 5, 3);
    // SAFETY: only test metadata changes; AVBufferRef remains the actual storage owner.
    unsafe { (*image.as_mut_ptr()).linesize[0] = 1 };
    assert!(SoftwareFrameLayout::inspect(&image).is_none());
}

#[test]
fn decoded_and_converter_layouts_are_explicit_and_tight_published_bytes_stay_separate() {
    let mut image = ffmpeg::frame::Video::new(ffmpeg::format::Pixel::RGBA, 65, 4);
    image.data_mut(0).fill(127);
    let frame = DecodedFrame {
        timestamp: 0,
        storage: FrameStorage::Software(image),
    };
    let input = frame.software_layout().unwrap();
    let mut converter = SoftwareImageConverter::default();
    assert!(converter.scratch_layout().is_none());
    let published = converter.convert(&frame).unwrap();
    let scratch = converter.scratch_layout().unwrap();
    assert_eq!(scratch.format, "rgba");
    assert_eq!((scratch.width, scratch.height), (65, 4));
    assert_eq!(input.planes[0].unwrap().row_bytes, 260);
    assert!(scratch.span_bytes > published.pixels().len() as u64);
    assert_eq!(published.pixels().len(), 65 * 4 * 4);
    drop(converter);
    drop(frame);
    assert_eq!(published.pixels().len(), 65 * 4 * 4);
}
