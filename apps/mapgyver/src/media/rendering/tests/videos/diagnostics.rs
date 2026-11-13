use super::*;
use crate::{
    media::video::protocol::{VideoSourceErrorKind, VideoSourceState, VideoSourceStatus},
    preview::{capture::capture_preview, protocol::PreviewSource},
};

fn status(engine: &mut AppEngine, reference: ChannelReference) -> VideoSourceStatus {
    let MediaEditResult::VideoSource { status } = edit(engine, MediaEdit::InspectVideoSource { reference }) else {
        panic!()
    };
    status
}

#[test]
fn in_band_growth_withdraws_failed_frames_and_valid_relink_recovers_without_disturbing_other_sources() {
    let mut engine = engine();
    let growing = source(&mut engine, "h264-growing.mp4");
    let healthy = source(&mut engine, "h264.mp4");
    let references = [growing, healthy];
    let routes: Vec<_> = references
        .iter()
        .enumerate()
        .map(|(index, reference)| ChannelRoute {
            output: OutputId::new(index as u64 + 1).unwrap(),
            channel: *reference,
        })
        .collect();
    let mut renderer = renderer();
    renderer.enable_videos(&fixture_root(), None).unwrap();
    renderer
        .publish(capture_channels(&engine, &references, 8).unwrap(), routes.clone())
        .unwrap();
    wait(|| {
        references
            .iter()
            .all(|reference| pixel(&renderer, *reference).is_some())
    });
    let retirement = renderer.video_retirement().unwrap();
    let old = renderer.videos.as_ref().unwrap().stream(growing).unwrap();
    let unaffected = renderer.videos.as_ref().unwrap().stream(healthy).unwrap();
    let healthy_pixel = pixel(&renderer, healthy).unwrap();
    assert_eq!(status(&mut engine, growing).state, VideoSourceState::Ready);
    edit(
        &mut engine,
        MediaEdit::Video {
            media: growing.media,
            edit: VideoEdit::Seek { position_ms: 6000.0 },
        },
    );
    let history = engine.undo_len();
    wait(|| {
        renderer.refresh().unwrap();
        status(&mut engine, growing).error.is_some() && old.sample().unwrap().image.is_none()
    });
    let failure = status(&mut engine, growing);
    assert_eq!(failure.state, VideoSourceState::Recovering);
    assert!(failure.retrying);
    assert!(matches!(
        failure.error.unwrap().kind,
        VideoSourceErrorKind::Decode | VideoSourceErrorKind::Capacity
    ));
    assert!(pixel(&renderer, growing).is_none());
    assert!(unaffected.sample().unwrap().image.is_some());
    assert_eq!(pixel(&renderer, healthy), Some(healthy_pixel.clone()));
    assert_eq!(status(&mut engine, healthy).state, VideoSourceState::Ready);
    assert_eq!(renderer.snapshot().state, golden_video::runtime::RuntimeState::Running);
    assert_eq!(engine.undo_len(), history, "worker failure must not author history");

    edit(
        &mut engine,
        MediaEdit::ConfigureVideoFile {
            reference: growing,
            file: "h264.mp4".into(),
        },
    );
    edit(
        &mut engine,
        MediaEdit::Video {
            media: growing.media,
            edit: VideoEdit::Seek { position_ms: 0.0 },
        },
    );
    renderer
        .publish(capture_channels(&engine, &references, 8).unwrap(), routes)
        .unwrap();
    wait(|| {
        renderer.refresh().unwrap();
        status(&mut engine, growing).state == VideoSourceState::Ready
            && pixel(&renderer, growing) == Some(healthy_pixel.clone())
    });
    let recovered = status(&mut engine, growing);
    assert!(recovered.error.is_none());
    assert!(!recovered.retrying);
    assert!(
        old.sample().unwrap().image.is_none(),
        "a retired stream must stay withdrawn"
    );
    assert!(unaffected.sample().unwrap().image.is_some());
    assert_eq!(retirement.active_workers(), 2);
    close(renderer);
    assert!(retirement.wait(Duration::from_secs(5)));
    assert_eq!(retirement.active_workers(), 0);
    assert_eq!(retirement.retained_bytes(), 0);
    let admission = retirement.cpu_admission();
    assert_eq!(admission.retained_bytes, 0);
    assert_eq!(admission.allocations, 0);
}

