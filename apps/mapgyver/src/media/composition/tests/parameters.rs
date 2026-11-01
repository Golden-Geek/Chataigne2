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
