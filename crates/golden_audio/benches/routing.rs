mod support;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};

fn sparse_routing_benchmarks(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("sparse-routing");
    for routes in [0, 16, 128, 1_024, 4_096, 16_384] {
        let channels = if routes <= 16 {
            8
        } else if routes <= 128 {
            32
        } else if routes <= 1_024 {
            128
        } else {
            256
        };
        let effective_routes = routes.max(channels * 2);
        let mut fixture = support::fixture(channels, effective_routes, 128);
        group.throughput(Throughput::Elements(effective_routes as u64));
        group.bench_with_input(
            BenchmarkId::new("requested-routes", routes),
            &effective_routes,
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

criterion_group!(benches, sparse_routing_benchmarks);
criterion_main!(benches);
