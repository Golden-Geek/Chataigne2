use std::sync::Mutex;

use golden_io::{PayloadPool, RevisionSource};

use super::*;
use crate::plan::RenderPlan;
use crate::runtime::PlanSubmission;

struct Log(Mutex<Vec<(PlanIdentity, PlanStage)>>);

impl PlanObserver for Log {
    fn observe(&self, identity: PlanIdentity, stage: PlanStage) {
        self.0.lock().unwrap().push((identity, stage));
    }
}

fn setup() -> (Shared, Arc<Log>, PayloadPool) {
    let clock = Arc::new(SystemClock::new());
    let mut authority = TimeAuthority::new(clock.clone(), ShowPosition::from_nanos(0), PlaybackRate::NORMAL).unwrap();
    let log = Arc::new(Log(Mutex::new(Vec::with_capacity(16))));
    let shared = Shared::new(
        ProjectEpoch::new(Uuid::new_v4(), 0),
        authority.sample().unwrap(),
        clock,
        8,
        None,
    )
    .with_plan_observer(Some(log.clone()));
    let pool = PayloadPool::new("plan-observation-test", 8, 4096).unwrap();
    (shared, log, pool)
}

fn publish(shared: &Shared, pool: &PayloadPool, request: u64) -> PlanIdentity {
    let guards = RevisionSource::default();
    let mut state = shared.lock();
    let identity = PlanIdentity {
        project: state.project,
        revision: PlanRevision::new(request),
    };
    state.latest = Some(DesiredPlan {
        request: RequestId(request),
        identity,
        input: pool
            .try_retain(
                PlanSubmission::new(RenderPlan {
                    external_textures: Vec::new(),
                    passes: Vec::new(),
                    outputs: Vec::new(),
                }),
                1,
            )
            .unwrap(),
        project_guard: guards.reader().guard_at(0),
        desired_guard: guards.reader().guard_at(0),
        activation: PlanActivation::NextFrame(state.snapshot.time.epoch()),
        late_policy: LatePolicy::Activate,
    });
    shared.publish_plan(&mut state, RequestId(request), identity);
    identity
}

#[test]
fn first_receipt_is_recorded_once_across_device_retry_and_terminal_races() {
    let (shared, log, pool) = setup();
    let shared = Arc::new(shared);
    let identity = publish(&shared, &pool, 1);
    let desired = shared.take_plan().unwrap();
    shared.lock().latest = Some(desired);
    drop(shared.take_plan());
    let barrier = Arc::new(Barrier::new(3));
    let threads = (0..2)
        .map(|_| {
            let shared = shared.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                shared.complete(RequestId(1), OutcomeKind::Cancelled);
            })
        })
        .collect::<Vec<_>>();
    barrier.wait();
    for thread in threads {
        thread.join().unwrap();
    }
    assert_eq!(
        *log.0.lock().unwrap(),
        vec![
            (identity, PlanStage::Published),
            (identity, PlanStage::Received),
            (identity, PlanStage::Cancelled)
        ]
    );
    assert_eq!(shared.lock().outcomes.drain(..).count(), 1);
    // The observer owns no input payload or additional reliable outcome credits.
    assert!(pool.wait_for_idle(std::time::Duration::from_secs(2)));
    assert_eq!(shared.snapshot().outstanding, 1);
    assert!(shared.lock().requests.remove(&RequestId(1)).unwrap().completed);
    shared.complete(RequestId(1), OutcomeKind::Superseded);
    assert_eq!(log.0.lock().unwrap().len(), 3);
}

#[test]
fn superseded_unread_input_has_no_invented_receipt_and_commands_have_no_plan_events() {
    let (shared, log, pool) = setup();
    let identity = publish(&shared, &pool, 1);
    drop(shared.lock().latest.take());
    shared.complete(RequestId(1), OutcomeKind::Superseded);
    shared.lock().requests.insert(RequestId(2), RequestState::command());
    shared.complete(RequestId(2), OutcomeKind::Cancelled);
    assert_eq!(
        *log.0.lock().unwrap(),
        vec![(identity, PlanStage::Published), (identity, PlanStage::Superseded)]
    );
}

#[test]
fn rejection_keeps_its_exact_authored_identity_and_typed_failure() {
    let (shared, log, pool) = setup();
    let identity = publish(&shared, &pool, 1);
    drop(shared.take_plan());
    shared.complete(
        RequestId(1),
        OutcomeKind::Rejected(RuntimeFailure::new(FailureCode::InvalidPlan, "invalid DAG")),
    );
    assert_eq!(
        log.0.lock().unwrap().last().copied(),
        Some((
            identity,
            PlanStage::Rejected {
                code: FailureCode::InvalidPlan
            }
        ))
    );
    assert_eq!(shared.snapshot().outstanding, 1);
}
