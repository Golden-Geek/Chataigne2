//! Reproduce actual app Parameter declarations through Golden's animation control path.
use super::*;
use golden_core::{
    edit::Edit,
    node::{
        Node, NodeId, NodeReference, PARAMETER_ANIMATION_AMPLITUDE_DECL_ID, PARAMETER_ANIMATION_CONTROL_DECL_ID,
        PARAMETER_ANIMATION_OFFSET_DECL_ID, PARAMETER_CONTROL_REFERENCE_DECL_ID,
    },
    parameter::{ParamValue, ParameterControlMode, ParameterControlSpec, ParameterControlState, RangeConstraint},
    script::ScriptPropertyWrite,
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

fn value(engine: &AppEngine, parameter: NodeId) -> ParamValue {
    engine
        .nodes
        .get(parameter)
        .unwrap()
        .engine_param_snapshot()
        .unwrap()
        .value
}

fn opacity(engine: &mut AppEngine) -> NodeId {
    let (_, surface) = create(engine);
    let surface = engine.node_id_by_uuid(surface.surface.0).unwrap();
    super::parameters::parameters(engine, surface)["parameters/opacity"]
}

fn child(engine: &AppEngine, parent: NodeId, declaration: &str) -> NodeId {
    let mut pending = vec![parent];
    while let Some(parent) = pending.pop() {
        let mut next = engine.nodes.get(parent).unwrap().node_data().first_child;
        while let Some(id) = next {
            let data = engine.nodes.get(id).unwrap().node_data();
            if data.meta.decl_id.0 == declaration {
                return id;
            }
            pending.push(id);
            next = data.next_sibling;
        }
    }
    panic!("missing {declaration}")
}

fn write(engine: &mut AppEngine, parameter: NodeId, next: f64) {
    assert!(super::parameters::set(engine, parameter, ParamValue::Float(next)).success);
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
    let uuid = engine.nodes.get(parameter).unwrap().node_data().meta.uuid;
    let constraints = engine
        .nodes
        .get(parameter)
        .unwrap()
        .engine_param_snapshot()
        .unwrap()
        .constraints;
    let saved = golden_core::app::to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded: AppEngine = golden_core::app::from_sparse_project_json(&saved).unwrap();
    loaded.settle_edit_notifications(16).unwrap();
    let parameter = loaded.node_id_by_uuid(uuid).unwrap();
    assert_eq!(
        loaded
            .nodes
            .get(parameter)
            .unwrap()
            .engine_param_snapshot()
            .unwrap()
            .constraints,
        constraints
    );
    loaded.run_tick(Duration::from_millis(17)).unwrap();
    let ParamValue::Float(value) = loaded
        .nodes
        .get(parameter)
        .unwrap()
        .engine_param_snapshot()
        .unwrap()
        .value
    else {
        panic!()
    };
    assert!((value * 100.0 - (value * 100.0).round()).abs() < 1e-9);
}

#[test]
fn video_declarations_restore_omitted_bounds_and_keep_explicit_valid_overrides() {
    use golden_core::{engine::ProjectNodeRecord, node::NodeUuid};
    fn find(record: &mut ProjectNodeRecord, uuid: NodeUuid) -> Option<&mut ProjectNodeRecord> {
        if record.uuid == uuid {
            return Some(record);
        }
        record.children.iter_mut().find_map(|child| find(child, uuid))
    }
    let mut engine = fixture();
    use crate::media::protocol::{MEDIA_NAMESPACE, MEDIA_OPERATION, MediaEdit, MediaEditResult, MediaKind};
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
    assert!(ack.success);
    let MediaEditResult::Created { reference } = serde_json::from_value(ack.result.unwrap().value).unwrap() else {
        panic!()
    };
    let owner = engine.node_id_by_uuid(reference.media.0).unwrap();
    let parameter = super::parameters::parameters(&engine, owner)["parameters/video_speed"];
    let uuid = engine.nodes.get(parameter).unwrap().node_data().meta.uuid;
    animate(&mut engine, parameter);
    engine.run_tick(Duration::from_millis(17)).unwrap();
    let canonical = engine
        .nodes
        .get(parameter)
        .unwrap()
        .engine_param_snapshot()
        .unwrap()
        .constraints;
    let saved = golden_core::app::to_sparse_project_json_pretty(&engine).unwrap();
    for explicit in [false, true] {
        let mut envelope: serde_json::Value = serde_json::from_str(&saved).unwrap();
        let mut project: golden_core::engine::ProjectFile = serde_json::from_str(&saved).unwrap();
        let data = find(&mut project.root, uuid)
            .unwrap()
            .data
            .as_mut()
            .unwrap()
            .as_object_mut()
            .unwrap();
        let mut expected = canonical.clone();
        if explicit {
            expected.range = RangeConstraint::uniform(Some(-8.), Some(8.));
            data.insert("constraints".into(), serde_json::to_value(&expected).unwrap());
        } else {
            data.remove("constraints");
        }
        envelope["root"] = serde_json::to_value(project.root).unwrap();
        let mut loaded: AppEngine =
            golden_core::app::from_sparse_project_json(&serde_json::to_string(&envelope).unwrap()).unwrap();
        loaded.settle_edit_notifications(16).unwrap();
        let parameter = loaded.node_id_by_uuid(uuid).unwrap();
        assert_eq!(
            loaded
                .nodes
                .get(parameter)
                .unwrap()
                .engine_param_snapshot()
                .unwrap()
                .constraints,
            expected
        );
        loaded.run_tick(Duration::from_millis(17)).unwrap();
        let ParamValue::Float(value) = value(&loaded, parameter) else {
            panic!()
        };
        assert!((value * 100. - (value * 100.).round()).abs() < 1e-9);
    }
}

#[test]
fn proxy_and_both_binding_directions_use_current_target_constraints() {
    use golden_core::parameter::{Parameter, ParameterChangeCheck};
    for (mode, spec) in [
        (ParameterControlMode::Proxy, ParameterControlSpec::Proxy),
        (ParameterControlMode::Binding, ParameterControlSpec::Binding),
    ] {
        let mut engine = fixture();
        let target = opacity(&mut engine);
        engine.add_node(
            Parameter::new(
                "External source",
                ParamValue::Float(0.5),
                ParameterChangeCheck::ValueChange,
            )
            .into(),
            Some(engine.root),
        );
        engine.apply_edits().unwrap();
        let source = engine
            .nodes
            .iter()
            .find_map(|(id, node)| (node.node_data().meta.label == "External source").then_some(id))
            .unwrap();
        let uuid = engine.nodes.get(source).unwrap().node_data().meta.uuid;
        engine
            .set_param_control_state(target, ParameterControlState::new(mode, spec))
            .unwrap();
        let reference = child(&engine, target, PARAMETER_CONTROL_REFERENCE_DECL_ID);
        assert!(
            super::parameters::set(&mut engine, reference, ParamValue::Reference(NodeReference::new(uuid))).success
        );
        for (incoming, expected) in [(4., 1.), (-4., 0.), (0.25, 0.25)] {
            write(&mut engine, source, incoming);
            engine.run_tick(Duration::from_millis(17)).unwrap();
            assert_eq!(
                value(&engine, target),
                ParamValue::Float(expected),
                "{mode:?}: {:?}",
                engine
                    .nodes
                    .get(target)
                    .unwrap()
                    .engine_param_snapshot()
                    .unwrap()
                    .control
            );
        }
        if mode == ParameterControlMode::Binding {
            let history = engine.undo_len();
            let before = golden_core::app::to_sparse_project_json_pretty(&engine).unwrap();
            write(&mut engine, target, 0.75);
            engine.run_tick(Duration::from_millis(17)).unwrap();
            assert_eq!(value(&engine, source), ParamValue::Float(0.75));
            assert_eq!(
                engine.undo_len(),
                history + 1,
                "dependent writes share the UI transaction"
            );
            assert!(engine.undo().unwrap());
            assert_eq!(
                golden_core::app::to_sparse_project_json_pretty(&engine).unwrap(),
                before
            );
        }
    }
}

#[test]
fn script_properties_clamp_finite_inputs_and_reject_nonfinite_without_mutation() {
    let mut engine = fixture();
    let target = opacity(&mut engine);
    for (incoming, expected) in [(-8., 0.), (8., 1.), (0.25, 0.25)] {
        engine.edits.push(Edit::SetNodeScriptProperty {
            node: target,
            write: ScriptPropertyWrite::prepare("value", ParamValue::Float(incoming)).unwrap(),
        });
        engine.apply_edits().unwrap();
        engine.apply_edits().unwrap();
        assert_eq!(value(&engine, target), ParamValue::Float(expected));
    }
    let before = golden_core::app::to_sparse_project_json_pretty(&engine).unwrap();
    for incoming in [f64::NAN, f64::INFINITY] {
        engine.edits.push(Edit::SetNodeScriptProperty {
            node: target,
            write: ScriptPropertyWrite::prepare("value", ParamValue::Float(incoming)).unwrap(),
        });
        assert!(engine.apply_edits().is_err());
        assert_eq!(
            golden_core::app::to_sparse_project_json_pretty(&engine).unwrap(),
            before
        );
    }
}

#[test]
fn animation_reads_changed_constraints_and_holds_invalid_samples_without_stopping_siblings() {
    let mut engine = fixture();
    let healthy = opacity(&mut engine);
    write(&mut engine, healthy, 0.5);
    let mut constraints = engine
        .nodes
        .get(healthy)
        .unwrap()
        .engine_param_snapshot()
        .unwrap()
        .constraints;
    constraints.range = RangeConstraint::uniform(Some(0.2), Some(0.8));
    engine.edits.push(Edit::SetParamConstraints {
        node: healthy,
        constraints,
    });
    engine.apply_edits().unwrap();
    animate(&mut engine, healthy);
    let overflowing = opacity(&mut engine);
    animate(&mut engine, overflowing);
    let animation = child(&engine, overflowing, PARAMETER_ANIMATION_CONTROL_DECL_ID);
    for declaration in [
        PARAMETER_ANIMATION_AMPLITUDE_DECL_ID,
        PARAMETER_ANIMATION_OFFSET_DECL_ID,
    ] {
        let parameter = child(&engine, animation, declaration);
        write(&mut engine, parameter, f64::MAX);
    }
    let held = value(&engine, overflowing);
    engine.run_tick(Duration::from_millis(250)).unwrap();
    assert_eq!(
        value(&engine, overflowing),
        held,
        "overflowing waveform retains its last valid target"
    );
    assert_eq!(value(&engine, healthy), ParamValue::Float(0.8));
    engine.run_tick(Duration::from_millis(500)).unwrap();
    assert_eq!(value(&engine, healthy), ParamValue::Float(0.2));
}
