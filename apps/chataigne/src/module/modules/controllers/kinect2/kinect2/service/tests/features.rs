use super::*;
pub(in super::super::super) fn reference_space_changes_joint_output_origin() {
    let (mut engine, id, control, native) = create();
    emit(&mut engine, id, &control, frame(0));
    let target = path(&engine, id, "values/joints/head");
    assert_vec(value(&engine, target), (1.2, 2.5, 3.1));
    set_space(&mut engine, id, KINECT2_SPACE_TORSO);
    quiet(&mut engine);
    emit(&mut engine, id, &control, frame(0));
    assert_vec(value(&engine, target), (0.2, 0.5, 0.1));
    set_space(&mut engine, id, KINECT2_SPACE_HEAD);
    quiet(&mut engine);
    emit(&mut engine, id, &control, frame(0));
    assert_vec(
        value(&engine, path(&engine, id, "values/joints/hand_left")),
        (-0.3, -0.3, -0.05),
    );
    assert_eq!(control.opens.load(Ordering::Acquire), 1);
    finish(&mut engine, id, &native);
}
pub(in super::super::super) fn hand_metrics_compute_distance_rotation_and_speed() {
    let (mut engine, id, control, native) = create();
    let moving = |time, x| {
        sample(
            time,
            body(
                7,
                &[
                    (KinectJoint::SpineMid, KinectVec3::new(0.0, 1.0, 2.0)),
                    (KinectJoint::HandLeft, KinectVec3::new(x, 0.0, 1.0)),
                    (KinectJoint::HandRight, KinectVec3::new(x + 2.0, 0.0, 1.0)),
                ],
            ),
        )
    };
    emit(&mut engine, id, &control, moving(0, 0.0));
    assert_eq!(
        value(&engine, path(&engine, id, "values/hands/hands_distance")),
        ParamValue::Float(2.0)
    );
    assert_eq!(
        value(&engine, path(&engine, id, "values/hands/hands_rotation")),
        ParamValue::Vec2(1.0, 0.0)
    );
    assert_vec(
        value(&engine, path(&engine, id, "values/hands/hands_speed")),
        (0.0, 0.0, 0.0),
    );
    emit(&mut engine, id, &control, moving(1_000_000, 1.0));
    assert_vec(
        value(&engine, path(&engine, id, "values/hands/hands_speed")),
        (10.0, 0.0, 0.0),
    );
    finish(&mut engine, id, &native);
}
fn assert_vec(actual: ParamValue, expected: (f64, f64, f64)) {
    let ParamValue::Vec3(x, y, z) = actual else {
        panic!("Kinect Vec3")
    };
    assert!((x - expected.0).abs() < 1e-6 && (y - expected.1).abs() < 1e-6 && (z - expected.2).abs() < 1e-6);
}
#[test]
fn eight_reference_changes_reproject_held_raw_frame_and_keep_one_owner() {
    let (mut engine, id, control, native) = create();
    emit(&mut engine, id, &control, frame(0));
    let target = path(&engine, id, "values/joints/head");
    for i in 0..8 {
        let torso = i % 2 == 0;
        set_space(
            &mut engine,
            id,
            if torso {
                KINECT2_SPACE_TORSO
            } else {
                KINECT2_SPACE_ABSOLUTE
            },
        );
        until(|| {
            tick(&mut engine);
            matches!(value(&engine,target),ParamValue::Vec3(x,_,_) if (x-if torso {0.2}else{1.2}).abs()<1e-6)
        });
        quiet(&mut engine);
        assert_eq!(control.opens.load(Ordering::Acquire), 1);
        assert_eq!(native.metrics().active, 1);
    }
    finish(&mut engine, id, &native);
}
#[test]
fn sensor_loss_clears_values_and_speed_history_without_reopening_the_sensor() {
    let (mut engine, id, control, native) = create();
    emit(&mut engine, id, &control, frame(0));
    emit(
        &mut engine,
        id,
        &control,
        KinectRuntimePoll {
            sensor_available: false,
            frame: None,
        },
    );
    assert_eq!(
        value(&engine, path(&engine, id, "connection/connected")),
        ParamValue::Bool(false)
    );
    assert_eq!(
        value(&engine, path(&engine, id, "values/info/tracking_id")),
        ParamValue::Str(String::new())
    );
    assert_vec(value(&engine, path(&engine, id, "values/joints/head")), (0.0, 0.0, 0.0));
    emit(&mut engine, id, &control, frame(0));
    assert_eq!(
        value(&engine, path(&engine, id, "connection/connected")),
        ParamValue::Bool(true)
    );
    assert_vec(
        value(&engine, path(&engine, id, "values/hands/hands_speed")),
        (0.0, 0.0, 0.0),
    );
    assert_eq!(control.opens.load(Ordering::Acquire), 1);
    finish(&mut engine, id, &native);
}