#[test]
fn missing_corrupt_and_recovered_files_report_scoped_status_without_faulting_healthy_sources_or_history() {
    let root = tempfile::tempdir().unwrap();
    std::fs::copy(fixture_root().join("h264.mp4"), root.path().join("healthy.mp4")).unwrap();
    let mut engine = engine();
    let failed = source(&mut engine, "missing.mp4");
    let healthy = source(&mut engine, "healthy.mp4");
    let MediaEditResult::ReferenceCreated { media } = edit(
        &mut engine,
        MediaEdit::CreateReference {
            target: Some(failed.media),
            label: None,
        },
    ) else {
        panic!()
    };
    let alias = crate::media::resolve_source_node(&engine, media.0).unwrap();
    let mut renderer = renderer();
    renderer.enable_videos(root.path(), None).unwrap();
    renderer
        .publish(
            capture_channels(&engine, &[failed, healthy, alias], 8).unwrap(),
            vec![
                ChannelRoute {
                    output: OutputId::new(1).unwrap(),
                    channel: alias,
                },
                ChannelRoute {
                    output: OutputId::new(2).unwrap(),
                    channel: healthy,
                },
            ],
        )
        .unwrap();
    let history = engine.undo_len();
    wait(|| {
        renderer.refresh().unwrap();
        status(&mut engine, failed)
            .error
            .is_some_and(|error| error.kind == VideoSourceErrorKind::Missing)
            && pixel(&renderer, healthy).is_some()
    });
    assert_eq!(status(&mut engine, healthy).state, VideoSourceState::Ready);
    let capture = capture_preview(&engine, &[PreviewSource::Channel { reference: alias }], 8, 8).unwrap();
    let statuses = capture.statuses(renderer.channels());
    let preview = statuses
        .iter()
        .find(|status| status.source == PreviewSource::Channel { reference: alias })
        .unwrap();
    assert!(
        preview
            .message
            .as_ref()
            .is_some_and(|message| message.contains("Retrying automatically"))
    );
    assert_eq!(renderer.snapshot().state, golden_video::runtime::RuntimeState::Running);
    std::fs::write(root.path().join("missing.mp4"), b"not a video file").unwrap();
    wait(|| {
        status(&mut engine, failed)
            .error
            .is_some_and(|error| error.kind == VideoSourceErrorKind::Decode)
    });
    std::fs::copy(fixture_root().join("hevc.mp4"), root.path().join("missing.mp4")).unwrap();
    wait(|| {
        renderer.refresh().unwrap();
        status(&mut engine, failed).state == VideoSourceState::Ready
    });
    assert!(status(&mut engine, failed).error.is_none());
    assert!(!status(&mut engine, failed).retrying);
    assert_eq!(status(&mut engine, healthy).state, VideoSourceState::Ready);
    assert_eq!(engine.undo_len(), history);
    let capture = capture_preview(&engine, &[PreviewSource::Channel { reference: alias }], 8, 8).unwrap();
    let statuses = capture.statuses(renderer.channels());
    assert!(
        statuses
            .iter()
            .find(|status| status.source == PreviewSource::Channel { reference: alias })
            .unwrap()
            .message
            .is_none()
    );
    let retirement = renderer.video_retirement().unwrap();
    close(renderer);
    assert!(retirement.wait(Duration::from_secs(5)));
}

#[test]
fn actual_unsupported_color_sources_report_typed_reasons_and_relink_to_ready() {
    for name in ["h264-bt2020.mp4", "h264-pq.mp4"] {
        let mut engine = engine();
        let reference = source(&mut engine, name);
        let mut renderer = renderer();
        renderer.enable_videos(&fixture_root(), None).unwrap();
        renderer
            .publish(capture_channels(&engine, &[reference], 8).unwrap(), routes(reference))
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let observed = status(&mut engine, reference);
            if observed
                .error
                .as_ref()
                .is_some_and(|error| error.kind == VideoSourceErrorKind::Unsupported)
            {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "{name}: expected unsupported source, observed {observed:?}"
            );
            std::thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(status(&mut engine, reference).state, VideoSourceState::Unavailable);
        assert!(status(&mut engine, reference).retrying);
        edit(
            &mut engine,
            MediaEdit::ConfigureVideoFile {
                reference,
                file: "h264.mp4".into(),
            },
        );
        renderer
            .publish(capture_channels(&engine, &[reference], 8).unwrap(), routes(reference))
            .unwrap();
        wait(|| status(&mut engine, reference).state == VideoSourceState::Ready);
        let retirement = renderer.video_retirement().unwrap();
        close(renderer);
        assert!(retirement.wait(Duration::from_secs(5)));
    }
}
