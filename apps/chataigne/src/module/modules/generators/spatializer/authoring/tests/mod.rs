use super::super::tests::{create_spatializer_module, find_path, stabilize};
use super::*;
use golden_core::{
    application::apply_project_intent,
    ui_sync::{UiAck, UiEditIntent},
};

fn identity(engine: &AppEngine, node: NodeId) -> SpatializerIdentity {
    SpatializerIdentity {
        node,
        uuid: engine.nodes.get(node).unwrap().node_data().meta.uuid,
    }
}

#[test]
fn spatializer_created_through_production_actor_can_be_saved() {
    use golden_core::application::{ProductionRuntime, ProjectSaveRequest};
    use golden_core::ui_sync::UiProjectFileSpec;
    let mut engine = golden_core::app::create_new_project_engine::<AppNode>().unwrap();
    golden_core::app::prepare_engine_for_runtime(&mut engine).unwrap();
    let manager = engine
        .nodes
        .iter()
        .find(|(_, node)| {
            node.user_creatable_items()
                .iter()
                .any(|item| item.node_type == SpatializerModule::NODE_TYPE)
        })
        .unwrap()
        .0;
    let runtime = ProductionRuntime::new(engine, UiProjectFileSpec::default()).unwrap();
    let applied = runtime
        .apply_ui_transaction(
            UiEditIntent::CreateUserItem {
                parent: manager,
                node_type: SpatializerModule::NODE_TYPE.into(),
                label: Some("Spatializer".into()),
                initial_params: Default::default(),
            },
            None,
        )
        .unwrap();
    assert!(applied.acknowledgement.success, "{:?}", applied.acknowledgement);
    let directory = tempfile::tempdir().unwrap();
    runtime
        .save_project(ProjectSaveRequest {
            path: directory
                .path()
                .join("spatializer.noisette")
                .to_string_lossy()
                .into_owned(),
            ui_state: None,
        })
        .unwrap();
}
fn fixture() -> (AppEngine, SpatializerIdentity, [SpatializerIdentity; 2]) {
    let (engine, module) = create_spatializer_module();
    let items = ["sources", "targets"].map(|list| {
        let list = find_path(&engine, module, &format!("parameters/{list}")).unwrap();
        identity(
            &engine,
            engine.nodes.get(list).unwrap().node_data().first_child.unwrap(),
        )
    });
    let module = identity(&engine, module);
    (engine, module, items)
}
fn invoke(engine: &mut AppEngine, module: SpatializerIdentity, edit: SpatializerEdit) -> UiAck {
    let parameters = find_path(engine, module.node, "parameters").unwrap();
    let space = if enumeration(engine, parameters, "dimensions").unwrap() == "3d" {
        SpatializerSpace::Xz
    } else {
        SpatializerSpace::Xy
    };
    apply_project_intent(
        engine,
        UiEditIntent::InvokeAppOperation {
            namespace: SPATIALIZER_NAMESPACE.into(),
            operation: SPATIALIZER_EDIT_OPERATION.into(),
            payload: serde_json::to_value(SpatializerEditRequest { module, space, edit }).unwrap(),
        },
    )
}
fn result(ack: UiAck) -> SpatializerEditResult {
    assert!(ack.success, "{ack:?}");
    serde_json::from_value(ack.result.unwrap().value).unwrap()
}
fn value(engine: &AppEngine, item: SpatializerIdentity, key: &str) -> ParamValue {
    engine
        .nodes
        .get(child(engine, item.node, key).unwrap())
        .unwrap()
        .engine_param_snapshot()
        .unwrap()
        .value
}
fn configure(engine: &mut AppEngine, module: SpatializerIdentity, key: &str, requested: &str) {
    let node = find_path(engine, module.node, &format!("parameters/{key}")).unwrap();
    engine.edits.push(Edit::SetParam {
        node,
        value: ParamValue::Enum(requested.into()),
        behaviour: golden_core::parameter::ParameterEventBehaviour::Coalesce,
    });
    stabilize(engine);
}

