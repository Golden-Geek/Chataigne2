use super::*;
use crate::applications::{driver::ApplicationDriver, headless::HeadlessApplications};
use golden_core::{
    application::{ProductionRuntime, ProjectReplacement, ProjectTransactions},
    events::EventKind,
    script::{ScriptNode, ScriptNodeConfig, ScriptSource},
    ui_sync::UiProjectFileSpec,
};
use std::time::{Duration, Instant};

fn launch(engine: &mut AppEngine, media: MediaId, column: ColumnId) {
    edit(
        engine,
        MediaEdit::Matrix {
            media,
            edit: MatrixEdit::LaunchColumn {
                column,
                mode: LaunchMode::Cut,
            },
        },
    );
}
fn transaction(runtime: &ProductionRuntime<AppNode>, edit: MediaEdit) {
    runtime
        .apply_transaction(UiEditIntent::InvokeAppOperation {
            namespace: MEDIA_NAMESPACE.into(),
            operation: MEDIA_OPERATION.into(),
            payload: serde_json::to_value(edit).unwrap(),
        })
        .unwrap();
}
fn snapshot(runtime: &ProductionRuntime<AppNode>) -> ApplicationRuntimeSnapshot {
    let ack = runtime
        .apply_transaction(UiEditIntent::InvokeAppOperation {
            namespace: MEDIA_NAMESPACE.into(),
            operation: MEDIA_OPERATION.into(),
            payload: serde_json::to_value(MediaEdit::Applications {
                edit: ApplicationEdit::InspectRuntime {},
            })
            .unwrap(),
        })
        .unwrap();
    let MediaEditResult::ApplicationRuntime { snapshot } = serde_json::from_value(ack.result.unwrap().value).unwrap()
    else {
        panic!()
    };
    snapshot
}
fn wait(runtime: &ProductionRuntime<AppNode>, phase: ApplicationPhase) -> ApplicationRuntimeSnapshot {
    let deadline = Instant::now() + Duration::from_secs(4);
    loop {
        let snapshot = snapshot(runtime);
        if snapshot
            .applications
            .first()
            .is_some_and(|status| status.phase == phase)
            && snapshot.callbacks.pending == 0
            && snapshot.callbacks.acknowledged > 0
        {
            return snapshot;
        }
        assert!(
            Instant::now() < deadline,
            "headless service did not settle: {snapshot:?}"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn immutable_service_capture_cannot_launch_or_admit_after_definition_or_scope_changes() {
    let mut engine = fixture();
    let application = create(&mut engine);
    let (media, columns) = matrix(&mut engine);
    configure_column(&mut engine, media, columns[2], application);
    launch(&mut engine, media, columns[2]);
    let mut driver = ApplicationDriver::default();
    let prepared = driver.capture_service(&engine).unwrap();
    assert!(driver.snapshot().is_none());
    app_edit(
        &mut engine,
        ApplicationEdit::Configure {
            application,
            definition: ApplicationDefinition {
                executable: Some(std::env::current_exe().unwrap().to_string_lossy().into()),
                ..Default::default()
            },
        },
    );
    assert!(
        driver
            .apply_prepared(prepared, golden_io::WorkerNotifier::new(|| {}))
            .is_err()
    );
    assert!(driver.snapshot().is_none());
    let prepared = driver.capture_service(&engine).unwrap();
    edit(
        &mut engine,
        MediaEdit::Matrix {
            media,
            edit: MatrixEdit::ClearPlayback,
        },
    );
    assert!(
        driver
            .apply_prepared(prepared, golden_io::WorkerNotifier::new(|| {}))
            .is_err()
    );
    assert!(
        driver.snapshot().is_none(),
        "a stale capture must not claim the live runtime"
    );
}

#[test]
fn headless_host_service_observes_launch_unload_and_project_replacement_without_a_renderer() {
    let mut engine = fixture();
    let application = create(&mut engine);
    let (media, columns) = matrix(&mut engine);
    configure_column(&mut engine, media, columns[2], application);
    let manager = crate::media::authoring::manager(&engine).unwrap();
    let mut script = ScriptNode::new("Lifecycle listener", ScriptNodeConfig {
        source: ScriptSource::Inline { text: "function init(){local.listen({level:1});} function onApplicationLifecycle(event){gc.emit('headless-lifecycle',event);}".into() },
    });
    script.budgets.max_wall_time_us_per_load = 1_000_000;
    script.budgets.max_wall_time_us_per_callback = 1_000_000;
    engine.add_node(script.into(), Some(manager));
    engine.apply_edits().unwrap();
    let document = to_sparse_project_json_pretty(&engine).unwrap();
    launch(&mut engine, media, columns[2]);
    let history = engine.undo_len();
    let runtime = ProductionRuntime::new(engine, UiProjectFileSpec::default()).unwrap();
    let service = HeadlessApplications::start(runtime.clone()).unwrap();
    let initial = wait(&runtime, ApplicationPhase::Unavailable);
    assert_eq!(initial.applications[0].application, application);
    transaction(
        &runtime,
        MediaEdit::Matrix {
            media,
            edit: MatrixEdit::ClearPlayback,
        },
    );
    let idle = wait(&runtime, ApplicationPhase::Idle);
    assert_eq!(idle.project, initial.project);
    assert!(idle.callbacks.acknowledged > initial.callbacks.acknowledged);
    let (phases, unchanged_history) = runtime
        .capture_app(move |engine| {
            let phases = engine
                .ui_event_log()
                .iter()
                .filter_map(|event| match &event.kind {
                    EventKind::Custom(event) if event.topic == "headless-lifecycle" => {
                        Some(event.payload["status"]["phase"].as_str().unwrap().to_owned())
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            (phases, engine.undo_len() == history)
        })
        .unwrap();
    assert_eq!(phases, ["Unavailable", "Idle"]);
    assert!(unchanged_history);
    let replacement = from_sparse_project_json::<AppNode>(&document).unwrap();
    runtime
        .replace_project(ProjectReplacement {
            engine: replacement,
            project_file: UiProjectFileSpec::default(),
            reason: "headless service replacement".into(),
            recover: false,
        })
        .unwrap();
    transaction(
        &runtime,
        MediaEdit::Matrix {
            media,
            edit: MatrixEdit::LaunchColumn {
                column: columns[2],
                mode: LaunchMode::Cut,
            },
        },
    );
    let replaced = wait(&runtime, ApplicationPhase::Unavailable);
    assert_ne!(replaced.project, initial.project);
    assert_eq!(replaced.callbacks.acknowledged, 1);
    assert!(replaced.callbacks.fault.is_none());
    drop(service);
}
