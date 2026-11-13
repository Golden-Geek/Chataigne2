//! Deterministic BC blocks, encoded by the reference codec and muxed by pinned libavformat.
use super::*;
use crate::decode::{ConversionError, DecodeLimits, Decoder, SoftwareImageConverter, ffmpeg};
use std::path::{Path, PathBuf};

struct Case {
    name: &'static str,
    format: HapFormat,
    tag: [u8; 4],
    extent: (u32, u32),
    rate: (i32, i32),
}
fn cases() -> [Case; 3] {
    [
        Case {
            name: "hap.mov",
            format: HapFormat::Bc1Rgb,
            tag: *b"Hap1",
            extent: (64, 48),
            rate: (24, 1),
        },
        Case {
            name: "hap-alpha.mov",
            format: HapFormat::Bc3Rgba,
            tag: *b"Hap5",
            extent: (17, 9),
            rate: (25, 1),
        },
        Case {
            name: "hap-q.mov",
            format: HapFormat::Bc3YCoCg,
            tag: *b"HapY",
            extent: (1920, 1080),
            rate: (30000, 1001),
        },
    ]
}
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src/decode/tests/fixtures")
}
fn blocks(format: HapFormat, extent: Extent2D, index: u8) -> Vec<u8> {
    let mut block = Vec::new();
    if format != HapFormat::Bc1Rgb {
        let alpha = if format == HapFormat::Bc3YCoCg {
            32 + index * 24
        } else {
            index * 51
        };
        block.extend_from_slice(&[alpha, alpha, 0, 0, 0, 0, 0, 0]);
    }
    let color = if format == HapFormat::Bc3YCoCg {
        (16u16 << 11) | (32u16 << 5)
    } else {
        (u16::from(4 + index * 3) << 11) | (u16::from(8 + index * 6) << 5) | u16::from(4 + index * 3)
    };
    block.extend_from_slice(&color.to_le_bytes());
    block.extend_from_slice(&color.to_le_bytes());
    block.extend_from_slice(&[0; 4]);
    block.repeat(format.byte_len(extent).unwrap() / block.len())
}
fn reference_encode(format: HapFormat, input: &[u8]) -> Vec<u8> {
    let native = match format {
        HapFormat::Bc1Rgb => reference::HapTextureFormat_HapTextureFormat_RGB_DXT1,
        HapFormat::Bc3Rgba => reference::HapTextureFormat_HapTextureFormat_RGBA_DXT5,
        HapFormat::Bc3YCoCg => reference::HapTextureFormat_HapTextureFormat_YCoCg_DXT5,
    };
    let mut lengths = [input.len() as c_ulong];
    let mut formats = [native as u32];
    let mut chunks = [4];
    let mut compressors = [reference::HapCompressor_HapCompressorSnappy as u32];
    let mut inputs = [input.as_ptr().cast::<c_void>()];
    let capacity =
        unsafe { reference::HapMaxEncodedLength(1, lengths.as_mut_ptr(), formats.as_mut_ptr(), chunks.as_mut_ptr()) };
    assert!(capacity > 0);
    let mut output = vec![0; capacity as usize];
    let mut used = 0;
    assert_eq!(
        unsafe {
            reference::HapEncode(
                1,
                inputs.as_mut_ptr(),
                lengths.as_mut_ptr(),
                formats.as_mut_ptr(),
                compressors.as_mut_ptr(),
                chunks.as_mut_ptr(),
                output.as_mut_ptr().cast(),
                capacity,
                &mut used,
            )
        },
        0
    );
    output.truncate(used as usize);
    output
}

#[test]
#[ignore = "explicitly regenerates only the three checked-in reference HAP fixtures"]
fn generate_reference_hap_mov_fixtures() {
    ffmpeg::init().unwrap();
    for Case {
        name,
        format,
        tag,
        extent: (width, height),
        rate: (rate, denominator),
    } in cases()
    {
        let extent = Extent2D::new(width, height).unwrap();
        let mut output = ffmpeg::format::output(&root().join(name)).unwrap();
        let base = ffmpeg::Rational::new(1, rate);
        {
            let mut stream = output.add_stream(ffmpeg::codec::Id::HAP).unwrap();
            stream.set_time_base(base);
            stream.set_rate(ffmpeg::Rational::new(rate, denominator));
            let mut parameters = stream.parameters();
            unsafe {
                let raw = parameters.as_mut_ptr();
                (*raw).codec_type = ffmpeg::ffi::AVMediaType::AVMEDIA_TYPE_VIDEO;
                (*raw).codec_id = ffmpeg::codec::Id::HAP.into();
                (*raw).codec_tag = u32::from_le_bytes(tag);
                (*raw).width = width as i32;
                (*raw).height = height as i32;
            }
        }
        output.write_header().unwrap();
        let actual_base = output.stream(0).unwrap().time_base();
        for index in 0..6 {
            let encoded = reference_encode(format, &blocks(format, extent, index));
            let mut packet = ffmpeg::Packet::copy(&encoded);
            packet.set_stream(0);
            packet.set_pts(Some(i64::from(index) * i64::from(denominator)));
            packet.set_dts(packet.pts());
            packet.set_duration(i64::from(denominator));
            packet.set_flags(ffmpeg::packet::Flags::KEY);
            packet.rescale_ts(base, actual_base);
            packet.write_interleaved(&mut output).unwrap();
        }
        output.write_trailer().unwrap();
    }
}

#[test]
fn real_hap_mov_preserves_blocks_timestamps_seek_and_retained_frame_without_pixel_decode() {
    for Case {
        name,
        format,
        extent: (width, height),
        rate: (rate, denominator),
        ..
    } in cases()
    {
        let limits = DecodeLimits {
            max_pixels: 4096 * 2160,
            max_software_frame_bytes: 64 * 1024 * 1024,
            max_packet_bytes: 16 * 1024 * 1024,
            max_packets_per_frame: 32,
            software_threads: crate::decode::SoftwareDecodeThreads::One,
        };
        let mut decoder = Decoder::open(&root().join(name), limits).unwrap();
        assert!(decoder.decoder.is_none(), "HAP opens no libavcodec pixel decoder");
        let info = decoder.info().clone();
        assert_eq!(info.codec, "hap");
        assert_eq!((info.width, info.height), (width, height));
        let mut held = Vec::new();
        for index in 0..6 {
            let frame = decoder.next_frame().unwrap().unwrap();
            assert_eq!(
                i128::from(frame.timestamp()) * i128::from(info.time_base.numerator) * i128::from(rate),
                i128::from(index) * i128::from(info.time_base.denominator) * i128::from(denominator)
            );
            assert!(frame.software_image().is_none());
            assert!(matches!(
                SoftwareImageConverter::default().convert(&frame),
                Err(ConversionError::CompressedStorage)
            ));
            let compressed = frame.hap_frame().unwrap();
            assert_eq!(compressed.format(), format);
            assert_eq!(compressed.blocks(), blocks(format, compressed.extent(), index));
            held.push(frame);
        }
        assert!(decoder.next_frame().unwrap().is_none());
        for index in [5, 0, 3, 1] {
            decoder.seek_before(held[index].timestamp()).unwrap();
            let frame = decoder.next_frame().unwrap().unwrap();
            assert_eq!(frame.timestamp(), held[index].timestamp());
            assert_eq!(
                frame.hap_frame().unwrap().blocks(),
                held[index].hap_frame().unwrap().blocks()
            );
        }
        drop(decoder);
        assert_eq!(
            held[0].hap_frame().unwrap().blocks(),
            blocks(format, Extent2D::new(width, height).unwrap(), 0)
        );
    }
}
