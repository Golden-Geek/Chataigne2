use std::env;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use golden_runtime::{
    DirtySet, ExecutionMode, KernelId, PersistentBatchScheduler, RuntimeMetrics, RuntimeSchedule, ScheduledWork,
    WorkUnitId,
};

const TOTAL_LANES: usize = 100_000;
const DENSE_P95_LIMIT: Duration = Duration::from_millis(8);
const DENSE_P99_LIMIT: Duration = Duration::from_millis(12);
const SPARSE_P95_LIMIT: Duration = Duration::from_millis(2);
const IDLE_P95_LIMIT: Duration = Duration::from_micros(500);
const DEADLINE: Duration = Duration::from_micros(16_670);
const MAX_DEADLINE_MISS_RATE: f64 = 0.001;
const DEFAULT_SAMPLES: usize = 1_000;
const DEFAULT_WARMUP: usize = 20;
const DEFAULT_MEASUREMENT_WORKERS: usize = 4;
const DETERMINISM_WORKERS: [usize; 4] = [1, 2, 4, 8];

#[derive(Clone, Copy, Debug)]
struct Partition {
    name: &'static str,
    processors: usize,
    lanes_per_processor: usize,
}

impl Partition {
    const fn lane_count(self) -> usize {
        self.processors * self.lanes_per_processor
    }
}

const PARTITIONS: [Partition; 2] = [
    Partition {
        name: "p1000-l100",
        processors: 1_000,
        lanes_per_processor: 100,
    },
    Partition {
        name: "p10000-l10",
        processors: 10_000,
        lanes_per_processor: 10,
    },
];

#[derive(Debug)]
struct ScenarioMetrics {
    p95: Duration,
    p99: Duration,
    max: Duration,
    deadline_misses: usize,
    digest: u64,
}

fn env_usize(name: &str, default: usize) -> usize {
    env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn schedule_for(partition: Partition) -> RuntimeSchedule {
    let units = (0..partition.processors)
        .map(|processor| ScheduledWork {
            id: WorkUnitId(u32::try_from(processor).expect("processor index fits u32")),
            kernel: KernelId(0),
            first_lane: u32::try_from(processor * partition.lanes_per_processor).expect("first lane fits u32"),
            lane_count: u32::try_from(partition.lanes_per_processor).expect("lane count fits u32"),
        })
        .collect();
    RuntimeSchedule::new(units, 0.15).expect("scale-test schedule must be valid")
}

fn thresholds() -> Arc<Vec<f64>> {
    Arc::new(
        (0..TOTAL_LANES)
            .map(|lane| ((lane as u64 * 2_654_435_761u64) % TOTAL_LANES as u64) as f64 / TOTAL_LANES as f64)
            .collect(),
    )
}

fn comparison_executor(
    thresholds: Arc<Vec<f64>>,
    shared_value_bits: Arc<AtomicU64>,
) -> impl Fn(ScheduledWork) -> u64 + Send + Sync + 'static {
    move |work| {
        let shared_value = f64::from_bits(shared_value_bits.load(Ordering::Relaxed));
        let mut digest = 0xcbf2_9ce4_8422_2325u64;
        let first_lane = work.first_lane as usize;
        let end = first_lane + work.lane_count as usize;
        for lane in first_lane..end {
            let comparison = shared_value >= thresholds[lane];
            digest ^= (lane as u64).rotate_left(17) ^ u64::from(comparison);
            digest = digest.wrapping_mul(0x0000_0100_0000_01b3);
        }
        digest
    }
}

fn output_digest(outputs: &[(WorkUnitId, u64)]) -> u64 {
    outputs.iter().fold(0x9e37_79b9_7f4a_7c15, |digest, (work_id, value)| {
        digest.rotate_left(9) ^ value.wrapping_add((work_id.0 as u64).wrapping_mul(0x9e37_79b9))
    })
}

fn dense_dirty_set(partition: Partition) -> DirtySet {
    let mut dirty = DirtySet::new(partition.processors);
    dirty.mark_all();
    dirty
}

fn sparse_dirty_set(partition: Partition) -> DirtySet {
    let mut dirty = DirtySet::new(partition.processors);
    for processor in (0..partition.processors).step_by(100) {
        dirty
            .mark(WorkUnitId(u32::try_from(processor).expect("processor index fits u32")))
            .expect("1% sparse work ID must be valid");
    }
    dirty
}

fn percentile(samples: &[Duration], percentile: f64) -> Duration {
    assert!(!samples.is_empty());
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    let index = ((sorted.len() as f64 * percentile).ceil() as usize)
        .saturating_sub(1)
        .min(sorted.len() - 1);
    sorted[index]
}