#[test]
fn spatializer_moves_are_atomic_and_reversible_with_exact_results() {
    let (mut engine, module, items) = fixture();
    let history = engine.undo_len();
    let moves = items
        .iter()
        .enumerate()
        .map(|(i, endpoint)| SpatializerMove {
            endpoint: *endpoint,
            position: [2.0 + i as f64, 5.0],
        })
        .collect();
    assert_eq!(
        result(invoke(&mut engine, module, SpatializerEdit::Move { moves })).endpoints,
        items
    );
    assert_eq!(engine.undo_len(), history + 1);
    assert_eq!(
        value(&engine, items[0], POSITION_2D_DECL_ID),
        ParamValue::Vec2(2.0, 5.0)
    );
    assert!(engine.undo().unwrap());
    assert_eq!(
        value(&engine, items[0], POSITION_2D_DECL_ID),
        ParamValue::Vec2(0.0, 0.0)
    );
    assert!(engine.redo().unwrap());
    assert_eq!(
        value(&engine, items[1], POSITION_2D_DECL_ID),
        ParamValue::Vec2(3.0, 5.0)
    );
}

#[test]
fn readonly_later_move_target_cannot_commit_a_prefix() {
    let (mut engine, module, items) = fixture();
    let history = engine.undo_len();
    let parameter = child(&engine, items[1].node, POSITION_2D_DECL_ID).unwrap();
    let AppNode::Parameter(parameter) = engine.nodes.get_mut(parameter).unwrap() else {
        panic!("parameter")
    };
    parameter.read_only = true;
    let moves = items
        .map(|endpoint| SpatializerMove {
            endpoint,
            position: [9.0, 8.0],
        })
        .to_vec();
    let ack = invoke(&mut engine, module, SpatializerEdit::Move { moves });
    assert!(!ack.success);
    assert!(ack.result.is_none());
    assert_eq!(engine.undo_len(), history);
    assert_eq!(
        value(&engine, items[0], POSITION_2D_DECL_ID),
        ParamValue::Vec2(0.0, 0.0)
    );
}

#[test]
fn xz_edits_preserve_the_backend_height_and_nudge_the_latest_value() {
    let (mut engine, module, items) = fixture();
    configure(&mut engine, module, "dimensions", "3d");
    let parameter = child(&engine, items[0].node, POSITION_3D_DECL_ID).unwrap();
    engine.edits.push(Edit::SetParam {
        node: parameter,
        value: ParamValue::Vec3(1.0, 7.0, 2.0),
        behaviour: golden_core::parameter::ParameterEventBehaviour::Coalesce,
    });
    stabilize(&mut engine);
    for _ in 0..2 {
        result(invoke(
            &mut engine,
            module,
            SpatializerEdit::Nudge {
                endpoint: items[0],
                delta: [0.5, -0.25],
            },
        ));
    }
    assert_eq!(
        value(&engine, items[0], POSITION_3D_DECL_ID),
        ParamValue::Vec3(2.0, 7.0, 1.5)
    );
    result(invoke(
        &mut engine,
        module,
        SpatializerEdit::Move {
            moves: vec![SpatializerMove {
                endpoint: items[0],
                position: [9.0, 11.0],
            }],
        },
    ));
    assert_eq!(
        value(&engine, items[0], POSITION_3D_DECL_ID),
        ParamValue::Vec3(9.0, 7.0, 11.0)
    );
}

