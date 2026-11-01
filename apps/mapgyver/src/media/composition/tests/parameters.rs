use super::*;
use golden_core::{
    application::apply_project_intent,
    node::Node,
    parameter::{ParamValue, ParameterEventBehaviour},
    ui_sync::UiEditIntent,
};

#[test]
fn generic_parameter_edits_are_the_only_authored_instance_state_and_roundtrip_with_history() {
    let mut engine = fixture();
    let source = create(&mut engine, MediaKind::SolidColor);
    let composition = create(&mut engine, MediaKind::Composition);
    let instance = add(&mut engine, composition, vec![source])[0];
    let root = engine.node_id_by_uuid(instance.instance.0).unwrap();
    let folder = engine.nodes.get(root).unwrap().node_data().first_child.unwrap();
    let mut params = Vec::new();
    let mut child = engine.nodes.get(folder).unwrap().node_data().first_child;
    while let Some(id) = child {
        let node = engine.nodes.get(id).unwrap();
        assert!(node.engine_param_snapshot().is_some());
        params.push((node.node_data().meta.decl_id.0.clone(), id));
        child = node.node_data().next_sibling;
    }
    assert_eq!(params.len(), 9);
    assert!(
        engine.nodes.get(root).unwrap().project_encode_data().unwrap().is_null(),
        "No second authored DTO"
    );
    let opacity = params.iter().find(|(name, _)| name == "parameters/opacity").unwrap().1;
    let before = inspect(&mut engine, composition);
    let history = engine.undo_len();
    let ack = apply_project_intent(
        &mut engine,
        UiEditIntent::SetParam {
            node: opacity,
            value: ParamValue::Float(0.25),
            behaviour: ParameterEventBehaviour::Coalesce,
        },
    );
    assert!(ack.success, "{ack:?}");
    assert_eq!(engine.undo_len(), history + 1);
    assert_eq!(inspect(&mut engine, composition).instances[0].definition.opacity, 0.25);
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
    loaded.apply_edits().unwrap();
    assert_eq!(inspect(&mut loaded, composition), inspect(&mut engine, composition));
    assert!(engine.undo().unwrap());
    assert_eq!(inspect(&mut engine, composition), before);
    assert!(engine.redo().unwrap());
    assert_eq!(inspect(&mut engine, composition).instances[0].definition.opacity, 0.25);
    // Low-level parameter writes use the same source of truth as UI and typed operations.
    engine.edits.push(golden_core::edit::Edit::SetParam {
        node: opacity,
        value: ParamValue::Float(0.5),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().unwrap();
    assert_eq!(inspect(&mut engine, composition).instances[0].definition.opacity, 0.5);
}

#[test]
fn direct_parameter_edits_reject_invalid_geometry_and_reference_cycles_without_state_or_history() {
    let mut engine = fixture();
    let source = create(&mut engine, MediaKind::SolidColor);
    let composition = create(&mut engine, MediaKind::Composition);
    let instance = add(&mut engine, composition, vec![source])[0];
    let root = engine.node_id_by_uuid(instance.instance.0).unwrap();
    let folder = engine.nodes.get(root).unwrap().node_data().first_child.unwrap();
    let find = |engine: &AppEngine, name: &str| {
        let mut child = engine.nodes.get(folder).unwrap().node_data().first_child;
        loop {
            let id = child.unwrap();
            let node = engine.nodes.get(id).unwrap();
            if node.node_data().meta.decl_id.0 == format!("parameters/{name}") {
                break id;
            }
            child = node.node_data().next_sibling;
        }
    };
    let scale = find(&engine, "scale");
    let channel = find(&engine, "source_channel");
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    for (parameter, value) in [
        (scale, ParamValue::Vec2(0.0, 1.0)),
        (scale, ParamValue::Vec2(f64::MAX, 1.0)),
        (
            channel,
            ParamValue::Reference(golden_core::node::NodeReference::new(composition.channel.0)),
        ),
    ] {
        let ack = apply_project_intent(
            &mut engine,
            UiEditIntent::SetParam {
                node: parameter,
                value: value.clone(),
                behaviour: ParameterEventBehaviour::Coalesce,
            },
        );
        assert!(!ack.success, "{ack:?}");
        assert_eq!(engine.undo_len(), history);
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
        engine.edits.push(golden_core::edit::Edit::SetParam {
            node: parameter,
            value,
            behaviour: ParameterEventBehaviour::Coalesce,
        });
        assert!(
            engine.apply_edits().is_err(),
            "low-level callers cannot bypass owner validation"
        );
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    }
}

#[test]
fn copying_preserves_parameter_configuration_and_control_subtrees_with_independent_identities() {
    use golden_core::{
        edit::Edit,
        parameter::{ParameterControlMode, ParameterControlSpec, ParameterControlState},
    };
    let mut engine = fixture();
    let source = create(&mut engine, MediaKind::SolidColor);
    let composition = create(&mut engine, MediaKind::Composition);
    let original = add(&mut engine, composition, vec![source])[0];
    let root = engine.node_id_by_uuid(original.instance.0).unwrap();
    let opacity = opacity_parameter(&engine, root);
    // Only user-editable constraints are authored; producer-owned catalogs/ranges
    // are deliberately rebuilt from their declaration on sparse project load.
    engine
        .nodes
        .get_mut(opacity)
        .unwrap()
        .node_data_mut()
        .meta
        .user_permissions
        .can_edit_constraints = true;
    engine.edits.push(Edit::SetParam {
        node: opacity,
        value: ParamValue::Float(0.25),
        behaviour: ParameterEventBehaviour::Append,
    });
    let mut constraints = engine
        .nodes
        .get(opacity)
        .unwrap()
        .engine_param_snapshot()
        .unwrap()
        .constraints;
    constraints.step = Some(0.25);
    engine.edits.push(Edit::SetParamConstraints {
        node: opacity,
        constraints,
    });
    engine.edits.push(Edit::SetParamControlState {
        node: opacity,
        state: ParameterControlState::new(ParameterControlMode::Expression, ParameterControlSpec::Expression),
    });
    engine.apply_edits().unwrap();
    let parameter = engine.nodes.get(opacity).unwrap().engine_param_snapshot().unwrap();
    assert_ne!(parameter.value, parameter.default_value);
    let control = engine.nodes.get(opacity).unwrap().node_data().first_child.unwrap();
    let before = inspect(&mut engine, composition);
    let history = engine.undo_len();
    let MediaEditResult::InstancesCreated { instances } = edit(
        &mut engine,
        composition,
        CompositionEdit::DuplicateInstances {
            instances: vec![original.instance],
        },
    ) else {
        panic!()
    };
    assert_eq!(engine.undo_len(), history + 1);
    let copy = engine.node_id_by_uuid(instances[0].instance.0).unwrap();
    let copied_opacity = opacity_parameter(&engine, copy);
    let copied_parameter = engine
        .nodes
        .get(copied_opacity)
        .unwrap()
        .engine_param_snapshot()
        .unwrap();
    assert_eq!(copied_parameter, parameter);
    assert_ne!(opacity, copied_opacity);
    let copied_control = engine
        .nodes
        .get(copied_opacity)
        .unwrap()
        .node_data()
        .first_child
        .unwrap();
    assert_ne!(control, copied_control);
    assert_eq!(
        engine.nodes.get(control).unwrap().get_type(),
        engine.nodes.get(copied_control).unwrap().get_type()
    );
    let after = inspect(&mut engine, composition);
    assert!(engine.undo().unwrap());
    assert_eq!(inspect(&mut engine, composition), before);
    assert!(engine.redo().unwrap());
    assert_eq!(inspect(&mut engine, composition), after);
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
    let copied_uuid = engine.nodes.get(copied_opacity).unwrap().node_data().meta.uuid;
    let loaded_parameter = loaded
        .nodes
        .get(loaded.node_id_by_uuid(copied_uuid).unwrap())
        .unwrap()
        .engine_param_snapshot()
        .unwrap();
    assert_eq!(loaded_parameter, parameter);
}

fn opacity_parameter(engine: &AppEngine, root: golden_core::node::NodeId) -> golden_core::node::NodeId {
    let folder = engine.nodes.get(root).unwrap().node_data().first_child.unwrap();
    let mut child = engine.nodes.get(folder).unwrap().node_data().first_child;
    loop {
        let id = child.unwrap();
        let node = engine.nodes.get(id).unwrap();
        if node.node_data().meta.decl_id.0 == "parameters/opacity" {
            return id;
        }
        child = node.node_data().next_sibling;
    }
}