fn assert_worker_determinism(partition: Partition, sparse: bool) {
    let mut expected = None;
    for workers in DETERMINISM_WORKERS {
        let shared_value_bits = Arc::new(AtomicU64::new(0.625f64.to_bits()));
        let scheduler = PersistentBatchScheduler::new(
            workers,
            comparison_executor(thresholds(), shared_value_bits),
            Arc::new(RuntimeMetrics::default()),
        )
        .expect("scale-test scheduler must start");
        let schedule = schedule_for(partition);
        let dirty = if sparse {
            sparse_dirty_set(partition)
        } else {
            dense_dirty_set(partition)
        };
        let mut outputs = Vec::with_capacity(partition.processors);
        scheduler
            .execute_into(&schedule, &dirty, &mut outputs)
            .expect("deterministic scale execution must succeed");
        let digest = output_digest(&outputs);
        match expected {
            Some(expected) => assert_eq!(
                digest,
                expected,
                "{} {} digest changed with {workers} workers",
                partition.name,
                if sparse { "sparse" } else { "dense" }
            ),
            None => expected = Some(digest),
        }
    }
}

fn measure_partition(
    partition: Partition,
    samples: usize,
    warmup: usize,
    workers: usize,
) -> (ScenarioMetrics, ScenarioMetrics, ScenarioMetrics) {
    assert_eq!(partition.lane_count(), TOTAL_LANES);
    let shared_value_bits = Arc::new(AtomicU64::new(0.5f64.to_bits()));
    let scheduler = PersistentBatchScheduler::new(
        workers,
        comparison_executor(thresholds(), Arc::clone(&shared_value_bits)),
        Arc::new(RuntimeMetrics::default()),
    )
    .expect("scale-test scheduler must start");
    let schedule = schedule_for(partition);
    let dense_dirty = dense_dirty_set(partition);
    let sparse_dirty = sparse_dirty_set(partition);
    let idle_dirty = DirtySet::new(partition.processors);
    let mut outputs = Vec::with_capacity(partition.processors);
    let output_capacity = outputs.capacity();

    for sample in 0..warmup {
        let shared_value = 0.25 + (sample % 2) as f64 * 0.5;
        shared_value_bits.store(shared_value.to_bits(), Ordering::Relaxed);
        scheduler
            .execute_into(&schedule, &dense_dirty, &mut outputs)
            .expect("scale-test warmup must succeed");
    }

    let mut dense_samples = Vec::with_capacity(samples);
    let mut dense_digest = 0;
    let mut deadline_misses = 0;
    for sample in 0..samples {
        let shared_value = 0.25 + (sample % 2) as f64 * 0.5;
        shared_value_bits.store(shared_value.to_bits(), Ordering::Relaxed);
        let started = Instant::now();
        let mode = scheduler
            .execute_into(&schedule, &dense_dirty, &mut outputs)
            .expect("dense scale execution must succeed");
        let elapsed = started.elapsed();
        assert_eq!(mode, ExecutionMode::Dense);
        assert_eq!(outputs.len(), partition.processors);
        assert_eq!(outputs.capacity(), output_capacity);
        dense_digest ^= output_digest(&outputs).rotate_left((sample % 64) as u32);
        deadline_misses += usize::from(elapsed > DEADLINE);
        dense_samples.push(elapsed);
    }

    let mut sparse_samples = Vec::with_capacity(samples);
    let mut sparse_digest = 0;
    let expected_sparse_processors = partition.processors / 100;
    for sample in 0..samples {
        let shared_value = 0.25 + (sample % 2) as f64 * 0.5;
        shared_value_bits.store(shared_value.to_bits(), Ordering::Relaxed);
        let started = Instant::now();
        let mode = scheduler
            .execute_into(&schedule, &sparse_dirty, &mut outputs)
            .expect("sparse scale execution must succeed");
        let elapsed = started.elapsed();
        assert_eq!(mode, ExecutionMode::Sparse);
        assert_eq!(outputs.len(), expected_sparse_processors);
        assert_eq!(outputs.capacity(), output_capacity);
        sparse_digest ^= output_digest(&outputs).rotate_left((sample % 64) as u32);
        sparse_samples.push(elapsed);
    }

    let mut idle_samples = Vec::with_capacity(samples);
    let mut idle_digest = 0;
    for sample in 0..samples {
        shared_value_bits.store((sample as f64).to_bits(), Ordering::Relaxed);
        let started = Instant::now();
        let mode = scheduler
            .execute_into(&schedule, &idle_dirty, &mut outputs)
            .expect("idle scale execution must succeed");
        let elapsed = started.elapsed();
        assert_eq!(mode, ExecutionMode::Sparse);
        assert!(outputs.is_empty(), "idle execution evaluated lanes");
        assert_eq!(outputs.capacity(), output_capacity);
        idle_digest ^= output_digest(&outputs);
        idle_samples.push(elapsed);
    }

    let metrics = |samples: &[Duration], misses, digest| ScenarioMetrics {
        p95: percentile(samples, 0.95),
        p99: percentile(samples, 0.99),
        max: samples.iter().copied().max().unwrap_or_default(),
        deadline_misses: misses,
        digest,
    };
    (
        metrics(&dense_samples, deadline_misses, dense_digest),
        metrics(&sparse_samples, 0, sparse_digest),
        metrics(&idle_samples, 0, idle_digest),
    )
}

