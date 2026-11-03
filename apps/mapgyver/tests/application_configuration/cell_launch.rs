use super::*;
use mapgyver::{matrix::protocol::*, media::protocol::MediaKind};

#[test]
fn disabled_destination_keeps_shared_column_and_cell_process_without_a_renderer_through_history() {
    use golden_core::ui_sync::NodeMetaPatch;
    use mapgyver::applications::logical::capture_logical_applications;
    let mut engine = create_new_project_engine::<AppNode>().unwrap();
    engine.apply_edits().unwrap();
    let MediaEditResult::Created { reference } = media_edit(
        &mut engine,
        MediaEdit::CreateMedia {
            media_kind: MediaKind::Matrix,
            label: None,
        },
    ) else {
        panic!()
    };
    let matrix = |engine: &mut AppEngine, edit| {
        media_edit(
            engine,
            MediaEdit::Matrix {
                media: reference.media,
                edit,
            },
        )
    };
    let MediaEditResult::MatrixCreated { columns, .. } = matrix(&mut engine, MatrixEdit::AddColumns { count: 2 })
    else {
        panic!()
    };
    let MediaEditResult::MatrixCreated { items, .. } =
        matrix(&mut engine, MatrixEdit::AddLayers { parent: None, count: 1 })
    else {
        panic!()
    };
    let MatrixItemId::Layer { id: layer } = items[0] else {
        panic!()
    };
    let definition = ApplicationDefinition {
        executable: Some(env!("CARGO_BIN_EXE_application_process_fixture").into()),
        stop: ApplicationStop {
            graceful_ms: 30,
            retirement_ms: 1000,
        },
        restart: ApplicationRestart {
            enabled: false,
            ..Default::default()
        },
        ..Default::default()
    };
    matrix(
        &mut engine,
        MatrixEdit::ConfigureColumnApplication {
            column: columns[0],
            definition: definition.clone(),
            pre: 0,
            post: 0,
        },
    );
    let mut cells = Vec::new();
    for column in &columns {
        let MediaEditResult::MatrixCellPrepared { cell, .. } = matrix(
            &mut engine,
            MatrixEdit::ConfigureCellApplication {
                layer,
                column: *column,
                definition: definition.clone(),
                pre: 0,
                post: 0,
            },
        ) else {
            panic!()
        };
        cells.push(cell);
    }
    let node = engine.node_id_by_uuid(cells[1].0).unwrap();
    assert!(
        apply_project_intent(
            &mut engine,
            UiEditIntent::PatchMeta {
                node,
                patch: NodeMetaPatch {
                    enabled: Some(false),
                    ..Default::default()
                },
            }
        )
        .success
    );
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    let mut runtime = ApplicationRuntime::start(&engine, None).unwrap();
    let retirement = runtime.retirement();
    let mut first = None;
    for column in &columns {
        matrix(
            &mut engine,
            MatrixEdit::LaunchColumn {
                column: *column,
                mode: LaunchMode::Cut,
            },
        );
        let capture = capture_logical_applications(&engine).unwrap();
        assert_eq!(capture.demand.len(), 2);
        runtime.reconcile(&engine, &capture.demand).unwrap();
        let ready = observe(&runtime, |status| {
            status.phase == ApplicationPhase::Ready && status.consumers.len() == 2
        });
        assert_eq!(runtime.snapshot().applications.len(), 1);
        if let Some((pid, launch)) = first {
            assert_eq!((ready.process_id, ready.launch), (pid, launch));
        } else {
            first = Some((ready.process_id, ready.launch));
        }
    }
    assert_eq!(engine.undo_len(), history);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), saved);
    matrix(
        &mut engine,
        MatrixEdit::ReorderColumn {
            column: columns[0],
            after: Some(columns[1]),
        },
    );
    runtime
        .reconcile(&engine, &capture_logical_applications(&engine).unwrap().demand)
        .unwrap();
    assert_eq!(wait(&runtime, ApplicationPhase::Ready).process_id, first.unwrap().0);
    assert!(engine.undo().unwrap());
    matrix(&mut engine, MatrixEdit::RemoveColumn { column: columns[0] });
    assert!(capture_logical_applications(&engine).unwrap().demand.is_empty());
    assert!(engine.undo().unwrap());
    let restored = capture_logical_applications(&engine).unwrap();
    assert_eq!(restored.demand.len(), 2);
    runtime.reconcile(&engine, &restored.demand).unwrap();
    assert_eq!(wait(&runtime, ApplicationPhase::Ready).process_id, first.unwrap().0);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), saved);
    // Saving restores authored owner IDs, never an OS-process or playback lease.
    let loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
    assert!(capture_logical_applications(&loaded).unwrap().demand.is_empty());
    matrix(&mut engine, MatrixEdit::ClearPlayback);
    runtime
        .reconcile(&engine, &capture_logical_applications(&engine).unwrap().demand)
        .unwrap();
    wait(&runtime, ApplicationPhase::Idle);
    drop(runtime);
    assert!(retirement.wait_for_idle(Duration::from_secs(4)));
}

