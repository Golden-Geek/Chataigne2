use super::*;

#[test]
fn device_recovery_does_not_reconstruct_a_retired_authoring_source() {
    let mut h = Harness::new(config(), NullConfig::default());
    let owner = golden_io::RevisionSource::default();
    let mut input = crate::runtime::PlanSubmission::new(plan(20, 1, 1));
    input.guards = Arc::from([owner.reader().guard_at(0)]);
    let epoch = h.runtime().read_time().unwrap().epoch();
    let project = h.runtime().project_epoch();
    let request = h
        .runtime_mut()
        .submit(project, input, PlanActivation::NextFrame(epoch), LatePolicy::Activate)
        .unwrap();
    let (generation, _) = h.installed(request);
    drop(owner);
    h.null.lose_device();
    h.advance(1);
    h.wait("device reconstruction without retired source", |h| {
        h.runtime()
            .snapshot()
            .resources
            .is_some_and(|resources| resources.last_submitted.device != generation.device && !resources.device_lost)
    });
    assert!(h.runtime().snapshot().resources.unwrap().active.is_none());
    assert_eq!(h.null.snapshot().created, 1);
}

#[test]
fn authoring_guards_are_rechecked_after_preparation_before_scheduled_install() {
    let mut h = Harness::new(config(), NullConfig::default());
    let initial = h.runtime().read_time().unwrap();
    let project = h.runtime().project_epoch();
    let source = golden_io::RevisionSource::default();
    let mut input = crate::runtime::PlanSubmission::new(plan(10, 1, 1));
    input.guards = Arc::from([source.reader().guard_at(0)]);
    let request = h
        .runtime_mut()
        .submit(
            project,
            input,
            PlanActivation::Scheduled(ActivationStamp::new(
                initial.epoch(),
                MonotonicTime::from_nanos(1_000_000_000),
            )),
            LatePolicy::Activate,
        )
        .unwrap();
    h.wait("guarded preparation", |h| h.runtime().snapshot().prepared);
    source.advance();
    h.advance(1_000_000_000);
    assert_eq!(rejected_code(h.outcome(request)), FailureCode::StaleSource);
    assert!(h.runtime().snapshot().resources.unwrap().active.is_none());
}

#[test]
fn scheduled_shared_outputs_install_at_the_fake_deadline_without_reader_advancement() {
    let mut h = Harness::new(config(), NullConfig::default());
    let initial = h.runtime().read_time().unwrap();
    for _ in 0..4 {
        assert_eq!(h.runtime().read_time().unwrap(), initial);
    }
    let project = h.runtime().project_epoch();
    let request = h
        .runtime_mut()
        .submit_plan(
            project,
            plan(10, 1, 2),
            PlanActivation::Scheduled(ActivationStamp::new(
                initial.epoch(),
                MonotonicTime::from_nanos(1_000_000_000),
            )),
            LatePolicy::RejectAfter(Duration::ZERO),
        )
        .unwrap();
    h.wait("prepared scheduled plan", |h| h.runtime().snapshot().prepared);
    assert_eq!(h.runtime().snapshot().frames_submitted, 0);
    h.advance(999_999_999);
    h.wait("pre-deadline clock observation", |h| {
        h.runtime().snapshot().time.monotonic().as_nanos() == 999_999_999
    });
    assert_eq!(h.runtime().snapshot().frames_submitted, 0);

    h.advance(1_000_000_000);
    let (generation, at) = h.installed(request);
    assert_eq!(at.monotonic().as_nanos(), 1_000_000_000);
    assert_eq!(at.show_position().as_nanos(), 1_000_000_000);
    assert_eq!(generation.output_count, 2);
    h.wait("first shared frame", |h| h.runtime().snapshot().frames_submitted == 1);
    let frame = h.runtime().snapshot().last_frame.unwrap();
    assert_eq!(frame.time, at);
    assert_eq!(h.null.snapshot().last_submission_time, Some(at));
    assert_eq!(
        h.null.snapshot().created,
        1,
        "two consumers share one producer resource"
    );
    for _ in 0..4 {
        assert_eq!(h.runtime().read_time().unwrap(), at);
        h.runtime().wake();
    }
    thread::sleep(Duration::from_millis(25));
    assert_eq!(h.runtime().snapshot().frames_submitted, 1);
}

