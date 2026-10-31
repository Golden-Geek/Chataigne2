use super::super::supervision::Source;
use super::*;

#[test]
fn service_health_is_bounded_change_driven_and_fenced_by_project_and_owner() {
    let mut engine: AppEngine = golden_core::app::create_new_project_engine().unwrap();
    engine.apply_edits().unwrap();
    let context = super::super::capture::Context::capture(&engine).unwrap();
    let view = context.view.clone();
    let project = context.project;
    assert_eq!(
        view.snapshot(project).supervision,
        ApplicationSupervisionSnapshot::default()
    );
    let history = engine.undo_len();
    let owner = Source::capture(&engine)
        .unwrap()
        .claim(ApplicationHost::Headless)
        .unwrap();
    assert!(
        Source::capture(&engine)
            .unwrap()
            .claim(ApplicationHost::Desktop)
            .is_err()
    );
    let writer = owner.publisher();
    assert!(owner.validate_project(project).is_ok());
    assert!(owner.validate_project(uuid::Uuid::new_v4()).is_err());
    writer.demand(Err("x".repeat(1000)));
    let failed = view.snapshot(project);
    assert_eq!(failed.supervision.host, Some(ApplicationHost::Headless));
    let ApplicationServiceStatus::Retrying { message } = failed.supervision.demand else {
        panic!()
    };
    assert_eq!(message.len(), 512);
    writer.demand(Err("x".repeat(1500)));
    assert_eq!(
        view.snapshot(project).revision,
        failed.revision,
        "unchanged bounded errors must not republish"
    );
    writer.demand(Ok(()));
    let attempt = writer.callback_attempt();
    writer.callbacks(Err("transport unavailable".into()));
    writer.callbacks_started(attempt);
    let recovered = view.snapshot(project);
    assert_eq!(recovered.supervision.demand, ApplicationServiceStatus::Running);
    assert!(matches!(
        recovered.supervision.callbacks,
        ApplicationServiceStatus::Retrying { .. }
    ));
    let attempt = writer.callback_attempt();
    writer.callbacks(Err("transport unavailable".into()));
    writer.callbacks_started(attempt);
    assert_eq!(
        view.snapshot(project).revision,
        recovered.revision,
        "a stale startup receipt cannot clear an unchanged delivery error"
    );
    writer.callbacks_started(writer.callback_attempt());
    assert_eq!(
        view.snapshot(project).supervision.callbacks,
        ApplicationServiceStatus::Running
    );
    assert_eq!(engine.undo_len(), history);
    drop(owner);
    let replacement = Source::capture(&engine)
        .unwrap()
        .claim(ApplicationHost::Desktop)
        .unwrap();
    let current = view.snapshot(project);
    writer.demand(Err("retired consumer".into()));
    assert_eq!(view.snapshot(project).revision, current.revision);
    let writer = replacement.publisher();
    drop(engine);
    assert!(!replacement.is_current());
    writer.callbacks(Ok(()));
    assert_eq!(view.snapshot(project).revision, current.revision);
}
