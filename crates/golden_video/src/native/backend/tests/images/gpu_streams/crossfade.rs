use super::*;
use crate::image::ImageData;
use crate::native::GpuFrameInput;
use std::sync::Arc;

#[test]
#[ignore = "native GPU linear-light crossfade pixels and completion ownership"]
fn hardware_crossfade_publishes_one_alpha_correct_image_to_both_consumers() {
    let targets: Vec<_> = adapters().unwrap().into_iter().filter(|a| !a.software).collect();
    assert!(!targets.is_empty());
    for target in targets {
        let desc = descriptor(2, ColorEncoding::Srgb, AlphaMode::Straight);
        let fixture = prepared_image(desc, &[0; 8]);
        let mut first = Renderer::new(target.id, &fixture, 2);
        let mut second = Renderer::new(target.id, &fixture, 2);
        let budget = GpuFrameBudget::new_with_upload_accounting(56);
        let mut publisher = GpuFramePublisher::new_software(desc, target.id, &budget, None).unwrap();
        let stream = publisher.stream();
        install(&mut first, stream.clone());
        install(&mut second, stream.clone());

        assert_eq!(budget.retained_bytes(), 24);
        let other_allocation = budget
            .reserve(1, crate::native::AllocationKind::SoftwareProducer)
            .unwrap();
        assert!(matches!(publisher.prepare_crossfade(), Err(GpuFrameError::Capacity)));
        drop(other_allocation);
        publisher.prepare_crossfade().unwrap();
        publisher.prepare_crossfade().unwrap();
        assert_eq!(budget.retained_bytes(), 56);

        let image = |rgba: [u8; 8]| Arc::new(ImageData::new(desc, rgba.to_vec()).unwrap());
        let tail = image([255, 0, 0, 128, 255, 0, 0, 0]);
        let head = image([0, 0, 255, 255, 0, 255, 0, 128]);
        let weak = [Arc::downgrade(&tail), Arc::downgrade(&head)];
        assert!(
            publisher
                .submit_crossfade(GpuFrameInput::Software(tail), GpuFrameInput::Software(head), 0.5)
                .unwrap()
        );
        assert!(!publisher.release_crossfade());
        assert!(weak.iter().all(|image| image.upgrade().is_some()));
        stream.publish(Some(completed(&mut publisher))).unwrap();
        assert!(publisher.release_crossfade());
        assert_eq!(budget.retained_bytes(), 24);
        assert!(weak.iter().all(|image| image.upgrade().is_none()));

        let expected = [[64u8, 0, 128, 192], [0, 64, 0, 64]];
        for renderer in [&mut first, &mut second] {
            let pixels = renderer.pixel_frame();
            assert_eq!(pixels.len(), expected.len());
            for (pixel, expected) in pixels.into_iter().zip(expected) {
                assert!(
                    pixel
                        .into_iter()
                        .zip(expected)
                        .all(|(actual, expected)| actual.abs_diff(expected) <= 2),
                    "{}: {pixel:?} expected {expected:?}",
                    target.description
                );
            }
        }
        let uploads = budget.snapshot().uploads.unwrap();
        assert_eq!((uploads.software.calls, uploads.software.bytes), (2, 16));
        drop(publisher);
        assert!(first.pixel_frame().iter().all(|pixel| *pixel == [0; 4]));
        assert!(second.pixel_frame().iter().all(|pixel| *pixel == [0; 4]));
        assert_eq!(budget.retained_bytes(), 0);
    }
}