#[test]
fn discontinuity_and_replacement_authority_reject_old_activation_epochs() {
    let mut h = Harness::new(config(), NullConfig::default());
    let original = h.runtime().read_time().unwrap();
    h.advance(10);
    let changed = h
        .runtime_mut()
        .discontinuity(ShowPosition::from_nanos(4_000), PlaybackRate::PAUSED)
        .unwrap();
    let paused = match h.outcome(changed).result {
        OutcomeKind::TimeChanged(time) => time,
        result => panic!("expected changed time, got {result:?}"),
    };
    assert_ne!(paused.epoch(), original.epoch());
    assert!(paused.epoch().same_authority(original.epoch()));
    let project = h.runtime().project_epoch();
    let stale = h
        .runtime_mut()
        .submit_plan(
            project,
            plan(1, 1, 1),
            PlanActivation::NextFrame(original.epoch()),
            LatePolicy::Activate,
        )
        .unwrap();
    assert_eq!(rejected_code(h.outcome(stale)), FailureCode::StaleTime);
    let live = h.submit(plan(2, 1, 1));
    let (_, at) = h.installed(live);
    assert_eq!(at.show_position().as_nanos(), 4_000);
    h.advance(1_000_000_010);
    h.wait("paused show with advancing monotonic clock", |h| {
        h.runtime().snapshot().time.monotonic().as_nanos() == 1_000_000_010
    });
    assert_eq!(h.runtime().snapshot().time.show_position().as_nanos(), 4_000);

    let replacement = TestClock::at(100);
    let changed = h
        .runtime_mut()
        .replace_clock(
            replacement.clone(),
            ShowPosition::from_nanos(7_000),
            PlaybackRate::NORMAL,
        )
        .unwrap();
    let now = match h.outcome(changed).result {
        OutcomeKind::TimeChanged(time) => time,
        result => panic!("expected replacement time, got {result:?}"),
    };
    assert_eq!(now.epoch().generation(), 0);
    assert!(!now.epoch().same_authority(original.epoch()));
    h.clock = replacement.clone();
    let mut foreign =
        TimeAuthority::new(TestClock::at(100), ShowPosition::from_nanos(0), PlaybackRate::NORMAL).unwrap();
    let foreign_epoch = foreign.sample().unwrap().epoch();
    assert_eq!(foreign_epoch.generation(), now.epoch().generation());
    let stale = h
        .runtime_mut()
        .submit_plan(
            project,
            plan(3, 1, 1),
            PlanActivation::NextFrame(foreign_epoch),
            LatePolicy::Activate,
        )
        .unwrap();
    assert_eq!(rejected_code(h.outcome(stale)), FailureCode::StaleTime);
    let valid = h.submit(plan(4, 1, 1));
    h.drive("new-authority plan activation", |h| {
        h.collect();
        h.outcomes.contains_key(&valid)
    });
    let (_, installed_at) = h.installed(valid);
    assert_eq!(installed_at.epoch(), now.epoch());
    assert!(installed_at.monotonic().as_nanos() >= 100);
    assert_eq!(
        installed_at.show_position().as_nanos(),
        7_000 + (installed_at.monotonic().as_nanos() - 100) as i64
    );
    let later = replacement.0.load(Ordering::Acquire) + 1_000_000_000;
    replacement.set(later);
    h.runtime().wake();
    h.wait("replacement authority observation", |h| {
        h.runtime().snapshot().time.monotonic().as_nanos() == later
    });
    assert_eq!(
        h.runtime().read_time().unwrap().show_position().as_nanos(),
        7_000 + (later - 100) as i64
    );
}

#[test]
fn project_replacement_fences_immediately_but_acknowledges_only_after_gpu_retirement() {
    let mut h = Harness::new(
        config(),
        NullConfig {
            auto_complete: false,
            ..Default::default()
        },
    );
    let old_project = h.runtime().project_epoch();
    let initial = h.submit(plan(1, 1, 1));
    h.installed(initial);
    h.wait("old-project submitted use", |h| {
        h.runtime().snapshot().frames_submitted == 1
    });
    let old_token = h.null.snapshot().last_submitted.unwrap();
    let (request, new_project) = h.runtime_mut().replace_project().unwrap();
    assert_eq!(h.runtime().project_epoch(), new_project);
    let epoch = h.runtime().read_time().unwrap().epoch();
    assert_eq!(
        h.runtime_mut().submit_plan(
            old_project,
            plan(2, 1, 1),
            PlanActivation::NextFrame(epoch),
            LatePolicy::Activate
        ),
        Err(AdmissionError::StaleProject)
    );
    h.wait("old live generation removed", |h| {
        h.runtime()
            .snapshot()
            .resources
            .is_some_and(|resources| resources.active.is_none())
    });
    let prior_frames = h.runtime().snapshot().frames_submitted;
    h.advance(1_000_000_000);
    h.no_outcome(request, Duration::from_millis(30));
    assert_eq!(h.runtime().snapshot().frames_submitted, prior_frames);
    assert_eq!(h.null.snapshot().live, 1);
    assert!(h.null.complete_through(old_token));
    match h.outcome(request).result {
        OutcomeKind::ProjectChanged(project) => assert_eq!(project, new_project),
        result => panic!("expected project retirement acknowledgement, got {result:?}"),
    }
    h.wait("old resources released", |h| {
        h.null.snapshot().live == 0
            && h.runtime()
                .snapshot()
                .resources
                .is_some_and(|resources| resources.bytes_used == 0)
    });
    assert_eq!(h.runtime().snapshot().resources.unwrap().bytes_used, 0);
}

#[test]
fn closing_a_prepared_future_plan_cancels_it_before_the_deadline_can_install_it() {
    let mut h = Harness::new(
        config(),
        NullConfig {
            auto_complete: false,
            ..Default::default()
        },
    );
    let time = h.runtime().read_time().unwrap();
    let project = h.runtime().project_epoch();
    let request = h
        .runtime_mut()
        .submit_plan(
            project,
            plan(1, 1, 1),
            PlanActivation::Scheduled(ActivationStamp::new(
                time.epoch(),
                MonotonicTime::from_nanos(1_000_000_000),
            )),
            LatePolicy::Activate,
        )
        .unwrap();
    h.wait("future prepared resources", |h| h.runtime().snapshot().prepared);
    assert_eq!(h.null.snapshot().live, 1);
    assert_eq!(h.runtime().snapshot().frames_submitted, 0);
    h.runtime().close();
    h.advance(1_000_000_000);
    assert!(matches!(h.outcome(request).result, OutcomeKind::Cancelled));
    assert!(h.retirement.wait(DEADLINE));
    assert_eq!(h.null.snapshot().live, 0);
    assert_eq!(h.runtime().snapshot().frames_submitted, 0);
}
