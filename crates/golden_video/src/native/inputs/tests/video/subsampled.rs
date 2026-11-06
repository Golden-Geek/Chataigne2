use super::*;
use crate::native::device::AdapterInfo;

pub(super) fn verify(
    adapter: &AdapterInfo,
    native: u32,
    mode: u32,
    extent: Extent2D,
    bytes: &[u8],
    linear: &[[f32; 4]],
    label: &str,
) {
    let pixel_bytes = match native {
        103 => 1,
        104 | 105 | 107 => 2,
        108 | 109 => 4,
        _ => panic!("not subsampled video"),
    };
    let name = format!("GoldenSubsampledVideo-{}", uuid::Uuid::new_v4());
    let sender = Sender::from_rows(
        adapter.id,
        &name,
        DXGI_FORMAT(native as i32),
        extent,
        bytes,
        extent.width() * pixel_bytes as u32,
    );
    if mode == 1 {
        sender.set_registered_format(0);
    }
    assert_eq!(sender.pixels(pixel_bytes), bytes);
    let declared = mode >= 2;
    let descriptor = TextureDescriptor {
        extent,
        format: PixelFormat::Rgba32Float,
        color_encoding: if mode == 2 {
            ColorEncoding::Srgb
        } else {
            ColorEncoding::Linear
        },
        alpha_mode: match mode {
            2 => AlphaMode::Premultiplied,
            3 => AlphaMode::Straight,
            _ => AlphaMode::Opaque,
        },
    };
    let nominal = if declared {
        descriptor
    } else {
        SpoutSource::discovered_descriptor(extent, PixelFormat::Bgra8Unorm)
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
    wait("subsampled source must stream", || {
        pool.inputs[&id].shared.status.lock().unwrap().state == SourceState::Streaming
    });
    assert!(budget.claim(1).is_none());
    let device = Device::new(adapter.id).unwrap();
    let capture = Texture::new(&device.device, descriptor).unwrap();
    let mut gpu = GpuUse::new(&device);
    gpu.submit(|| {
        wait("subsampled decoded image must copy", || {
            pool.copy(id, &device, &capture, 1, CopyFrame::default())
                .unwrap()
                .is_some_and(|f| f.sequence > 0)
        })
    });
    gpu.wait();
    let mut expected = linear.to_vec();
    if mode == 2 {
        for pixel in &mut expected {
            for value in &mut pixel[..3] {
                *value = encode_srgb(*value);
            }
        }
    }
    let decoded_error = compare_image(&readback(&device, &capture, 16), &expected);
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
    let composited_error = compare_image(&readback(&device, &target, 16), linear);
    pool.retire(1);
    drop(demand);
    drop(pool);
    assert!(
        budget.claim(charge).is_some(),
        "all subsampled source storage must retire"
    );
    let raw = bytes.len();
    let probe_budget = PayloadPool::new("subsampled-video-readiness", 2, raw).unwrap();
    let mut probe = SpoutProbe::new_for_sender(&name, probe_budget.clone()).unwrap();
    wait("subsampled native readiness requires actual SDK copy", || {
        probe.poll(true).unwrap().first_frame
    });
    assert!(probe_budget.try_reserve(1).is_err());
    assert!(probe.capture_pixels().is_err());
    drop(probe);
    assert!(probe_budget.try_reserve(raw).is_ok());
    println!(
        "{label} {}",
        serde_json::json!({
            "adapter":format!("{:?}", adapter.id), "description":adapter.description,
            "native_format":native, "mode":mode, "source_bits":bytes,
            "decoded_reference":expected, "decoded_error":decoded_error,
            "composited_reference":linear, "composited_error":composited_error,
            "source_charge":charge, "actual_storage":actual, "probe_charge":raw,
            "ready":true, "retired":true,
        })
    );
}
