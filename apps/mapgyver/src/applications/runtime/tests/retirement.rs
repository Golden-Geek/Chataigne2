use super::*;

#[test]
fn unverified_retirement_explains_each_missing_barrier_without_releasing_or_replacing_ownership() {
    for (root, active, pending, unobserved) in [
        (RootProcessState::Running, 0, 0, 0),
        (RootProcessState::Exited(17), 1, 0, 0),
        (RootProcessState::Exited(17), 0, 1, 0),
        (RootProcessState::Exited(17), 0, 0, 1),
    ] {
        let mut agent = agent();
        let definition = definition();
        tick(&mut agent, 0, true, Some(definition.clone()), false);
        tick(&mut agent, 1, false, None, false);
        agent.backend.observation = Some(Ok(ProcessSnapshot {
            root_id: 42,
            root,
            active_processes: active,
            pending_observed_processes: pending,
            unobserved_processes: unobserved,
        }));
        tick(&mut agent, 31, true, Some(definition.clone()), false);
        assert_eq!(agent.status.phase, ApplicationPhase::RetirementUnverified);
        assert_eq!(agent.status.process_id, Some(42));
        assert_eq!(agent.status.next_action_ms, None);
        assert_eq!(
            agent.status.message.as_deref(),
            Some(format!("Owned tree retirement unverified: root={root:?}, active={active}, pending={pending}, unobserved={unobserved}").as_str())
        );
        tick(&mut agent, 100, true, Some(definition), false);
        assert_eq!(agent.backend.creates, 1);
        assert!(agent.has_process());
    }
}

#[test]
fn retirement_observation_errors_are_visible_bounded_and_never_authorize_replacement() {
    let mut agent = agent();
    let definition = definition();
    tick(&mut agent, 0, true, Some(definition.clone()), false);
    tick(&mut agent, 1, false, None, false);
    agent.backend.observation = Some(Err(format!("access denied (os error 5): {}", "λ".repeat(1024))));
    tick(&mut agent, 31, true, Some(definition.clone()), false);
    assert_eq!(agent.status.phase, ApplicationPhase::RetirementUnverified);
    let message = agent.status.message.as_deref().unwrap();
    assert!(message.starts_with("Owned tree retirement observation failed: access denied (os error 5):"));
    assert_eq!(message.chars().count(), 512);
    tick(&mut agent, 100, true, Some(definition), false);
    assert_eq!(agent.backend.creates, 1);
    assert!(agent.has_process());
}
