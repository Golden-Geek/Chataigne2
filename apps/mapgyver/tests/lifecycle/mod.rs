use super::*;
use std::{
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
    path::{Path, PathBuf},
};
use windows::Win32::{
    Foundation::{HANDLE, WAIT_OBJECT_0},
    System::Threading::{OpenProcess, PROCESS_ACCESS_RIGHTS, WaitForSingleObject},
};

fn monitor(id: u32) -> OwnedHandle {
    // SYNCHRONIZE only; pin the known fixture lifetime before requesting cleanup.
    unsafe { OwnedHandle::from_raw_handle(OpenProcess(PROCESS_ACCESS_RIGHTS(0x0010_0000), false, id).unwrap().0) }
}
fn exited(handle: &OwnedHandle) -> bool {
    unsafe { WaitForSingleObject(HANDLE(handle.as_raw_handle()), 0) == WAIT_OBJECT_0 }
}
fn tree_fixture() -> PathBuf {
    let executable = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("golden_io_process_fixture.exe");
    assert!(executable.is_file(), "build golden_io with process-fixture first");
    executable
}
fn tree_definition(report: &Path, mode: &str) -> ApplicationDefinition {
    ApplicationDefinition {
        executable: Some(tree_fixture().to_string_lossy().into()),
        arguments: vec![
            mode.into(),
            report.to_string_lossy().into(),
            report.with_extension("stop").to_string_lossy().into(),
        ],
        stop: ApplicationStop {
            graceful_ms: 150,
            retirement_ms: 1500,
        },
        restart: ApplicationRestart {
            enabled: false,
            ..Default::default()
        },
        ..Default::default()
    }
}
fn tree_handles(report: &Path) -> Vec<OwnedHandle> {
    let until = Instant::now() + Duration::from_secs(3);
    let paths = [
        report.to_owned(),
        report.with_extension("branch"),
        report.with_extension("leaf"),
    ];
    while paths.iter().any(|path| !path.exists()) {
        assert!(Instant::now() < until, "owned tree fixture did not report descendants");
        std::thread::sleep(Duration::from_millis(5));
    }
    paths
        .iter()
        .map(|path| {
            monitor(
                std::fs::read_to_string(path)
                    .unwrap()
                    .lines()
                    .next()
                    .unwrap()
                    .parse()
                    .unwrap(),
            )
        })
        .collect()
}

