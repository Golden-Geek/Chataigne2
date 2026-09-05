use std::time::Duration;

use golden_core::{
    edit::Edit,
    node::{DeclaredUserItemNode, Folder, Node, NodeId},
    parameter::{ParamValue, ParameterEventBehaviour},
    process_ctx::ExecutionPhase,
    script::ScriptSource,
};

use super::{
    kinect2_runtime::{
        KinectBodySnapshot, KinectFrameSnapshot, KinectJoint, KinectJointSample,
        KinectTrackingState, KinectVec3,
    },
    Kinect2Module, KINECT2_SPACE_HEAD, KINECT2_SPACE_TORSO,
};

#[test]
fn kinect2_module_is_a_module_item() {
    assert_eq!(
        <Kinect2Module as DeclaredUserItemNode>::ITEM_KIND,
        crate::app::module::MODULE_ITEM_KIND
    );
    assert!(crate::app::declared_user_item_type_matches(
        Kinect2Module::NODE_TYPE,
        crate::app::module::MODULE_ITEM_KIND
    ));
}

#[test]
fn reference_space_changes_joint_output_origin() {
    let (mut engine, module_id) = create_kinect_module();
    let frame = test_frame(
        1,
        body_with_positions(
            42,
            &[
                (KinectJoint::SpineMid, KinectVec3::new(1.0, 2.0, 3.0)),
                (KinectJoint::Head, KinectVec3::new(1.2, 2.5, 3.1)),
                (KinectJoint::HandLeft, KinectVec3::new(0.9, 2.2, 3.05)),
            ],
        ),
    );

    enqueue_frame(&mut engine, module_id, frame.clone());
    run_kinect_tick(&mut engine);
    assert_vec3_close(
        vec3_param_value(&engine, module_id, "values/joints/head"),
        (1.2, 2.5, 3.1),
        "absolute reference space should expose raw joint positions",
    );

    set_enum_param(&mut engine, module_id, "parameters/reference_space", KINECT2_SPACE_TORSO);
    enqueue_frame(&mut engine, module_id, frame.clone());
    run_kinect_tick(&mut engine);
    assert_vec3_close(
        vec3_param_value(&engine, module_id, "values/joints/head"),
        (0.2, 0.5, 0.1),
        "torso reference space should offset positions by the spine midpoint",
    );

    set_enum_param(&mut engine, module_id, "parameters/reference_space", KINECT2_SPACE_HEAD);
    enqueue_frame(&mut engine, module_id, frame);
    run_kinect_tick(&mut engine);
    assert_vec3_close(
        vec3_param_value(&engine, module_id, "values/joints/hand_left"),
        (-0.3, -0.3, -0.05),
        "head reference space should offset positions by the head joint",
    );
}

#[test]
fn hand_metrics_compute_distance_rotation_and_speed() {
    let (mut engine, module_id) = create_kinect_module();

    enqueue_frame(
        &mut engine,
        module_id,
        test_frame(
            0,
            body_with_positions(
                7,
                &[
                    (KinectJoint::SpineMid, KinectVec3::new(0.0, 1.0, 2.0)),
                    (KinectJoint::HandLeft, KinectVec3::new(0.0, 0.0, 1.0)),
                    (KinectJoint::HandRight, KinectVec3::new(2.0, 0.0, 1.0)),
                ],
            ),
        ),
    );
    run_kinect_tick(&mut engine);

    assert_eq!(
        float_param_value(&engine, module_id, "values/hands/hands_distance"),
        Some(2.0)
    );
    assert_eq!(
        vec2_param_value(&engine, module_id, "values/hands/hands_rotation"),
        Some((1.0, 0.0))
    );
    assert_eq!(
        vec3_param_value(&engine, module_id, "values/hands/hands_speed"),
        Some((0.0, 0.0, 0.0))
    );

    enqueue_frame(
        &mut engine,
        module_id,
        test_frame(
            1_000_000,
            body_with_positions(
                7,
                &[
                    (KinectJoint::SpineMid, KinectVec3::new(0.0, 1.0, 2.0)),
                    (KinectJoint::HandLeft, KinectVec3::new(1.0, 0.0, 1.0)),
                    (KinectJoint::HandRight, KinectVec3::new(3.0, 0.0, 1.0)),
                ],
            ),
        ),
    );
    run_kinect_tick(&mut engine);

    assert_eq!(
        vec3_param_value(&engine, module_id, "values/hands/hands_speed"),
        Some((10.0, 0.0, 0.0))
    );
}

#[test]
fn kinect2_script_template_resolves_to_inline_module_template() {
    let config = crate::app::module::script_api::module_script_config(Kinect2Module::NODE_TYPE);
    let ScriptSource::Inline { text: source } = config.source else {
        panic!("kinect2 module script template should resolve to inline source");
    };

    assert!(source.contains("function moduleConnectionChanged"));
    assert!(!source.contains("function noteOnReceived"));
}

