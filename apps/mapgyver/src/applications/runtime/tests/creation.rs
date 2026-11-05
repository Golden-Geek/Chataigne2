use super::*;

pub(super) enum CreationRejection {
    BeforeCreation,
    AfterCreation,
}

#[test]
fn a_rejected_created_target_never_resumes_and_retry_waits_for_verified_retirement() {
    let mut agent = agent();
    agent.backend.reject_create = Some(CreationRejection::AfterCreation);
    agent.backend.retire_on_kill = false;
    let definition = definition();
    tick(&mut agent, 0, true, Some(definition.clone()), false);
    assert_eq!(agent.status.phase, ApplicationPhase::Stopping);
    assert_eq!(agent.status.process_id, Some(42));
    assert_eq!(
        agent.status.message.as_deref(),
        Some("created process verification denied")
    );
    tick(&mut agent, 1, true, Some(definition.clone()), false);
    assert_eq!(agent.backend.kills, 1);
    tick(&mut agent, 31, true, Some(definition.clone()), false);
    assert_eq!(agent.status.phase, ApplicationPhase::RetirementUnverified);
    tick(&mut agent, 100, true, Some(definition.clone()), false);
    assert_eq!(agent.backend.creates, 1);
    assert_eq!(agent.backend.resumes, 0);
    assert!(agent.has_process());
    agent.backend.retired = true;
    tick(&mut agent, 101, true, Some(definition), false);
    assert_eq!(agent.status.phase, ApplicationPhase::Backoff);
    assert_eq!(agent.status.process_id, None);
    assert_eq!(agent.status.failures, 1);
    assert_eq!(agent.status.next_action_ms, Some(102));
}

#[test]
fn configuration_replacement_cannot_abandon_a_rejected_created_owner() {
    let mut agent = agent();
    agent.backend.reject_create = Some(CreationRejection::AfterCreation);
    agent.backend.retire_on_kill = false;
    let first = definition();
    tick(&mut agent, 0, true, Some(first.clone()), false);
    let launch = agent.status.launch;
    let next = key(ApplicationDefinition {
        arguments: vec!["replacement".into()],
        ..first.definition().clone()
    });
    tick(&mut agent, 1, true, Some(next.clone()), false);
    tick(&mut agent, 100, true, Some(next.clone()), false);
    assert_eq!(agent.status.phase, ApplicationPhase::RetirementUnverified);
    assert_eq!(agent.status.launch, launch);
    assert_eq!(agent.backend.creates, 1);
    assert_eq!(agent.backend.resumes, 0);
    agent.backend.retired = true;
    agent.backend.reject_create = None;
    tick(&mut agent, 101, true, Some(next.clone()), false);
    assert!(agent.idle());
    tick(&mut agent, 102, true, Some(next), false);
    assert_eq!(agent.status.phase, ApplicationPhase::Ready);
    assert_eq!(agent.backend.creates, 2);
    assert_eq!(agent.backend.resumes, 1);
    assert_ne!(agent.status.launch, launch);
}

#[test]
fn a_failure_without_a_created_owner_keeps_normal_backoff_without_a_retirement_slot() {
    let mut agent = agent();
    agent.backend.reject_create = Some(CreationRejection::BeforeCreation);
    let definition = definition();
    tick(&mut agent, 0, true, Some(definition.clone()), false);
    assert_eq!(agent.status.phase, ApplicationPhase::Backoff);
    assert_eq!(agent.status.process_id, None);
    assert!(!agent.has_process());
    assert_eq!(agent.backend.kills, 0);
    assert_eq!(agent.backend.resumes, 0);
    assert_eq!(agent.status.failures, 1);
    assert_eq!(agent.status.next_action_ms, Some(1));
    tick(&mut agent, 1, true, Some(definition), false);
    assert_eq!(agent.backend.creates, 2);
    assert_eq!(agent.status.failures, 2);
    assert_eq!(agent.status.next_action_ms, Some(3));
}
