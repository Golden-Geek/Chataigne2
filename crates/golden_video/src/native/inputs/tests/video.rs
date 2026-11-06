use super::super::*;
use super::formats::support::{GpuUse, Sender, readback, wait};
use crate::native::{
    D3d11Control, InputLimits, SpoutProbe, adapters, device::Device, pipeline::Pipeline, texture::Texture,
};
use crate::plan::{
    BlendMode, ExternalTexture, PlanLimits, RenderPlan, SampleFilter, SpoutFrameCount, SpoutMetadata, SpoutSource,
};
use crate::{AlphaMode, ColorEncoding, Extent2D, PixelFormat, TextureDescriptor, TextureId};
use golden_io::PayloadPool;
use std::time::Duration;
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT;

#[test]
#[ignore = "explicit native packed-video precision/alpha/storage qualification on both GPUs"]
fn hardware_packed_video_decodes_444_precision_and_retires_storage() {
    let done = super::formats::support::watchdog("packed video qualification");
    let hardware = adapters()
        .unwrap()
        .into_iter()
        .filter(|adapter| !adapter.software)
        .collect::<Vec<_>>();
    assert!(!hardware.is_empty());
    for adapter in hardware {
        for native in [100, 101, 102] {
            let scale = match native {
                100 => 1,
                101 => 4,
                _ => 256,
            };
            let maximum = match native {
                100 => 255,
                101 => 1023,
                _ => 65535,
            };
            let alpha_max = match native {
                100 => 255,
                101 => 3,
                _ => 65535,
            };
            for (pattern, (y, u, v, a)) in [
                (126 * scale, 128 * scale, 128 * scale, alpha_max),
                (
                    100 * scale + u32::from(native == 102) * 13,
                    96 * scale + u32::from(native == 102) * 123,
                    180 * scale + u32::from(native == 102) * 47,
                    alpha_max / 2 + 1,
                ),
                (maximum, 32 * scale, 200 * scale, 0),
                (0, 128 * scale, 128 * scale, alpha_max),
            ]
            .into_iter()
            .enumerate()
            {
                let pixel = match native {
                    100 => vec![v as u8, u as u8, y as u8, a as u8],
                    101 => (u | (y << 10) | (v << 20) | (a << 30)).to_le_bytes().to_vec(),
                    _ => [u as u16, y as u16, v as u16, a as u16]
                        .into_iter()
                        .flat_map(u16::to_le_bytes)
                        .collect(),
                };
                let linear = reference(y, u, v, a, scale, alpha_max);
                assert!(linear[..3].iter().any(|v| v.abs() > 0.01));
                for mode in 0..3 {
                    let declared = mode == 2;
                    let name = format!("GoldenVideo-{}", uuid::Uuid::new_v4());
                    let sender = Sender::from_bytes(adapter.id, &name, DXGI_FORMAT(native as i32), &pixel);
                    if mode == 1 {
                        sender.set_registered_format(0);
                    }
                    assert_eq!(sender.pixels(pixel.len()), pixel.repeat(4));
                    let descriptor = TextureDescriptor {
                        extent: Extent2D::new(2, 2).unwrap(),
                        format: PixelFormat::Rgba32Float,
                        color_encoding: if declared {
                            ColorEncoding::Srgb
                        } else {
                            ColorEncoding::Linear
                        },
                        alpha_mode: if declared {
                            AlphaMode::Premultiplied
                        } else {
                            AlphaMode::Straight
                        },
                    };
                    let nominal = if declared {
                        descriptor
                    } else {
                        SpoutSource::discovered_descriptor(descriptor.extent, PixelFormat::Bgra8Unorm)
                    };
                    let id = TextureId::new(1).unwrap();
                    let plan = RenderPlan {
                        external_textures: vec![ExternalTexture {
                            id,
                            descriptor: nominal,
                            source: Some(
                                SpoutSource {
                                    sender: name.clone(),
                                    descriptor: nominal,
                                    metadata: if declared {
                                        SpoutMetadata::Declared
                                    } else {
                                        SpoutMetadata::Discover
                                    },
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
                    let actual = InputLimits::source_storage_bytes_for_native(descriptor, native).unwrap();
                    let charge = if declared {
                        actual.max(InputLimits::source_storage_bytes(descriptor).unwrap())
                    } else {
                        actual
                    };
                    let mut pool = SourcePool::new(
                        adapter.id,
                        InputLimits {
                            max_sources: 1,
                            max_storage_bytes: charge,
                        },
                        &D3d11Control::default(),
                        None,
                        crate::runtime::DeviceGeneration::new(uuid::Uuid::new_v4(), 0),
                    );
                    let budget = pool.storage_budget.clone();
                    pool.ensure(&plan).unwrap();
                    let demand = pool.demands(&plan);
                    wait("video source must decode and stream", || {
                        pool.inputs[&id].shared.status.lock().unwrap().state == SourceState::Streaming
                    });
                    assert!(budget.claim(1).is_none());
                    let device = Device::new(adapter.id).unwrap();
                    let capture = Texture::new(&device.device, descriptor).unwrap();
                    let mut gpu = GpuUse::new(&device);
                    gpu.submit(|| {
                        wait("decoded frame must copy", || {
                            pool.copy(id, &device, &capture, 1, CopyFrame::default())
                                .unwrap()
                                .is_some_and(|f| f.sequence > 0)
                        })
                    });
                    gpu.wait();
                    let stored = readback(&device, &capture, 16);
                    let mut expected = linear;
                    if declared {
                        for value in &mut expected[..3] {
                            *value = encode_srgb(*value) * linear[3];
                        }
                    }
                    let error = compare(&stored, expected);
                    let target = Texture::new(
                        &device.device,
                        TextureDescriptor {
                            color_encoding: ColorEncoding::Linear,
                            alpha_mode: AlphaMode::Premultiplied,
                            ..descriptor
                        },
                    )
                    .unwrap();
                    let white = Texture::new(&device.device, target.descriptor).unwrap();
                    let pipeline = Pipeline::new(&device.device).unwrap();
                    unsafe {
                        device.context.ClearRenderTargetView(&white.rtv, &[1.; 4]);
                    }
                    pipeline.draw(
                        &device.context,
                        &capture,
                        Some(&white),
                        (&target).into(),
                        1.,
                        BlendMode::Normal,
                        SampleFilter::Nearest,
                    );
                    let composed = readback(&device, &target, 16);
                    let mut rgba = [1.; 4];
                    for i in 0..3 {
                        rgba[i] = linear[i] * linear[3] + 1. - linear[3];
                    }
                    let composed_error = compare(&composed, rgba);
                    pool.retire(1);
                    drop(demand);
                    drop(pool);
                    let reclaimed = budget
                        .claim(charge)
                        .expect("full raw/decoder/bridge charge must retire");
                    drop(reclaimed);
                    let raw = super::super::video::Format::from_native(native)
                        .unwrap()
                        .image_bytes(descriptor)
                        .unwrap() as usize;
                    let probe_budget = PayloadPool::new("packed-video-readiness", 2, raw).unwrap();
                    let mut probe = SpoutProbe::new_for_sender(&name, probe_budget.clone()).unwrap();
                    wait("native video copy must prove readiness", || {
                        probe.poll(true).unwrap().first_frame
                    });
                    assert!(probe_budget.try_reserve(1).is_err());
                    assert!(probe.capture_pixels().is_err());
                    drop(probe);
                    assert!(probe_budget.try_reserve(raw).is_ok());
                    println!(
                        "VIDEO_IMPORT {}",
                        serde_json::json!({"adapter":format!("{:?}",adapter.id),"description":adapter.description,
                        "native_format":native,"pattern":pattern,"mode":mode,"source_pixel":pixel,"source_charge":charge,
                        "actual_storage":actual,"probe_charge":raw,"decoded_reference":expected,"decoded_error":error,
                        "composited_reference":rgba,"composited_error":composed_error,"ready":true,"retired":true})
                    );
                }
            }
        }
    }
    done.send(()).unwrap();
}

fn reference(y: u32, u: u32, v: u32, a: u32, scale: u32, alpha_max: u32) -> [f32; 4] {
    let s = scale as f64;
    let y = (y as f64 - 16. * s) / (219. * s);
    let u = (u as f64 - 128. * s) / (224. * s);
    let v = (v as f64 - 128. * s) / (224. * s);
    let decode = |x: f64| {
        if x < 0.081 {
            x / 4.5
        } else {
            ((x + 0.099) / 1.099).powf(1. / 0.45)
        }
    };
    [
        decode(y + 1.5748 * v) as f32,
        decode(y - (0.0722 * 1.8556 / 0.7152) * u - (0.2126 * 1.5748 / 0.7152) * v) as f32,
        decode(y + 1.8556 * u) as f32,
        a as f32 / alpha_max as f32,
    ]
}
fn encode_srgb(v: f32) -> f32 {
    if v <= 0.0031308 {
        v * 12.92
    } else {
        1.055 * v.powf(1. / 2.4) - 0.055
    }
}
fn compare(bytes: &[u8], expected: [f32; 4]) -> f32 {
    let mut maximum = 0_f32;
    for texel in bytes.chunks_exact(16) {
        for i in 0..4 {
            let actual = f32::from_le_bytes(texel[i * 4..i * 4 + 4].try_into().unwrap());
            let error = (actual - expected[i]).abs();
            assert!(
                actual.is_finite() && error <= 0.000001,
                "video component {actual} vs {}",
                expected[i]
            );
            maximum = maximum.max(error);
        }
    }
    maximum
}