#[test]
fn direct_cell_without_column_anchor_launches_its_owned_process_and_unload_retires_it() {
    let mut engine = create_new_project_engine::<AppNode>().unwrap();
    engine.apply_edits().unwrap();
    let MediaEditResult::Created { reference } = media_edit(
        &mut engine,
        MediaEdit::CreateMedia {
            media_kind: MediaKind::Matrix,
            label: None,
        },
    ) else {
        panic!()
    };
    let matrix = |engine: &mut AppEngine, edit| {
        media_edit(
            engine,
            MediaEdit::Matrix {
                media: reference.media,
                edit,
            },
        )
    };
    let MediaEditResult::MatrixCreated { columns, .. } = matrix(&mut engine, MatrixEdit::AddColumns { count: 2 })
    else {
        panic!()
    };
    let MediaEditResult::MatrixCreated { items, .. } =
        matrix(&mut engine, MatrixEdit::AddLayers { parent: None, count: 1 })
    else {
        panic!()
    };
    let MatrixItemId::Layer { id: layer } = items[0] else {
        panic!()
    };
    let MediaEditResult::MatrixCellPrepared { application, .. } = matrix(
        &mut engine,
        MatrixEdit::ConfigureCellApplication {
            layer,
            column: columns[1],
            definition: ApplicationDefinition {
                executable: Some(env!("CARGO_BIN_EXE_application_process_fixture").into()),
                stop: ApplicationStop {
                    graceful_ms: 30,
                    retirement_ms: 1000,
                },
                restart: ApplicationRestart {
                    enabled: false,
                    ..Default::default()
                },
                ..Default::default()
            },
            pre: 0,
            post: 0,
        },
    ) else {
        panic!()
    };
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    let MediaEditResult::MatrixLaunched { snapshot } = matrix(
        &mut engine,
        MatrixEdit::LaunchCell {
            layer,
            column: columns[1],
            mode: LaunchMode::Configured,
        },
    ) else {
        panic!()
    };
    assert_eq!(snapshot.desired_column, None);
    assert_eq!(snapshot.launch_id, None);
    let capture = mapgyver::applications::logical::capture_logical_applications(&engine).unwrap();
    assert_eq!(capture.demand.len(), 1);
    assert_eq!(capture.demand[0].application, application);
    let mut runtime = ApplicationRuntime::start(&engine, None).unwrap();
    let retirement = runtime.retirement();
    runtime.reconcile(&engine, &capture.demand).unwrap();
    assert!(wait(&runtime, ApplicationPhase::Ready).process_id.is_some());
    assert_eq!(engine.undo_len(), history);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), saved);
    matrix(&mut engine, MatrixEdit::ClearPlayback);
    assert!(
        mapgyver::applications::logical::capture_logical_applications(&engine)
            .unwrap()
            .demand
            .is_empty()
    );
    runtime.reconcile(&engine, &[]).unwrap();
    wait(&runtime, ApplicationPhase::Idle);
    drop(runtime);
    assert!(retirement.wait_for_idle(Duration::from_secs(4)));
}
