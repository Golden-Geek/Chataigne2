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
