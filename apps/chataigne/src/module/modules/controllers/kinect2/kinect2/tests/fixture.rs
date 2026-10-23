use super::super::kinect2_runtime::*;
pub(in super::super) fn body(tracking_id: u64, positions: &[(KinectJoint, KinectVec3)]) -> KinectBodySnapshot {
    let mut joints = [KinectJointSample::default(); KinectJoint::COUNT];
    for (joint, position) in positions {
        joints[joint.index()] = KinectJointSample {
            position: *position,
            tracking_state: KinectTrackingState::Tracked,
        };
    }
    KinectBodySnapshot { tracking_id, joints }
}
pub(in super::super) fn sample(timestamp_100ns: u64, body: KinectBodySnapshot) -> KinectRuntimePoll {
    KinectRuntimePoll {
        sensor_available: true,
        frame: Some(KinectFrameSnapshot {
            sensor_available: true,
            timestamp_100ns,
            tracked_bodies: vec![body],
        }),
    }
}
pub(in super::super) fn frame(index: usize) -> KinectRuntimePoll {
    sample(
        index as u64 * 1_000_000,
        body(
            index as u64 + 1,
            &[
                (KinectJoint::SpineMid, KinectVec3::new(1.0, 2.0, 3.0)),
                (KinectJoint::Head, KinectVec3::new(1.2, 2.5, 3.1)),
                (KinectJoint::HandLeft, KinectVec3::new(0.9, 2.2, 3.05)),
                (KinectJoint::HandRight, KinectVec3::new(1.1, 2.2, 3.05)),
            ],
        ),
    )
}
