use super::*;

#[test]
fn undrained_reliable_outcomes_bound_admission_without_blocking_live_frames() {
    let mut settings = config();
    settings.max_outstanding = NonZeroUsize::new(2).unwrap();
    let mut h = Harness::new(settings, NullConfig::default());
    let first = h.submit(plan(1, 1, 1));
    h.wait("first installation without draining", |h| {
        h.runtime()
            .snapshot()
            .resources
            .is_some_and(|resources| resources.active.is_some())
            && h.runtime().snapshot().frames_submitted >= 1
    });
    let first_identity = h.runtime().snapshot().resources.unwrap().active.unwrap().identity;
    let second = h.submit(plan(2, 1, 1));
    h.wait("second prepared generation", |h| h.runtime().snapshot().prepared);
    h.advance(20_000_000);
    h.wait("second installation without draining", |h| {
        h.runtime()
            .snapshot()
            .resources
            .is_some_and(|resources| resources.active.is_some_and(|active| active.identity != first_identity))
    });
    assert_eq!(h.runtime().snapshot().outstanding, 2);
    let project = h.runtime().project_epoch();
    let epoch = h.runtime().read_time().unwrap().epoch();
    assert!(matches!(
        h.runtime_mut().submit_plan(
            project,
            plan(3, 1, 1),
            PlanActivation::NextFrame(epoch),
            LatePolicy::Activate
        ),
        Err(AdmissionError::OutstandingCapacity | AdmissionError::InputCapacity)
    ));
    let frames = h.runtime().snapshot().frames_submitted;
    h.advance(40_000_000);
    h.wait("frames with undrained replies", |h| {
        h.runtime().snapshot().frames_submitted > frames
    });
    assert_eq!(h.runtime().snapshot().outstanding, 2);
    let mut outcomes = Vec::new();
    assert_eq!(
        h.runtime().drain_outcomes(&mut outcomes, NonZeroUsize::new(2).unwrap()),
        2
    );
    outcomes.sort_unstable_by_key(|outcome| outcome.request);
    assert_eq!(
        outcomes.iter().map(|outcome| outcome.request).collect::<Vec<_>>(),
        [first, second]
    );
    assert!(
        outcomes
            .iter()
            .all(|outcome| matches!(outcome.result, OutcomeKind::Installed { .. }))
    );
    h.wait(
        "admission after drained outcomes and asynchronous input disposal",
        |h| {
            h.runtime_mut()
                .submit_plan(
                    project,
                    plan(3, 1, 1),
                    PlanActivation::NextFrame(epoch),
                    LatePolicy::Activate,
                )
                .is_ok()
        },
    );
}

#[test]
fn superseding_cpu_preparation_keeps_every_outcome_but_allocates_only_the_latest_plan() {
    let mut h = Harness::new(
        config(),
        NullConfig {
            preparation_delay: Duration::from_secs(1),
            ..Default::default()
        },
    );
    let mut requests = vec![h.submit(plan(1, 1, 1))];
    h.wait("first CPU preparation", |h| h.runtime().snapshot().preparing);
    for value in 2..=10 {
        requests.push(h.submit(plan(value, 1, 1)));
    }
    let last = *requests.last().unwrap();
    for request in requests {
        match h.outcome(request).result {
            OutcomeKind::Installed { generation, .. } if request == last => {
                assert_eq!(generation.identity.revision.get(), last.get())
            }
            OutcomeKind::Superseded if request != last => {}
            result => panic!("request {} got unexpected result {result:?}", request.get()),
        }
    }
    assert_eq!(h.null.snapshot().created, 1);
    assert_eq!(h.null.snapshot().live, 1);
    h.collect();
    assert!(h.outcomes.is_empty(), "each request receives one terminal outcome");
}

