use super::*;
fn frame(count: usize, x: f64) -> UltraleapRuntimePoll {
    UltraleapRuntimePoll {
        service_connected: true,
        connected_devices: 1,
        frame: Some(UltraleapFrameSnapshot {
            hand_count: count,
            left: UltraleapHandSnapshot {
                active: count > 0,
                palm_position: UltraleapVec3::new(x, 0.0, 0.0),
                ..Default::default()
            },
            ..Default::default()
        }),
        last_event: None,
    }
}
#[test]
fn consecutive_frames_coalesce_without_crossing_status_or_tracking_boundaries() {
    let status = UltraleapRuntimePoll {
        service_connected: true,
        connected_devices: 1,
        last_event: Some("device".into()),
        ..Default::default()
    };
    let rows = coalesce(vec![
        frame(1, 1.0),
        frame(1, 2.0),
        status,
        frame(1, 3.0),
        frame(0, 0.0),
        frame(1, 4.0),
    ])
    .unwrap();
    assert_eq!(rows.len(), 5);
    assert_eq!(rows[0].frame.unwrap().left.palm_position.x, 2.0);
    assert_eq!(rows[1].last_event.as_deref(), Some("device"));
    let mut preparer = Preparer::default();
    let messages = rows
        .into_iter()
        .map(|row| preparer.prepare(row).unwrap().poll.last_event)
        .collect::<Vec<_>>();
    assert_eq!(
        messages,
        vec![
            Some("Tracking 1 hand".into()),
            Some("device".into()),
            None,
            Some("Lost Ultraleap tracking".into()),
            Some("Tracking 1 hand".into())
        ]
    );
}
#[test]
fn bounds_reject_spare_storage_nonfinite_values_and_large_native_counts() {
    let mut row = frame(1, 0.0);
    row.last_event = Some(String::with_capacity(1025));
    assert!(Preparer::default().prepare(row).is_err());
    assert!(Preparer::default().prepare(frame(3, 0.0)).is_err());
    assert!(Preparer::default().prepare(frame(1, f64::NAN)).is_err());
    let rows = Vec::with_capacity(17);
    assert!(coalesce(rows).is_err());
    let row = UltraleapRuntimePoll {
        connected_devices: 65,
        ..Default::default()
    };
    assert!(Preparer::default().prepare(row).is_err());
}
#[test]
fn distance_and_loss_reset_are_prepared_before_actor_projection() {
    let mut row = frame(2, 0.1);
    row.frame.as_mut().unwrap().right = UltraleapHandSnapshot {
        active: true,
        palm_position: UltraleapVec3::new(0.4, 0.0, 0.0),
        ..Default::default()
    };
    let mut preparer = Preparer::default();
    let prepared = preparer.prepare(row).unwrap();
    assert!((prepared.distance - 0.3).abs() < 1e-9);
    assert_eq!(prepared.poll.last_event.as_deref(), Some("Tracking 2 hands"));
    let lost = preparer
        .prepare(UltraleapRuntimePoll {
            last_event: Some("lost service".into()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(lost.poll.last_event.as_deref(), Some("lost service"));
    assert!(lost.poll.frame.is_none());
    assert_eq!(
        preparer.prepare(frame(1, 0.0)).unwrap().poll.last_event.as_deref(),
        Some("Tracking 1 hand")
    );
}
