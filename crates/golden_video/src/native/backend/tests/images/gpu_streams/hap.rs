use super::*;
use crate::decode::{HapFormat, HapFrame};

fn fixture(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/decode/tests/fixtures")
        .join(name)
}
fn open(name: &str) -> Decoder {
    Decoder::open(
        &fixture(name),
        DecodeLimits {
            max_pixels: 4096 * 2160,
            max_cpu_frame_bytes: 64 * 1024 * 1024,
            max_packet_bytes: 16 * 1024 * 1024,
            max_packets_per_frame: 32,
            software_threads: crate::decode::SoftwareDecodeThreads::One,
        },
    )
    .unwrap()
}
pub(super) fn linear(encoded: f64) -> f64 {
    if encoded <= 0.04045 {
        encoded / 12.92
    } else {
        ((encoded + 0.055) / 1.055).powf(2.4)
    }
}
fn expected(format: HapFormat, index: u8) -> [f64; 4] {
    let alpha = if format == HapFormat::Bc3Rgba {
        f64::from(index) / 5.0
    } else {
        1.0
    };
    let rgb = if format == HapFormat::Bc3YCoCg {
        let y = f64::from(32 + index * 24) / 255.0;
        [y + 2.0 / 255.0, y + 2.0 / 255.0, y - 6.0 / 255.0]
    } else {
        let r = 4 + index * 3;
        let g = 8 + index * 6;
        [
            f64::from((r << 3) | (r >> 2)) / 255.0,
            f64::from((g << 2) | (g >> 4)) / 255.0,
            f64::from((r << 3) | (r >> 2)) / 255.0,
        ]
    };
    [
        linear(rgb[0]) * alpha,
        linear(rgb[1]) * alpha,
        linear(rgb[2]) * alpha,
        alpha,
    ]
}
#[test]
fn hap_zero_admission_precedes_native_device_creation() {
    let mut decoder = open("hap.mov");
    let frame = decoder.next_frame().unwrap().unwrap();
    assert!(matches!(
        GpuFramePublisher::new_hap(
            frame.hap_frame().unwrap(),
            AdapterId {
                low: u32::MAX,
                high: i32::MAX
            },
            &GpuFrameBudget::new(0),
            None
        ),
        Err(GpuFrameError::Capacity)
    ));
}
#[test]
#[ignore = "requires both local Windows D3D11 GPUs"]
fn hardware_hap_blocks_sample_convert_alpha_odd_extents_and_retire_on_both_gpus() {
    let adapters: Vec<_> = adapters().unwrap().into_iter().filter(|a| !a.software).collect();
    assert_eq!(adapters.len(), 2);
    for adapter in adapters {
        for name in ["hap.mov", "hap-alpha.mov", "hap-q.mov"] {
            eprintln!("HAP native: {}, {name}", adapter.description);
            let mut decoder = open(name);
            let dummy = prepared_image(descriptor(2, ColorEncoding::Srgb, AlphaMode::Straight), &[0; 8]);
            let mut renderer = Renderer::new(adapter.id, &dummy, 2);
            let budget = GpuFrameBudget::new(64 * 1024 * 1024);
            let mut publisher = None;
            for index in 0..6 {
                let frame = decoder.next_frame().unwrap().unwrap();
                let hap = frame.hap_frame().unwrap();
                let expected = expected(hap.format(), index);
                if publisher.is_none() {
                    let next = GpuFramePublisher::new_hap(hap, adapter.id, &budget, None).unwrap();
                    install_admitted(&mut renderer, next.stream(), 64 * 1024 * 1024);
                    publisher = Some(next);
                }
                let owner = publisher.as_mut().unwrap();
                assert!(owner.submit_hap(hap).unwrap());
                drop(frame);
                owner.stream().publish(Some(completed(owner))).unwrap();
                for rgba in renderer.pixel_frame() {
                    for component in 0..4 {
                        let limit = if component == 3 { 1.0 / 255.0 } else { 4.0 / 255.0 };
                        assert!(
                            (f64::from(rgba[component]) / 255.0 - expected[component]).abs() <= limit,
                            "{}, {name}, frame{index}: {rgba:?}, expected{expected:?}",
                            adapter.description
                        );
                    }
                }
            }
            drop(publisher);
            assert!(renderer.pixel_frame().iter().all(|p| *p == [0; 4]));
            assert_eq!(budget.retained_bytes(), 0);
        }
        // HAP RGB is opaque even in BC1's three-color palette mode.
        let packet = [8, 0, 0, 0xAB, 0, 0, 0, 0, 255, 255, 255, 255];
        let frame = HapFrame::decode(
            &packet,
            Extent2D::new(4, 4).unwrap(),
            &crate::image::ImageStorageBudget::new(16).unwrap(),
        )
        .unwrap();
        let budget = GpuFrameBudget::new(1024 * 1024);
        let mut producer = GpuFramePublisher::new_hap(&frame, adapter.id, &budget, None).unwrap();
        let dummy = prepared_image(descriptor(2, ColorEncoding::Srgb, AlphaMode::Straight), &[0; 8]);
        let mut renderer = Renderer::new(adapter.id, &dummy, 2);
        install(&mut renderer, producer.stream());
        assert!(producer.submit_hap(&frame).unwrap());
        producer.stream().publish(Some(completed(&mut producer))).unwrap();
        assert!(renderer.pixel_frame().iter().all(|p| *p == [0, 0, 0, 255]));
        drop(producer);
        renderer.pixel_frame();
        assert_eq!(budget.retained_bytes(), 0);
    }
}

