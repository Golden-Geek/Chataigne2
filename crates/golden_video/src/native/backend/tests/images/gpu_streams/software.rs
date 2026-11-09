use super::*;
use crate::image::ImageData;
use crate::native::{DiagnosticEvent, DiagnosticsConfig};
use std::{num::NonZeroUsize, sync::Arc};

#[test]
fn software_upload_rejects_unadmitted_storage_before_device_creation() {
    let desc = descriptor(2, ColorEncoding::Srgb, AlphaMode::Straight);
    let missing = AdapterId {
        low: u32::MAX,
        high: i32::MAX,
    };
    for limit in [0, 23] {
        assert!(matches!(
            GpuFramePublisher::new_software(desc, missing, &GpuFrameBudget::new(limit), None),
            Err(GpuFrameError::Capacity)
        ));
    }
    let unsupported = TextureDescriptor {
        format: PixelFormat::Rgba16Float,
        ..desc
    };
    assert!(matches!(
        GpuFramePublisher::new_software(unsupported, missing, &GpuFrameBudget::new(1024), None),
        Err(GpuFrameError::Unsupported)
    ));
}

#[test]
#[ignore = "explicit software source upload, color/alpha, shared consumers and slot retirement on both GPUs"]
fn hardware_software_upload_preserves_pixels_shared_consumers_and_bounded_slots_on_both_gpus() {
    let hardware: Vec<_> = adapters()
        .unwrap()
        .into_iter()
        .filter(|adapter| !adapter.software)
        .collect();
    assert_eq!(hardware.len(), 2);
    for adapter in hardware {
        for encoding in [ColorEncoding::Srgb, ColorEncoding::Linear] {
            let desc = descriptor(2, encoding, AlphaMode::Straight);
            let fixture = prepared_image(desc, &[0; 8]);
            let config = Some(DiagnosticsConfig {
                capacity: NonZeroUsize::new(1024).unwrap(),
            });
            let mut first = Renderer::with_diagnostics(adapter.id, &fixture, 2, config);
            let mut second = Renderer::new(adapter.id, &fixture, 2);
            let budget = GpuFrameBudget::new(24);
            let mut owner = GpuFramePublisher::new_software(desc, adapter.id, &budget, None).unwrap();
            install(&mut first, owner.stream());
            install(&mut second, owner.stream());
            for (index, pixels) in [
                [0, 1, 11, 0, 128, 254, 255, 255],
                [255, 128, 0, 128, 11, 64, 200, 64],
                [17, 39, 89, 255, 250, 101, 23, 1],
            ]
            .into_iter()
            .enumerate()
            {
                let image = Arc::new(ImageData::new(desc, pixels.to_vec()).unwrap());
                let weak = Arc::downgrade(&image);
                if index > 0 {
                    owner.record_software_upload_timing();
                }
                assert!(owner.submit_software(image.clone()).unwrap());
                assert!(
                    !owner.submit_software(image.clone()).unwrap(),
                    "one pending upload only"
                );
                drop(image);
                assert!(weak.upgrade().is_some(), "pending API/GPU input retains CPU admission");
                let started = Instant::now();
                let completed = completed(&mut owner);
                if index > 0 {
                    assert!(
                        started.elapsed() < Duration::from_millis(100),
                        "warm copy-only source completion must progress without a rendering query batch"
                    );
                }
                assert!(weak.upgrade().is_none());
                let metadata = completed.software_upload.unwrap();
                assert_eq!(
                    (metadata.sequence, metadata.bytes, metadata.row_pitch, metadata.rows),
                    (index as u64 + 1, 8, 8, 1)
                );
                assert_eq!(metadata.cpu.is_some(), index > 0);
                owner.stream().publish(Some(completed)).unwrap();
                let expected: Vec<_> = pixels
                    .chunks_exact(4)
                    .map(|rgba| {
                        let alpha = f64::from(rgba[3]) / 255.0;
                        let mut result = [0; 4];
                        for component in 0..3 {
                            let encoded = f64::from(rgba[component]) / 255.0;
                            let linear = if encoding == ColorEncoding::Srgb {
                                hap::linear(encoded)
                            } else {
                                encoded
                            };
                            result[component] = (linear * alpha * 255.0).round() as u8;
                        }
                        result[3] = rgba[3];
                        result
                    })
                    .collect();
                for renderer in [&mut first, &mut second] {
                    for _ in 0..2 {
                        let actual = renderer.pixel_frame();
                        for (actual, expected) in actual.iter().zip(&expected) {
                            for component in 0..4 {
                                assert!(
                                    actual[component].abs_diff(expected[component]) <= 1,
                                    "{}: {actual:?} expected{expected:?}",
                                    adapter.description
                                );
                            }
                        }
                    }
                }
            }
            let mut rows = Vec::new();
            first
                .backend
                .control
                .drain_diagnostics(&mut rows, NonZeroUsize::new(1024).unwrap());
            assert!(
                !rows
                    .iter()
                    .any(|event| matches!(event, DiagnosticEvent::ImageUpload(_))),
                "renderers must not perform the source CPU upload"
            );
            let uploads: Vec<_> = rows
                .iter()
                .filter_map(|row| match row {
                    DiagnosticEvent::SourceUse(row) => row.software_upload.map(|upload| upload.sequence),
                    _ => None,
                })
                .collect();
            assert_eq!(
                uploads,
                [1, 1, 2, 2, 3, 3],
                "held frames retain the original upload identity"
            );
            drop(owner);
            assert!(first.pixel_frame().iter().all(|pixel| *pixel == [0; 4]));
            assert!(second.pixel_frame().iter().all(|pixel| *pixel == [0; 4]));
            assert_eq!(budget.retained_bytes(), 0);

            let mut owner = GpuFramePublisher::new_software(desc, adapter.id, &budget, None).unwrap();
            let image = Arc::new(ImageData::new(desc, vec![127; 8]).unwrap());
            let mut held = Vec::new();
            for _ in 0..3 {
                assert!(owner.submit_software(image.clone()).unwrap());
                held.push(completed(&mut owner));
            }
            assert!(!owner.submit_software(image.clone()).unwrap());
            held.pop();
            assert!(owner.submit_software(image).unwrap());
            held.push(completed(&mut owner));
            drop(owner);
            assert_eq!(budget.retained_bytes(), 24);
            drop(held);
            assert_eq!(budget.retained_bytes(), 0);
        }
    }
}
