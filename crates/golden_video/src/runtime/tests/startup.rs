use super::*;

fn starting(limits: RuntimeConfig, failures: usize) -> Harness {
    let clock = TestClock::at(0);
    let runtime = VideoRuntime::spawn_null_with_clock(
        limits,
        NullConfig {
            initialization_failures: failures,
            ..Default::default()
        },
        clock.clone(),
    )
    .unwrap();
    let null = runtime.null_control().unwrap();
    let retirement = runtime.retirement();
    let mut harness = Harness {
        runtime: Some(runtime),
        clock,
        null,
        retirement,
        outcomes: BTreeMap::new(),
    };
    harness.wait("initial allocation attempt", |h| {
        h.null.snapshot().initialization_attempts != 0
    });
    harness
}

fn pending() -> Harness {
    let mut limits = config();
    limits.recovery_delay = Duration::from_millis(100);
    limits.max_recovery_delay = Duration::from_millis(100);
    let mut h = starting(limits, usize::MAX);
    h.wait("initial allocation failure remains recoverable", |h| {
        h.runtime().snapshot().state == RuntimeState::Faulted
    });
    assert!(!h.null.snapshot().initialized);
    assert!(!h.null.snapshot().device_lost);
    assert_eq!(h.null.snapshot().created, 0);
    h
}

#[test]
fn initial_allocation_failures_retry_within_the_first_device_generation() {
    let mut h = starting(config(), 2);
    let device = h.null.snapshot().device.unwrap();
    h.wait("third initialization starts service", |h| {
        h.null.snapshot().initialized && h.runtime().snapshot().state == RuntimeState::Running
    });
    let snapshot = h.null.snapshot();
    assert_eq!(snapshot.initialization_attempts, 3);
    assert_eq!(snapshot.device, Some(device));
    assert!(!snapshot.device_lost);
    assert_eq!(snapshot.reset_attempts, 0);
    let request = h.submit(plan(1, 1, 1));
    let (generation, _) = h.installed(request);
    assert_eq!(generation.device, device);
    h.drive("first device submits after initialization", |h| {
        h.runtime()
            .snapshot()
            .last_frame
            .is_some_and(|frame| frame.identity == generation.identity)
    });
}

#[test]
fn pending_initialization_preserves_time_control_and_only_prepares_the_latest_plan() {
    let mut h = pending();
    let device = h.null.snapshot().device.unwrap();
    let change = h
        .runtime_mut()
        .discontinuity(ShowPosition::from_nanos(123_000_000), PlaybackRate::NORMAL)
        .unwrap();
    let time = match h.outcome(change).result {
        OutcomeKind::TimeChanged(time) => time,
        result => panic!("expected time intent during initialization, got {result:?}"),
    };
    assert_eq!(h.runtime().read_time().unwrap().epoch(), time.epoch());
    assert_eq!(h.runtime().snapshot().state, RuntimeState::Faulted);
    assert_eq!(h.runtime().snapshot().failure.unwrap().code(), FailureCode::Backend);
    let old = h.submit(plan(1, 1, 1));
    let latest = h.submit(plan(2, 1, 1));
    assert!(matches!(h.outcome(old).result, OutcomeKind::Superseded));
    h.no_outcome(latest, Duration::from_millis(20));
    assert_eq!(
        h.null.snapshot().created,
        0,
        "pending startup must not allocate plan resources"
    );
    assert!(!h.runtime().snapshot().preparing);

    h.null.fail_next_initializations(0);
    let (generation, at) = h.installed(latest);
    assert_eq!(generation.device, device);
    assert_eq!(at.epoch(), time.epoch());
    assert_eq!(h.null.snapshot().created, 1);
    assert_eq!(h.null.snapshot().reset_attempts, 0);
    h.drive("latest authored plan serves after initial recovery", |h| {
        h.runtime()
            .snapshot()
            .last_frame
            .is_some_and(|frame| frame.identity == generation.identity)
    });
}

#[test]
fn project_replacement_during_initialization_never_allocates_the_old_plan() {
    let mut h = pending();
    let device = h.null.snapshot().device.unwrap();
    let previous = h.runtime().project_epoch();
    let old = h.submit(plan(1, 1, 1));
    let (replacement, project) = h.runtime_mut().replace_project().unwrap();
    assert_ne!(project, previous);
    assert_eq!(rejected_code(h.outcome(old)), FailureCode::StaleProject);
    match h.outcome(replacement).result {
        OutcomeKind::ProjectChanged(current) => assert_eq!(current, project),
        result => panic!("expected project replacement during initialization, got {result:?}"),
    }
    assert_eq!(h.null.snapshot().created, 0);
    assert!(h.runtime().snapshot().last_frame.is_none());
    let request = h.submit(plan(2, 1, 1));
    h.null.fail_next_initializations(0);
    let (generation, _) = h.installed(request);
    assert_eq!(generation.identity.project, project);
    assert_eq!(generation.device, device);
    assert_eq!(h.null.snapshot().created, 1);
    assert_eq!(h.null.snapshot().reset_attempts, 0);
    h.drive("replacement project alone submits", |h| {
        h.runtime().snapshot().last_frame.is_some_and(|frame| {
            assert_eq!(frame.identity.project, project);
            frame.identity == generation.identity
        })
    });
}

#[test]
fn cancellation_wakes_initialization_backoff_without_claiming_device_loss() {
    let mut limits = config();
    limits.recovery_delay = Duration::from_secs(10);
    limits.max_recovery_delay = Duration::from_secs(10);
    let mut h = starting(limits, usize::MAX);
    h.wait("first failed initialization enters backoff", |h| {
        h.runtime().snapshot().state == RuntimeState::Faulted
    });
    let request = h.submit(plan(1, 1, 1));
    h.runtime().close();
    assert!(
        h.retirement.wait(DEADLINE),
        "close must wake the ten-second startup retry wait"
    );
    assert!(matches!(h.outcome(request).result, OutcomeKind::Cancelled));
    let snapshot = h.null.snapshot();
    assert_eq!(snapshot.initialization_attempts, 1);
    assert_eq!(snapshot.reset_attempts, 0);
    assert!(!snapshot.initialized);
    assert!(!snapshot.device_lost);
    assert_eq!(snapshot.created, 0);
    assert_eq!(snapshot.live, 0);
    assert_eq!(h.runtime().snapshot().state, RuntimeState::Stopped);
}
