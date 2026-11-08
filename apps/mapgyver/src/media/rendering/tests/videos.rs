use super::*;
mod retention;
mod steps;
use crate::media::video::protocol::VideoEdit;

fn fixture_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates/golden_video/src/decode/tests/fixtures")
}
fn source(engine: &mut AppEngine, name: &str) -> ChannelReference {
    let reference = create(engine, MediaKind::Video);
    edit(
        engine,
        MediaEdit::ConfigureVideoFile {
            reference,
            file: name.into(),
        },
    );
    reference
}
fn pixel(renderer: &ChannelRenderer, reference: ChannelReference) -> Option<Vec<u8>> {
    let stream = renderer.videos.as_ref()?.stream(reference)?;
    Some(stream.sample()?.image?.pixels()[..4].to_vec())
}

#[test]
fn file_workers_publish_real_frames_and_references_share_one_stream_without_per_frame_plans() {
    let mut engine = engine();
    let reference = source(&mut engine, "h264.mp4");
    let MediaEditResult::ReferenceCreated { media } = edit(
        &mut engine,
        MediaEdit::CreateReference {
            target: Some(reference.media),
            label: None,
        },
    ) else {
        panic!()
    };
    let alias = crate::media::resolve_source_node(&engine, media.0).unwrap();
    let mut renderer = renderer();
    renderer.enable_videos(&fixture_root(), None).unwrap();
    let capture = capture_channels(&engine, &[reference, alias], 16).unwrap();
    let routes = vec![
        ChannelRoute {
            output: OutputId::new(1).unwrap(),
            channel: reference,
        },
        ChannelRoute {
            output: OutputId::new(2).unwrap(),
            channel: alias,
        },
    ];
    renderer.publish(capture, routes).unwrap();
    wait(|| {
        renderer.refresh().unwrap();
        pixel(&renderer, reference).is_some()
    });
    wait(|| {
        renderer.refresh().unwrap();
        renderer
            .channels()
            .iter()
            .filter(|channel| channel.resolution.availability == crate::media::resolution::ChannelAvailability::Ready)
            .count()
            == 2
    });
    let before = pixel(&renderer, reference).unwrap();
    let original = renderer
        .channels()
        .iter()
        .find(|channel| channel.reference == reference)
        .unwrap();
    let mirrored = renderer
        .channels()
        .iter()
        .find(|channel| channel.reference == alias)
        .unwrap();
    assert_eq!(original.target, mirrored.target);
    let request = renderer.latest_request();
    edit(
        &mut engine,
        MediaEdit::Video {
            media: reference.media,
            edit: VideoEdit::Seek { position_ms: 210.0 },
        },
    );
    wait(|| pixel(&renderer, reference).is_some_and(|pixels| pixels != before));
    let last = pixel(&renderer, reference).unwrap();
    assert!(last[0] > before[0] + 100);
    assert!(
        renderer.refresh().unwrap().is_none(),
        "frame publication must not rebuild plans"
    );
    assert_eq!(renderer.latest_request(), request);
    edit(
        &mut engine,
        MediaEdit::Video {
            media: reference.media,
            edit: VideoEdit::Stop,
        },
    );
    wait(|| pixel(&renderer, reference) == Some(before.clone()));
    let stream = renderer.videos.as_ref().unwrap().stream(reference).unwrap();
    let sequence = stream.sample().unwrap().sequence;
    edit(
        &mut engine,
        MediaEdit::Video {
            media: reference.media,
            edit: VideoEdit::Loop { looping: true },
        },
    );
    edit(
        &mut engine,
        MediaEdit::Video {
            media: reference.media,
            edit: VideoEdit::Rate { rate: 0.5 },
        },
    );
    edit(
        &mut engine,
        MediaEdit::Video {
            media: reference.media,
            edit: VideoEdit::Play,
        },
    );
    wait(|| stream.sample().unwrap().sequence >= sequence + 8);
    edit(
        &mut engine,
        MediaEdit::Video {
            media: reference.media,
            edit: VideoEdit::Pause,
        },
    );
    std::thread::sleep(Duration::from_millis(30));
    let paused = stream.sample().unwrap().sequence;
    std::thread::sleep(Duration::from_millis(80));
    assert_eq!(
        stream.sample().unwrap().sequence,
        paused,
        "paused sources stop publishing frames"
    );
    assert_eq!(renderer.latest_request(), request);
    let retirement = renderer.video_retirement().unwrap();
    edit(
        &mut engine,
        MediaEdit::Video {
            media: reference.media,
            edit: VideoEdit::Play,
        },
    );
    let capture = capture_channels(&engine, &[reference, alias], 16).unwrap();
    let control = capture
        .channels
        .iter()
        .find(|channel| channel.reference == reference)
        .unwrap()
        .video
        .clone()
        .unwrap();
    let epoch = control.sample().unwrap().epoch;
    renderer
        .publish(capture_channels(&engine, &[], 16).unwrap(), vec![])
        .unwrap();
    wait(|| retirement.active_workers() == 0);
    assert!(
        control.sample().unwrap().playing,
        "consumer withdrawal must not reset media transport"
    );
    assert_eq!(control.sample().unwrap().epoch, epoch);
    renderer.publish(capture, super::routes(reference)).unwrap();
    wait(|| pixel(&renderer, reference).is_some());
    assert_eq!(
        control.sample().unwrap().epoch,
        epoch,
        "a returning consumer shares the existing timeline"
    );
    let held = renderer
        .videos
        .as_ref()
        .unwrap()
        .stream(reference)
        .unwrap()
        .sample()
        .unwrap();
    close(renderer);
    wait(|| retirement.active_workers() == 0);
    assert!(retirement.retained_bytes() > 0);
    assert!(
        !retirement.wait(Duration::ZERO),
        "an external image observer retains its admission"
    );
    drop(held);
    assert!(retirement.wait(Duration::from_secs(5)));
    assert_eq!(retirement.retained_bytes(), 0);
}

#[test]
fn missing_files_recover_and_relink_withdraws_the_exact_old_source() {
    let root = tempfile::tempdir().unwrap();
    let mut engine = engine();
    let reference = source(&mut engine, "missing.mov");
    let mut renderer = renderer();
    renderer.enable_videos(root.path(), None).unwrap();
    renderer
        .publish(capture_channels(&engine, &[reference], 8).unwrap(), routes(reference))
        .unwrap();
    assert!(pixel(&renderer, reference).is_none());
    std::fs::copy(fixture_root().join("prores.mov"), root.path().join("missing.mov")).unwrap();
    wait(|| {
        renderer.refresh().unwrap();
        pixel(&renderer, reference).is_some()
    });
    let old = renderer.videos.as_ref().unwrap().stream(reference).unwrap();
    edit(
        &mut engine,
        MediaEdit::ConfigureVideoFile {
            reference,
            file: "absent.mov".into(),
        },
    );
    renderer
        .publish(capture_channels(&engine, &[reference], 8).unwrap(), routes(reference))
        .unwrap();
    wait(|| old.sample().unwrap().image.is_none());
    assert!(pixel(&renderer, reference).is_none());
    let retirement = renderer.video_retirement().unwrap();
    close(renderer);
    assert!(retirement.wait(Duration::from_secs(5)));
}