fn micros(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1_000_000.0
}

#[test]
#[ignore = "explicit release-mode runtime scale qualification"]
fn runtime_100k_scalar_dense_sparse_idle_qualification() {
    let samples = env_usize("GC_SCALE_SAMPLES", DEFAULT_SAMPLES);
    let warmup = env_usize("GC_SCALE_WARMUP", DEFAULT_WARMUP);
    let workers = env_usize("GC_SCALE_WORKERS", DEFAULT_MEASUREMENT_WORKERS);
    assert!(
        samples >= DEFAULT_SAMPLES,
        "at least {DEFAULT_SAMPLES} samples are required to measure <0.1% deadline misses"
    );
    assert!(
        workers > 0,
        "scale qualification requires at least one measurement worker"
    );

    for partition in PARTITIONS {
        assert_worker_determinism(partition, false);
        assert_worker_determinism(partition, true);
        let (dense, sparse, idle) = measure_partition(partition, samples, warmup, workers);
        let miss_rate = dense.deadline_misses as f64 / samples as f64;

        println!(
            concat!(
                "RUNTIME_SCALE_RESULT={{\"partition\":\"{}\",\"processors\":{},",
                "\"lanes_per_processor\":{},\"lanes\":{},\"samples\":{},\"workers\":{},",
                "\"dense_p95_us\":{:.3},\"dense_p99_us\":{:.3},\"dense_max_us\":{:.3},",
                "\"dense_deadline_misses\":{},\"dense_deadline_miss_rate\":{:.6},",
                "\"sparse_p95_us\":{:.3},\"sparse_p99_us\":{:.3},\"sparse_max_us\":{:.3},",
                "\"idle_p95_us\":{:.3},\"idle_p99_us\":{:.3},\"idle_max_us\":{:.3},",
                "\"dense_digest\":{},\"sparse_digest\":{},\"idle_digest\":{}}}"
            ),
            partition.name,
            partition.processors,
            partition.lanes_per_processor,
            partition.lane_count(),
            samples,
            workers,
            micros(dense.p95),
            micros(dense.p99),
            micros(dense.max),
            dense.deadline_misses,
            miss_rate,
            micros(sparse.p95),
            micros(sparse.p99),
            micros(sparse.max),
            micros(idle.p95),
            micros(idle.p99),
            micros(idle.max),
            dense.digest,
            sparse.digest,
            idle.digest,
        );

        assert!(
            dense.p95 <= DENSE_P95_LIMIT,
            "{} dense p95 {:?} exceeds {:?}",
            partition.name,
            dense.p95,
            DENSE_P95_LIMIT
        );
        assert!(
            dense.p99 <= DENSE_P99_LIMIT,
            "{} dense p99 {:?} exceeds {:?}",
            partition.name,
            dense.p99,
            DENSE_P99_LIMIT
        );
        assert!(
            sparse.p95 <= SPARSE_P95_LIMIT,
            "{} sparse p95 {:?} exceeds {:?}",
            partition.name,
            sparse.p95,
            SPARSE_P95_LIMIT
        );
        assert!(
            idle.p95 <= IDLE_P95_LIMIT,
            "{} idle p95 {:?} exceeds {:?}",
            partition.name,
            idle.p95,
            IDLE_P95_LIMIT
        );
        assert!(
            miss_rate < MAX_DEADLINE_MISS_RATE,
            "{} deadline miss rate {:.4}% is not below {:.4}%",
            partition.name,
            miss_rate * 100.0,
            MAX_DEADLINE_MISS_RATE * 100.0
        );
    }
}
