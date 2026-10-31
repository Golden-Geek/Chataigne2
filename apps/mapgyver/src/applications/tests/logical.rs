use super::*;
use crate::applications::{driver::ApplicationDriver, logical::capture_logical_applications};

fn launch(engine: &mut AppEngine, media: MediaId, column: ColumnId) {
    edit(
        engine,
        MediaEdit::Matrix {
            media,
            edit: MatrixEdit::LaunchColumn {
                column,
                mode: LaunchMode::Configured,
            },
        },
    );
}

#[test]
fn committed_timed_column_preloads_without_video_time_renderer_or_history_changes() {
    let mut engine = fixture();
    let application = create(&mut engine);
    let (media, columns) = matrix(&mut engine);
    configure_column(&mut engine, media, columns[2], application);
    let idle = capture_logical_applications(&engine).unwrap();
    assert!(idle.demand.is_empty());
    let history = engine.undo_len();
    let authored = to_sparse_project_json_pretty(&engine).unwrap();
    launch(&mut engine, media, columns[1]);
    assert!(!idle.is_current());
    let preload = capture_logical_applications(&engine).unwrap();
    assert_eq!(preload.demand.len(), 1);
    assert_eq!(preload.demand[0].application, application);
    assert_eq!(
        preload.demand[0].reasons,
        [DemandReason::Neighbor].into_iter().collect()
    );
    assert!(preload.is_current());
    assert_eq!(engine.undo_len(), history);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), authored);
    assert!(
        !invoke(
            &mut engine,
            MediaEdit::Matrix {
                media,
                edit: MatrixEdit::LaunchColumn {
                    column: ColumnId(golden_core::model::NodeUuid(uuid::Uuid::new_v4())),
                    mode: LaunchMode::Cut,
                }
            }
        )
        .success
    );
    assert!(
        preload.is_current(),
        "a rejected launch cannot change committed app demand"
    );
    launch(&mut engine, media, columns[3]);
    assert_eq!(
        capture_logical_applications(&engine).unwrap().demand.len(),
        1,
        "post=1 retains through column 4"
    );
    launch(&mut engine, media, columns[4]);
    assert!(!preload.is_current());
    assert!(capture_logical_applications(&engine).unwrap().demand.is_empty());
}

#[test]
fn shared_matrices_union_scopes_and_reorder_remove_and_undo_reconcile_column_distances() {
    let mut engine = fixture();
    let application = create(&mut engine);
    let (first, columns) = matrix(&mut engine);
    let (second, others) = matrix(&mut engine);
    configure_column(&mut engine, first, columns[2], application);
    configure_column(&mut engine, second, others[2], application);
    launch(&mut engine, first, columns[1]);
    launch(&mut engine, second, others[2]);
    let shared = capture_logical_applications(&engine).unwrap();
    assert_eq!(shared.demand.len(), 1);
    assert_eq!(
        shared.demand[0].reasons,
        [DemandReason::Neighbor, DemandReason::Preparation]
            .into_iter()
            .collect()
    );
    launch(&mut engine, second, others[4]);
    edit(
        &mut engine,
        MediaEdit::Matrix {
            media: first,
            edit: MatrixEdit::ReorderColumn {
                column: columns[2],
                after: Some(columns[4]),
            },
        },
    );
    assert!(!shared.is_current());
    assert!(capture_logical_applications(&engine).unwrap().demand.is_empty());
    assert!(engine.undo().unwrap());
    assert_eq!(capture_logical_applications(&engine).unwrap().demand.len(), 1);
    edit(
        &mut engine,
        MediaEdit::Matrix {
            media: first,
            edit: MatrixEdit::RemoveColumn { column: columns[1] },
        },
    );
    assert!(capture_logical_applications(&engine).unwrap().demand.is_empty());
}

#[test]
fn cell_authoring_does_not_invalidate_logical_column_capture_or_create_render_indices() {
    let mut engine = fixture();
    let application = create(&mut engine);
    let (media, columns) = matrix(&mut engine);
    configure_column(&mut engine, media, columns[2], application);
    let MediaEditResult::MatrixCreated { items, .. } = edit(
        &mut engine,
        MediaEdit::Matrix {
            media,
            edit: MatrixEdit::AddLayers { parent: None, count: 1 },
        },
    ) else {
        panic!()
    };
    let MatrixItemId::Layer { id: layer } = items[0] else {
        panic!()
    };
    let MediaEditResult::Created { reference } = edit(
        &mut engine,
        MediaEdit::CreateMedia {
            media_kind: MediaKind::SolidColor,
            label: None,
        },
    ) else {
        panic!()
    };
    launch(&mut engine, media, columns[2]);
    let capture = capture_logical_applications(&engine).unwrap();
    edit(
        &mut engine,
        MediaEdit::Matrix {
            media,
            edit: MatrixEdit::SetCells {
                assignments: vec![CellAssignment {
                    layer,
                    column: columns[0],
                    source: Some(reference),
                }],
            },
        },
    );
    assert!(capture.is_current(), "cell edits must not trigger app column rescans");
    let root = crate::matrix::playback::root(&engine, media).unwrap();
    let AppNode::MediaChannel(root) = engine.nodes.get(root).unwrap() else {
        panic!()
    };
    assert!(
        root.matrix_playback().unwrap().cached().is_none(),
        "logical capture must not build a render/cell index"
    );
    app_edit(&mut engine, ApplicationEdit::Remove { application });
    assert!(!capture.is_current());
    let missing = capture_logical_applications(&engine).unwrap();
    assert_eq!(missing.demand[0].availability, DefinitionAvailability::Missing);
}

#[test]
fn idle_driver_keeps_actor_capture_event_driven_and_same_contract_serves_headless_callers() {
    let mut engine = fixture();
    let (_, columns) = matrix(&mut engine);
    assert!(!columns.is_empty());
    let mut driver = ApplicationDriver::default();
    assert!(driver.needs_service());
    driver.service(&engine, golden_io::WorkerNotifier::new(|| {})).unwrap();
    assert!(!driver.needs_service());
    assert!(
        driver.snapshot().is_none(),
        "an idle logical project creates no process runtime"
    );
}

#[test]
fn unloaded_or_discarded_playback_never_creates_authored_history_or_a_phantom_scope() {
    let mut engine = fixture();
    let application = create(&mut engine);
    let (media, columns) = matrix(&mut engine);
    configure_column(&mut engine, media, columns[2], application);
    launch(&mut engine, media, columns[2]);
    let history = engine.undo_len();
    let authored = to_sparse_project_json_pretty(&engine).unwrap();
    crate::matrix::playback::unload(&mut engine, media).unwrap();
    assert_eq!(
        capture_logical_applications(&engine).unwrap().demand.len(),
        1,
        "a queued unload is not committed demand"
    );
    engine.edits.pending.clear();
    assert_eq!(capture_logical_applications(&engine).unwrap().demand.len(), 1);
    let MediaEditResult::MatrixPlayback { snapshot } = edit(
        &mut engine,
        MediaEdit::Matrix {
            media,
            edit: MatrixEdit::ClearPlayback,
        },
    ) else {
        panic!()
    };
    assert_eq!(snapshot.desired_column, None);
    assert!(capture_logical_applications(&engine).unwrap().demand.is_empty());
    assert_eq!(engine.undo_len(), history);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), authored);
}