#[test]
fn retiring_generations_delay_replacement_without_rejecting_or_exceeding_the_budget() {
    let (mut h, first_token, live) = retirement_pressure();
    let resources = h.runtime().snapshot().resources.unwrap();
    assert_eq!(resources.bytes_used, 48);
    assert_eq!(resources.retiring_generations, 1);
    assert_eq!(h.null.snapshot().live_bytes, 48);
    let replacement = h.submit(plan(3, 2, 1));
    h.wait("replacement awaiting retirement", |h| {
        h.collect();
        h.runtime().snapshot().waiting_for_resources || h.outcomes.contains_key(&replacement)
    });
    assert!(
        !h.outcomes.contains_key(&replacement),
        "temporary retirement pressure must not reject the edit: {:?}",
        h.outcomes.get(&replacement)
    );
    let resources = h.runtime().snapshot().resources.unwrap();
    assert_eq!(resources.active, Some(live));
    assert_eq!(resources.bytes_used, 48);
    assert_eq!(h.null.snapshot().created, 2, "waiting admission allocates nothing");
    assert!(resources.peak_bytes <= 64);

    assert!(h.null.complete_through(first_token));
    h.wait("replacement prepared after retirement", |h| {
        h.runtime().snapshot().prepared
    });
    assert!(h.runtime().snapshot().resources.unwrap().peak_bytes <= 64);
    h.advance(40_000_000);
    let (new_live, _) = h.installed(replacement);
    h.wait("replacement renders after retirement", |h| {
        h.runtime().snapshot().frames_submitted >= 3
    });
    assert_eq!(h.runtime().snapshot().last_frame.unwrap().identity, new_live.identity);
}

fn retirement_pressure() -> (Harness, crate::runtime::CompletionToken, GenerationInfo) {
    let mut settings = config();
    settings.resource_limits.max_bytes = 64;
    let mut h = Harness::new(
        settings,
        NullConfig {
            auto_complete: false,
            padding_per_texture: 8,
            ..Default::default()
        },
    );
    let first = h.submit(plan(1, 2, 1));
    h.installed(first);
    h.wait("first submitted use", |h| h.runtime().snapshot().frames_submitted == 1);
    let first_token = h.null.snapshot().last_submitted.unwrap();
    let second = h.submit(plan(2, 2, 1));
    h.wait("second prepared generation", |h| h.runtime().snapshot().prepared);
    h.advance(20_000_000);
    let (live, _) = h.installed(second);
    h.wait("second submitted use", |h| h.runtime().snapshot().frames_submitted == 2);
    (h, first_token, live)
}

fn awaiting_retirement(h: &mut Harness, request: RequestId) {
    h.wait("compiled replacement awaiting retirement", |h| {
        h.collect();
        h.runtime().snapshot().waiting_for_resources || h.outcomes.contains_key(&request)
    });
    assert!(
        !h.outcomes.contains_key(&request),
        "unexpected terminal outcome: {:?}",
        h.outcomes.get(&request)
    );
    assert_eq!(h.null.snapshot().created, 2);
}

#[test]
fn pressure_coalesces_new_edits_and_installs_only_the_latest_after_completion() {
    let (mut h, first_token, original) = retirement_pressure();
    let mut requests = Vec::new();
    for value in 3..=10 {
        let request = h.submit(plan(value, 2, 1));
        if let Some(previous) = requests.last() {
            assert!(matches!(h.outcome(*previous).result, OutcomeKind::Superseded));
        }
        awaiting_retirement(&mut h, request);
        h.no_outcome(request, Duration::from_millis(10));
        assert_eq!(h.runtime().snapshot().resources.unwrap().active, Some(original));
        assert!(h.runtime().snapshot().resources.unwrap().peak_bytes <= 64);
        requests.push(request);
    }
    let latest = requests.pop().unwrap();
    assert!(h.null.complete_through(first_token));
    h.wait("latest resources admitted", |h| h.runtime().snapshot().prepared);
    h.advance(40_000_000);
    let (installed, _) = h.installed(latest);
    assert_eq!(installed.identity.revision.get(), latest.get());
    assert_eq!(h.null.snapshot().created, 3);
    h.collect();
    assert!(h.outcomes.is_empty(), "each superseded edit has exactly one outcome");
}

