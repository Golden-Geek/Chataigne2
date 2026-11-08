use super::*;
use golden_video::{
    native::{GpuFrameBudget, adapters},
    plan::ExternalSource,
};

#[test]
#[ignore = "requires Intel D3D11 decoding on the local Windows machine"]
fn hardware_file_worker_steps_exactly_and_falls_back_without_losing_its_clock() {
    let intel = adapters()
        .unwrap()
        .into_iter()
        .find(|adapter| adapter.vendor_id == 0x8086 && !adapter.software)
        .unwrap();
    for (name, capacity, hardware) in [
        ("h264.mp4", 64 * 1024 * 1024, true),
        ("h264.mp4", 0, false),
        ("prores.mov", 64 * 1024 * 1024, false),
    ] {
        let mut engine = engine();
        let reference = source(&mut engine, name);
        let budget = GpuFrameBudget::new(capacity);
        let mut renderer = renderer();
        renderer
            .enable_hardware_videos(&fixture_root(), None, intel.id, budget.clone())
            .unwrap();
        renderer
            .publish(capture_channels(&engine, &[reference], 8).unwrap(), routes(reference))
            .unwrap();
        let mut current = steps::at(&mut engine, reference, 0.0);
        wait(|| renderer.videos.as_ref().unwrap().source(reference).is_some());
        let (_, published) = renderer.videos.as_ref().unwrap().source(reference).unwrap();
        assert_eq!(
            matches!(published, ExternalSource::GpuFrames(_)),
            hardware,
            "{name}, capacity{capacity}"
        );
        if hardware {
            assert!(budget.retained_bytes() > 0);
        }
        let mut timestamps = vec![current.position_ms];
        while let Some(next) = current.next_ms {
            assert!(timestamps.len() < 10);
            steps::step(
                &mut engine,
                reference,
                crate::media::video::protocol::VideoStepDirection::Next,
            );
            current = steps::at(&mut engine, reference, next);
            timestamps.push(current.position_ms);
        }
        assert_eq!(timestamps.len(), 6);
        for expected in timestamps[..5].iter().rev() {
            steps::step(
                &mut engine,
                reference,
                crate::media::video::protocol::VideoStepDirection::Previous,
            );
            current = steps::at(&mut engine, reference, *expected);
            assert!((current.position_ms - expected).abs() < 0.000001);
        }
        edit(
            &mut engine,
            MediaEdit::Video {
                media: reference.media,
                edit: VideoEdit::Seek { position_ms: 125.0 },
            },
        );
        steps::at(&mut engine, reference, if name == "prores.mov" { 120.0 } else { 125.0 });
        edit(
            &mut engine,
            MediaEdit::Video {
                media: reference.media,
                edit: VideoEdit::Stop,
            },
        );
        steps::at(&mut engine, reference, 0.0);
        let retirement = renderer.video_retirement().unwrap();
        renderer.close();
        drop(renderer);
        drop(published);
        assert!(
            retirement.wait(Duration::from_secs(5)),
            "{name}: complete worker and pool retirement"
        );
        assert_eq!(budget.retained_bytes(), 0);
        eprintln!("file worker verified {name}: hardware={hardware}, {timestamps:?}, retired=0");
    }
}