fn create_kinect_module() -> (crate::app::AppEngine, NodeId) {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    let mut module = Kinect2Module::create();
    module.disable_runtime_for_test();
    engine.add_node(module.into(), None);
    engine.apply_edits().expect("kinect2 module should attach");
    for _ in 0..12 {
        engine
            .apply_edits()
            .expect("kinect2 module defaults should materialize");
    }
    engine
        .resolve()
        .expect("kinect2 module schedule should resolve");

    let module_id = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("kinect2 module should be attached under root");

    (engine, module_id)
}

fn enqueue_frame(engine: &mut crate::app::AppEngine, module_id: NodeId, frame: KinectFrameSnapshot) {
    let crate::app::AppNode::Kinect2Module(module) =
        engine.nodes.get_mut(module_id).expect("kinect2 module should exist")
    else {
        panic!("expected Kinect2Module node");
    };
    module.enqueue_frame_for_test(frame);
}

fn run_kinect_tick(engine: &mut crate::app::AppEngine) {
    engine
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .expect("kinect2 inbox should dispatch");
    engine
        .run_tick(Duration::from_millis(40))
        .expect("kinect2 tick should run");
    engine
        .apply_edits()
        .expect("kinect2 tick edits should apply");
    engine
        .resolve()
        .expect("kinect2 schedule should resolve after tick");
}

fn test_frame(timestamp_100ns: u64, body: KinectBodySnapshot) -> KinectFrameSnapshot {
    KinectFrameSnapshot {
        sensor_available: true,
        timestamp_100ns,
        tracked_bodies: vec![body],
    }
}

fn body_with_positions(tracking_id: u64, joints: &[(KinectJoint, KinectVec3)]) -> KinectBodySnapshot {
    let mut samples = [KinectJointSample::default(); KinectJoint::COUNT];
    for (joint, position) in joints {
        samples[joint.index()] = KinectJointSample {
            position: *position,
            tracking_state: KinectTrackingState::Tracked,
        };
    }

    KinectBodySnapshot {
        tracking_id,
        joints: samples,
    }
}

fn set_enum_param(engine: &mut crate::app::AppEngine, start: NodeId, path: &str, value: &str) {
    let Some(param_id) = find_path(engine, start, path) else {
        panic!("parameter {path} should exist");
    };
    engine.edits.push(Edit::SetParam {
        node: param_id,
        value: ParamValue::Enum(value.to_string()),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().expect("enum parameter edit should apply");
}

fn find_path(engine: &crate::app::AppEngine, start: NodeId, path: &str) -> Option<NodeId> {
    let mut current = start;
    for segment in path.split('/') {
        if segment.trim().is_empty() {
            continue;
        }
        current = find_child_by_key(engine, current, segment)?;
    }
    Some(current)
}

fn find_child_by_key(engine: &crate::app::AppEngine, parent: NodeId, key: &str) -> Option<NodeId> {
    let mut current = engine.nodes.get(parent)?.node_data().first_child;
    while let Some(child_id) = current {
        let child = engine.nodes.get(child_id)?;
        if node_key_matches(child.node_data(), key) {
            return Some(child_id);
        }
        current = child.node_data().next_sibling;
    }
    None
}

fn node_key_matches(node: &golden_core::node::NodeData, key: &str) -> bool {
    node.meta.decl_id.0 == key
        || node.meta.decl_id.0.rsplit('/').next() == Some(key)
        || node.meta.short_name == key
        || node.meta.label == key
}

fn param_value(engine: &crate::app::AppEngine, start: NodeId, path: &str) -> Option<ParamValue> {
    let param_id = find_path(engine, start, path)?;
    engine
        .nodes
        .get(param_id)?
        .engine_param_snapshot()
        .map(|snapshot| snapshot.value)
}

fn float_param_value(engine: &crate::app::AppEngine, start: NodeId, path: &str) -> Option<f64> {
    match param_value(engine, start, path)? {
        ParamValue::Float(value) => Some(value),
        ParamValue::Int(value) => Some(f64::from(value)),
        _ => None,
    }
}

fn vec2_param_value(engine: &crate::app::AppEngine, start: NodeId, path: &str) -> Option<(f64, f64)> {
    match param_value(engine, start, path)? {
        ParamValue::Vec2(x, y) => Some((x, y)),
        _ => None,
    }
}

fn vec3_param_value(engine: &crate::app::AppEngine, start: NodeId, path: &str) -> Option<(f64, f64, f64)> {
    match param_value(engine, start, path)? {
        ParamValue::Vec3(x, y, z) => Some((x, y, z)),
        _ => None,
    }
}

fn assert_vec3_close(actual: Option<(f64, f64, f64)>, expected: (f64, f64, f64), context: &str) {
    let Some((x, y, z)) = actual else {
        panic!("{context}: expected a Vec3 value");
    };
    assert!(
        (x - expected.0).abs() <= 0.000_001
            && (y - expected.1).abs() <= 0.000_001
            && (z - expected.2).abs() <= 0.000_001,
        "{context}: expected {:?}, got {:?}",
        expected,
        (x, y, z)
    );
}
