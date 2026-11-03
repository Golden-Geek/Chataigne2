use super::parameters::instance_parameter;
use super::*;
use golden_core::{
    application::apply_project_intent,
    parameter::{ParamValue, ParameterEventBehaviour},
    ui_sync::UiEditIntent,
};

#[test]
fn degree_parameters_public_projection_history_and_project_share_one_versioned_contract() {
    let mut engine = fixture();
    let source = create(&mut engine, MediaKind::SolidColor);
    let owner = create(&mut engine, MediaKind::Composition);
    let item = add(&mut engine, owner, vec![source])[0];
    let media = engine.node_id_by_uuid(item.instance.0).unwrap();
    let parameter = instance_parameter(&engine, media, "rotation_degrees");
    let before = inspect(&mut engine, owner);
    let history = engine.undo_len();
    for degrees in [90.0, -450.0, 720.0] {
        let ack = apply_project_intent(
            &mut engine,
            UiEditIntent::SetParam {
                node: parameter,
                value: ParamValue::Float(degrees),
                behaviour: ParameterEventBehaviour::Append,
            },
        );
        assert!(ack.success, "{ack:?}");
        let snapshot = inspect(&mut engine, owner);
        assert_eq!(
            snapshot.instances[0].definition.transform.rotation_degrees,
            degrees as f32
        );
        let json = to_sparse_project_json_pretty(&engine).unwrap();
        assert!(json.contains("mapgyver.composition.placement.v2"));
        assert!(json.contains("parameters/rotation_degrees"));
        let mut loaded = from_sparse_project_json::<AppNode>(&json).unwrap();
        loaded.apply_edits().unwrap();
        assert_eq!(inspect(&mut loaded, owner), snapshot);
    }
    assert_eq!(engine.undo_len(), history + 3);
    for _ in 0..3 {
        assert!(engine.undo().unwrap());
    }
    assert_eq!(inspect(&mut engine, owner), before);
    for _ in 0..3 {
        assert!(engine.redo().unwrap());
    }
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let old = saved.replace("mapgyver.composition.placement.v2", "mapgyver.composition.placement.v1");
    let error = match from_sparse_project_json::<AppNode>(&old) {
        Ok(_) => panic!("prior radian Placement must not be interpreted as degrees"),
        Err(error) => error,
    };
    assert!(
        error.to_string().contains("Unsupported Composition Placement schema"),
        "{error}"
    );
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), saved);
}

#[test]
fn legacy_radian_operation_fields_reject_atomically_instead_of_resetting_rotation() {
    let mut engine = fixture();
    let source = create(&mut engine, MediaKind::SolidColor);
    let owner = create(&mut engine, MediaKind::Composition);
    let item = add(&mut engine, owner, vec![source])[0];
    let mut definition = inspect(&mut engine, owner).instances[0].definition.clone();
    definition.transform.rotation_degrees = 90.0;
    let mut payload = serde_json::to_value(MediaEdit::Composition {
        reference: owner,
        edit: CompositionEdit::ConfigureInstance {
            instance: item.instance,
            definition,
        },
    })
    .unwrap();
    let transform = payload["edit"]["definition"]["transform"].as_object_mut().unwrap();
    transform.remove("rotation_degrees");
    transform.insert("rotation".into(), serde_json::json!(std::f32::consts::FRAC_PI_2));
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.ui_history_state();
    let ack = apply_project_intent(
        &mut engine,
        UiEditIntent::InvokeAppOperation {
            namespace: MEDIA_NAMESPACE.into(),
            operation: MEDIA_OPERATION.into(),
            payload,
        },
    );
    assert!(!ack.success, "{ack:?}");
    assert!(ack.error_message.unwrap().contains("rotation"));
    assert_eq!(engine.ui_history_state(), history);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
}

#[test]
fn inspection_preserves_active_degree_edits_and_redo_including_rejected_reads() {
    let mut engine = fixture();
    let source = create(&mut engine, MediaKind::SolidColor);
    let owner = create(&mut engine, MediaKind::Composition);
    let item = add(&mut engine, owner, vec![source])[0];
    let media = engine.node_id_by_uuid(item.instance.0).unwrap();
    let rotation = instance_parameter(&engine, media, "rotation_degrees");
    assert!(
        apply_project_intent(
            &mut engine,
            UiEditIntent::BeginEdit {
                client_edit_id: "degree-gesture".into(),
                label: Some("Rotate child media".into()),
            }
        )
        .success
    );
    assert!(
        apply_project_intent(
            &mut engine,
            UiEditIntent::SetParam {
                node: rotation,
                value: ParamValue::Float(90.0),
                behaviour: ParameterEventBehaviour::Append,
            }
        )
        .success
    );
    let history = engine.ui_history_state();
    assert!(history.active_edit_session);
    let project = to_sparse_project_json_pretty(&engine).unwrap();
    for reference in [owner, source] {
        let ack = invoke(
            &mut engine,
            MediaEdit::Composition {
                reference,
                edit: CompositionEdit::Inspect,
            },
        );
        assert_eq!(ack.success, reference == owner, "{ack:?}");
        assert_eq!(
            engine.ui_history_state(),
            history,
            "inspection must not close a gesture"
        );
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), project);
    }
    assert!(
        apply_project_intent(
            &mut engine,
            UiEditIntent::EndEdit {
                client_edit_id: "degree-gesture".into()
            }
        )
        .success
    );
    assert!(engine.undo().unwrap());
    let history = engine.ui_history_state();
    assert!(history.can_redo);
    assert_eq!(
        inspect(&mut engine, owner).instances[0]
            .definition
            .transform
            .rotation_degrees,
        0.0
    );
    assert_eq!(engine.ui_history_state(), history);
    assert!(engine.redo().unwrap());
    assert_eq!(
        inspect(&mut engine, owner).instances[0]
            .definition
            .transform
            .rotation_degrees,
        90.0
    );
}
