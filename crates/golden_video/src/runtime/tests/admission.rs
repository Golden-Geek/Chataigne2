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
fn backend_padding_and_retiring_generations_remain_charged_when_a_new_plan_is_rejected() {
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
    let resources = h.runtime().snapshot().resources.unwrap();
    assert_eq!(resources.bytes_used, 48);
    assert_eq!(resources.retiring_generations, 1);
    assert_eq!(h.null.snapshot().live_bytes, 48);
    let rejected = h.submit(plan(3, 2, 1));
    assert_eq!(rejected_code(h.outcome(rejected)), FailureCode::ResourcePressure);
    let resources = h.runtime().snapshot().resources.unwrap();
    assert_eq!(resources.active, Some(live));
    assert_eq!(resources.bytes_used, 48);
    assert_eq!(h.null.snapshot().created, 2, "rejected admission allocates nothing");
    assert!(resources.peak_bytes <= 64);

    assert!(h.null.complete_through(first_token));
    h.wait("retiring generation release", |h| {
        h.runtime()
            .snapshot()
            .resources
            .is_some_and(|resources| resources.bytes_used == 24)
    });
    h.advance(40_000_000);
    h.wait("prior live plan still rendering after rejection", |h| {
        h.runtime().snapshot().frames_submitted >= 3
    });
    assert_eq!(h.runtime().snapshot().last_frame.unwrap().identity, live.identity);
}
