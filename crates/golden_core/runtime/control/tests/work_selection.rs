use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use golden_runtime::{
    DirtySet, ExecutionMode, KernelId, PersistentBatchScheduler, RuntimeMetrics, RuntimeSchedule, ScheduledWork,
    WorkSelector, WorkUnitId,
};

const UNIT_COUNT: usize = 100_000;

struct CountingAllocator;

static TRACK_ALLOCATIONS: AtomicBool = AtomicBool::new(false);
static ALLOCATION_COUNT: AtomicUsize = AtomicUsize::new(0);

// SAFETY: every operation delegates to the process System allocator without changing layouts or
// returned pointers. The atomics only count allocation calls during an explicit measurement window.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if TRACK_ALLOCATIONS.load(Ordering::Relaxed) {
            ALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
        }
        // SAFETY: delegated with the caller-provided layout.
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        if TRACK_ALLOCATIONS.load(Ordering::Relaxed) {
            ALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
        }
        // SAFETY: delegated with the caller-provided layout.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: delegated with the original pointer and layout.
        unsafe { System.dealloc(pointer, layout) }
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if TRACK_ALLOCATIONS.load(Ordering::Relaxed) {
            ALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
        }
        // SAFETY: delegated with the original pointer, layout, and requested size.
        unsafe { System.realloc(pointer, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL_ALLOCATOR: CountingAllocator = CountingAllocator;

fn schedule(unit_count: usize) -> RuntimeSchedule {
    schedule_with_threshold(unit_count, 0.5)
}

fn schedule_with_threshold(unit_count: usize, dense_threshold: f32) -> RuntimeSchedule {
    RuntimeSchedule::new(
        (0..unit_count)
            .map(|index| ScheduledWork {
                id: WorkUnitId(index as u32),
                kernel: KernelId(0),
                first_lane: index as u32,
                lane_count: 1,
            })
            .collect(),
        dense_threshold,
    )
    .expect("fixture schedule should be valid")
}

fn density_dirty(unit_count: usize, per_thousand: usize) -> DirtySet {
    let mut dirty = DirtySet::new(unit_count);
    if per_thousand == 1_000 {
        dirty.mark_all();
        return dirty;
    }
    let selected_count = unit_count.saturating_mul(per_thousand) / 1_000;
    for ordinal in (0..selected_count).rev() {
        let index = ordinal * unit_count / selected_count;
        dirty
            .mark(WorkUnitId(index as u32))
            .expect("fixture work should be in bounds");
    }
    dirty
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

fn measure_selection(
    schedule: &RuntimeSchedule,
    dirty: &DirtySet,
    samples: usize,
) -> (Vec<Duration>, usize, golden_runtime::SelectionReport) {
    let mut selector = WorkSelector::new(Arc::new(RuntimeMetrics::default()));
    let mut selected = Vec::with_capacity(schedule.work_count());
    selector
        .select_into(schedule, dirty, &mut selected)
        .expect("selection warmup should succeed");
    let mut timings = Vec::with_capacity(samples);
    let mut maximum_allocations = 0;
    let mut last_report = None;
    for _ in 0..samples {
        ALLOCATION_COUNT.store(0, Ordering::Relaxed);
        TRACK_ALLOCATIONS.store(true, Ordering::Relaxed);
        let started = Instant::now();
        let report = selector
            .select_into(schedule, dirty, &mut selected)
            .expect("selection sample should succeed");
        let elapsed = started.elapsed();
        TRACK_ALLOCATIONS.store(false, Ordering::Relaxed);
        maximum_allocations = maximum_allocations.max(ALLOCATION_COUNT.load(Ordering::Relaxed));
        timings.push(elapsed);
        last_report = Some(report);
    }
    (
        timings,
        maximum_allocations,
        last_report.expect("at least one selection sample is required"),
    )
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
fn sparse_selection_visits_dirty_work_in_compile_order() {
    let schedule = schedule(10_000);
    let mut selector = WorkSelector::new(Arc::new(RuntimeMetrics::default()));
    selector.prepare(schedule.work_count());
    let mut selected = Vec::new();

    for per_thousand in [0, 1, 10, 100] {
        let dirty = density_dirty(10_000, per_thousand);
        let report = selector
            .select_into(&schedule, &dirty, &mut selected)
            .expect("sparse selection should succeed");
        assert_eq!(report.mode, ExecutionMode::Sparse);
        assert_eq!(report.selected_units, dirty.count());
        assert_eq!(report.visited_units, dirty.count());
        assert!(report.visited_dirty_words <= dirty.count());
        assert!(selected.windows(2).all(|pair| pair[0].id < pair[1].id));
    }

    let dirty = density_dirty(10_000, 100);
    let mut dense_selector = WorkSelector::new(Arc::new(RuntimeMetrics::default()));
    let dense_schedule = schedule_with_threshold(10_000, 0.0);
    let mut dense_selected = Vec::new();
    dense_selector
        .select_into(&dense_schedule, &dirty, &mut dense_selected)
        .expect("dense selection should succeed");
    selector
        .select_into(&schedule, &dirty, &mut selected)
        .expect("sparse selection should succeed");
    assert_eq!(selected, dense_selected);
    let scheduler = PersistentBatchScheduler::new(
        2,
        |work: ScheduledWork| (work.first_lane, work.lane_count),
        Arc::new(RuntimeMetrics::default()),
    )
    .expect("value scheduler should start");
    let mut sparse_outputs = Vec::new();
    let mut dense_outputs = Vec::new();
    scheduler
        .execute_into(&schedule, &dirty, &mut sparse_outputs)
        .expect("sparse execution should succeed");
    scheduler
        .execute_into(&dense_schedule, &dirty, &mut dense_outputs)
        .expect("dense execution should succeed");
    assert_eq!(sparse_outputs, dense_outputs);

    let mut dirty = density_dirty(10_000, 10);
    let count_before_duplicate = dirty.count();
    dirty.mark(WorkUnitId(500)).expect("duplicate mark should remain valid");
    dirty.mark(WorkUnitId(500)).expect("duplicate mark should remain valid");
    assert_eq!(dirty.count(), count_before_duplicate);
    dirty.clear();
    let report = selector
        .select_into(&schedule, &dirty, &mut selected)
        .expect("cleared selection should succeed");
    assert_eq!(report.visited_units, 0);
    assert!(selected.is_empty());

    dirty.mark_all();
    let report = selector
        .select_into(&schedule, &dirty, &mut selected)
        .expect("dense selection should succeed");
    assert_eq!(report.mode, ExecutionMode::Dense);
    assert_eq!(report.visited_units, 10_000);
    assert_eq!(report.selected_units, 10_000);

    let mismatched = DirtySet::new(9_999);
    assert!(selector.select_into(&schedule, &mismatched, &mut selected).is_err());
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

#[test]
#[ignore = "explicit T14 sparse/dense crossover qualification"]
fn measure_sparse_dense_selection_crossover() {
    const SAMPLES: usize = 30;

    let sparse_schedule = schedule_with_threshold(UNIT_COUNT, 1.0);
    let dense_schedule = schedule_with_threshold(UNIT_COUNT, 0.0);
    for per_thousand in [0, 1, 10, 100, 250, 500, 1_000] {
        let dirty = density_dirty(UNIT_COUNT, per_thousand);
        let (dense_timings, dense_allocations, dense_report) = measure_selection(&dense_schedule, &dirty, SAMPLES);
        if per_thousand == 1_000 {
            println!(
                "t14_density density_percent=100 selected={} actual_mode={:?} visited={} p50_us={} p95_us={} p99_us={} max_us={} max_allocations={}",
                dense_report.selected_units,
                dense_report.mode,
                dense_report.visited_units,
                percentile(&dense_timings, 50, 100).as_micros(),
                percentile(&dense_timings, 95, 100).as_micros(),
                percentile(&dense_timings, 99, 100).as_micros(),
                dense_timings.iter().max().expect("dense samples").as_micros(),
                dense_allocations,
            );
            continue;
        }
        let (sparse_timings, sparse_allocations, sparse_report) = measure_selection(&sparse_schedule, &dirty, SAMPLES);
        println!(
            "t14_density density_percent={:.1} selected={} sparse_visited={} sparse_words={} sparse_p50_us={} sparse_p95_us={} sparse_p99_us={} sparse_max_us={} sparse_max_allocations={} dense_visited={} dense_p50_us={} dense_p95_us={} dense_p99_us={} dense_max_us={} dense_max_allocations={}",
            per_thousand as f64 / 10.0,
            sparse_report.selected_units,
            sparse_report.visited_units,
            sparse_report.visited_dirty_words,
            percentile(&sparse_timings, 50, 100).as_micros(),
            percentile(&sparse_timings, 95, 100).as_micros(),
            percentile(&sparse_timings, 99, 100).as_micros(),
            sparse_timings.iter().max().expect("sparse samples").as_micros(),
            sparse_allocations,
            dense_report.visited_units,
            percentile(&dense_timings, 50, 100).as_micros(),
            percentile(&dense_timings, 95, 100).as_micros(),
            percentile(&dense_timings, 99, 100).as_micros(),
            dense_timings.iter().max().expect("dense samples").as_micros(),
            dense_allocations,
        );
        assert_eq!(sparse_report.selected_units, dense_report.selected_units);
        assert_eq!(sparse_allocations, 0);
        assert_eq!(dense_allocations, 0);
    }
}
