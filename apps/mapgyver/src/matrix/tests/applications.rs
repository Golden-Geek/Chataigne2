use super::*;
use crate::applications::{demand::DemandReason, protocol::*};
use golden_core::{
    node::{Node, NodeId},
    parameter::{ParamValue, ParameterEventBehaviour},
    ui_sync::NodeMetaPatch,
};

#[test]
fn column_owns_typed_transition_and_control_through_copy_history_and_project_replay() {
    let mut engine = fixture();
    let (media, _, columns) = scene(&mut engine);
    let column = columns[2];
    let application = ApplicationId(column.0);
    let history = engine.undo_len();
    edit(
        &mut engine,
        media,
        MatrixEdit::ConfigureColumnApplication {
            column,
            definition: definition(),
            pre: 1,
            post: 2,
        },
    );
    assert_eq!(engine.undo_len(), history + 1);
    let transition = ColumnDefinition {
        transition: Some(TransitionDefinition {
            duration_ms: u32::MAX,
            curve: TransitionCurve::SmoothStep,
        }),
    };
    edit(
        &mut engine,
        media,
        MatrixEdit::ConfigureColumn {
            column,
            definition: transition.clone(),
        },
    );
    let before_copy = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    edit(&mut engine, media, MatrixEdit::DuplicateColumn { column });
    assert_eq!(engine.undo_len(), history + 1);
    let state = crate::matrix::inspect(&engine, media).unwrap();
    let copy = state.columns.iter().find(|c| !columns.contains(&c.id)).unwrap();
    assert_ne!(copy.application, application);
    assert_eq!(copy.application.0, copy.id.0);
    assert_eq!(copy.definition, transition);
    assert_eq!(
        crate::applications::inspect(&engine, copy.application)
            .unwrap()
            .definition,
        definition()
    );
    let pre = parameter(&engine, copy.application, "pre");
    let AppNode::Parameter(pre) = engine.nodes.get(pre).unwrap() else {
        panic!()
    };
    assert_eq!(pre.value, ParamValue::Int(1));
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
    assert_eq!(crate::matrix::inspect(&loaded, media).unwrap(), state);
    assert_eq!(
        crate::applications::inspect(&loaded, copy.application)
            .unwrap()
            .definition,
        definition()
    );
    assert!(engine.undo().unwrap());
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before_copy);
    assert!(engine.redo().unwrap());
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), saved);
    assert!(
        from_sparse_project_json::<AppNode>(&saved.replace("mapgyver.matrix.column.v2", "mapgyver.matrix.column"))
            .is_err()
    );
    crate::media::validate_project(&engine).unwrap();
}

fn scene(engine: &mut AppEngine) -> (MediaId, LayerId, Vec<ColumnId>) {
    let matrix = create(engine, MediaKind::Matrix).media;
    let row = layer(layers(engine, matrix, None, 1)[0]);
    let columns = columns(engine, matrix, 5);
    (matrix, row, columns)
}
fn definition() -> ApplicationDefinition {
    ApplicationDefinition {
        executable: Some(std::env::current_exe().unwrap().to_string_lossy().into_owned()),
        arguments: vec!["literal & value".into()],
        environment: Some(vec![("EXACT".into(), "value".into())]),
        ..Default::default()
    }
}
fn configure(engine: &mut AppEngine, media: MediaId, layer: LayerId, column: ColumnId) -> (CellId, ApplicationId) {
    let MediaEditResult::MatrixCellPrepared { cell, application } = edit(
        engine,
        media,
        MatrixEdit::ConfigureCellApplication {
            layer,
            column,
            definition: definition(),
            pre: 1,
            post: 2,
        },
    ) else {
        panic!()
    };
    (cell, application)
}
fn parameter(engine: &AppEngine, owner: ApplicationId, field: &str) -> NodeId {
    let owner = match engine.nodes.get(engine.node_id_by_uuid(owner.0).unwrap()).unwrap() {
        AppNode::MatrixCell(node) => node.application_node().unwrap(),
        AppNode::MatrixColumn(node) => node.application_node().unwrap(),
        _ => panic!(),
    };
    let folder = engine
        .nodes
        .iter()
        .find(|(_, node)| node.node_data().parent == Some(owner) && crate::parameters::is_parameter_folder(*node))
        .unwrap()
        .0;
    engine
        .nodes
        .iter()
        .find(|(_, node)| {
            node.node_data().parent == Some(folder) && node.node_data().meta.decl_id.0 == format!("parameters/{field}")
        })
        .unwrap()
        .0
}

#[test]
fn empty_cell_owns_typed_app_control_with_atomic_history_stable_identity_and_project_replay() {
    let mut engine = fixture();
    let (media, row, columns) = scene(&mut engine);
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    let (cell, application) = configure(&mut engine, media, row, columns[2]);
    assert_eq!(engine.undo_len(), history + 1);
    let state = crate::matrix::inspect(&engine, media).unwrap();
    assert_eq!(state.cells.len(), 1);
    assert_eq!(state.cells[0].id, cell);
    assert!(state.cells[0].media.is_none());
    assert!(state.cells[0].definition.source.is_none());
    assert_eq!(state.cells[0].definition.application, application);
    assert_eq!(
        crate::applications::inspect(&engine, application).unwrap().definition,
        definition()
    );
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
    assert_eq!(
        crate::applications::inspect(&loaded, application).unwrap().definition,
        definition()
    );
    assert_eq!(crate::matrix::inspect(&loaded, media).unwrap(), state);
    let history = engine.undo_len();
    assert!(
        matches!(edit(&mut engine, media, MatrixEdit::EnsureCell { layer: row, column: columns[2] }), MediaEditResult::MatrixCellPrepared { cell: current, application: settings } if current == cell && settings == application)
    );
    assert_eq!(engine.undo_len(), history);
    assert!(engine.undo().unwrap());
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    assert!(engine.redo().unwrap());
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), saved);
    crate::media::validate_project(&engine).unwrap();
}

