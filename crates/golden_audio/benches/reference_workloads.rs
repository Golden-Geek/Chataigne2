use std::time::Duration;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use golden_audio::qualification::{ReferenceWorkload, ReferenceWorkloadHarness};

const BLOCK_FRAMES: u64 = 128;
const BLOCKS_PER_FIXTURE: u64 = 256;

fn reference_workloads(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("reference-workloads");
    group.sample_size(20);
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(3));
    for workload in ReferenceWorkload::ALL {
        let specification = workload.specification();
        group.throughput(Throughput::Elements(
            u64::from(specification.channels).saturating_mul(BLOCK_FRAMES),
        ));
        group.bench_with_input(
            BenchmarkId::from_parameter(name(workload)),
            &workload,
            |bencher, workload| {
                bencher.iter_custom(|iterations| {
                    let mut remaining = iterations;
                    let mut measured = Duration::ZERO;
                    while remaining > 0 {
                        let blocks = remaining.min(BLOCKS_PER_FIXTURE);
                        let mut harness = ReferenceWorkloadHarness::new(*workload).unwrap();
                        let started = std::time::Instant::now();
                        harness.render_blocks(blocks as usize).unwrap();
                        measured += started.elapsed();
                        std::hint::black_box(harness.observation().peak_output);
                        remaining -= blocks;
                    }
                    measured
                });
            },
        );
    }
    group.finish();
}

const fn name(workload: ReferenceWorkload) -> &'static str {
    match workload {
        ReferenceWorkload::Small => "small",
        ReferenceWorkload::Medium => "medium",
        ReferenceWorkload::Large => "large",
        ReferenceWorkload::ExtremeOffline => "extreme-offline",
    }
}

criterion_group!(benches, reference_workloads);
criterion_main!(benches);
