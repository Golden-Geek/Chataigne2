use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use golden_audio::{
    AudioChannelId, MeterAccumulator, PitchAnalysisConfiguration, PitchAnalyzer, PlanarBuffer, SampleRate,
    SpectrumAnalysisConfiguration, SpectrumAnalyzer,
};
use uuid::Uuid;

fn analysis_benchmarks(criterion: &mut Criterion) {
    meter_throughput(criterion);
    pitch_throughput(criterion);
    spectrum_throughput(criterion);
}

fn meter_throughput(criterion: &mut Criterion) {
    let channels = 256;
    let frames = 128;
    let ids = (0..channels)
        .map(|index| AudioChannelId::from_uuid(Uuid::from_u128(index as u128 + 1)))
        .collect();
    let mut meter = MeterAccumulator::new(ids, frames).unwrap();
    let mut source = PlanarBuffer::new(channels, frames).unwrap();
    for channel in 0..channels {
        source.channel_mut(channel).fill(0.25);
    }
    criterion.bench_function("analysis/rms_peak_256_channels_128_frames", |bencher| {
        bencher.iter(|| {
            meter
                .accumulate(&source, frames, |observations| {
                    std::hint::black_box(observations);
                })
                .unwrap();
        });
    });
}

fn pitch_throughput(criterion: &mut Criterion) {
    let mut analyzer =
        PitchAnalyzer::new(PitchAnalysisConfiguration::default(), SampleRate::new(48_000).unwrap()).unwrap();
    let samples = sine(440.0, 2_048);
    criterion.bench_function("analysis/yin_2048", |bencher| {
        bencher.iter(|| {
            std::hint::black_box(analyzer.analyze(&samples));
        });
    });
}

fn spectrum_throughput(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("analysis/real_fft");
    for size in [256_u32, 2_048, 16_384] {
        let configuration = SpectrumAnalysisConfiguration {
            fft_size: size,
            ..SpectrumAnalysisConfiguration::default()
        };
        let mut analyzer = SpectrumAnalyzer::new(configuration, SampleRate::new(48_000).unwrap()).unwrap();
        let samples = sine(1_500.0, size as usize);
        group.throughput(Throughput::Elements(u64::from(size)));
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |bencher, _| {
            bencher.iter(|| {
                std::hint::black_box(analyzer.analyze(&samples).unwrap());
            });
        });
    }
    group.finish();
}

fn sine(frequency: f32, frames: usize) -> Vec<f32> {
    (0..frames)
        .map(|frame| (std::f32::consts::TAU * frequency * frame as f32 / 48_000.0).sin())
        .collect()
}

criterion_group!(benches, analysis_benchmarks);
criterion_main!(benches);
