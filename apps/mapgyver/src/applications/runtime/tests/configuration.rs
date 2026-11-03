use super::*;

#[test]
fn equivalent_runtime_keys_keep_the_owned_launch_and_effective_changes_wait_for_retirement() {
    let mut agent = agent();
    let mut authored = definition().definition().clone();
    authored.environment = Some(vec![("B".into(), "two".into()), ("A".into(), "one".into())]);
    let first = key(authored.clone());
    tick(&mut agent, 0, true, Some(first), false);
    let launch = agent.status.launch;
    authored.environment.as_mut().unwrap().reverse();
    #[cfg(windows)]
    {
        let path = std::path::PathBuf::from(authored.executable.as_ref().unwrap());
        authored.executable = Some(
            path.parent()
                .unwrap()
                .join(".")
                .join(path.file_name().unwrap())
                .to_str()
                .unwrap()
                .replace('\\', "/"),
        );
    }
    for now in 1..=20 {
        tick(&mut agent, now, true, Some(key(authored.clone())), false);
        assert_eq!(agent.status.launch, launch);
        assert_eq!(agent.status.configuration, ApplicationConfigurationState::Current);
    }
    assert_eq!(agent.backend.creates, 1);
    assert_eq!(agent.backend.resumes, 1);
    assert_eq!(agent.backend.closes, 0);
    assert_eq!(agent.backend.kills, 0);
    let launched = &agent.backend.specifications[0];
    assert_eq!(launched.environment.as_ref().unwrap()[0].0, "A");
    authored.arguments.push("different launch".into());
    tick(&mut agent, 21, true, Some(key(authored.clone())), false);
    assert_eq!(agent.status.phase, ApplicationPhase::Stopping);
    assert_eq!(agent.backend.creates, 1);
    tick(&mut agent, 31, true, Some(key(authored.clone())), false);
    tick(&mut agent, 32, true, Some(key(authored.clone())), false);
    tick(&mut agent, 33, true, Some(key(authored)), false);
    assert_eq!(agent.backend.creates, 2);
    assert_ne!(agent.status.launch, launch);
}
