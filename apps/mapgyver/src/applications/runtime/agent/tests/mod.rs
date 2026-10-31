use super::*;
use std::{
    cell::Cell,
    time::{Duration, Instant},
};

#[test]
#[ignore = "requires the explicitly built Golden IO process fixture"]
fn native_pre_resume_cancellation_keeps_suspended_ownership_and_never_executes_the_target() {
    let directory = tempfile::tempdir().unwrap();
    let report = directory.path().join("cancelled.txt");
    let executable = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("golden_io_process_fixture.exe");
    assert!(executable.is_file(), "build golden_io with process-fixture first");
    let definition = Arc::new(ApplicationDefinition {
        executable: Some(executable.to_string_lossy().into()),
        arguments: vec![
            "report".into(),
            report.to_string_lossy().into(),
            directory.path().join("stop").to_string_lossy().into(),
        ],
        stop: ApplicationStop {
            graceful_ms: 10,
            retirement_ms: 1000,
        },
        ..Default::default()
    });
    let mut agent = Agent::new(
        ApplicationId(golden_core::model::NodeUuid(uuid::Uuid::new_v4())),
        Native,
    );
    let calls = Cell::new(0);
    agent.tick(
        0,
        Desired {
            demanded: true,
            definition: Some(definition),
            retain_existing: false,
        },
        false,
        || {
            let count = calls.get();
            calls.set(count + 1);
            count == 0
        },
        |_| {},
    );
    assert_eq!(calls.get(), 2);
    assert!(matches!(&agent.process, Some(NativeProcess::Suspended(_))));
    assert_eq!(agent.status.phase, ApplicationPhase::Stopping);
    let until = Instant::now() + Duration::from_secs(2);
    let origin = Instant::now();
    while agent.has_process() {
        agent.tick(
            origin.elapsed().as_millis() as u64,
            Desired {
                demanded: false,
                definition: None,
                retain_existing: false,
            },
            false,
            || false,
            |_| {},
        );
        assert!(
            Instant::now() < until,
            "suspended cancellation failed: {:?}",
            agent.status
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(agent.status.phase, ApplicationPhase::Idle);
    assert_eq!(agent.status.process_id, None);
    assert!(
        !report.exists(),
        "cancelled primary thread must never execute fixture code"
    );
}
