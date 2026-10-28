use std::sync::{Arc, Barrier};

use golden_time::{
    Clock, MonotonicTime, PlaybackRate, ShowPosition, SystemClock, TimeAuthority, TimeError, TimeSnapshot,
};
use uuid::Uuid;

use crate::runtime::identity::{CompletionToken, DeviceGeneration, GenerationId, PlanIdentity, PlanRevision};
use crate::runtime::types::FrameObservation;

use super::*;

#[test]
fn concurrent_terminal_claims_publish_once_even_after_the_result_is_drained() {
    let clock = Arc::new(SystemClock::new());
    let mut authority = TimeAuthority::new(clock.clone(), ShowPosition::from_nanos(0), PlaybackRate::NORMAL).unwrap();
    let shared = Arc::new(Shared::new(
        ProjectEpoch::new(Uuid::new_v4(), 0),
        authority.sample().unwrap(),
        clock,
        1,
        None,
    ));
    let request = RequestId(1);
    shared.lock().requests.insert(request, false);
    let barrier = Arc::new(Barrier::new(3));
    let contenders = (0..2)
        .map(|_| {
            let shared = shared.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                shared.complete(request, OutcomeKind::Superseded);
            })
        })
        .collect::<Vec<_>>();
    barrier.wait();
    for contender in contenders {
        contender.join().unwrap();
    }
    {
        let mut state = shared.lock();
        assert_eq!(state.outcomes.len(), 1);
        assert_eq!(state.outcomes.pop_front().unwrap().request, request);
        assert_eq!(state.requests.remove(&request), Some(true));
    }
    shared.complete(request, OutcomeKind::Cancelled);
    assert!(shared.lock().outcomes.is_empty());
}

struct FixedClock(MonotonicTime);

impl Clock for FixedClock {
    fn now(&self) -> Result<MonotonicTime, TimeError> {
        Ok(self.0)
    }
}

fn clock_pair(nanos: u64, show: i64) -> (Arc<dyn Clock>, TimeSnapshot) {
    let clock: Arc<dyn Clock> = Arc::new(FixedClock(MonotonicTime::from_nanos(nanos)));
    let mut authority =
        TimeAuthority::new(clock.clone(), ShowPosition::from_nanos(show), PlaybackRate::NORMAL).unwrap();
    (clock, authority.sample().unwrap())
}

#[test]
fn an_old_admitted_frame_cannot_repopulate_the_replaced_projects_latest_frame() {
    let runtime = Uuid::new_v4();
    let old_project = ProjectEpoch::new(runtime, 0);
    let new_project = ProjectEpoch::new(runtime, 1);
    let (clock, time) = clock_pair(100, 0);
    let shared = Shared::new(old_project, time, clock, 4, None);
    let frame = |project, serial| FrameObservation {
        frame_index: serial,
        scheduled_at: time.monotonic(),
        completion: CompletionToken {
            device: DeviceGeneration::new(runtime, 1),
            serial,
        },
        generation: GenerationId::new(serial),
        identity: PlanIdentity {
            project,
            revision: PlanRevision::new(serial),
        },
        time,
    };
    shared.record_frame(frame(old_project, 1));
    assert_eq!(shared.snapshot().last_frame.unwrap().identity.project, old_project);
    {
        let mut state = shared.lock();
        state.project = new_project;
        state.snapshot.last_frame = None;
    }
    shared.record_frame(frame(old_project, 2));
    let after_old = shared.snapshot();
    assert_eq!(
        after_old.frames_submitted, 2,
        "admitted work remains in lifetime counters"
    );
    assert!(after_old.last_frame.is_none());
    shared.record_frame(frame(new_project, 3));
    let current = shared.snapshot();
    assert_eq!(current.frames_submitted, 3);
    assert_eq!(current.last_frame.unwrap().identity.project, new_project);
}

#[test]
fn concurrent_time_publication_never_pairs_one_authoritys_mapping_with_another_clock_origin() {
    const ROUNDS: usize = 5_000;
    let (clock_a, time_a) = clock_pair(100, 10);
    let (clock_b, time_b) = clock_pair(1_000_000, 50);
    let shared = Arc::new(Shared::new(
        ProjectEpoch::new(Uuid::new_v4(), 0),
        time_a,
        clock_a.clone(),
        1,
        None,
    ));
    let barrier = Arc::new(Barrier::new(2));
    let publisher = {
        let shared = shared.clone();
        let barrier = barrier.clone();
        std::thread::spawn(move || {
            for round in 0..ROUNDS {
                barrier.wait();
                let (clock, time) = if round % 2 == 0 {
                    (clock_b.clone(), time_b)
                } else {
                    (clock_a.clone(), time_a)
                };
                shared.publish_time(time, Some(clock));
                barrier.wait();
            }
        })
    };
    let mut invalid = None;
    for _ in 0..ROUNDS {
        barrier.wait();
        let observed = shared.read_time();
        if observed != Ok(time_a) && observed != Ok(time_b) {
            invalid.get_or_insert(observed);
        }
        // Finish both barriers before asserting, so a failing regression cannot strand its peer.
        barrier.wait();
    }
    publisher.join().unwrap();
    assert!(
        invalid.is_none(),
        "reader observed mixed authority/clock publication: {invalid:?}"
    );
}
