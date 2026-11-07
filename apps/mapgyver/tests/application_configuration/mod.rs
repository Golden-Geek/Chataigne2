use super::*;
mod cell_launch;
mod column_groups;
mod scoped_stops;
mod sharing;

#[test]
fn cell_owned_control_launches_without_media_and_preserves_pid_across_neighbor_to_exact_demand() {
    use mapgyver::{matrix::protocol::*, media::protocol::MediaKind};
    fn operation(engine: &mut AppEngine, edit: MediaEdit) -> MediaEditResult {
        let ack = apply_project_intent(
            engine,
            UiEditIntent::InvokeAppOperation {
                namespace: MEDIA_NAMESPACE.into(),
                operation: MEDIA_OPERATION.into(),
                payload: serde_json::to_value(edit).unwrap(),
            },
        );
        assert!(ack.success, "{ack:?}");
        golden_core::application::service_script_app_operations(engine).unwrap();
        serde_json::from_value(ack.result.unwrap().value).unwrap()
    }
    let mut engine = create_new_project_engine::<AppNode>().unwrap();
    engine.apply_edits().unwrap();
    golden_core::app::prepare_engine_for_runtime(&mut engine).unwrap();
    let MediaEditResult::Created { reference } = operation(
        &mut engine,
        MediaEdit::CreateMedia {
            media_kind: MediaKind::Matrix,
            label: None,
        },
    ) else {
        panic!()
    };
    let MediaEditResult::MatrixCreated { items, .. } = operation(
        &mut engine,
        MediaEdit::Matrix {
            media: reference.media,
            edit: MatrixEdit::AddLayers { parent: None, count: 1 },
        },
    ) else {
        panic!()
    };
    let MatrixItemId::Layer { id: layer } = items[0] else {
        panic!()
    };
    let MediaEditResult::MatrixCreated { columns, .. } = operation(
        &mut engine,
        MediaEdit::Matrix {
            media: reference.media,
            edit: MatrixEdit::AddColumns { parent: None, count: 3 },
        },
    ) else {
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
    let MediaEditResult::MatrixCellPrepared { application, .. } = operation(
        &mut engine,
        MediaEdit::Matrix {
            media: reference.media,
            edit: MatrixEdit::ConfigureCellApplication {
                layer,
                column: columns[1],
                definition,
                pre: 1,
                post: 0,
            },
        },
    ) else {
        panic!()
    };
    let mut runtime = ApplicationRuntime::start(&engine, None).unwrap();
    let retirement = runtime.retirement();
    let mut first = None;
    for column in &columns[..2] {
        operation(
            &mut engine,
            MediaEdit::Matrix {
                media: reference.media,
                edit: MatrixEdit::LaunchColumn {
                    column: *column,
                    mode: LaunchMode::Cut,
                },
            },
        );
        let capture = mapgyver::applications::logical::capture_logical_applications(&engine).unwrap();
        assert_eq!(capture.demand.len(), 1);
        assert_eq!(capture.demand[0].application, application);
        runtime.reconcile(&engine, &capture.demand).unwrap();
        let status = wait(&runtime, ApplicationPhase::Ready);
        if let Some((pid, launch)) = first {
            assert_eq!((status.process_id, status.launch), (pid, launch));
        } else {
            first = Some((status.process_id, status.launch));
        }
    }
    operation(
        &mut engine,
        MediaEdit::Matrix {
            media: reference.media,
            edit: MatrixEdit::LaunchColumn {
                column: columns[2],
                mode: LaunchMode::Cut,
            },
        },
    );
    let capture = mapgyver::applications::logical::capture_logical_applications(&engine).unwrap();
    assert!(capture.demand.is_empty());
    runtime.reconcile(&engine, &capture.demand).unwrap();
    wait(&runtime, ApplicationPhase::Idle);
    runtime.close();
    drop(runtime);
    assert!(retirement.wait_for_idle(Duration::from_secs(4)));
}

#[test]
fn equivalent_path_and_environment_edits_preserve_the_actual_owned_process() {
    let (mut engine, application) = fixture(ApplicationReadiness::ProcessStarted);
    let mut definition = mapgyver::applications::inspect(&engine, application)
        .unwrap()
        .definition;
    definition.environment = Some(vec![("B".into(), "two".into()), ("A".into(), "one".into())]);
    edit(
        &mut engine,
        ApplicationEdit::Configure {
            application,
            definition: definition.clone(),
        },
    );
    let mut runtime = ApplicationRuntime::start(&engine, None).unwrap();
    let retirement = runtime.retirement();
    runtime
        .reconcile(&engine, &demand(application, &[DemandReason::Active]))
        .unwrap();
    let first = wait(&runtime, ApplicationPhase::Ready);
    assert!(first.process_id.is_some());
    let executable = std::path::PathBuf::from(definition.executable.as_ref().unwrap());
    definition.executable = Some(
        executable
            .parent()
            .unwrap()
            .join(".")
            .join(executable.file_name().unwrap())
            .to_str()
            .unwrap()
            .replace('\\', "/"),
    );
    definition.environment.as_mut().unwrap().reverse();
    edit(
        &mut engine,
        ApplicationEdit::Configure {
            application,
            definition: definition.clone(),
        },
    );
    let expected = mapgyver::applications::inspect(&engine, application).unwrap();
    let authored = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.ui_history_state();
    runtime
        .reconcile(&engine, &demand(application, &[DemandReason::Active]))
        .unwrap();
    let current = observe(&runtime, |status| {
        status.phase == ApplicationPhase::Ready
            && status.consumers.iter().any(|consumer| {
                consumer.application == application && consumer.definition_revision == Some(expected.revision)
            })
    });
    assert_eq!(current.configuration, ApplicationConfigurationState::Current);
    assert_eq!(current.process_id, first.process_id);
    assert_eq!(current.launch, first.launch);
    std::thread::sleep(Duration::from_millis(80));
    assert_eq!(wait(&runtime, ApplicationPhase::Ready).launch, first.launch);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), authored);
    assert_eq!(engine.ui_history_state(), history);
    assert_eq!(
        mapgyver::applications::inspect(&engine, application)
            .unwrap()
            .definition,
        definition
    );
    runtime.close();
    wait(&runtime, ApplicationPhase::Idle);
    drop(runtime);
    assert!(retirement.wait_for_idle(Duration::from_secs(4)));
}
