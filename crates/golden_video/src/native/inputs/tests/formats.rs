use super::super::*;
use crate::native::{D3d11Control, InputLimits, adapters, device::Device, pipeline::Pipeline, texture::Texture};
use crate::plan::{
    BlendMode, ExternalTexture, PlanLimits, RenderPlan, SampleFilter, SpoutFrameCount, SpoutMetadata, SpoutSource,
};
use crate::{AlphaMode, ColorEncoding, Extent2D, PixelFormat};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_R16G16B16A16_FLOAT, DXGI_FORMAT_R32G32B32A32_FLOAT};

mod support;
use support::{GpuUse, Sender, readback, wait};

#[test]
#[ignore = "requires explicit hardware D3D11/Spout qualification; no desktop input"]
fn hardware_float_sources_preserve_bits_linear_alpha_shader_pixels_and_pool_charges() {
    let (done, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        if matches!(
            receiver.recv_timeout(Duration::from_secs(90)),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout)
        ) {
            eprintln!("float source qualification exceeded90seconds");
            std::process::exit(1);
        }
    });
    let inventory = adapters().unwrap();
    let hardware = inventory.iter().filter(|adapter| !adapter.software).collect::<Vec<_>>();
    assert!(!hardware.is_empty());
    for adapter in hardware {
        for (format, raw_format, color, expected_bytes) in [
            (
                PixelFormat::Rgba16Float,
                DXGI_FORMAT_R16G16B16A16_FLOAT,
                [0.125, 1.25, -0.25, 0.5],
                [0x3000_u16, 0x3d00, 0xb400, 0x3800]
                    .into_iter()
                    .flat_map(u16::to_le_bytes)
                    .collect::<Vec<_>>(),
            ),
            (
                PixelFormat::Rgba32Float,
                DXGI_FORMAT_R32G32B32A32_FLOAT,
                [0.1234567, 1.25, -0.25, 0.5],
                [0.1234567_f32, 1.25, -0.25, 0.5]
                    .into_iter()
                    .flat_map(f32::to_le_bytes)
                    .collect::<Vec<_>>(),
            ),
        ] {
            let sender_name = format!("GoldenFloat-{}", uuid::Uuid::new_v4());
            let _sender = Sender::new(adapter.id, &sender_name, raw_format, color);
            let device = Device::new(adapter.id).unwrap();
            let descriptor = TextureDescriptor {
                extent: Extent2D::new(2, 2).unwrap(),
                format,
                color_encoding: ColorEncoding::Linear,
                alpha_mode: AlphaMode::Straight,
            };
            let nominal = TextureDescriptor {
                format: PixelFormat::Bgra8Unorm,
                color_encoding: ColorEncoding::Srgb,
                ..descriptor
            };
            let id = TextureId::new(1).unwrap();
            let plan = RenderPlan {
                external_textures: vec![ExternalTexture {
                    id,
                    descriptor: nominal,
                    source: Some(
                        SpoutSource {
                            sender: sender_name.clone(),
                            descriptor: nominal,
                            metadata: SpoutMetadata::Discover,
                            poll_interval: Duration::from_millis(16),
                            frame_count: SpoutFrameCount::Unversioned,
                        }
                        .into(),
                    ),
                }],
                ..Default::default()
            }
            .compile_with_source_demands(
                &PlanLimits {
                    max_passes: 0,
                    max_external_textures: 1,
                    max_outputs: 0,
                    max_dependencies: 0,
                    max_texture_dimension: 8,
                    max_texture_bytes: 4096,
                },
                &[id],
            )
            .unwrap();
            let bytes = InputLimits::source_storage_bytes(descriptor).unwrap();
            let mut pool = SourcePool::new(
                adapter.id,
                InputLimits {
                    max_sources: 1,
                    max_storage_bytes: bytes,
                },
                &D3d11Control::default(),
                None,
                crate::runtime::DeviceGeneration::new(uuid::Uuid::new_v4(), 0),
            );
            let budget = pool.storage_budget.clone();
            pool.ensure(&plan).unwrap();
            let demand = pool.demands(&plan);
            let mut uses = GpuUse::new(&device);
            wait(
                "automatic float source must stream with its exact observed descriptor",
                || {
                    let snapshot = *pool.inputs[&id].shared.status.lock().unwrap();
                    snapshot.state == SourceState::Streaming && snapshot.observed_descriptor == Some(descriptor)
                },
            );
            assert!(
                budget.claim(1).is_none(),
                "full float precision must be charged before allocation"
            );
            let capture = Texture::new_sampled(&device.device, descriptor).unwrap();
            let mut copied = false;
            uses.submit(|| {
                wait("float source import must copy", || {
                    copied = pool
                        .copy(id, &device, &capture, 1, CopyFrame::default())
                        .unwrap()
                        .is_some_and(|copy| copy.sequence > 0);
                    copied
                });
            });
            uses.wait();
            assert!(copied);
            let raw = readback(&device, &capture, expected_bytes.len());
            assert_eq!(
                raw,
                expected_bytes.repeat(4),
                "native capture must preserve every source bit"
            );
            let target = Texture::new(
                &device.device,
                TextureDescriptor {
                    format: PixelFormat::Rgba32Float,
                    alpha_mode: AlphaMode::Premultiplied,
                    ..descriptor
                },
            )
            .unwrap();
            let pipeline = Pipeline::new(&device.device).unwrap();
            pipeline.draw(
                &device.context,
                &capture,
                None,
                (&target).into(),
                1.,
                BlendMode::Normal,
                SampleFilter::Nearest,
            );
            let rendered = readback(&device, &target, 16);
            let expected = [color[0] * color[3], color[1] * color[3], color[2] * color[3], color[3]];
            for pixel in rendered.chunks_exact(16) {
                for (component, expected) in pixel.chunks_exact(4).zip(expected) {
                    let actual = f32::from_le_bytes(component.try_into().unwrap());
                    assert!(
                        (actual - expected).abs() <= 0.000001,
                        "linear/HDR/straight alpha: {actual} versus {expected}"
                    );
                }
            }
            let probe_budget = golden_io::PayloadPool::new("float-readiness", 4, raw.len() * 3).unwrap();
            let mut probe = crate::native::SpoutProbe::new_for_sender(&sender_name, probe_budget.clone()).unwrap();
            wait("float first-frame readiness must receive a real image", || {
                probe.poll(true).unwrap().first_frame
            });
            let remaining = probe_budget.try_reserve(raw.len() * 2).unwrap();
            assert!(probe_budget.try_reserve(1).is_err());
            drop(remaining);
            assert!(
                probe.capture_pixels().is_err(),
                "byte-only pixel export must reject floats before staging allocation"
            );
            drop(probe);
            assert!(
                probe_budget.try_reserve(raw.len() * 3).is_ok(),
                "readiness storage must retire"
            );
            pool.retire(1);
            drop(demand);
            drop(pool);
            assert!(budget.claim(bytes).is_some(), "float source storage must fully retire");
            eprintln!(
                "FLOAT_IMPORT {}",
                serde_json::json!({"adapter": format!("{:?}", adapter.id), "description":adapter.description,
                "format":format, "dxgi_format":raw_format.0, "capture_bytes":raw.len(), "source_charge":bytes,
                "source_bits_exact":true, "shader_expected":expected, "shader_tolerance":0.000001,
                "first_frame_probe":true, "probe_reserved_bytes":raw.len(), "retired":true})
            );
        }
    }
    done.send(()).unwrap();
}
