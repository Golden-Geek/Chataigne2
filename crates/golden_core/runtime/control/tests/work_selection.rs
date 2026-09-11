use std::sync::Arc;
use std::time::{Duration, Instant};

use golden_runtime::{
    DirtySet, KernelId, PersistentBatchScheduler, RuntimeMetrics, RuntimeSchedule, ScheduledWork, WorkSelector,
    WorkUnitId,
};

const UNIT_COUNT: usize = 100_000;

fn schedule(unit_count: usize) -> RuntimeSchedule {
    RuntimeSchedule::new(
        (0..unit_count)
            .map(|index| ScheduledWork {
                id: WorkUnitId(index as u32),
                kernel: KernelId(0),
                first_lane: index as u32,
                lane_count: 1,
            })
            .collect(),
        0.5,
    )
    .expect("fixture schedule should be valid")
}

fn one_percent_dirty(unit_count: usize) -> DirtySet {
    let mut dirty = DirtySet::new(unit_count);
    for index in (0..unit_count).step_by(100) {
        dirty
            .mark(WorkUnitId(index as u32))
            .expect("fixture work should be in bounds");
    }
    dirty
}

fn percentile(samples: &[Duration], numerator: usize, denominator: usize) -> Duration {
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    let rank = sorted.len().saturating_mul(numerator).div_ceil(denominator);
    sorted[rank.saturating_sub(1).min(sorted.len() - 1)]
}

#[test]
fn direct_selection_matches_identity_worker_dispatch() {
    let schedule = schedule(1_000);
    let dirty = one_percent_dirty(1_000);
    let mut selector = WorkSelector::new(Arc::new(RuntimeMetrics::default()));
    let scheduler =
        PersistentBatchScheduler::new(2, |work: ScheduledWork| work.id, Arc::new(RuntimeMetrics::default()))
            .expect("identity scheduler should start");
    let mut direct = Vec::new();
    let mut dispatched = Vec::new();

    selector
        .select_into(&schedule, &dirty, &mut direct)
        .expect("direct selection should succeed");
    scheduler
        .execute_into(&schedule, &dirty, &mut dispatched)
        .expect("identity dispatch should succeed");

    assert_eq!(
        direct.iter().map(|work| work.id).collect::<Vec<_>>(),
        dispatched.iter().map(|(_, work)| *work).collect::<Vec<_>>()
    );
}

#[test]
#[ignore = "explicit T14 direct-selection qualification"]
fn measure_direct_selection_against_identity_worker_dispatch() {
    const SAMPLES: usize = 20;

    let schedule = schedule(UNIT_COUNT);
    let dirty = one_percent_dirty(UNIT_COUNT);
    let mut selector = WorkSelector::new(Arc::new(RuntimeMetrics::default()));
    let scheduler =
        PersistentBatchScheduler::new(8, |work: ScheduledWork| work.id, Arc::new(RuntimeMetrics::default()))
            .expect("identity scheduler should start");
    let mut direct = Vec::with_capacity(dirty.count());
    let mut dispatched = Vec::with_capacity(dirty.count());

    selector
        .select_into(&schedule, &dirty, &mut direct)
        .expect("direct warmup should succeed");
    scheduler
        .execute_into(&schedule, &dirty, &mut dispatched)
        .expect("identity warmup should succeed");

    let mut direct_samples = Vec::with_capacity(SAMPLES);
    let mut dispatch_samples = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        let started = Instant::now();
        selector
            .select_into(&schedule, &dirty, &mut direct)
            .expect("direct sample should succeed");
        direct_samples.push(started.elapsed());

        let started = Instant::now();
        scheduler
            .execute_into(&schedule, &dirty, &mut dispatched)
            .expect("identity sample should succeed");
        dispatch_samples.push(started.elapsed());
    }

    assert_eq!(
        direct.iter().map(|work| work.id).collect::<Vec<_>>(),
        dispatched.iter().map(|(_, work)| *work).collect::<Vec<_>>()
    );
    let direct_p95 = percentile(&direct_samples, 95, 100);
    let dispatch_p50 = percentile(&dispatch_samples, 50, 100);
    println!(
        "t14_selection units={UNIT_COUNT} dirty={} direct_p50_us={} direct_p95_us={} direct_p99_us={} direct_max_us={} identity_p50_us={} identity_p95_us={} identity_p99_us={} identity_max_us={}",
        dirty.count(),
        percentile(&direct_samples, 50, 100).as_micros(),
        direct_p95.as_micros(),
        percentile(&direct_samples, 99, 100).as_micros(),
        direct_samples.iter().max().expect("direct samples").as_micros(),
        dispatch_p50.as_micros(),
        percentile(&dispatch_samples, 95, 100).as_micros(),
        percentile(&dispatch_samples, 99, 100).as_micros(),
        dispatch_samples.iter().max().expect("dispatch samples").as_micros(),
    );
    assert!(
        direct_p95 < dispatch_p50,
        "direct selection p95 {direct_p95:?} should beat identity dispatch p50 {dispatch_p50:?}"
    );
}
