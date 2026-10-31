use super::*;
use std::sync::mpsc;
use std::time::Duration;

#[test]
fn configuration_observations_distinguish_retained_pending_current_and_fully_released_owners() {
    let mut agent = agent();
    let original = definition();
    tick(&mut agent, 0, true, Some(original.clone()), false);
    assert_eq!(agent.status.configuration, ApplicationConfigurationState::Current);
    let changed = Arc::new(ApplicationDefinition {
        arguments: vec!["new saved settings".into()],
        ..(*original).clone()
    });
    agent.tick(
        1,
        Desired {
            demanded: true,
            definition: Some(changed.clone()),
            retain_existing: true,
        },
        false,
        || true,
        |_| {},
    );
    assert_eq!(agent.status.configuration, ApplicationConfigurationState::Pending);
    assert!(
        !serde_json::to_string(&agent.status)
            .unwrap()
            .contains("new saved settings")
    );
    assert_eq!(agent.backend.creates, 1);
    tick(&mut agent, 2, true, None, false);
    assert_eq!(agent.status.configuration, ApplicationConfigurationState::Retained);
    tick(&mut agent, 3, false, None, false);
    agent.backend.running = false;
    agent.backend.retired = true;
    tick(&mut agent, 4, false, None, false);
    assert_eq!(agent.status.configuration, ApplicationConfigurationState::NotRunning);
    tick(&mut agent, 5, true, Some(changed), false);
    assert_eq!(agent.status.configuration, ApplicationConfigurationState::Current);
    assert_eq!(agent.backend.creates, 2);
}

#[test]
fn observation_publication_coalesces_backpressure_rejects_old_owners_and_cancels_retired_projects() {
    let view = RuntimeView::default();
    let agent = agent();
    let id = agent.status.application;
    let owner = uuid::Uuid::new_v4();
    view.statuses.lock().unwrap().insert(id, (owner, agent.status.clone()));
    view.changed();
    let guard = golden_io::RevisionSource::default();
    let reader = guard.reader();
    let project = uuid::Uuid::new_v4();
    let source = ApplicationObservations::new(project, reader.guard_at(reader.current()), view.clone());
    let (send, received) = mpsc::sync_channel(4);
    let (release, gate) = mpsc::sync_channel(1);
    let gate = Mutex::new(Some(gate));
    let subscription = source
        .subscribe(move |snapshot| {
            send.send(snapshot).unwrap();
            if let Some(gate) = gate.lock().unwrap().take() {
                gate.recv().unwrap();
            }
        })
        .unwrap();
    assert!(source.subscribe(|_| {}).is_err());
    let first = received.recv_timeout(Duration::from_secs(2)).unwrap();
    let mut status = agent.status;
    for failures in 1..=1000 {
        status.failures = failures;
        view.publish(id, owner, &status, None, project, 0);
    }
    let revision = view.snapshot(project).revision;
    status.failures = 9000;
    view.publish(id, uuid::Uuid::new_v4(), &status, None, project, 0);
    assert_eq!(view.snapshot(project).revision, revision);
    release.send(()).unwrap();
    let latest = received.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(latest.project, project);
    assert!(latest.revision > first.revision);
    assert_eq!(latest.revision, revision);
    assert_eq!(latest.applications[0].failures, 1000);
    guard.advance();
    view.publish(id, owner, &status, None, project, 0);
    assert!(received.recv_timeout(Duration::from_secs(2)).is_err());
    assert!(!source.is_current());
    assert!(source.subscribe(|_| {}).is_err());
    drop(subscription);
}