#[test]
#[ignore = "requires both local Windows D3D11 GPUs"]
fn hardware_hap_chromatic_alpha_and_ycocg_scales_match_known_blocks_on_both_gpus() {
    for adapter in adapters().unwrap().into_iter().filter(|adapter| !adapter.software) {
        let dummy = prepared_image(descriptor(2, ColorEncoding::Srgb, AlphaMode::Straight), &[0; 8]);
        let mut renderer = Renderer::new(adapter.id, &dummy, 2);
        let budget = GpuFrameBudget::new(1024 * 1024);
        let mut cases = Vec::new();
        for color in [0xF800u16, 0x07E0, 0x001F] {
            cases.push((0xAB, color, 255));
            for alpha in [0, 64, 128, 255] {
                cases.push((0xAE, color, alpha));
            }
        }
        for scale_bits in [0, 1, 3] {
            cases.push((0xAF, (20 << 11) | (28 << 5) | scale_bits, 128));
        }
        for (tag, color, a) in cases {
            let mut block = Vec::new();
            if tag != 0xAB {
                block.extend_from_slice(&[a, a, 0, 0, 0, 0, 0, 0]);
            }
            block.extend_from_slice(&color.to_le_bytes());
            block.extend_from_slice(&color.to_le_bytes());
            block.extend_from_slice(&[0; 4]);
            let mut packet = vec![block.len() as u8, 0, 0, tag];
            packet.extend(block);
            let frame = HapFrame::decode(
                &packet,
                Extent2D::new(4, 4).unwrap(),
                &crate::image::ImageStorageBudget::new(16).unwrap(),
            )
            .unwrap();
            let mut rgb = [
                f64::from(((color >> 11) << 3) | ((color >> 11) >> 2)) / 255.0,
                f64::from((((color >> 5) & 63) << 2) | (((color >> 5) & 63) >> 4)) / 255.0,
                f64::from(((color & 31) << 3) | ((color & 31) >> 2)) / 255.0,
            ];
            if tag == 0xAF {
                let scale = rgb[2] * (255.0 / 8.0) + 1.0;
                let co = (rgb[0] - 128.0 / 255.0) / scale;
                let cg = (rgb[1] - 128.0 / 255.0) / scale;
                let y = f64::from(a) / 255.0;
                rgb = [y + co - cg, y + cg, y - co - cg];
            }
            let alpha = if tag == 0xAE { f64::from(a) / 255.0 } else { 1.0 };
            let expected = [
                linear(rgb[0]) * alpha,
                linear(rgb[1]) * alpha,
                linear(rgb[2]) * alpha,
                alpha,
            ];
            let mut producer = GpuFramePublisher::new_hap(&frame, adapter.id, &budget, None).unwrap();
            install(&mut renderer, producer.stream());
            assert!(producer.submit_hap(&frame).unwrap());
            producer.stream().publish(Some(completed(&mut producer))).unwrap();
            for pixel in renderer.pixel_frame() {
                for component in 0..4 {
                    let limit = if component == 3 { 1.0 / 255.0 } else { 4.0 / 255.0 };
                    assert!(
                        (f64::from(pixel[component]) / 255.0 - expected[component]).abs() <= limit,
                        "{}, tag{tag:x}, color{color:x}, alpha{a}: {pixel:?}, expected{expected:?}",
                        adapter.description
                    );
                }
            }
            drop(producer);
            renderer.pixel_frame();
            assert_eq!(budget.retained_bytes(), 0);
        }
        eprintln!("HAP {}: 18 chromatic/alpha/YCoCg scale cases pass", adapter.description);
    }
}
