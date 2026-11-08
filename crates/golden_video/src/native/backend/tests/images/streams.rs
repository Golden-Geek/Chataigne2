use super::*;
use crate::image::{ImageData, ImageStream};

#[test]
#[ignore = "requires explicitly available Windows hardware D3D11"]
fn hardware_stream_frames_update_one_installed_texture_and_withdraw_on_both_gpus() {
    let hardware = adapters()
        .unwrap()
        .into_iter()
        .filter(|adapter| !adapter.software)
        .collect::<Vec<_>>();
    assert!(!hardware.is_empty());
    for adapter in hardware {
        eprintln!("stream texture pixels GPU: {}", adapter.description);
        let source_descriptor = descriptor(2, ColorEncoding::Srgb, AlphaMode::Straight);
        let image = prepared_image(source_descriptor, &[255, 0, 0, 255, 255, 0, 0, 255]);
        let stream = ImageStream::new(source_descriptor);
        let mut renderer = Renderer::new(adapter.id, &image, 2);
        install(&mut renderer, stream.clone());
        let created = renderer.backend.control.snapshot().created_textures;
        for (rgba, expected) in [
            ([255, 0, 0, 255], [255, 0, 0, 255]),
            ([0, 255, 0, 128], [0, 128, 0, 128]),
            ([0, 0, 255, 255], [0, 0, 255, 255]),
        ] {
            stream
                .publish(Some(ImageData::new(source_descriptor, rgba.repeat(2)).unwrap()))
                .unwrap();
            expect(&renderer.pixel_frame(), &[expected, expected]);
            assert_eq!(
                renderer.backend.control.snapshot().created_textures,
                created,
                "frame updates must reuse admitted native storage: {}",
                adapter.description
            );
        }
        stream.publish(None).unwrap();
        expect(&renderer.pixel_frame(), &[[0; 4]; 2]);
    }
}

fn install(renderer: &mut Renderer, stream: ImageStream) {
    renderer.plan = RenderPlan {
        external_textures: vec![ExternalTexture {
            id: TextureId::new(1).unwrap(),
            descriptor: stream.descriptor(),
            source: Some(ExternalSource::Frames(stream)),
        }],
        passes: vec![RenderPass {
            id: PassId::new(1).unwrap(),
            output: TextureId::new(2).unwrap(),
            descriptor: descriptor(2, ColorEncoding::Linear, AlphaMode::Premultiplied),
            operation: RenderOperation::Sample {
                source: TextureId::new(1).unwrap(),
                filter: SampleFilter::Linear,
            },
        }],
        outputs: vec![PlanOutput {
            id: OutputId::new(1).unwrap(),
            source: TextureId::new(2).unwrap(),
        }],
    }
    .compile(&PlanLimits {
        max_passes: 4,
        max_external_textures: 4,
        max_outputs: 4,
        max_dependencies: 16,
        max_texture_dimension: 1024,
        max_texture_bytes: 65536,
    })
    .unwrap();
    renderer.resources = Some(
        renderer
            .backend
            .prepare(
                &renderer.plan,
                super::super::generation(&renderer.backend, &renderer.plan, renderer.device),
            )
            .unwrap(),
    );
}

#[cfg(feature = "ffmpeg")]
#[test]
#[ignore = "requires explicitly available Windows hardware D3D11"]
fn hardware_common_codec_sdr_frames_match_independent_linear_pixels_on_both_gpus() {
    use crate::decode::{DecodeLimits, Decoder, SoftwareImageConverter};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/decode/tests/fixtures");
    for adapter in adapters().unwrap().into_iter().filter(|adapter| !adapter.software) {
        eprintln!("common codec pixels GPU: {}", adapter.description);
        let dummy = prepared_image(descriptor(2, ColorEncoding::Srgb, AlphaMode::Straight), &[0; 8]);
        let mut renderer = Renderer::new(adapter.id, &dummy, 2);
        for name in ["h264.mp4", "hevc.mp4", "prores.mov"] {
            let mut decoder = Decoder::open(
                &root.join(name),
                DecodeLimits {
                    max_pixels: 4096 * 2160,
                    max_packet_bytes: 1024 * 1024,
                    max_packets_per_frame: 256,
                },
            )
            .unwrap();
            let mut converter = SoftwareImageConverter::default();
            let mut stream = None;
            let mut count = 0;
            while let Some(frame) = decoder.next_frame().unwrap() {
                let image = converter.convert(&frame).unwrap();
                if stream.is_none() {
                    let source = ImageStream::new(image.descriptor());
                    install(&mut renderer, source.clone());
                    stream = Some(source);
                }
                let created = renderer.backend.control.snapshot().created_textures;
                stream.as_ref().unwrap().publish(Some(image)).unwrap();
                // The owning fixture generator authors limited-range Y = 32 + 24*N.
                let encoded = (16.0 + count as f64 * 24.0) / 219.0;
                let linear = if encoded < 0.081 {
                    encoded / 4.5
                } else {
                    ((encoded + 0.099) / 1.099).powf(1.0 / 0.45)
                };
                let expected = (linear * 255.0).round() as u8;
                for rgba in renderer.pixel_frame() {
                    assert_eq!(rgba[3], 255, "{name}: alpha");
                    for component in &rgba[..3] {
                        assert!(
                            component.abs_diff(expected) <= 4,
                            "{} {name} frame{count}: {rgba:?}, expected {expected}",
                            adapter.description
                        );
                    }
                }
                assert_eq!(renderer.backend.control.snapshot().created_textures, created);
                count += 1;
            }
            assert_eq!(count, 6);
        }
    }
}
