use super::*;
use golden_io::WorkerNotifier;
use std::sync::mpsc::{self, Receiver};

fn notifications() -> (WorkerNotifier, Receiver<()>) {
    let (sender, receiver) = mpsc::sync_channel(1);
    let notifier = WorkerNotifier::new(move || {
        let _ = sender.try_send(());
    });
    (notifier, receiver)
}

fn sources(engine: &mut AppEngine, count: usize) -> Vec<ChannelReference> {
    (0..count)
        .map(|_| {
            let reference = source(engine, "prores.mov");
            edit(
                engine,
                MediaEdit::ConfigureMediaResolution {
                    media: reference.media,
                    size: SizePolicy::Fixed {
                        extent: Extent2D::new(16, 16).unwrap(),
                    },
                },
            );
            reference
        })
        .collect()
}

fn outputs(references: &[ChannelReference]) -> Vec<ChannelRoute> {
    references
        .iter()
        .enumerate()
        .map(|(index, channel)| ChannelRoute {
            output: OutputId::new(index as u64 + 1).unwrap(),
            channel: *channel,
        })
        .collect()
}

fn ready(renderer: &mut ChannelRenderer, references: &[ChannelReference]) {
    wait(|| {
        let mut outcomes = Vec::new();
        renderer.drain_outcomes(&mut outcomes, NonZeroUsize::new(16).unwrap());
        renderer.refresh().unwrap();
        references.iter().all(|reference| pixel(renderer, *reference).is_some())
    });
}

fn notified_ready(renderer: &mut ChannelRenderer, references: &[ChannelReference], notifications: &Receiver<()>) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let mut outcomes = Vec::new();
        renderer.drain_outcomes(&mut outcomes, NonZeroUsize::new(16).unwrap());
        renderer.refresh().unwrap();
        if references.iter().all(|reference| pixel(renderer, *reference).is_some()) {
            return;
        }
        notifications
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .expect("file progress must wake its owner without polling");
    }
}

#[test]
fn relinking_at_eight_file_workers_recovers_without_blocking_or_retiring_healthy_sources() {
    let mut engine = engine();
    let references = sources(&mut engine, 8);
    let mut renderer = renderer();
    let (notifier, notifications) = notifications();
    renderer.enable_videos(&fixture_root(), Some(notifier)).unwrap();
    renderer
        .publish(
            capture_channels(&engine, &references, 16).unwrap(),
            outputs(&references),
        )
        .unwrap();
    notified_ready(&mut renderer, &references, &notifications);
    let retirement = renderer.video_retirement().unwrap();
    let healthy: Vec<_> = references[1..]
        .iter()
        .map(|reference| renderer.videos.as_ref().unwrap().stream(*reference).unwrap())
        .collect();
    for cycle in 0..16 {
        let old = renderer.videos.as_ref().unwrap().stream(references[0]).unwrap();
        edit(
            &mut engine,
            MediaEdit::ConfigureVideoFile {
                reference: references[0],
                file: if cycle % 2 == 0 { "hevc.mp4" } else { "prores.mov" }.into(),
            },
        );
        renderer
            .publish(
                capture_channels(&engine, &references, 16).unwrap(),
                outputs(&references),
            )
            .expect("retiring workers must not make an otherwise admitted relink fail");
        notified_ready(&mut renderer, &references, &notifications);
        wait(|| old.sample().unwrap().image.is_none());
        assert_eq!(retirement.active_workers(), 8);
        assert!(healthy.iter().all(|stream| stream.sample().unwrap().image.is_some()));
    }
    close(renderer);
    assert!(retirement.wait(Duration::from_secs(5)));
    assert_eq!(retirement.active_workers(), 0);
    assert_eq!(retirement.retained_bytes(), 0);
}

#[test]
fn replacing_projects_at_capacity_wakes_pending_new_sources_and_withdraws_the_old_project() {
    let mut engine = engine();
    let mut references = sources(&mut engine, 8);
    let mut renderer = renderer();
    let (notifier, notifications) = notifications();
    renderer.enable_videos(&fixture_root(), Some(notifier)).unwrap();
    renderer
        .publish(
            capture_channels(&engine, &references, 16).unwrap(),
            outputs(&references),
        )
        .unwrap();
    notified_ready(&mut renderer, &references, &notifications);
    let retirement = renderer.video_retirement().unwrap();
    for _ in 0..4 {
        let old: Vec<_> = references
            .iter()
            .map(|reference| renderer.videos.as_ref().unwrap().stream(*reference).unwrap())
            .collect();
        engine = super::super::engine();
        references = sources(&mut engine, 8);
        renderer
            .publish(
                capture_channels(&engine, &references, 16).unwrap(),
                outputs(&references),
            )
            .expect("a new admitted project waits for actual old-worker retirement");
        notified_ready(&mut renderer, &references, &notifications);
        wait(|| old.iter().all(|stream| stream.sample().unwrap().image.is_none()));
        assert_eq!(retirement.active_workers(), 8);
    }
    close(renderer);
    assert!(retirement.wait(Duration::from_secs(5)));
    assert_eq!(retirement.active_workers(), 0);
    assert_eq!(retirement.retained_bytes(), 0);
}

#[test]
fn ninth_demanded_file_rejects_before_displacing_the_eight_admitted_sources() {
    let mut engine = engine();
    let references = sources(&mut engine, 9);
    let mut renderer = renderer();
    renderer.enable_videos(&fixture_root(), None).unwrap();
    renderer
        .publish(
            capture_channels(&engine, &references[..8], 16).unwrap(),
            outputs(&references[..8]),
        )
        .unwrap();
    ready(&mut renderer, &references[..8]);
    let request = renderer.latest_request();
    let retirement = renderer.video_retirement().unwrap();
    let error = renderer
        .publish(
            capture_channels(&engine, &references, 16).unwrap(),
            outputs(&references),
        )
        .unwrap_err();
    assert!(error.contains("eight admitted file workers"), "{error}");
    assert_eq!(renderer.latest_request(), request);
    assert_eq!(retirement.active_workers(), 8);
    assert!(
        references[..8]
            .iter()
            .all(|reference| pixel(&renderer, *reference).is_some())
    );
    close(renderer);
    assert!(retirement.wait(Duration::from_secs(5)));
    assert_eq!(retirement.retained_bytes(), 0);
}
