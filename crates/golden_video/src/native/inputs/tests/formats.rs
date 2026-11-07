use super::super::*;
use crate::native::{D3d11Control, InputLimits, adapters, device::Device, pipeline::Pipeline, texture::Texture};
use crate::plan::{
    BlendMode, ExternalTexture, PlanLimits, RenderPlan, SampleFilter, SpoutFrameCount, SpoutMetadata, SpoutSource,
};
use crate::{AlphaMode, ColorEncoding, Extent2D, PixelFormat};
use windows::Win32::Graphics::Dxgi::Common::*;

mod bridge;
mod channels;
#[cfg(feature = "qualification")]
mod dx9_source_fixture;
mod integers;
pub(super) mod inventory;
mod legacy;
mod normalized;
mod packed;
mod source_fixture;
mod srgb;
pub(super) mod support;
mod typeless;
mod unspecified;
use support::{GpuUse, Sender, readback, wait};

#[test]
#[ignore = "requires explicit hardware D3D11/Spout qualification; no desktop input"]
fn hardware_source_formats_preserve_bits_color_alpha_shader_pixels_and_pool_charges() {
    let mut cases = vec![
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
    ];
    cases.extend(normalized::cases());
    verify_formats(&cases, true);
}

#[test]
#[ignore = "requires explicit hardware D3D11/Spout qualification; no desktop input"]
fn hardware_extended_formats_preserve_bits_implicit_channels_and_pool_charges() {
    verify_formats(&channels::cases(), false);
}

#[test]
#[ignore = "requires explicit hardware D3D11/Spout qualification; no desktop input"]
fn hardware_conversion_formats_preserve_native_bits_and_exact_bridge_charges() {
    verify_formats(&bridge::cases(), false);
}

#[test]
#[ignore = "requires explicit hardware D3D11/Spout qualification; unavailable layouts remain unaccepted"]
fn hardware_packed_formats_preserve_xr_bias_precision_and_bgra4444_storage() {
    verify_formats(&packed::cases(), false);
}

#[test]
#[ignore = "requires explicit native integer color/bit/storage qualification on every hardware adapter"]
fn hardware_integer_narrow_formats_preserve_bits_color_alpha_and_charges() {
    verify_formats(&integers::narrow_cases(), false);
}

#[test]
#[ignore = "requires explicit native 32-bit integer transport/color qualification; no float alias"]
fn hardware_integer_wide_formats_preserve_bits_color_alpha_and_charges() {
    verify_formats(&integers::wide_cases(), false);
}

#[test]
#[ignore = "requires native typeless default-view qualification on both adapters"]
fn hardware_typeless_defaults_preserve_bits_color_alpha_and_charges() {
    verify_formats(&typeless::default_cases(), false);
}

#[test]
#[ignore = "requires explicit compatible declared integer views over native typeless storage"]
fn hardware_typeless_declared_views_preserve_integer_precision_and_charges() {
    verify_formats(&typeless::declared_cases(), false);
}

