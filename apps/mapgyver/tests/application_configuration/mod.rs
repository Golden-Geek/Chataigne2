use super::*;

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
        status.phase == ApplicationPhase::Ready && status.definition_revision == Some(expected.revision)
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
