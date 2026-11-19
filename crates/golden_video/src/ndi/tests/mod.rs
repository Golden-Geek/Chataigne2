use super::{NdiDiscovery, NdiReceiver, Runtime};
use crate::{AlphaMode, Extent2D, PixelFormat};
use std::thread;
use std::time::{Duration, Instant};

#[test]
#[ignore = "requires an independent Windows NDI sender from apps/mapgyver/tests/native/ndi_sender.py"]
fn external_moving_video_and_cpu_storage_retirement() {
    let source_name = std::env::var("GOLDEN_NDI_TEST_SOURCE").expect("exact sender name from the fixture ready file");
    let mut discovery = NdiDiscovery::new().unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    let source = loop {
        if let Some(source) = discovery
            .sources()
            .into_iter()
            .find(|source| source.name() == source_name)
        {
            break source;
        }
        assert!(Instant::now() < deadline, "sender {source_name} was never discovered");
        thread::sleep(Duration::from_millis(40));
    };
    let initial = Runtime::get().unwrap().images.snapshot();
    let mut receiver = NdiReceiver::connect(&source).unwrap();
    let mut previous_marker = None;
    let deadline = Instant::now() + Duration::from_secs(6);
    loop {
        assert!(
            Instant::now() < deadline,
            "moving pixels did not arrive from {source_name}"
        );
        let Some(image) = receiver.capture_video(Duration::from_millis(100)).unwrap() else {
            continue;
        };
        assert_eq!(image.descriptor().extent, Extent2D::new(320, 240).unwrap());
        assert!(matches!(
            (image.descriptor().format, image.descriptor().alpha_mode),
            (PixelFormat::Bgrx8Unorm, AlphaMode::Opaque) | (PixelFormat::Bgra8Unorm, AlphaMode::Straight)
        ));
        let row = &image.pixels()[205 * 320 * 4..206 * 320 * 4];
        let marker = row
            .chunks_exact(4)
            .position(|pixel| pixel[1] > 145 && pixel[1] > pixel[0].saturating_add(90));
        let Some(marker) = marker else { continue };
        if previous_marker.is_some_and(|previous| previous != marker) {
            println!("Exact NDI sender {source_name} at 320×240, moving marker {previous_marker:?} to {marker}");
            assert!(receiver.connection_count() > 0);
            assert!(Runtime::get().unwrap().images.snapshot().retained_bytes > initial.retained_bytes);
            break;
        }
        previous_marker = Some(marker);
    }
    drop(receiver);
    drop(discovery);
    let final_images = Runtime::get().unwrap().images.snapshot();
    assert_eq!(final_images.retained_bytes, initial.retained_bytes);
    assert_eq!(final_images.allocations, initial.allocations);
}
