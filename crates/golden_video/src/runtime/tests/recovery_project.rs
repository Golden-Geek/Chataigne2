use super::*;

#[test]
fn project_replacement_during_failed_reset_never_revives_old_authoring() {
    let mut limits = config();
    limits.recovery_delay = Duration::from_millis(200);
    limits.max_recovery_delay = Duration::from_millis(200);
    let mut h = Harness::new(
        limits,
        NullConfig {
            auto_complete: false,
            ..Default::default()
        },
    );
    let request = h.submit(plan(1, 1, 1));
    let (original, _) = h.installed(request);
    h.wait("old-project submission", |h| {
        h.null.snapshot().last_submitted.is_some_and(|token| token.serial != 0)
    });
    let old_token = h.null.snapshot().last_submitted.unwrap();

    h.null.fail_next_resets(usize::MAX);
    h.null.lose_device();
    h.wait(
        "failed reset retains recovery metadata without old native resources",
        |h| {
            h.null.snapshot().reset_attempts != 0
                && h.runtime().snapshot().resources.is_some_and(|resources| {
                    resources.device_lost && resources.active.is_none() && resources.bytes_used == 0
                })
        },
    );
    assert_eq!(h.null.snapshot().live, 0);

    let (replacement, project) = h.runtime_mut().replace_project().unwrap();
    assert_ne!(project, original.identity.project);
    match h.outcome(replacement).result {
        OutcomeKind::ProjectChanged(current) => assert_eq!(current, project),
        result => panic!("expected reliable project retirement, got {result:?}"),
    }
    assert!(h.runtime().snapshot().resources.unwrap().device_lost);
    assert!(h.runtime().snapshot().last_frame.is_none());

    h.null.fail_next_resets(0);
    h.drive("new device recovers without reinstalling the retired project", |h| {
        let snapshot = h.runtime().snapshot();
        assert!(snapshot.last_frame.is_none());
        let resources = snapshot.resources.unwrap();
        assert!(resources.active.is_none());
        !resources.device_lost && resources.last_submitted.device == original.device.next().unwrap()
    });
    assert_eq!(h.null.snapshot().created, 1, "old authoring must not allocate again");
    assert_eq!(h.null.snapshot().live, 0);

    let request = h.submit(plan(2, 1, 1));
    let (current, _) = h.installed(request);
    assert_eq!(current.identity.project, project);
    assert!(current.identity.revision > original.identity.revision);
    assert_eq!(current.device, original.device.next().unwrap());
    assert_ne!(current.generation, original.generation);
    h.drive("new project submits on its recovered device", |h| {
        h.runtime().snapshot().last_frame.is_some_and(|frame| {
            assert_eq!(frame.identity, current.identity);
            frame.completion.device == current.device
        })
    });

    h.runtime().close();
    h.wait("shutdown waits for the current device's actual completion", |h| {
        h.runtime().snapshot().state == RuntimeState::Stopping
    });
    let current_token = h.null.snapshot().last_submitted.unwrap();
    assert_eq!(current_token.device, current.device);
    assert!(!h.null.complete_through(old_token));
    assert!(!h.retirement.wait(Duration::from_millis(20)));
    assert_eq!(h.null.snapshot().live, 1);
    assert!(h.null.complete_through(current_token));
    assert!(h.retirement.wait(DEADLINE));
    assert_eq!(h.null.snapshot().live, 0);
    assert_eq!(h.null.snapshot().created, h.null.snapshot().dropped);
    assert_eq!(h.runtime().snapshot().state, RuntimeState::Stopped);
}