#[test]
fn actual_crashes_advance_capped_backoff_and_intentional_unload_cancels_all_retries() {
    let (mut engine, application) =
        fixture_arguments(ApplicationReadiness::ProcessStarted, vec!["--lifecycle-exit".into()]);
    let mut definition = mapgyver::applications::inspect(&engine, application)
        .unwrap()
        .definition;
    definition.restart = ApplicationRestart {
        enabled: true,
        initial_ms: 150,
        maximum_ms: 300,
    };
    edit(
        &mut engine,
        ApplicationEdit::Configure {
            application,
            definition,
        },
    );
    let mut runtime = ApplicationRuntime::start(&engine, None).unwrap();
    let retirement = runtime.retirement();
    runtime
        .reconcile(&engine, &demand(application, &[DemandReason::Active]))
        .unwrap();
    let mut deadlines = Vec::new();
    let mut launches = Vec::new();
    for failures in 1..=3 {
        let status = observe(&runtime, |status| {
            status.phase == ApplicationPhase::Backoff && status.failures == failures
        });
        assert_eq!(status.exit_code, Some(23));
        let deadline = status.next_action_ms.unwrap();
        let launch = status.launch.unwrap();
        let remaining = deadline.saturating_sub(runtime.snapshot().elapsed_ms);
        assert!(
            remaining <= if failures == 1 { 150 } else { 300 },
            "restart exceeded its configured backoff cap"
        );
        if let Some(previous) = deadlines.last() {
            assert!(deadline >= previous + 300);
        }
        deadlines.push(deadline);
        launches.push(launch);
        loop {
            let snapshot = runtime.snapshot();
            if snapshot.elapsed_ms >= deadline {
                break;
            }
            assert_eq!(
                snapshot.applications[0].launch,
                Some(launch),
                "restart before its backoff deadline"
            );
            if failures == 3 {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    assert!(launches.windows(2).all(|pair| pair[0] != pair[1]));
    runtime.reconcile(&engine, &[]).unwrap();
    let stopped = wait(&runtime, ApplicationPhase::Idle);
    std::thread::sleep(Duration::from_millis(400));
    let final_status = runtime.snapshot().applications[0].clone();
    assert_eq!(final_status.phase, ApplicationPhase::Idle);
    assert_eq!(final_status.launch, stopped.launch);
    assert_eq!(final_status.next_action_ms, None);
    drop(runtime);
    assert!(retirement.wait_for_idle(Duration::from_secs(3)));
}

#[test]
#[ignore = "requires the explicitly built Golden IO process fixture"]
fn owned_tree_forced_stop_and_retired_project_close_every_pinned_descendant() {
    let directory = tempfile::tempdir().unwrap();
    let report = directory.path().join("tree.root");
    let (mut engine, application) = fixture(ApplicationReadiness::ProcessStarted);
    edit(
        &mut engine,
        ApplicationEdit::Configure {
            application,
            definition: tree_definition(&report, "tree"),
        },
    );
    let mut runtime = ApplicationRuntime::start(&engine, None).unwrap();
    let retirement = runtime.retirement();
    runtime
        .reconcile(&engine, &demand(application, &[DemandReason::Active]))
        .unwrap();
    wait(&runtime, ApplicationPhase::Ready);
    let handles = tree_handles(&report);
    assert!(handles.iter().all(|handle| !exited(handle)));
    std::thread::sleep(Duration::from_millis(75));
    runtime.reconcile(&engine, &[]).unwrap();
    let stopping = wait(&runtime, ApplicationPhase::Stopping);
    assert!(stopping.next_action_ms.is_some());
    wait(&runtime, ApplicationPhase::Idle);
    assert!(handles.iter().all(exited));
    let next = directory.path().join("replacement.root");
    edit(
        &mut engine,
        ApplicationEdit::Configure {
            application,
            definition: tree_definition(&next, "tree"),
        },
    );
    runtime
        .reconcile(&engine, &demand(application, &[DemandReason::Active]))
        .unwrap();
    wait(&runtime, ApplicationPhase::Ready);
    let handles = tree_handles(&next);
    std::thread::sleep(Duration::from_millis(75));
    drop(engine);
    wait(&runtime, ApplicationPhase::Idle);
    assert!(handles.iter().all(exited));
    drop(runtime);
    assert!(retirement.wait_for_idle(Duration::from_secs(3)));
}

#[test]
fn absent_external_readiness_times_out_and_retires_the_owned_process_without_restart() {
    let (engine, application) = fixture(ApplicationReadiness::Spout {
        sender: format!("MapGyver-absent-{}", uuid::Uuid::new_v4()),
        first_frame: true,
        timeout_ms: 100,
    });
    let mut runtime = ApplicationRuntime::start(&engine, None).unwrap();
    let retirement = runtime.retirement();
    runtime
        .reconcile(&engine, &demand(application, &[DemandReason::Preparation]))
        .unwrap();
    let waiting = wait(&runtime, ApplicationPhase::WaitingForSender);
    let handle = monitor(waiting.process_id.unwrap());
    let failed = wait(&runtime, ApplicationPhase::Failed);
    assert!(
        failed
            .message
            .as_deref()
            .unwrap()
            .contains("sender readiness timed out")
    );
    assert!(exited(&handle));
    assert_eq!(failed.process_id, None);
    assert_eq!(failed.launch, waiting.launch);
    assert_eq!(failed.next_action_ms, None);
    runtime.close();
    wait(&runtime, ApplicationPhase::Idle);
    drop(runtime);
    assert!(retirement.wait_for_idle(Duration::from_secs(3)));
}
