use super::*;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
#[repr(C)]
struct FakeBody {
    body: IBody,
    releases: Arc<AtomicUsize>,
}
unsafe extern "system" fn release(body: *mut IBody) -> ULong {
    let fake = unsafe { &*body.cast::<FakeBody>() };
    fake.releases.fetch_add(1, Ordering::AcqRel);
    0
}
unsafe extern "system" fn failed_tracking(_body: *mut IBody, _value: *mut Boolean) -> HResult {
    -1
}
const BODY: IBodyVtbl = IBodyVtbl {
    query_interface: None,
    add_ref: None,
    release: Some(release),
    get_joints: None,
    get_joint_orientations: 0,
    get_engaged: 0,
    get_expression_detection_results: 0,
    get_activity_detection_results: 0,
    get_appearance_detection_results: 0,
    get_hand_left_state: 0,
    get_hand_left_confidence: 0,
    get_hand_right_state: 0,
    get_hand_right_confidence: 0,
    get_clipped_edges: 0,
    get_tracking_id: None,
    get_is_tracked: Some(failed_tracking),
    get_is_restricted: 0,
    get_lean: 0,
    get_lean_tracking_state: 0,
};
#[test]
fn inspection_failure_releases_every_returned_body_reference() {
    let counts: [Arc<AtomicUsize>; BODY_COUNT] = std::array::from_fn(|_| Arc::new(AtomicUsize::new(0)));
    let mut fake: [Box<FakeBody>; BODY_COUNT] = std::array::from_fn(|i| {
        Box::new(FakeBody {
            body: IBody { lp_vtbl: &BODY },
            releases: counts[i].clone(),
        })
    });
    let bodies = std::array::from_fn(|i| BodyHandle { ptr: &mut fake[i].body });
    assert!(collect_bodies(bodies).is_err());
    assert!(counts.iter().all(|count| count.load(Ordering::Acquire) == 1));
}
fn joints() -> [JointRaw; KinectJoint::COUNT] {
    std::array::from_fn(|i| JointRaw {
        joint_type: i as i32,
        position: CameraSpacePointRaw { x: 1.0, y: 2.0, z: 3.0 },
        tracking_state: 2,
    })
}
#[test]
fn native_joint_decode_preserves_units_and_rejects_invalid_fixed_joint_data() {
    let decoded = decode_joints(joints()).unwrap();
    assert_eq!(
        decoded[KinectJoint::Head.index()].position,
        KinectVec3::new(1.0, 2.0, 3.0)
    );
    for variant in 0..4 {
        let mut input = joints();
        match variant {
            0 => input[0].joint_type = 25,
            1 => input[1].joint_type = 0,
            2 => input[0].position.z = f32::NAN,
            _ => input[0].tracking_state = 3,
        };
        assert!(decode_joints(input).is_err());
    }
}
#[test]
fn signed_sdk_timestamps_reject_negative_values_before_publication() {
    assert_eq!(frame_timestamp(10_000_000).unwrap(), 10_000_000);
    assert!(frame_timestamp(-1).is_err());
}
