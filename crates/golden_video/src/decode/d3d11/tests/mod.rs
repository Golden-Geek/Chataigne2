use super::*;
use crate::decode::{ConversionError, SoftwareImageConverter};

fn limits() -> DecodeLimits {
    DecodeLimits {
        max_pixels: 4096 * 2160,
        max_packet_bytes: 8 * 1024 * 1024,
        max_packets_per_frame: 128,
        software_threads: crate::decode::SoftwareDecodeThreads::Four,
    }
}
fn fixture(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/decode/tests/fixtures")
        .join(name)
}

#[test]
fn hardware_pool_admission_bounds_surface_count_and_padded_extent() {
    assert!(admitted_pool(64, 64, 32, 4096));
    for (width, height, count, pixels) in [
        (0, 64, 1, 4096),
        (64, 0, 1, 4096),
        (64, 64, 0, 4096),
        (64, 64, 33, 4096),
        (64, 64, 1, 4095),
        (i32::MAX, i32::MAX, 1, 4096),
    ] {
        assert!(!admitted_pool(width, height, count, pixels));
    }
}

#[test]
fn pool_storage_admission_accounts_format_count_overflow_and_retained_owners() {
    use ffmpeg::ffi::AVPixelFormat::*;
    assert_eq!(pool_bytes(1920, 1088, 24, AV_PIX_FMT_NV12), Some(1920 * 1088 * 36));
    assert_eq!(pool_bytes(1920, 1088, 24, AV_PIX_FMT_P010LE), Some(1920 * 1088 * 72));
    assert_eq!(pool_bytes(1920, 1088, 24, AV_PIX_FMT_RGBA), None);
    assert_eq!(pool_bytes(-1, 1088, 24, AV_PIX_FMT_NV12), None);
    assert_eq!(pool_bytes(i32::MAX, i32::MAX, i32::MAX, AV_PIX_FMT_P010LE), None);
    let budget = GpuFrameBudget::new(20);
    let pool = budget.reserve(20).unwrap();
    let retained_frame = pool.clone();
    assert!(budget.reserve(1).is_none());
    drop(pool);
    assert_eq!(budget.retained_bytes(), 20);
    drop(retained_frame);
    assert_eq!(budget.retained_bytes(), 0);
    assert!(budget.reserve(20).is_some());
}

#[test]
fn unsupported_codec_and_absent_adapter_never_silently_select_software() {
    let absent = AdapterId {
        low: u32::MAX,
        high: i32::MAX,
    };
    assert!(matches!(
        Decoder::open_native(&fixture("prores.mov"), limits(), absent, GpuFrameBudget::new(0)),
        Err(DecodeError::UnsupportedHardware)
    ));
    assert!(matches!(
        Decoder::open_native(&fixture("h264.mp4"), limits(), absent, GpuFrameBudget::new(0)),
        Err(DecodeError::Hardware(_))
    ));
    let mut software = Decoder::open(&fixture("prores.mov"), limits()).unwrap();
    assert!(
        software.next_frame().unwrap().unwrap().software_image().is_some(),
        "explicit software fallback"
    );
}

/// Test-only blocking readback. The production decoder never maps/transfers hardware pixels.
fn luma(frame: &D3d11Frame) -> u8 {
    use windows::Win32::Graphics::Direct3D11::*;
    assert_eq!(frame.storage(), D3d11Storage::Nv12);
    frame.check_device().unwrap();
    let device = unsafe { frame.texture.GetDevice().unwrap() };
    let context = unsafe { device.GetImmediateContext().unwrap() };
    let mut descriptor = D3D11_TEXTURE2D_DESC::default();
    unsafe { frame.texture.GetDesc(&mut descriptor) };
    descriptor.ArraySize = 1;
    descriptor.Usage = D3D11_USAGE_STAGING;
    descriptor.BindFlags = 0;
    descriptor.MiscFlags = 0;
    descriptor.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
    let mut staging = None;
    unsafe { device.CreateTexture2D(&descriptor, None, Some(&mut staging)).unwrap() };
    let staging = staging.unwrap();
    // Map blocks until this copy completes; the borrowed frame pins the source pool slot.
    unsafe { context.CopySubresourceRegion(&staging, 0, 0, 0, 0, &frame.texture, frame.slice, None) };
    let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
    unsafe { context.Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped)).unwrap() };
    let value = unsafe { *mapped.pData.cast::<u8>() };
    unsafe { context.Unmap(&staging, 0) };
    value
}

