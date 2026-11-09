use super::*;
mod hap;
mod hap_external;
use crate::native::frames::tests::{completed, prepared, submit};
use crate::{
    decode::{D3d11Storage, DecodeLimits, Decoder},
    native::{GpuFrameBudget, GpuFrameError, GpuFramePublisher, GpuFrameStream},
};

fn install(renderer: &mut Renderer, stream: GpuFrameStream) {
    install_admitted(renderer, stream, 65536);
}
fn install_admitted(renderer: &mut Renderer, stream: GpuFrameStream, max_texture_bytes: u64) {
    let extent = stream.descriptor().extent;
    renderer.plan = RenderPlan {
        external_textures: vec![ExternalTexture {
            id: TextureId::new(1).unwrap(),
            descriptor: stream.descriptor(),
            source: Some(ExternalSource::GpuFrames(stream)),
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
        max_texture_dimension: extent.width().max(extent.height()).max(2048),
        max_texture_bytes,
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
fn expect_gray(pixels: &[[u8; 4]], linear: f64) {
    for rgba in pixels {
        assert_eq!(rgba[3], 255);
        for component in &rgba[..3] {
            assert!(
                (*component as f64 / 255.0 - linear).abs() <= 4.0 / 255.0,
                "{rgba:?}, expected linear{linear}"
            );
        }
    }
}
fn bt709(encoded: f64) -> f64 {
    if encoded < 0.081 {
        encoded / 4.5
    } else {
        ((encoded + 0.099) / 1.099).powf(1.0 / 0.45)
    }
}

#[test]
#[ignore = "requires explicitly available Windows hardware D3D11"]
fn hardware_gpu_frames_convert_import_and_retain_native_slots_on_both_gpus() {
    for adapter in adapters().unwrap().into_iter().filter(|adapter| !adapter.software) {
        eprintln!("GPU frame conversion/import: {}", adapter.description);
        let dummy = prepared_image(descriptor(2, ColorEncoding::Srgb, AlphaMode::Straight), &[0; 8]);
        let mut renderer = Renderer::new(adapter.id, &dummy, 2);
        let budget = GpuFrameBudget::new(1024 * 1024);
        for storage in [D3d11Storage::Nv12, D3d11Storage::P010] {
            let scale = if storage == D3d11Storage::Nv12 { 1 } else { 4 };
            for flags in 0..6 {
                for level in 0..3 {
                    let full = flags & 1 != 0;
                    let maximum: u16 = if scale == 1 { 255 } else { 1023 };
                    let y = if full {
                        [0, maximum.div_ceil(2), maximum][level]
                    } else {
                        [16, 126, 235][level] * scale
                    };
                    let (mut publisher, source) = prepared(adapter.id, storage, flags, y, &budget);
                    let stream = publisher.stream();
                    install(&mut renderer, stream.clone());
                    let textures = renderer.backend.control.snapshot().created_textures;
                    assert!(submit(&mut publisher, &source));
                    assert!(!submit(&mut publisher, &source), "one pending conversion");
                    let image = completed(&mut publisher);
                    stream.publish(Some(image)).unwrap();
                    let encoded = if full {
                        y as f64 / maximum as f64
                    } else {
                        (y - 16 * scale) as f64 / (219 * scale) as f64
                    };
                    let expected = if flags & 4 != 0 {
                        encoded
                    } else if flags & 2 != 0 {
                        if encoded <= 0.04045 {
                            encoded / 12.92
                        } else {
                            ((encoded + 0.055) / 1.055).powf(2.4)
                        }
                    } else {
                        bt709(encoded)
                    };
                    expect_gray(&renderer.pixel_frame(), expected);
                    expect_gray(&renderer.pixel_frame(), expected);
                    assert_eq!(renderer.backend.control.snapshot().created_textures, textures);
                    stream.publish(None).unwrap();
                    assert!(renderer.pixel_frame().iter().all(|rgba| *rgba == [0; 4]));
                    drop(publisher);
                    assert!(renderer.pixel_frame().iter().all(|rgba| *rgba == [0; 4]));
                    assert_eq!(
                        budget.retained_bytes(),
                        0,
                        "completed readers must not retain slot budgets"
                    );
                }
            }
        }
        // Three held publications fill the fixed pool; releasing one permits reuse.
        let (mut publisher, source) = prepared(adapter.id, D3d11Storage::Nv12, 0, 126, &budget);
        let mut held = Vec::new();
        for _ in 0..3 {
            assert!(submit(&mut publisher, &source));
            held.push(completed(&mut publisher));
        }
        assert!(!submit(&mut publisher, &source));
        held.pop();
        assert!(submit(&mut publisher, &source));
        held.push(completed(&mut publisher));
        drop(publisher);
        assert!(
            budget.retained_bytes() > 0,
            "images retain real NT handles and allocation admission"
        );
        drop(held);
        assert_eq!(budget.retained_bytes(), 0);

        // Source teardown cannot return admission while an old renderer use is withheld.
        let (mut publisher, source) = prepared(adapter.id, D3d11Storage::Nv12, 0, 126, &budget);
        let stream = publisher.stream();
        assert!(submit(&mut publisher, &source));
        stream.publish(Some(completed(&mut publisher))).unwrap();
        install(&mut renderer, stream);
        let gate = renderer.backend.control.completion_gate.clone();
        struct Release(crate::native::tests::completion_gate::CompletionGate);
        impl Drop for Release {
            fn drop(&mut self) {
                self.0.release();
            }
        }
        let _release = Release(gate.clone());
        gate.hold();
        renderer.serial += 1;
        let serial = renderer.serial;
        let time = TimeAuthority::new(SystemClock::new(), ShowPosition::from_nanos(0), PlaybackRate::NORMAL)
            .unwrap()
            .sample()
            .unwrap();
        renderer
            .backend
            .submit(
                renderer.resources.as_ref().unwrap(),
                &renderer.plan,
                CompletionToken {
                    device: renderer.device,
                    serial,
                },
                time,
            )
            .unwrap();
        drop(publisher);
        drop(source);
        let deadline = Instant::now() + WAIT;
        loop {
            assert!(renderer.backend.completion().unwrap().serial < serial);
            if gate.snapshot().observed.is_some_and(|done| done.serial >= serial) {
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(budget.retained_bytes() > 0);
        gate.release();
        while renderer.backend.completion().unwrap().serial < serial {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(renderer.pixel_frame().iter().all(|rgba| *rgba == [0; 4]));
        assert_eq!(budget.retained_bytes(), 0);
    }
}

#[test]
#[ignore = "requires the verified local Intel hardware decoder"]
fn hardware_gpu_frames_import_actual_intel_decoder_without_cpu_conversion() {
    let adapter = adapters()
        .unwrap()
        .into_iter()
        .find(|adapter| adapter.vendor_id == 0x8086 && !adapter.software)
        .unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/decode/tests/fixtures");
    let limits = DecodeLimits {
        max_pixels: 4096 * 2160,
        max_packet_bytes: 1024 * 1024,
        max_packets_per_frame: 128,
    };
    let budget = GpuFrameBudget::new(1024 * 1024);
    let mut decoder = Decoder::open_native(&root.join("h264.mp4"), limits, adapter.id, budget.clone()).unwrap();
    let dummy = prepared_image(descriptor(2, ColorEncoding::Srgb, AlphaMode::Straight), &[0; 8]);
    let mut renderer = Renderer::new(adapter.id, &dummy, 2);
    let mut publisher = None;
    let mut count = 0;
    while let Some(frame) = decoder.next_frame().unwrap() {
        let gpu = frame.d3d11_frame().unwrap();
        if publisher.is_none() {
            let prepared = GpuFramePublisher::new(gpu, &budget, None).unwrap();
            install(&mut renderer, prepared.stream());
            publisher = Some(prepared);
        }
        let publisher = publisher.as_mut().unwrap();
        assert!(publisher.submit(gpu).unwrap());
        drop(frame);
        let image = completed(publisher);
        publisher.stream().publish(Some(image)).unwrap();
        expect_gray(&renderer.pixel_frame(), bt709((16.0 + count as f64 * 24.0) / 219.0));
        count += 1;
    }
    assert_eq!(count, 6);
    drop(decoder);
    drop(publisher);
    assert!(renderer.pixel_frame().iter().all(|rgba| *rgba == [0; 4]));
    assert_eq!(budget.retained_bytes(), 0);
    for name in ["h264-bt2020.mp4", "h264-pq.mp4"] {
        let mut decoder = Decoder::open_native(&root.join(name), limits, adapter.id, budget.clone()).unwrap();
        let frame = decoder.next_frame().unwrap().unwrap();
        assert!(matches!(
            GpuFramePublisher::new(frame.d3d11_frame().unwrap(), &budget, None),
            Err(GpuFrameError::Unsupported)
        ));
        assert!(budget.retained_bytes() > 0, "live decoder pool remains admitted");
        drop(frame);
        drop(decoder);
        assert_eq!(budget.retained_bytes(), 0);
    }
}
