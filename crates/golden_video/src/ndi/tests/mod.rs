use super::{NdiDiscovery, NdiFrame, NdiReceiver, NdiSource, Runtime};
use crate::{AlphaMode, Extent2D, PixelFormat};
use std::collections::HashMap;
use std::thread;
use std::time::{Duration, Instant};

#[test]
#[ignore = "requires an independent Windows NDI sender from apps/mapgyver/tests/native/ndi_sender.py"]
fn external_moving_video_and_cpu_storage_retirement() {
    let source_name = std::env::var("GOLDEN_NDI_TEST_SOURCE").expect("exact sender name from the fixture ready file");
    let source = find_source(&source_name);
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
    let final_images = Runtime::get().unwrap().images.snapshot();
    assert_eq!(final_images.retained_bytes, initial.retained_bytes);
    assert_eq!(final_images.allocations, initial.allocations);
}

fn find_source(name: &str) -> NdiSource {
    let mut discovery = NdiDiscovery::new().unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(source) = discovery.sources().into_iter().find(|source| source.name() == name) {
            return source;
        }
        assert!(Instant::now() < deadline, "sender {name} was never discovered");
        thread::sleep(Duration::from_millis(40));
    }
}

#[test]
#[ignore = "requires the independent stereo Windows NDI sender"]
fn external_stereo_pulses_follow_the_video_timecodes() {
    let name = std::env::var("GOLDEN_NDI_TEST_SOURCE").unwrap();
    let mut receiver = NdiReceiver::connect(&find_source(&name)).unwrap();
    let deadline = Instant::now() + Duration::from_secs(8);
    let mut pictures = HashMap::new();
    let mut sounds = HashMap::new();
    let mut paired = 0;
    let mut pulses = [0; 2];
    while paired < 45 || pulses.iter().any(|count| *count < 3) {
        assert!(
            Instant::now() < deadline,
            "only {paired} pairs and {pulses:?} pulses arrived from {name}"
        );
        let Some(frame) = receiver.capture_av(Duration::from_millis(100)).unwrap() else {
            continue;
        };
        let timecode = match frame {
            NdiFrame::Video(video) => {
                assert_eq!(video.image.descriptor().extent, Extent2D::new(320, 240).unwrap());
                let timecode = video.timing.timecode_100ns.unwrap();
                let marks = [80, 240].map(|x| video.image.pixels()[(135 * 320 + x) * 4 + 1] > 145);
                pictures.insert(timecode, marks);
                timecode
            }
            NdiFrame::Audio(audio) => {
                assert_eq!(audio.sample_rate().get(), 48_000);
                assert_eq!(audio.channels(), 2);
                assert_eq!(audio.frames(), 1_600);
                assert!(audio.plane(2).is_none());
                let timecode = audio.timing().timecode_100ns.unwrap();
                let signals = [0, 1].map(|channel| {
                    let plane = audio.plane(channel).unwrap();
                    let rms = (plane.iter().map(|value| value * value).sum::<f32>() / plane.len() as f32).sqrt();
                    let crossings = plane.windows(2).filter(|pair| pair[0] <= 0.0 && pair[1] > 0.0).count();
                    (rms, crossings)
                });
                sounds.insert(timecode, signals);
                timecode
            }
        };
        if let (Some(marks), Some(signals)) = (pictures.get(&timecode), sounds.get(&timecode)) {
            for (channel, &(rms, crossings)) in signals.iter().enumerate() {
                assert_eq!(
                    rms > 0.05,
                    marks[channel],
                    "pulse differs from video at {timecode}, channel {channel}"
                );
                if marks[channel] {
                    let expected = [14..=16, 28..=31];
                    assert!(
                        expected[channel].contains(&crossings),
                        "wrong tone in channel {channel}: {crossings}"
                    );
                    pulses[channel] += 1;
                }
            }
            paired += 1;
            pictures.remove(&timecode);
            sounds.remove(&timecode);
        }
    }
    println!("Matched {paired} video and audio timecodes with separate 440 Hz and 880 Hz pulses {pulses:?}");
}

#[test]
#[ignore = "requires the independent video-only Windows NDI sender"]
fn external_video_without_audio_yields_only_picture() {
    let name = std::env::var("GOLDEN_NDI_TEST_SOURCE").unwrap();
    let mut receiver = NdiReceiver::connect(&find_source(&name)).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut pictures = 0;
    while pictures < 30 {
        assert!(
            Instant::now() < deadline,
            "only {pictures} video frames arrived from {name}"
        );
        match receiver.capture_av(Duration::from_millis(100)).unwrap() {
            Some(NdiFrame::Video(_)) => pictures += 1,
            Some(NdiFrame::Audio(_)) => panic!("video-only fixture returned audio"),
            None => {}
        }
    }
}
