use super::super::super::tests::fixture::{body, frame, sample};
use super::*;
#[test]
fn nearest_body_selection_reference_fallback_and_units_are_preserved() {
    let mut prep = Preparer::new();
    let mut poll = frame(0);
    let mut closer = body(
        9,
        &[
            (KinectJoint::SpineBase, KinectVec3::new(0.0, 1.0, 2.0)),
            (KinectJoint::Head, KinectVec3::new(0.0, 2.0, 2.0)),
        ],
    );
    poll.frame.as_mut().unwrap().tracked_bodies.push(closer.clone());
    let row = prep.prepare(poll, KinectReferenceSpace::Torso).unwrap();
    assert_eq!(row.tracking_id, "9");
    assert_eq!(row.tracked_bodies, 2);
    assert_eq!(row.joints[KinectJoint::Head.index()], KinectVec3::new(0.0, 1.0, 0.0));
    closer.joints[KinectJoint::Head.index()].tracking_state = KinectTrackingState::NotTracked;
    assert_eq!(closer.joint(KinectJoint::Head).available_position(), KinectVec3::ZERO);
}
#[test]
fn sdk_time_body_space_loss_and_gap_bound_hand_speed_history() {
    let mut prep = Preparer::new();
    let moving = |time, x| {
        sample(
            time,
            body(
                7,
                &[
                    (KinectJoint::HandLeft, KinectVec3::new(x, 0.0, 1.0)),
                    (KinectJoint::HandRight, KinectVec3::new(x + 2.0, 0.0, 1.0)),
                ],
            ),
        )
    };
    assert_eq!(
        prep.prepare(moving(0, 0.0), KinectReferenceSpace::Absolute)
            .unwrap()
            .speed,
        KinectVec3::ZERO
    );
    assert_eq!(
        prep.prepare(moving(1_000_000, 1.0), KinectReferenceSpace::Absolute)
            .unwrap()
            .speed,
        KinectVec3::new(10.0, 0.0, 0.0)
    );
    for timestamp in [1_000_000, 0, 10_000_000] {
        assert_eq!(
            prep.prepare(moving(timestamp, 2.0), KinectReferenceSpace::Absolute)
                .unwrap()
                .speed,
            KinectVec3::ZERO
        );
    }
    assert_eq!(
        prep.prepare(moving(11_000_000, 3.0), KinectReferenceSpace::Head)
            .unwrap()
            .speed,
        KinectVec3::ZERO
    );
    let lost = prep
        .prepare(
            KinectRuntimePoll {
                sensor_available: false,
                frame: None,
            },
            KinectReferenceSpace::Head,
        )
        .unwrap();
    assert_eq!(lost.last_event.as_deref(), Some("Lost tracked body"));
    assert_eq!(lost.tracking_id, "");
    assert_eq!(
        prep.prepare(moving(12_000_000, 4.0), KinectReferenceSpace::Head)
            .unwrap()
            .speed,
        KinectVec3::ZERO
    );
}
#[test]
fn worker_applies_existing_noise_threshold_without_corrupting_raw_history() {
    let mut prep = Preparer::new();
    let mut input = frame(0);
    let first = prep.prepare(input.clone(), KinectReferenceSpace::Absolute).unwrap();
    input.frame.as_mut().unwrap().tracked_bodies[0].joints[KinectJoint::Head.index()]
        .position
        .x += 0.000_000_5;
    let tiny = prep.prepare(input.clone(), KinectReferenceSpace::Absolute).unwrap();
    assert_eq!(tiny.joints, first.joints);
    assert_eq!(tiny.last_event, first.last_event);
    input.frame.as_mut().unwrap().tracked_bodies[0].joints[KinectJoint::Head.index()]
        .position
        .x += 0.000_001;
    let changed = prep.prepare(input, KinectReferenceSpace::Absolute).unwrap();
    assert_ne!(changed.joints, first.joints);
}
#[test]
fn six_body_backing_finite_identity_and_derived_bounds_reject_before_projection() {
    let mut oversized = frame(0);
    oversized.frame.as_mut().unwrap().tracked_bodies = Vec::with_capacity(7);
    assert!(validate(&oversized).is_err());
    let mut duplicate = frame(0);
    let body = duplicate.frame.as_ref().unwrap().tracked_bodies[0].clone();
    duplicate.frame.as_mut().unwrap().tracked_bodies.push(body);
    assert!(validate(&duplicate).is_err());
    let mut bad = frame(0);
    bad.frame.as_mut().unwrap().tracked_bodies[0].joints[0].position.x = f64::NAN;
    assert!(validate(&bad).is_err());
    let mut extreme = frame(0);
    extreme.frame.as_mut().unwrap().tracked_bodies[0].joints[KinectJoint::HandLeft.index()]
        .position
        .x = f64::MAX;
    assert!(Preparer::new()
        .prepare(extreme, KinectReferenceSpace::Absolute)
        .is_err());
}