#[test]
#[ignore = "requires explicitly available Windows hardware D3D11 decoding"]
fn hardware_frames_preserve_adapter_timestamps_seek_and_pool_leases_on_both_gpus() {
    let adapters: Vec<_> = crate::native::adapters()
        .unwrap()
        .into_iter()
        .filter(|a| !a.software)
        .collect();
    assert_eq!(adapters.len(), 2, "this qualification requires both local GPUs");
    for (name, rate, extent, base_luma) in [
        ("h264-color.mp4", (24, 1), (1920, 1080), 32),
        ("hevc-main.mp4", (30000, 1001), (1920, 1080), 32),
    ] {
        for adapter in &adapters {
            eprintln!("hardware decode GPU: {}, file: {name}", adapter.description);
            let mut decoder = Decoder::open_native(
                &fixture(name),
                limits(),
                adapter.id,
                GpuFrameBudget::new(512 * 1024 * 1024),
            )
            .unwrap();
            assert_eq!(decoder.software_decode_threads(), None);
            let threading = decoder.decoder.as_ref().unwrap().threading();
            assert_eq!(threading.kind, ffmpeg::codec::threading::Type::None);
            assert_eq!(
                threading.count, 1,
                "software slice admission must not alter hardware pools"
            );
            let base = decoder.info().time_base;
            let mut held = Vec::new();
            while let Some(frame) = decoder.next_frame().unwrap() {
                let index = held.len();
                assert!(index < 6);
                assert_eq!(
                    i128::from(frame.timestamp()) * i128::from(base.numerator) * rate.0,
                    index as i128 * i128::from(base.denominator) * rate.1
                );
                assert!(frame.software_image().is_none());
                assert!(matches!(
                    SoftwareImageConverter::default().convert(&frame),
                    Err(ConversionError::HardwareStorage)
                ));
                let gpu = frame.d3d11_frame().unwrap();
                assert_eq!(gpu.adapter(), adapter.id);
                assert_eq!((gpu.extent().width(), gpu.extent().height()), extent);
                assert!(
                    luma(gpu).abs_diff(base_luma + index as u8 * 24) <= 3,
                    "{name} frame{index}"
                );
                held.push(frame);
            }
            assert_eq!(held.len(), 6);
            for index in [0, 5, 2, 4, 1] {
                let target = held[index].timestamp();
                decoder.seek_before(target).unwrap();
                let mut found = None;
                for _ in 0..6 {
                    let frame = decoder.next_frame().unwrap().unwrap();
                    if frame.timestamp() == target {
                        found = Some(frame);
                        break;
                    }
                }
                let frame = found.expect("exact original seek timestamp");
                assert!(luma(frame.d3d11_frame().unwrap()).abs_diff(base_luma + index as u8 * 24) <= 3);
                assert!(
                    luma(held[0].d3d11_frame().unwrap()).abs_diff(base_luma) <= 3,
                    "old pool slot must survive each replacement decoder"
                );
            }
            drop(decoder);
            for (index, frame) in held.iter().enumerate() {
                assert!(
                    luma(frame.d3d11_frame().unwrap()).abs_diff(base_luma + index as u8 * 24) <= 3,
                    "{name}: old pool slot remains valid after replacement/decoder shutdown"
                );
            }
        }
    }
}