#[test]
fn unreclaimable_replacement_pressure_rejects_without_withdrawing_the_active_frame() {
    let (mut h, _, original) = retirement_pressure();
    for extent in [3, 4] {
        // 44 bytes cannot coexist with the 24-byte active image, even after retirement.
        // 72 bytes cannot fit alone. Neither case may wait forever.
        let request = h.submit(plan(3, extent, 1));
        assert_eq!(rejected_code(h.outcome(request)), FailureCode::ResourcePressure);
        assert!(!h.runtime().snapshot().waiting_for_resources);
        assert_eq!(h.null.snapshot().created, 2);
        assert_eq!(h.runtime().snapshot().resources.unwrap().active, Some(original));
        assert!(h.runtime().snapshot().resources.unwrap().peak_bytes <= 64);
    }
}

#[test]
fn waiting_replacement_survives_device_recovery_without_accepting_old_completions() {
    let (mut h, old_completion, original) = retirement_pressure();
    let request = h.submit(plan(3, 2, 1));
    awaiting_retirement(&mut h, request);
    h.null.lose_device();
    h.drive("waiting edit installs on the recovered device", |h| {
        h.runtime().snapshot().resources.is_some_and(|resources| {
            resources
                .active
                .is_some_and(|active| active.identity.revision.get() == request.get())
        })
    });
    let (installed, _) = h.installed(request);
    assert_eq!(installed.device, original.device.next().unwrap());
    assert!(!h.null.complete_through(old_completion));
    assert!(!h.runtime().snapshot().waiting_for_resources);
    assert!(h.runtime().snapshot().resources.unwrap().peak_bytes <= 64);
    h.collect();
    assert!(h.outcomes.is_empty(), "recovery preserves one terminal receipt");
}

#[test]
fn waiting_admission_honors_source_project_deadline_and_shutdown_fences() {
    use crate::runtime::{PlanLifetime, PlanSubmission};
    for action in ["source", "project", "deadline", "close"] {
        let (mut h, _, _) = retirement_pressure();
        let source = golden_io::RevisionSource::default();
        let lifetime = PlanLifetime::default();
        let released = lifetime.observer();
        let mut input = PlanSubmission::new(plan(3, 2, 1));
        input.guards = Arc::from([source.reader().guard_at(0)]);
        input.lifetime = Some(lifetime);
        let project = h.runtime().project_epoch();
        let time = h.runtime().read_time().unwrap();
        let request = h
            .runtime_mut()
            .submit(
                project,
                input,
                PlanActivation::Scheduled(ActivationStamp::new(time.epoch(), time.monotonic())),
                if action == "deadline" {
                    LatePolicy::RejectAfter(Duration::from_millis(5))
                } else {
                    LatePolicy::Activate
                },
            )
            .unwrap();
        awaiting_retirement(&mut h, request);
        let replacement = match action {
            "source" => {
                source.advance();
                h.runtime().wake();
                None
            }
            "project" => Some(h.runtime_mut().replace_project().unwrap()),
            "deadline" => {
                h.advance(40_000_000);
                None
            }
            "close" => {
                h.runtime().close();
                None
            }
            _ => unreachable!(),
        };
        let outcome = h.outcome(request);
        if action == "close" {
            assert!(matches!(outcome.result, OutcomeKind::Cancelled));
        } else {
            assert_eq!(
                rejected_code(outcome),
                match action {
                    "source" => FailureCode::StaleSource,
                    "project" => FailureCode::StaleProject,
                    "deadline" => FailureCode::LateActivation,
                    _ => unreachable!(),
                }
            );
        }
        h.wait("pending immutable inputs released", |_| released.is_complete());
        assert_eq!(
            h.null.snapshot().created,
            2,
            "{action} must not allocate the abandoned plan"
        );
        let last = h.null.snapshot().last_submitted.unwrap();
        assert!(h.null.complete_through(last));
        if let Some((changed, project)) = replacement {
            assert!(matches!(h.outcome(changed).result, OutcomeKind::ProjectChanged(current) if current == project));
        }
        if action == "close" {
            assert!(h.retirement.wait(DEADLINE));
            assert_eq!(h.null.snapshot().live_bytes, 0);
        }
    }
}