#[test]
fn cell_demand_is_clamped_nonwrapping_and_updates_on_direct_distance_reorder_disable_and_delete() {
    let mut engine = fixture();
    let (media, row, columns) = scene(&mut engine);
    let (cell, application) = configure(&mut engine, media, row, columns[2]);
    for (index, column) in columns.iter().enumerate() {
        edit(
            &mut engine,
            media,
            MatrixEdit::LaunchColumn {
                column: *column,
                mode: LaunchMode::Cut,
            },
        );
        let capture = crate::applications::logical::capture_logical_applications(&engine).unwrap();
        assert_eq!(capture.demand.len(), usize::from(index >= 1));
        if index >= 1 {
            assert_eq!(capture.demand[0].application, application);
            assert!(capture.demand[0].reasons.contains(&if index == 2 {
                DemandReason::Preparation
            } else {
                DemandReason::Neighbor
            }));
        }
    }
    let capture = crate::applications::logical::capture_logical_applications(&engine).unwrap();
    let post = parameter(&engine, application, "post");
    assert!(
        apply_project_intent(
            &mut engine,
            UiEditIntent::SetParam {
                node: post,
                value: ParamValue::Int(0),
                behaviour: ParameterEventBehaviour::Append
            }
        )
        .success
    );
    assert!(!capture.is_current());
    assert!(
        crate::applications::logical::capture_logical_applications(&engine)
            .unwrap()
            .demand
            .is_empty()
    );
    edit(
        &mut engine,
        media,
        MatrixEdit::ReorderColumn {
            column: columns[2],
            after: None,
        },
    );
    edit(
        &mut engine,
        media,
        MatrixEdit::LaunchColumn {
            column: columns[4],
            mode: LaunchMode::Cut,
        },
    );
    assert!(
        crate::applications::logical::capture_logical_applications(&engine)
            .unwrap()
            .demand
            .is_empty()
    );
    edit(
        &mut engine,
        media,
        MatrixEdit::LaunchColumn {
            column: columns[2],
            mode: LaunchMode::Cut,
        },
    );
    let id = engine.node_id_by_uuid(cell.0).unwrap();
    assert!(
        apply_project_intent(
            &mut engine,
            UiEditIntent::PatchMeta {
                node: id,
                patch: NodeMetaPatch {
                    enabled: Some(false),
                    ..Default::default()
                }
            }
        )
        .success
    );
    assert!(
        crate::applications::logical::capture_logical_applications(&engine)
            .unwrap()
            .demand
            .is_empty()
    );
    assert!(engine.undo().unwrap());
    assert_eq!(
        crate::applications::logical::capture_logical_applications(&engine)
            .unwrap()
            .demand[0]
            .application,
        application
    );
    edit(&mut engine, media, MatrixEdit::RemoveColumn { column: columns[2] });
    assert!(crate::applications::inspect(&engine, application).is_err());
    assert!(engine.undo().unwrap());
    assert_eq!(
        crate::applications::inspect(&engine, application).unwrap().definition,
        definition()
    );
}

#[test]
fn embedded_settings_reject_standalone_crud_and_invalid_parameters_without_authored_changes() {
    let mut engine = fixture();
    let (media, row, columns) = scene(&mut engine);
    let (_, application) = configure(&mut engine, media, row, columns[2]);
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    for kind in [
        "Create",
        "Remove",
        "Duplicate",
        "Rename",
        "SetEnabled",
        "InspectCatalog",
        "InspectReferences",
    ] {
        assert!(
            serde_json::from_value::<ApplicationEdit>(
                serde_json::json!({"kind":kind,"application":application,"label":"Invalid","enabled":false})
            )
            .is_err()
        );
    }
    let pre = parameter(&engine, application, "pre");
    for value in [-1, 65536] {
        assert!(
            !apply_project_intent(
                &mut engine,
                UiEditIntent::SetParam {
                    node: pre,
                    value: ParamValue::Int(value),
                    behaviour: ParameterEventBehaviour::Append
                }
            )
            .success
        );
    }
    assert_eq!(engine.undo_len(), history);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    let settings = match engine
        .nodes
        .get(engine.node_id_by_uuid(application.0).unwrap())
        .unwrap()
    {
        AppNode::MatrixCell(node) => node.application_node().unwrap(),
        _ => panic!(),
    };
    assert!(!apply_project_intent(&mut engine, UiEditIntent::RemoveNode { node: settings }).success);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
}

#[test]
fn cell_app_control_duplicates_with_its_column_and_rejects_obsolete_schema() {
    let mut engine = fixture();
    let (media, row, columns) = scene(&mut engine);
    let (_, application) = configure(&mut engine, media, row, columns[2]);
    let history = engine.undo_len();
    edit(&mut engine, media, MatrixEdit::DuplicateColumn { column: columns[2] });
    assert_eq!(engine.undo_len(), history + 1);
    let state = crate::matrix::inspect(&engine, media).unwrap();
    assert_eq!(state.cells.len(), 2);
    let copy = state
        .cells
        .iter()
        .find(|cell| cell.definition.application != application)
        .unwrap()
        .definition
        .application;
    assert_eq!(
        crate::applications::inspect(&engine, copy).unwrap().definition,
        definition()
    );
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    assert!(
        from_sparse_project_json::<AppNode>(&saved.replace("mapgyver.matrix.cell.v3", "mapgyver.matrix.cell.v1"))
            .is_err()
    );
    assert!(engine.undo().unwrap());
    assert!(crate::applications::inspect(&engine, copy).is_err());
}
