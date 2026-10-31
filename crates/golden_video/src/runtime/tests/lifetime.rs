use super::*;
use crate::runtime::{PlanLifetime, PlanSubmission};

#[test]
fn replaced_plan_lifetime_waits_for_actual_gpu_resource_retirement() {
    let mut h = Harness::new(
        config(),
        NullConfig {
            auto_complete: false,
            ..Default::default()
        },
    );
    let lifetime = PlanLifetime::default();
    let observer = lifetime.observer();
    let mut submission = PlanSubmission::new(plan(1, 2, 1));
    submission.lifetime = Some(lifetime);
    let project = h.runtime().project_epoch();
    let epoch = h.runtime().read_time().unwrap().epoch();
    let first = h
        .runtime_mut()
        .submit(
            project,
            submission,
            PlanActivation::NextFrame(epoch),
            LatePolicy::Activate,
        )
        .unwrap();
    h.installed(first);
    h.wait("first GPU use", |h| h.runtime().snapshot().frames_submitted == 1);
    let completion = h.null.snapshot().last_submitted.unwrap();
    let second = h.submit(plan(2, 2, 1));
    h.wait("replacement prepared", |h| h.runtime().snapshot().prepared);
    h.advance(20_000_000);
    h.installed(second);
    assert!(!observer.is_complete(), "Installed is not final-use completion");
    assert!(h.null.complete_through(completion));
    h.wait("actual old resource release", |_| observer.is_complete());
}

#[test]
fn rejected_input_lifetime_releases_after_replacement_without_backend_resources() {
    let mut h = Harness::new(config(), NullConfig::default());
    let lifetime = PlanLifetime::default();
    let observer = lifetime.observer();
    let mut invalid = plan(1, 2, 1);
    invalid.passes[0].output = TextureId::new(2).unwrap();
    let mut submission = PlanSubmission::new(invalid);
    submission.lifetime = Some(lifetime);
    let project = h.runtime().project_epoch();
    let epoch = h.runtime().read_time().unwrap().epoch();
    let request = h
        .runtime_mut()
        .submit(
            project,
            submission,
            PlanActivation::NextFrame(epoch),
            LatePolicy::Activate,
        )
        .unwrap();
    assert!(matches!(h.outcome(request).result, OutcomeKind::Rejected(_)));
    // The latest raw desired input may remain retained for retry/recovery; replacement releases it.
    let next = h.submit(plan(2, 2, 1));
    h.installed(next);
    h.wait("rejected input disposal", |_| observer.is_complete());
}
