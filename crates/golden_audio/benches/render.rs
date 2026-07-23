mod support;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};

fn render_benchmarks(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("render");
    for (channels, routes, frames) in [
        (8, 16, 32),
        (8, 16, 1_024),
        (32, 128, 128),
        (128, 1_024, 256),
        (256, 16_384, 1_024),
    ] {
        let mut fixture = support::fixture(channels, routes, frames);
        group.throughput(Throughput::Elements((channels * frames) as u64));
        group.bench_with_input(
            BenchmarkId::new(format!("{channels}ch-{routes}routes"), frames),
            &frames,
            |bencher, _| {
                bencher.iter(|| {
                    fixture
                        .processor
                        .render(
                            &fixture.physical_inputs,
                            &fixture.playback_inputs,
                            &mut fixture.physical_outputs,
                            fixture.frames,
                        )
                        .unwrap();
                });
            },
        );
    }
    group.finish();
}

criterion_group!(benches, render_benchmarks);
criterion_main!(benches);
