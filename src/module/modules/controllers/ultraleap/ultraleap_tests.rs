use std::time::Duration;

use golden_core::{
    node::{DeclaredUserItemNode, Folder, Node, NodeId},
    parameter::ParamValue,
    process_ctx::ExecutionPhase,
    script::ScriptSource,
};

use super::{
    ultraleap_runtime::{UltraleapFrameSnapshot, UltraleapHandSnapshot, UltraleapRuntimePoll, UltraleapVec3},
    UltraleapModule,
};

#[test]
fn ultraleap_module_is_a_module_item() {
    assert_eq!(
        <UltraleapModule as DeclaredUserItemNode>::ITEM_KIND,
        crate::app::module::MODULE_ITEM_KIND
    );
    assert!(crate::app::declared_user_item_type_matches(
        UltraleapModule::NODE_TYPE,
        crate::app::module::MODULE_ITEM_KIND
    ));
}

#[test]
fn ultraleap_frame_updates_hand_values_and_distance() {
    let (mut engine, module_id) = create_ultraleap_module();

    enqueue_poll(
        &mut engine,
        module_id,
        UltraleapRuntimePoll {
            service_connected: true,
            connected_devices: 1,
            frame: Some(UltraleapFrameSnapshot {
                hand_count: 2,
                left: hand_snapshot(
                    0.65,
                    0.85,
                    0.014,
                    (true, true, false, false, false),
                    UltraleapVec3::new(-0.04, 0.12, 0.015),
                    UltraleapVec3::new(-0.038, 0.118, 0.014),
                    UltraleapVec3::new(0.12, 0.0, 0.0),
                    UltraleapVec3::new(0.0, 0.0, -1.0),
                    UltraleapVec3::new(0.0, 1.0, 0.0),
                ),
                right: hand_snapshot(
                    0.10,
                    0.20,
                    0.042,
                    (true, true, true, true, true),
                    UltraleapVec3::new(0.06, 0.12, 0.015),
                    UltraleapVec3::new(0.058, 0.118, 0.014),
                    UltraleapVec3::new(-0.12, 0.0, 0.0),
                    UltraleapVec3::new(0.0, 0.0, -1.0),
                    UltraleapVec3::new(0.0, 1.0, 0.0),
                ),
            }),
            last_event: Some("Ultraleap device connected".to_string()),
        },
    );

    run_ultraleap_tick(&mut engine);

    assert_eq!(
        bool_param_value(&engine, module_id, "values/info/service_connected"),
        Some(true)
    );
    assert_eq!(
        bool_param_value(&engine, module_id, "values/info/device_available"),
        Some(true)
    );
    assert_eq!(
        bool_param_value(&engine, module_id, "values/info/tracking_active"),
        Some(true)
    );
    assert_eq!(
        int_param_value(&engine, module_id, "values/info/connected_devices"),
        Some(1)
    );
    assert_eq!(
        int_param_value(&engine, module_id, "values/info/visible_hands"),
        Some(2)
    );
    assert_eq!(
        string_param_value(&engine, module_id, "values/info/last_event"),
        Some("Tracking 2 hands".to_string())
    );

    assert_vec3_close(
        vec3_param_value(&engine, module_id, "values/left_hand/left_palm_position"),
        (-0.04, 0.12, 0.015),
        "left palm position should follow the tracking frame",
    );
    assert_vec3_close(
        vec3_param_value(&engine, module_id, "values/right_hand/right_palm_velocity"),
        (-0.12, 0.0, 0.0),
        "right palm velocity should follow the tracking frame",
    );
    assert_eq!(
        float_param_value(&engine, module_id, "values/metrics/hands_distance"),
        Some(0.1)
    );
    assert_eq!(
        float_param_value(&engine, module_id, "values/left_hand/left_grab_strength"),
        Some(0.65)
    );
    assert_eq!(
        float_param_value(&engine, module_id, "values/left_hand/left_pinch_strength"),
        Some(0.85)
    );
    assert_eq!(
        float_param_value(&engine, module_id, "values/left_hand/left_pinch_distance"),
        Some(0.014)
    );
    assert_eq!(
        bool_param_value(&engine, module_id, "values/left_hand/left_thumb_extended"),
        Some(true)
    );
    assert_eq!(
        bool_param_value(&engine, module_id, "values/left_hand/left_middle_extended"),
        Some(false)
    );
    assert_eq!(
        bool_param_value(&engine, module_id, "values/right_hand/right_pinky_extended"),
        Some(true)
    );
}

#[test]
fn ultraleap_disconnect_resets_tracking_outputs() {
    let (mut engine, module_id) = create_ultraleap_module();

    enqueue_poll(
        &mut engine,
        module_id,
        UltraleapRuntimePoll {
            service_connected: true,
            connected_devices: 1,
            frame: Some(UltraleapFrameSnapshot {
                hand_count: 1,
                left: hand_snapshot(
                    0.25,
                    0.5,
                    0.012,
                    (true, false, false, false, false),
                    UltraleapVec3::new(0.001, 0.002, 0.003),
                    UltraleapVec3::new(0.001, 0.002, 0.003),
                    UltraleapVec3::ZERO,
                    UltraleapVec3::new(0.0, 0.0, -1.0),
                    UltraleapVec3::new(0.0, 1.0, 0.0),
                ),
                right: UltraleapHandSnapshot::default(),
            }),
            last_event: None,
        },
    );
    run_ultraleap_tick(&mut engine);

    enqueue_poll(
        &mut engine,
        module_id,
        UltraleapRuntimePoll {
            service_connected: true,
            connected_devices: 0,
            frame: None,
            last_event: Some("Ultraleap device disconnected".to_string()),
        },
    );
    run_ultraleap_tick(&mut engine);

    assert_eq!(
        bool_param_value(&engine, module_id, "values/info/device_available"),
        Some(false)
    );
    assert_eq!(
        bool_param_value(&engine, module_id, "values/info/tracking_active"),
        Some(false)
    );
    assert_eq!(
        int_param_value(&engine, module_id, "values/info/visible_hands"),
        Some(0)
    );
    assert_eq!(
        bool_param_value(&engine, module_id, "values/left_hand/left_active"),
        Some(false)
    );
    assert_eq!(
        float_param_value(&engine, module_id, "values/left_hand/left_grab_strength"),
        Some(0.0)
    );
    assert_eq!(
        bool_param_value(&engine, module_id, "values/left_hand/left_thumb_extended"),
        Some(false)
    );
    assert_eq!(
        float_param_value(&engine, module_id, "values/metrics/hands_distance"),
        Some(0.0)
    );
    assert_eq!(
        string_param_value(&engine, module_id, "values/info/last_event"),
        Some("Ultraleap device disconnected".to_string())
    );
}

