#![cfg(all(windows, feature = "application-process-fixture"))]

use golden_core::{
    app::{create_new_project_engine, from_sparse_project_json, to_sparse_project_json_pretty},
    application::apply_project_intent,
    ui_sync::UiEditIntent,
};
use mapgyver::{
    app::{AppEngine, AppNode},
    applications::{
        demand::{ApplicationDemand, DefinitionAvailability, DemandReason},
        protocol::*,
        runtime::{ApplicationRuntime, ReadinessEvidence},
    },
    media::protocol::{MEDIA_NAMESPACE, MEDIA_OPERATION, MediaEdit, MediaEditResult},
};
use std::time::{Duration, Instant};

fn edit(engine: &mut AppEngine, edit: ApplicationEdit) -> MediaEditResult {
    let ack = apply_project_intent(
        engine,
        UiEditIntent::InvokeAppOperation {
            namespace: MEDIA_NAMESPACE.into(),
            operation: MEDIA_OPERATION.into(),
            payload: serde_json::to_value(MediaEdit::Applications { edit }).unwrap(),
        },
    );
    assert!(ack.success, "{ack:?}");
    serde_json::from_value(ack.result.unwrap().value).unwrap()
}
fn fixture(readiness: ApplicationReadiness) -> (AppEngine, ApplicationId) {
    let mut engine = create_new_project_engine().unwrap();
    engine.apply_edits().unwrap();
    let MediaEditResult::ApplicationCreated { application } =
        edit(&mut engine, ApplicationEdit::Create { label: None })
    else {
        panic!()
    };
    edit(
        &mut engine,
        ApplicationEdit::Configure {
            application,
            definition: ApplicationDefinition {
                executable: Some(env!("CARGO_BIN_EXE_application_process_fixture").into()),
                readiness,
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
        },
    );
    (engine, application)
}
fn demand(application: ApplicationId, reasons: &[DemandReason]) -> Vec<ApplicationDemand> {
    vec![ApplicationDemand {
        application,
        availability: DefinitionAvailability::Configured,
        reasons: reasons.iter().copied().collect(),
    }]
}
fn wait(runtime: &ApplicationRuntime, phase: ApplicationPhase) -> ApplicationRuntimeStatus {
    observe(runtime, |status| status.phase == phase)
}
fn observe(
    runtime: &ApplicationRuntime,
    expected: impl Fn(&ApplicationRuntimeStatus) -> bool,
) -> ApplicationRuntimeStatus {
    let deadline = Instant::now() + Duration::from_secs(4);
    loop {
        let snapshot = runtime.snapshot();
        if let Some(status) = snapshot.applications.first()
            && expected(status)
        {
            return status.clone();
        }
        assert!(
            Instant::now() < deadline,
            "expected lifecycle observation, got {snapshot:?}"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn actual_owned_process_shared_demand_missing_definition_and_final_release_preserve_identity_and_history() {
    let (mut engine, application) = fixture(ApplicationReadiness::ProcessStarted);
    let mut runtime = ApplicationRuntime::start(&engine, None).unwrap();
    let retirement = runtime.retirement();
    let history = engine.undo_len();
    runtime
        .reconcile(
            &engine,
            &demand(application, &[DemandReason::Neighbor, DemandReason::Active]),
        )
        .unwrap();
    let first = wait(&runtime, ApplicationPhase::Ready);
    assert!(first.process_id.is_some());
    for _ in 0..3 {
        runtime
            .reconcile(
                &engine,
                &demand(application, &[DemandReason::Preparation, DemandReason::Outgoing]),
            )
            .unwrap();
    }
    std::thread::sleep(Duration::from_millis(60));
    let shared = wait(&runtime, ApplicationPhase::Ready);
    assert_eq!(shared.launch, first.launch);
    assert_eq!(shared.process_id, first.process_id);
    let MediaEditResult::ApplicationRuntime { snapshot } = edit(&mut engine, ApplicationEdit::InspectRuntime {}) else {
        panic!()
    };
    assert_eq!(snapshot.applications[0].launch, first.launch);
    assert_eq!(engine.undo_len(), history);
    edit(&mut engine, ApplicationEdit::Remove { application });
    runtime
        .reconcile(&engine, &demand(application, &[DemandReason::FinalUse]))
        .unwrap();
    std::thread::sleep(Duration::from_millis(60));
    assert_eq!(wait(&runtime, ApplicationPhase::Ready).process_id, first.process_id);
    assert!(engine.undo().unwrap());
    runtime
        .reconcile(&engine, &demand(application, &[DemandReason::Active]))
        .unwrap();
    assert_eq!(wait(&runtime, ApplicationPhase::Ready).process_id, first.process_id);
    let mut definition = mapgyver::applications::inspect(&engine, application)
        .unwrap()
        .definition;
    definition.arguments.push("replacement".into());
    edit(
        &mut engine,
        ApplicationEdit::Configure {
            application,
            definition,
        },
    );
    runtime
        .reconcile(
            &engine,
            &demand(application, &[DemandReason::Outgoing, DemandReason::FinalUse]),
        )
        .unwrap();
    std::thread::sleep(Duration::from_millis(60));
    assert_eq!(wait(&runtime, ApplicationPhase::Ready).launch, first.launch);
    runtime
        .reconcile(&engine, &demand(application, &[DemandReason::Active]))
        .unwrap();
    observe(&runtime, |status| {
        status.phase == ApplicationPhase::Ready && status.launch != first.launch
    });
    runtime.close();
    let stopped = wait(&runtime, ApplicationPhase::Idle);
    assert_eq!(stopped.process_id, None);
    assert_eq!(stopped.exit_code, Some(1));
    assert_eq!(engine.undo_len(), history + 1);
    drop(runtime);
    assert!(retirement.wait_for_idle(Duration::from_secs(4)));
}

#[test]
fn owned_process_accepts_only_current_tagged_readiness_evidence_and_project_replacement_stops_it() {
    let (engine, application) = fixture(ApplicationReadiness::Spout {
        sender: "owned-renderer".into(),
        first_frame: true,
        timeout_ms: 3000,
    });
    let document = to_sparse_project_json_pretty(&engine).unwrap();
    let replacement = from_sparse_project_json::<AppNode>(&document).unwrap();
    let mut runtime = ApplicationRuntime::start(&engine, None).unwrap();
    let retirement = runtime.retirement();
    let requested = demand(application, &[DemandReason::Preparation]);
    runtime.reconcile(&engine, &requested).unwrap();
    let waiting = wait(&runtime, ApplicationPhase::WaitingForSender);
    let evidence = ReadinessEvidence {
        project: runtime.snapshot().project,
        application,
        launch: waiting.launch.unwrap(),
        sender: "owned-renderer".into(),
        first_frame: false,
    };
    runtime.readiness(evidence.clone()).unwrap();
    std::thread::sleep(Duration::from_millis(60));
    assert_eq!(
        runtime.snapshot().applications[0].phase,
        ApplicationPhase::WaitingForSender
    );
    runtime
        .readiness(ReadinessEvidence {
            first_frame: true,
            ..evidence.clone()
        })
        .unwrap();
    wait(&runtime, ApplicationPhase::Ready);
    runtime.reconcile(&engine, &[]).unwrap();
    wait(&runtime, ApplicationPhase::Idle);
    runtime.reconcile(&engine, &requested).unwrap();
    let next = wait(&runtime, ApplicationPhase::WaitingForSender);
    assert_ne!(next.launch, waiting.launch);
    assert!(runtime.readiness(evidence.clone()).is_err());
    assert!(runtime.reconcile(&replacement, &requested).is_err());
    assert!(
        runtime
            .readiness(ReadinessEvidence {
                project: uuid::Uuid::new_v4(),
                launch: next.launch.unwrap(),
                ..evidence.clone()
            })
            .is_err()
    );
    drop(engine);
    wait(&runtime, ApplicationPhase::Idle);
    assert!(
        runtime
            .readiness(ReadinessEvidence {
                launch: next.launch.unwrap(),
                ..evidence
            })
            .is_err()
    );
    drop(runtime);
    assert!(retirement.wait_for_idle(Duration::from_secs(4)));
}

#[test]
fn matrix_neighbors_and_actual_process_ownership_follow_renderer_final_completion_after_project_close() {
    use golden_video::{
        Extent2D, OutputId,
        plan::PlanLimits,
        runtime::{NullConfig, ResourceLimits, RuntimeConfig, VideoRuntime},
    };
    use mapgyver::{
        matrix::protocol::*,
        media::{
            capture_channels,
            rendering::{ChannelRenderer, ChannelRoute},
        },
    };
    use std::num::NonZeroUsize;
    fn media(engine: &mut AppEngine, operation: MediaEdit) -> MediaEditResult {
        let ack = apply_project_intent(
            engine,
            UiEditIntent::InvokeAppOperation {
                namespace: MEDIA_NAMESPACE.into(),
                operation: MEDIA_OPERATION.into(),
                payload: serde_json::to_value(operation).unwrap(),
            },
        );
        assert!(ack.success, "{ack:?}");
        serde_json::from_value(ack.result.unwrap().value).unwrap()
    }
    fn installed(renderer: &ChannelRenderer, request: golden_video::runtime::RequestId) {
        let until = Instant::now() + Duration::from_secs(4);
        loop {
            let mut outcomes = Vec::new();
            renderer.drain_outcomes(&mut outcomes, NonZeroUsize::new(16).unwrap());
            if let Some(outcome) = outcomes.iter().find(|outcome| outcome.request == request) {
                assert!(
                    matches!(outcome.result, golden_video::runtime::OutcomeKind::Installed { .. }),
                    "{outcome:?}"
                );
                return;
            }
            assert!(Instant::now() < until, "renderer installation deadline");
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    let (mut engine, application) = fixture(ApplicationReadiness::ProcessStarted);
    let MediaEditResult::Created { reference } = media(
        &mut engine,
        MediaEdit::CreateMedia {
            media_kind: mapgyver::media::protocol::MediaKind::Matrix,
            label: None,
        },
    ) else {
        panic!()
    };
    let matrix = |engine: &mut AppEngine, edit| {
        media(
            engine,
            MediaEdit::Matrix {
                media: reference.media,
                edit,
            },
        )
    };
    matrix(
        &mut engine,
        MatrixEdit::Configure {
            definition: MatrixDefinition {
                canvas: Extent2D::new(2, 2).unwrap(),
                ..Default::default()
            },
        },
    );
    let MediaEditResult::MatrixCreated { columns, .. } = matrix(&mut engine, MatrixEdit::AddColumns { count: 3 })
    else {
        panic!()
    };
    matrix(
        &mut engine,
        MatrixEdit::ConfigureColumn {
            column: columns[2],
            definition: ColumnDefinition {
                applications: vec![ColumnApplication {
                    application,
                    pre: 1,
                    post: 1,
                }],
                ..Default::default()
            },
        },
    );
    let mut runtime = ApplicationRuntime::start(&engine, None).unwrap();
    let process_retirement = runtime.retirement();
    matrix(
        &mut engine,
        MatrixEdit::LaunchColumn {
            column: columns[1],
            mode: LaunchMode::Cut,
        },
    );
    let preload = capture_channels(&engine, &[reference], 8).unwrap();
    assert_eq!(
        preload.application_demand()[0].reasons,
        [DemandReason::Neighbor].into_iter().collect()
    );
    runtime.reconcile(&engine, &preload.application_demand()).unwrap();
    let process = wait(&runtime, ApplicationPhase::Ready);
    let config = RuntimeConfig {
        plan_limits: PlanLimits {
            max_passes: 16,
            max_external_textures: 16,
            max_outputs: 4,
            max_dependencies: 64,
            max_texture_dimension: 4096,
            max_texture_bytes: 64 * 1024 * 1024,
        },
        resource_limits: ResourceLimits {
            max_bytes: 128 * 1024 * 1024,
            max_in_flight: NonZeroUsize::new(2).unwrap(),
            max_retiring: 2,
        },
        frame_rate: golden_time::RationalRate::new(60, 1).unwrap(),
        max_outstanding: NonZeroUsize::new(16).unwrap(),
        max_input_bytes: NonZeroUsize::new(65536).unwrap(),
        max_retained_input_bytes: NonZeroUsize::new(262144).unwrap(),
        recovery_delay: Duration::from_millis(10),
        max_recovery_delay: Duration::from_millis(100),
        notifier: None,
    };
    let video = VideoRuntime::spawn_null(
        config.clone(),
        NullConfig {
            auto_complete: false,
            ..Default::default()
        },
    )
    .unwrap();
    let control = video.null_control().unwrap();
    let retirement = video.retirement();
    let mut renderer = ChannelRenderer::new(video, config.plan_limits);
    renderer.bind_applications(runtime.plan_retainer());
    let startup = Instant::now() + Duration::from_secs(4);
    while renderer.snapshot().resources.is_none() {
        assert!(Instant::now() < startup, "renderer startup deadline");
        std::thread::sleep(Duration::from_millis(2));
    }
    matrix(
        &mut engine,
        MatrixEdit::LaunchColumn {
            column: columns[2],
            mode: LaunchMode::Cut,
        },
    );
    let capture = capture_channels(&engine, &[reference], 8).unwrap();
    runtime.reconcile(&engine, &capture.application_demand()).unwrap();
    let routes = || {
        vec![ChannelRoute {
            output: OutputId::new(1).unwrap(),
            channel: reference,
        }]
    };
    let first = renderer.publish(capture, routes()).unwrap();
    installed(&renderer, first);
    let until = Instant::now() + Duration::from_secs(4);
    while renderer.snapshot().frames_submitted == 0 {
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(2));
    }
    matrix(
        &mut engine,
        MatrixEdit::LaunchColumn {
            column: columns[0],
            mode: LaunchMode::Cut,
        },
    );
    let next = capture_channels(&engine, &[reference], 8).unwrap();
    assert!(next.application_demand().is_empty());
    runtime.reconcile(&engine, &[]).unwrap();
    let second = renderer.publish(next, routes()).unwrap();
    installed(&renderer, second);
    drop(engine);
    runtime.close();
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(
        wait(&runtime, ApplicationPhase::Ready).process_id,
        process.process_id,
        "project close cannot stop a process still used by an unfinished GPU plan"
    );
    assert!(control.complete_through(control.snapshot().last_submitted.unwrap()));
    wait(&runtime, ApplicationPhase::Idle);
    renderer.close();
    drop(renderer);
    let until = Instant::now() + Duration::from_secs(4);
    loop {
        if let Some(token) = control.snapshot().last_submitted {
            control.complete_through(token);
        }
        if retirement.wait(Duration::from_millis(5)) {
            break;
        }
        assert!(Instant::now() < until, "renderer retirement deadline");
    }
    drop(runtime);
    assert!(process_retirement.wait_for_idle(Duration::from_secs(4)));
}

#[test]
fn project_wide_driver_starts_and_unloads_shared_timed_column_demand_with_no_renderer_or_clock() {
    use mapgyver::{applications::driver::ApplicationDriver, matrix::protocol::*};
    fn matrix_edit(
        engine: &mut AppEngine,
        media: mapgyver::media::protocol::MediaId,
        edit: MatrixEdit,
    ) -> MediaEditResult {
        let ack = apply_project_intent(
            engine,
            UiEditIntent::InvokeAppOperation {
                namespace: MEDIA_NAMESPACE.into(),
                operation: MEDIA_OPERATION.into(),
                payload: serde_json::to_value(MediaEdit::Matrix { media, edit }).unwrap(),
            },
        );
        assert!(ack.success, "{ack:?}");
        serde_json::from_value(ack.result.unwrap().value).unwrap()
    }
    fn matrix(
        engine: &mut AppEngine,
        application: ApplicationId,
    ) -> (mapgyver::media::protocol::MediaId, Vec<ColumnId>) {
        let ack = apply_project_intent(
            engine,
            UiEditIntent::InvokeAppOperation {
                namespace: MEDIA_NAMESPACE.into(),
                operation: MEDIA_OPERATION.into(),
                payload: serde_json::to_value(MediaEdit::CreateMedia {
                    media_kind: mapgyver::media::protocol::MediaKind::Matrix,
                    label: None,
                })
                .unwrap(),
            },
        );
        assert!(ack.success, "{ack:?}");
        let MediaEditResult::Created { reference } = serde_json::from_value(ack.result.unwrap().value).unwrap() else {
            panic!()
        };
        let MediaEditResult::MatrixCreated { columns, .. } =
            matrix_edit(engine, reference.media, MatrixEdit::AddColumns { count: 3 })
        else {
            panic!()
        };
        matrix_edit(
            engine,
            reference.media,
            MatrixEdit::ConfigureColumn {
                column: columns[2],
                definition: ColumnDefinition {
                    applications: vec![ColumnApplication {
                        application,
                        pre: 1,
                        post: 1,
                    }],
                    ..Default::default()
                },
            },
        );
        (reference.media, columns)
    }
    fn service(driver: &mut ApplicationDriver, engine: &AppEngine) {
        driver.service(engine, golden_io::WorkerNotifier::new(|| {})).unwrap();
    }
    fn phase(driver: &ApplicationDriver, expected: ApplicationPhase) -> ApplicationRuntimeStatus {
        let until = Instant::now() + Duration::from_secs(4);
        loop {
            let snapshot = driver.snapshot();
            if let Some(status) = snapshot.as_ref().and_then(|snapshot| snapshot.applications.first())
                && status.phase == expected
            {
                return status.clone();
            }
            assert!(Instant::now() < until, "expected {expected:?}, got {snapshot:?}");
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    let (mut engine, application) = fixture(ApplicationReadiness::ProcessStarted);
    let (first, columns) = matrix(&mut engine, application);
    let (second, other) = matrix(&mut engine, application);
    let history = engine.undo_len();
    let authored = to_sparse_project_json_pretty(&engine).unwrap();
    let mut driver = ApplicationDriver::default();
    service(&mut driver, &engine);
    assert!(driver.snapshot().is_none());
    assert!(!driver.needs_service());
    matrix_edit(
        &mut engine,
        first,
        MatrixEdit::LaunchColumn {
            column: columns[1],
            mode: LaunchMode::Configured,
        },
    );
    assert!(driver.needs_service());
    service(&mut driver, &engine);
    let original = phase(&driver, ApplicationPhase::Ready);
    let retirement = driver.retirement().unwrap();
    assert!(!driver.needs_service());
    matrix_edit(
        &mut engine,
        second,
        MatrixEdit::LaunchColumn {
            column: other[2],
            mode: LaunchMode::Configured,
        },
    );
    service(&mut driver, &engine);
    matrix_edit(&mut engine, first, MatrixEdit::ClearPlayback);
    service(&mut driver, &engine);
    std::thread::sleep(Duration::from_millis(80));
    assert_eq!(phase(&driver, ApplicationPhase::Ready).process_id, original.process_id);
    assert_eq!(driver.snapshot().unwrap().applications.len(), 1);
    matrix_edit(&mut engine, second, MatrixEdit::ClearPlayback);
    service(&mut driver, &engine);
    phase(&driver, ApplicationPhase::Idle);
    assert_eq!(engine.undo_len(), history);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), authored);
    drop(driver);
    assert!(retirement.wait_for_idle(Duration::from_secs(4)));
}
