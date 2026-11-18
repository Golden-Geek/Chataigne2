use super::*;
use crate::media::protocol::MediaId;

#[test]
fn crop_relative_seek_and_relink_keep_the_authored_bounds() {
    let control = VideoControl::default();
    let media = MediaId(golden_core::node::NodeUuid(uuid::Uuid::new_v4()));
    control.prepare(seconds(10)).unwrap();
    let mut settings = VideoTransportSettings {
        start_ms: 2000.0,
        end_ms: Some(8000.0),
        ..Default::default()
    };
    control.configure(settings.clone(), false, None).unwrap();
    assert_eq!(control.source_duration_ms().unwrap(), Some(10_000.0));
    assert_eq!(control.snapshot(media).unwrap().length_ms, Some(6000.0));
    assert_eq!(control.bounded_position(9000.0).unwrap(), 6000.0);

    settings.position_ms = 6000.0;
    control.configure(settings.clone(), true, None).unwrap();
    let endpoint = control.sample().unwrap();
    assert_eq!(endpoint.position, seconds(6));
    assert_eq!(
        endpoint.channel_position(seconds(10)),
        seconds(8) - Duration::from_nanos(1)
    );

    settings.start_ms = 3000.0;
    settings.end_ms = Some(9000.0);
    control.configure(settings.clone(), false, None).unwrap();
    let shifted = control.sample().unwrap();
    assert_ne!(shifted.epoch, endpoint.epoch);
    assert_eq!(shifted.position, seconds(5));
    assert_eq!(shifted.channel_position(seconds(10)), seconds(8));

    control.prepare(Duration::from_millis(2500)).unwrap();
    let empty = control.snapshot(media).unwrap();
    assert_eq!(empty.length_ms, Some(0.0));
    assert!(!empty.ready && !empty.playing);
    assert_eq!((empty.settings.start_ms, empty.settings.end_ms), (3000.0, Some(9000.0)));

    control.prepare(seconds(7)).unwrap();
    let restored = control.snapshot(media).unwrap();
    assert!(restored.ready);
    assert_eq!(restored.length_ms, Some(4000.0));
    assert_eq!(restored.position_ms, 0.0);
    assert_eq!(restored.settings.end_ms, Some(9000.0));
}

#[test]
fn crop_overlap_uses_the_shortest_known_intersection() {
    let control = VideoControl::default();
    {
        let mut state = control.0.lock().unwrap();
        state.duration_changed(None, Some(seconds(10))).unwrap();
        state.duration_changed(None, Some(seconds(5))).unwrap();
    }
    let settings = VideoTransportSettings {
        start_ms: 2000.0,
        end_ms: Some(8000.0),
        position_ms: 5250.0,
        looping: true,
        auto_smooth_loop_ms: 4000.0,
        ..Default::default()
    };
    control.configure(settings, true, None).unwrap();
    let sample = control.sample().unwrap();
    let blend = sample.loop_blend().unwrap();
    assert_eq!(sample.duration, seconds(6));
    assert_eq!(sample.channel_position(seconds(5)), seconds(5));
    assert_eq!(blend.secondary_weight, 0.5);
    assert_eq!(
        sample.source_position(blend.secondary_position, seconds(5)),
        Duration::from_millis(2750)
    );

    control
        .0
        .lock()
        .unwrap()
        .duration_changed(Some(seconds(5)), Some(seconds(1)))
        .unwrap();
    assert!(control.sample().unwrap().timeline.is_none());
}

#[test]
fn matrix_start_and_stop_use_crop_zero() {
    let control = std::sync::Arc::new(VideoControl::default());
    control.prepare(seconds(20)).unwrap();
    let settings = VideoTransportSettings {
        start_ms: 4000.0,
        end_ms: Some(8000.0),
        position_ms: 3000.0,
        speed: 0.0,
        ..Default::default()
    };
    control.configure(settings, true, None).unwrap();
    let _use = control.activate_for_matrix(uuid::Uuid::new_v4()).unwrap();
    let started = control.sample().unwrap();
    assert_eq!(started.position, Duration::ZERO);
    assert_eq!(started.channel_position(seconds(20)), seconds(4));
    control.command(VideoPlaybackMode::Stopped).unwrap();
    let stopped = control.sample().unwrap();
    assert_eq!(stopped.position, Duration::ZERO);
    assert_eq!(stopped.channel_position(seconds(20)), seconds(4));
}