fn verify_formats(cases: &[normalized::Case], replacements: bool) {
    let done = support::watchdog("source format qualification");
    let inventory = adapters().unwrap();
    let hardware = inventory.iter().filter(|adapter| !adapter.software).collect::<Vec<_>>();
    assert!(!hardware.is_empty());
    for adapter in hardware {
        for (format, raw_format, color, expected_bytes) in cases.iter().cloned() {
            if format == PixelFormat::Bgra4444Unorm {
                // This specific format is adapter-dependent in the existing inventory.
                // Recheck only its allocator prerequisite, never infer pixel acceptance.
                let device = Device::new(adapter.id).unwrap();
                if let Err(error) = inventory::texture(
                    &device,
                    raw_format,
                    windows::Win32::Graphics::Direct3D11::D3D11_RESOURCE_MISC_SHARED.0 as u32,
                ) {
                    assert_eq!(error.code().0, 0x8007_0057_u32 as i32);
                    println!(
                        "FORMAT_UNAVAILABLE {}",
                        serde_json::json!({
                            "adapter":format!("{:?}",adapter.id), "description":adapter.description,
                            "format":format, "dxgi_format":raw_format.0,"hresult":error.code().0,
                            "scope":"SDK shared allocation unavailable; native reception remains unaccepted on this adapter"
                        })
                    );
                    continue;
                }
            }
            let native_format = if matches!(raw_format.0, 21 | 22) {
                DXGI_FORMAT_B8G8R8A8_UNORM
            } else {
                raw_format
            };
            let converted = matches!(
                format,
                PixelFormat::Rg32Float
                    | PixelFormat::Bgr565Unorm
                    | PixelFormat::Bgra5551Unorm
                    | PixelFormat::Bgra4444Unorm
                    | PixelFormat::Rg32Uint
                    | PixelFormat::Rg32Sint
            );
            let override_view = typeless::default_format(raw_format).is_some_and(|default| default != format);
            let registrations = if override_view {
                vec![(raw_format.0 as u32, true), (0, true)]
            } else {
                std::iter::once((raw_format.0 as u32, false))
                    .chain((!matches!(raw_format.0, 21 | 22)).then_some((0, false)))
                    .chain(
                        (converted
                            || format == PixelFormat::Rgb10XrA2Unorm
                            || integers::layout(format).is_some()
                            || typeless::default_format(raw_format).is_some())
                        .then_some((raw_format.0 as u32, true)),
                    )
                    .collect::<Vec<_>>()
            };
            for (registered, declared) in registrations {
                let sender_name = format!("GoldenFormats-{}", uuid::Uuid::new_v4());
                let sender = if integers::layout(format).is_some() {
                    Sender::from_bytes(adapter.id, &sender_name, native_format, &expected_bytes)
                } else {
                    Sender::new(adapter.id, &sender_name, native_format, color)
                };
                if registered != native_format.0 as u32 {
                    sender.set_registered_format(registered);
                }
                let pixel_bytes = format.bytes_per_pixel() as usize;
                let source_bits = sender.pixels(pixel_bytes);
                if !expected_bytes.is_empty() {
                    assert_eq!(
                        source_bits,
                        expected_bytes.repeat(4),
                        "independent producer storage bits"
                    );
                }
                let device = Device::new(adapter.id).unwrap();
                let mut descriptor = crate::native::spout_layout::descriptor_from_alias(
                    Extent2D::new(2, 2).unwrap(),
                    registered,
                    native_format,
                )
                .unwrap();
                if override_view {
                    descriptor = SpoutSource::discovered_descriptor(descriptor.extent, format);
                }
                assert_eq!(descriptor.format, format);
                let opaque = registered == 22
                    || format == PixelFormat::Bgrx8Unorm
                    || channels::opaque(format)
                    || integers::layout(format).is_some_and(|(_, channels, _)| channels < 4)
                    || matches!(format, PixelFormat::Rg32Float | PixelFormat::Bgr565Unorm);
                assert_eq!(
                    descriptor.alpha_mode,
                    if opaque { AlphaMode::Opaque } else { AlphaMode::Straight }
                );
                if declared && integers::layout(format).is_some_and(|(_, channels, _)| channels < 4) {
                    descriptor.alpha_mode = AlphaMode::Straight;
                }
                let nominal = if declared {
                    descriptor
                } else {
                    TextureDescriptor {
                        format: PixelFormat::Bgra8Unorm,
                        color_encoding: ColorEncoding::Srgb,
                        alpha_mode: AlphaMode::Straight,
                        ..descriptor
                    }
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
                let bytes = InputLimits::source_storage_bytes(descriptor).unwrap();
                if declared {
                    let mut denied = SourcePool::new(
                        adapter.id,
                        InputLimits {
                            max_sources: 1,
                            max_storage_bytes: bytes - 1,
                        },
                        &D3d11Control::default(),
                        None,
                        crate::runtime::DeviceGeneration::new(uuid::Uuid::new_v4(), 0),
                    );
                    assert!(matches!(
                        denied.ensure(&plan),
                        Err(crate::runtime::backend::BackendError::SourceCapacity)
                    ));
                    assert!(
                        denied.inputs.is_empty(),
                        "insufficient full bridge charge cannot spawn a worker"
                    );
                }
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
                    "automatic source format must stream with its exact observed descriptor",
                    || {
                        let snapshot = *pool.inputs[&id].shared.status.lock().unwrap();
                        snapshot.state == SourceState::Streaming && snapshot.observed_descriptor == Some(descriptor)
                    },
                );
                assert!(
                    budget.claim(1).is_none(),
                    "full native precision must be charged before allocation"
                );
                if converted {
                    let latest = pool.inputs[&id].shared.latest.lock().unwrap();
                    let surface = &latest.as_ref().unwrap().surface;
                    assert_eq!(surface.descriptor, descriptor, "retain the caller's native descriptor");
                    let bridge = match format {
                        PixelFormat::Rg32Uint => PixelFormat::Rgba32Uint,
                        PixelFormat::Rg32Sint => PixelFormat::Rgba32Sint,
                        _ => PixelFormat::Rgba32Float,
                    };
                    assert_eq!(surface.storage_descriptor.format, bridge);
                    assert_eq!(surface.storage_descriptor.color_encoding, descriptor.color_encoding);
                    assert_eq!(surface.storage_descriptor.alpha_mode, descriptor.alpha_mode);
                }
                let capture = Texture::new_sampled(&device.device, descriptor).unwrap();
                let mut copied = false;
                uses.submit(|| {
                    wait("source format import must copy", || {
                        copied = pool
                            .copy(id, &device, &capture, 1, CopyFrame::default())
                            .unwrap()
                            .is_some_and(|copy| copy.sequence > 0);
                        copied
                    });
                });
                uses.wait();
                assert!(copied);
                let raw = readback(&device, &capture, pixel_bytes);
                assert_eq!(raw, source_bits, "native capture must preserve every source bit");
                let target = Texture::new(
                    &device.device,
                    TextureDescriptor {
                        format: PixelFormat::Rgba32Float,
                        color_encoding: ColorEncoding::Linear,
                        alpha_mode: AlphaMode::Premultiplied,
                        ..descriptor
                    },
                )
                .unwrap();
                let pipeline = Pipeline::new(&device.device).unwrap();
                let background = (matches!(format, PixelFormat::A8Unorm | PixelFormat::Bgra5551Unorm)
                    || integers::layout(format).is_some_and(|(_, channels, _)| channels == 4))
                .then(|| {
                    let background = Texture::new(&device.device, target.descriptor).unwrap();
                    unsafe {
                        device.context.ClearRenderTargetView(&background.rtv, &[1.; 4]);
                    }
                    background
                });
                pipeline.draw(
                    &device.context,
                    &capture,
                    background.as_ref(),
                    (&target).into(),
                    1.,
                    BlendMode::Normal,
                    SampleFilter::Nearest,
                );
                let rendered = readback(&device, &target, 16);
                let mut expected = normalized::expected(format, &source_bits[..pixel_bytes], color, opaque);
                if background.is_some() {
                    for i in 0..3 {
                        expected[i] += 1. - expected[3];
                    }
                    expected[3] = 1.;
                }
                let tolerance = normalized::tolerance(format);
                let mut max_shader_error = 0_f32;
                for pixel in rendered.chunks_exact(16) {
                    for (component, expected) in pixel.chunks_exact(4).zip(expected) {
                        let actual = f32::from_le_bytes(component.try_into().unwrap());
                        max_shader_error = max_shader_error.max((actual - expected).abs());
                        assert!(
                            (actual - expected).abs() <= tolerance,
                            "linear/HDR/straight alpha: {actual} versus {expected}"
                        );
                    }
                }
                let probe_budget = golden_io::PayloadPool::new("format-readiness", 4, raw.len() * 3).unwrap();
                let mut probe = crate::native::SpoutProbe::new_for_sender(&sender_name, probe_budget.clone()).unwrap();
                wait("native first-frame readiness must receive a real image", || {
                    probe.poll(true).unwrap().first_frame
                });
                let remaining = probe_budget.try_reserve(raw.len() * 2).unwrap();
                assert!(probe_budget.try_reserve(1).is_err());
                drop(remaining);
                let byte_export = matches!(format, PixelFormat::Rgba8Unorm | PixelFormat::Bgra8Unorm);
                if byte_export {
                    wait("byte probe export must preserve the received raw RGBA pixels", || {
                        let Some(pixels) = probe.capture_pixels().unwrap() else {
                            return false;
                        };
                        for (actual, raw) in pixels.pixels().iter().zip(source_bits.chunks_exact(4)) {
                            let expected = if format == PixelFormat::Bgra8Unorm {
                                [raw[2], raw[1], raw[0], raw[3]]
                            } else {
                                raw.try_into().unwrap()
                            };
                            assert_eq!(*actual, expected);
                        }
                        assert_eq!(pixels.pixels().len(), 4);
                        true
                    });
                } else {
                    assert!(
                        probe.capture_pixels().is_err(),
                        "byte-only export must reject before staging"
                    );
                }
                drop(probe);
                assert!(
                    probe_budget.try_reserve(raw.len() * 3).is_ok(),
                    "readiness storage must retire"
                );
                pool.retire(1);
                drop(demand);
                drop(pool);
                assert!(budget.claim(bytes).is_some(), "source format storage must fully retire");
                eprintln!(
                    "FORMAT_IMPORT {}",
                    serde_json::json!({
                        "adapter": format!("{:?}", adapter.id),
                        "description":adapter.description,
                        "format":format,
                        "dxgi_format":native_format.0,
                        "registered_format":registered,
                        "declared":declared,
                        "alpha_mode":descriptor.alpha_mode,
                        "declared_family_override":override_view,
                        "capture_bytes":raw.len(),
                        "source_charge":bytes,
                        "source_bits_exact":true,
                        "source_bits":source_bits,
                        "shader_expected":expected,
                        "shader_tolerance":tolerance,
                        "max_shader_error":max_shader_error,
                        "first_frame_probe":true,
                        "probe_reserved_bytes":raw.len(),
                        "byte_pixel_export":byte_export,
                        "retired":true,
                        "alpha_only_over_white":format == PixelFormat::A8Unorm,
                        "composited_over_white":background.is_some(),
                        "converted_bridge": super::super::conversion::bridge_descriptor(descriptor) != descriptor,
                    })
                );
            }
        }
        if replacements {
            srgb::verify_replacements(adapter.id);
            legacy::verify_replacements(adapter.id);
            unspecified::verify_replacements(adapter.id);
        }
    }
    done.send(()).unwrap();
}