#[test]
fn stale_foreign_duplicate_and_invalid_requests_leave_no_history_or_authored_change() {
    let (mut engine, module, items) = fixture();
    let history = engine.undo_len();
    let mut stale = items[0];
    stale.uuid = NodeUuid(uuid::Uuid::new_v4());
    let cases = [
        SpatializerEdit::Nudge {
            endpoint: stale,
            delta: [1.0, 0.0],
        },
        SpatializerEdit::Move {
            moves: vec![
                SpatializerMove {
                    endpoint: items[0],
                    position: [1.0, 0.0],
                },
                SpatializerMove {
                    endpoint: items[0],
                    position: [2.0, 0.0],
                },
            ],
        },
        SpatializerEdit::Move { moves: vec![] },
        SpatializerEdit::SetRadius {
            endpoint: items[0],
            field: SpatializerRadius::Radius,
            value: -1.0,
        },
    ];
    for edit in cases {
        assert!(!invoke(&mut engine, module, edit).success);
    }
    let mut stale_module = module;
    stale_module.uuid = NodeUuid(uuid::Uuid::new_v4());
    assert!(
        !invoke(
            &mut engine,
            stale_module,
            SpatializerEdit::Nudge {
                endpoint: items[0],
                delta: [1.0, 0.0]
            }
        )
        .success
    );
    assert_eq!(engine.undo_len(), history);
    assert_eq!(
        value(&engine, items[0], POSITION_2D_DECL_ID),
        ParamValue::Vec2(0.0, 0.0)
    );
    engine.add_node(SpatializerModule::create().into(), None);
    stabilize(&mut engine);
    let other = engine
        .nodes
        .iter()
        .find(|(id, node)| *id != module.node && node.get_type() == SpatializerModule::NODE_TYPE)
        .unwrap()
        .0;
    let other = identity(&engine, other);
    assert!(
        !invoke(
            &mut engine,
            other,
            SpatializerEdit::Nudge {
                endpoint: items[0],
                delta: [1.0, 0.0]
            }
        )
        .success
    );
}

#[test]
fn radius_policy_and_missing_fields_are_backend_owned() {
    let (mut engine, module, items) = fixture();
    configure(&mut engine, module, "mode", "targetRadius");
    result(invoke(
        &mut engine,
        module,
        SpatializerEdit::SetRadius {
            endpoint: items[1],
            field: SpatializerRadius::Radius,
            value: 3.0,
        },
    ));
    assert_eq!(value(&engine, items[1], RADIUS_DECL_ID), ParamValue::Float(3.0));
    result(invoke(
        &mut engine,
        module,
        SpatializerEdit::AdjustRadius {
            endpoint: items[1],
            field: SpatializerRadius::Radius,
            delta: -10.0,
        },
    ));
    assert_eq!(value(&engine, items[1], RADIUS_DECL_ID), ParamValue::Float(0.0));
    assert!(
        !invoke(
            &mut engine,
            module,
            SpatializerEdit::SetRadius {
                endpoint: items[1],
                field: SpatializerRadius::FreezeRadius,
                value: 1.0
            }
        )
        .success
    );
}

#[test]
fn target_creation_materializes_the_current_dimension_and_mode_in_one_undo_step() {
    let (mut engine, module, _) = fixture();
    configure(&mut engine, module, "dimensions", "3d");
    configure(&mut engine, module, "mode", "overlap");
    let history = engine.undo_len();
    let created = result(invoke(
        &mut engine,
        module,
        SpatializerEdit::CreateTarget { position: [3.0, 4.0] },
    ))
    .created
    .unwrap();
    assert_eq!(
        value(&engine, created, POSITION_3D_DECL_ID),
        ParamValue::Vec3(3.0, 0.0, 4.0)
    );
    assert_eq!(value(&engine, created, RADIUS_DECL_ID), ParamValue::Float(1.0));
    assert!(child(&engine, created.node, FREEZE_RADIUS_DECL_ID).is_err());
    assert_eq!(engine.undo_len(), history + 1);
    assert!(engine.undo().unwrap());
    assert!(engine.node_id_by_uuid(created.uuid).is_none());
    assert!(engine.redo().unwrap());
    assert_eq!(engine.node_id_by_uuid(created.uuid), Some(created.node));
    let next = result(invoke(
        &mut engine,
        module,
        SpatializerEdit::CreateTarget { position: [3.0, 4.0] },
    ))
    .created
    .unwrap();
    assert_ne!(next.uuid, created.uuid);
    assert_ne!(
        engine.nodes.get(next.node).unwrap().node_data().meta.label,
        engine.nodes.get(created.node).unwrap().node_data().meta.label
    );
}
