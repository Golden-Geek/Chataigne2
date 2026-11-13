//! Independent FFmpeg pixel decoding is an oracle only; production preserves HAP blocks.
use super::*;
use ffmpeg_next as ffmpeg;
use std::path::Path;

fn reference_pixels(path: &Path) -> [[f64; 4]; 2] {
    let mut input = ffmpeg::format::input(path).unwrap();
    let stream = input.streams().best(ffmpeg::media::Type::Video).unwrap();
    let index = stream.index();
    let mut decoder = ffmpeg::codec::context::Context::from_parameters(stream.parameters())
        .unwrap()
        .decoder()
        .video()
        .unwrap();
    let mut decoded = ffmpeg::frame::Video::empty();
    for (stream, packet) in input.packets() {
        if stream.index() != index {
            continue;
        }
        decoder.send_packet(&packet).unwrap();
        if decoder.receive_frame(&mut decoded).is_ok() {
            break;
        }
    }
    assert!(decoded.width() > 0);
    let mut scaler = ffmpeg::software::scaling::Context::get(
        decoded.format(),
        decoded.width(),
        decoded.height(),
        ffmpeg::format::Pixel::RGBA,
        decoded.width(),
        decoded.height(),
        ffmpeg::software::scaling::Flags::POINT,
    )
    .unwrap();
    let mut rgba = ffmpeg::frame::Video::empty();
    scaler.run(&decoded, &mut rgba).unwrap();
    // The two output pixel centers sample the four neighbors at quarter/three-quarter x.
    // Decode and premultiply before filtering, independently of the GPU block decoder.
    [0.25, 0.75].map(|u| {
        let x = u * f64::from(rgba.width()) - 0.5;
        let y = f64::from(rgba.height()) / 2.0 - 0.5;
        let mut result = [0.0; 4];
        for dy in 0..2 {
            for dx in 0..2 {
                let weight = if dx == 0 { 1.0 - x.fract() } else { x.fract() }
                    * if dy == 0 { 1.0 - y.fract() } else { y.fract() };
                let offset = (y as usize + dy) * rgba.stride(0) + (x as usize + dx) * 4;
                let pixel = &rgba.data(0)[offset..offset + 4];
                let alpha = f64::from(pixel[3]) / 255.0;
                for component in 0..3 {
                    result[component] += weight * super::hap::linear(f64::from(pixel[component]) / 255.0) * alpha;
                }
                result[3] += weight * alpha;
            }
        }
        result
    })
}

#[test]
#[ignore = "requires both local GPUs and MAPGYVER_HAP_EXTERNAL with official FFmpeg test material"]
fn hardware_hap_external_ffmpeg_chunks_match_independent_pixel_decoder_on_both_gpus() {
    let root = std::path::PathBuf::from(std::env::var_os("MAPGYVER_HAP_EXTERNAL").unwrap());
    for adapter in adapters().unwrap().into_iter().filter(|adapter| !adapter.software) {
        let dummy = prepared_image(descriptor(2, ColorEncoding::Srgb, AlphaMode::Straight), &[0; 8]);
        let mut renderer = Renderer::new(adapter.id, &dummy, 2);
        let budget = GpuFrameBudget::new(128 * 1024 * 1024);
        for prefix in ["Hap", "Hap_Alpha", "Hap_Q"] {
            for chunks in [1, 2, 6, 64] {
                let name = format!("{prefix}_ffmpeg_{chunks}.mov");
                let path = root.join(&name);
                let expected = reference_pixels(&path);
                let mut decoder = Decoder::open(
                    &path,
                    DecodeLimits {
                        max_pixels: 4096 * 2160,
                        max_cpu_frame_bytes: 64 * 1024 * 1024,
                        max_packet_bytes: 16 * 1024 * 1024,
                        max_packets_per_frame: 32,
                        software_threads: crate::decode::SoftwareDecodeThreads::One,
                    },
                )
                .unwrap();
                assert_eq!((decoder.info().width, decoder.info().height), (4096, 2160));
                let frame = decoder.next_frame().unwrap().unwrap();
                let hap = frame.hap_frame().unwrap();
                let mut producer = GpuFramePublisher::new_hap(hap, adapter.id, &budget, None).unwrap();
                install_admitted(&mut renderer, producer.stream(), 128 * 1024 * 1024);
                assert!(producer.submit_hap(hap).unwrap());
                producer.stream().publish(Some(completed(&mut producer))).unwrap();
                let actual = renderer.pixel_frame();
                for (rgba, expected) in actual.iter().zip(expected) {
                    for component in 0..4 {
                        let limit = if component == 3 { 1.0 / 255.0 } else { 4.0 / 255.0 };
                        assert!(
                            (f64::from(rgba[component]) / 255.0 - expected[component]).abs() <= limit,
                            "{}, {name}: {rgba:?}, expected{expected:?}",
                            adapter.description
                        );
                    }
                }
                drop(producer);
                assert!(renderer.pixel_frame().iter().all(|pixel| *pixel == [0; 4]));
                assert_eq!(budget.retained_bytes(), 0);
                eprintln!(
                    "external HAP {} {name}: independent pixels and zero retirement pass",
                    adapter.description
                );
            }
        }
    }
}