#[test]
fn ultraleap_script_template_resolves_to_inline_module_template() {
    let config = crate::app::module::script_api::module_script_config(UltraleapModule::NODE_TYPE);
    let ScriptSource::Inline(source) = config.source else {
        panic!("ultraleap module script template should resolve to inline source");
    };

    assert!(source.contains("function moduleConnectionChanged"));
    assert!(!source.contains("function noteOnReceived"));
}

fn create_ultraleap_module() -> (crate::app::AppEngine, NodeId) {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    let mut module = UltraleapModule::create();
    module.disable_runtime_for_test();
    engine.add_node(module.into(), None);
    engine.apply_edits().expect("ultraleap module should attach");
    for _ in 0..12 {
        engine
            .apply_edits()
            .expect("ultraleap module defaults should materialize");
    }
    engine.resolve().expect("ultraleap module schedule should resolve");

    let module_id = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("ultraleap module should be attached under root");

    (engine, module_id)
}

fn enqueue_poll(engine: &mut crate::app::AppEngine, module_id: NodeId, poll: UltraleapRuntimePoll) {
    let crate::app::AppNode::UltraleapModule(module) =
        engine.nodes.get_mut(module_id).expect("ultraleap module should exist")
    else {
        panic!("expected UltraleapModule node");
    };
    module.enqueue_poll_for_test(poll);
}

fn run_ultraleap_tick(engine: &mut crate::app::AppEngine) {
    engine
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .expect("ultraleap inbox should dispatch");
    engine
        .run_tick(Duration::from_millis(20))
        .expect("ultraleap tick should run");
    engine.apply_edits().expect("ultraleap tick edits should apply");
    engine.resolve().expect("ultraleap schedule should resolve after tick");
}

fn hand_snapshot(
    grab_strength: f64,
    pinch_strength: f64,
    pinch_distance: f64,
    extended: (bool, bool, bool, bool, bool),
    position: UltraleapVec3,
    stabilized_position: UltraleapVec3,
    velocity: UltraleapVec3,
    direction: UltraleapVec3,
    normal: UltraleapVec3,
) -> UltraleapHandSnapshot {
    UltraleapHandSnapshot {
        active: true,
        grab_strength,
        pinch_strength,
        pinch_distance,
        thumb_extended: extended.0,
        index_extended: extended.1,
        middle_extended: extended.2,
        ring_extended: extended.3,
        pinky_extended: extended.4,
        palm_position: position,
        palm_stabilized_position: stabilized_position,
        palm_velocity: velocity,
        palm_direction: direction,
        palm_normal: normal,
    }
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
        .map(|snapshot| snapshot.value.clone())
}

fn bool_param_value(engine: &crate::app::AppEngine, start: NodeId, path: &str) -> Option<bool> {
    match param_value(engine, start, path)? {
        ParamValue::Bool(value) => Some(value),
        other => panic!("expected bool at {path}, got {other:?}"),
    }
}

fn int_param_value(engine: &crate::app::AppEngine, start: NodeId, path: &str) -> Option<i32> {
    match param_value(engine, start, path)? {
        ParamValue::Int(value) => Some(value),
        other => panic!("expected int at {path}, got {other:?}"),
    }
}

fn float_param_value(engine: &crate::app::AppEngine, start: NodeId, path: &str) -> Option<f64> {
    match param_value(engine, start, path)? {
        ParamValue::Float(value) => Some(value),
        other => panic!("expected float at {path}, got {other:?}"),
    }
}

fn string_param_value(engine: &crate::app::AppEngine, start: NodeId, path: &str) -> Option<String> {
    match param_value(engine, start, path)? {
        ParamValue::Str(value) => Some(value),
        other => panic!("expected string at {path}, got {other:?}"),
    }
}

fn vec3_param_value(engine: &crate::app::AppEngine, start: NodeId, path: &str) -> Option<(f64, f64, f64)> {
    match param_value(engine, start, path)? {
        ParamValue::Vec3(x, y, z) => Some((x, y, z)),
        other => panic!("expected vec3 at {path}, got {other:?}"),
    }
}

fn assert_vec3_close(actual: Option<(f64, f64, f64)>, expected: (f64, f64, f64), message: &str) {
    let Some(actual) = actual else {
        panic!("{message}: missing vec3 value");
    };
    assert!(
        (actual.0 - expected.0).abs() <= 0.000_001,
        "{message}: x {:?} != {:?}",
        actual.0,
        expected.0
    );
    assert!(
        (actual.1 - expected.1).abs() <= 0.000_001,
        "{message}: y {:?} != {:?}",
        actual.1,
        expected.1
    );
    assert!(
        (actual.2 - expected.2).abs() <= 0.000_001,
        "{message}: z {:?} != {:?}",
        actual.2,
        expected.2
    );
}
