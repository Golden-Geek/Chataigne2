//! Reproduce actual app Parameter declarations through Golden's animation control path.
use super::*;
use golden_core::{
    node::{Node, NodeId},
    parameter::{ParamValue, ParameterControlMode, ParameterControlSpec, ParameterControlState},
};
use std::time::Duration;

fn animate(engine: &mut AppEngine, parameter: NodeId) {
    engine
        .set_param_control_state(
            parameter,
            ParameterControlState::new(ParameterControlMode::Animation, ParameterControlSpec::Animation),
        )
        .unwrap();
}

#[test]
fn surface_opacity_animation_clamps_to_authored_range() {
    let mut engine = fixture();
    let (_, surface) = create(&mut engine);
    let surface = engine.node_id_by_uuid(surface.surface.0).unwrap();
    let parameter = super::parameters::parameters(&engine, surface)["parameters/opacity"];
    animate(&mut engine, parameter);
    engine
        .run_tick(Duration::from_millis(750))
        .expect("negative waveform must clamp, not fail the engine tick");
    assert_eq!(
        engine
            .nodes
            .get(parameter)
            .unwrap()
            .engine_param_snapshot()
            .unwrap()
            .value,
        ParamValue::Float(0.0)
    );
}

#[test]
fn video_speed_animation_adapts_to_declared_step() {
    use crate::media::protocol::*;
    let mut engine = fixture();
    let ack = apply_project_intent(
        &mut engine,
        UiEditIntent::InvokeAppOperation {
            namespace: MEDIA_NAMESPACE.into(),
            operation: MEDIA_OPERATION.into(),
            payload: serde_json::to_value(MediaEdit::CreateMedia {
                media_kind: MediaKind::Video,
                label: None,
            })
            .unwrap(),
        },
    );
    assert!(ack.success, "{ack:?}");
    let MediaEditResult::Created { reference } = serde_json::from_value(ack.result.unwrap().value).unwrap() else {
        panic!()
    };
    let media = engine.node_id_by_uuid(reference.media.0).unwrap();
    let parameter = super::parameters::parameters(&engine, media)["parameters/video_speed"];
    animate(&mut engine, parameter);
    engine
        .run_tick(Duration::from_millis(17))
        .expect("continuous waveform must adapt to the target step, not fail the engine tick");
    let ParamValue::Float(value) = engine
        .nodes
        .get(parameter)
        .unwrap()
        .engine_param_snapshot()
        .unwrap()
        .value
    else {
        panic!()
    };
    assert!((-16.0..=16.0).contains(&value));
    assert!((value * 100.0 - (value * 100.0).round()).abs() < 1e-9);
}
