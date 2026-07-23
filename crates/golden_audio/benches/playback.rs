use std::{
    io::Write,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use golden_audio::{
    AudioSourceFingerprint, DefaultPlaybackRoute, GainDb, PlanarBuffer, PlaybackId, PlaybackVoice,
    PlaybackVoiceController, PlaybackVoiceRenderer, PlaybackVoiceSource, ResidentAssetKey, ResidentAudioAsset,
    SampleRate, decode_audio_file, default_playback_routes, playback_voice_pool, streaming_playback_ring,
};
use tempfile::{Builder, NamedTempFile};

const BLOCK_FRAMES: usize = 128;
const BLOCKS_PER_SAMPLE: usize = 100;

fn playback_benchmarks(criterion: &mut Criterion) {
    resident_voices(criterion);
    mixed_voices(criterion);
    decoder_throughput(criterion);
}

fn resident_voices(criterion: &mut Criterion) {
    let asset = resident_asset(1, BLOCK_FRAMES * BLOCKS_PER_SAMPLE);
    let mut group = criterion.benchmark_group("playback/resident_voices");
    for voice_count in [1_u16, 16, 64, 128, 256] {
        group.throughput(Throughput::Elements(u64::from(voice_count) * BLOCKS_PER_SAMPLE as u64));
        group.bench_with_input(BenchmarkId::from_parameter(voice_count), &voice_count, |bencher, _| {
            bencher.iter_custom(|iterations| {
                let mut measured = Duration::ZERO;
                for _ in 0..iterations {
                    let (mut controller, mut renderer, mut destination) =
                        resident_pool(voice_count, Arc::clone(&asset));
                    let started_at = Instant::now();
                    for _ in 0..BLOCKS_PER_SAMPLE {
                        renderer.render(&mut destination, BLOCK_FRAMES).unwrap();
                        std::hint::black_box(destination.sample(0, 0));
                    }
                    measured += started_at.elapsed();
                    controller.reclaim(|_| {});
                    renderer.into_retirement().reclaim();
                }
                measured
            });
        });
    }
    group.finish();
}

fn mixed_voices(criterion: &mut Criterion) {
    let asset = resident_asset(1, BLOCK_FRAMES * BLOCKS_PER_SAMPLE);
    criterion.bench_function("playback/mixed_8_resident_8_streamed", |bencher| {
        bencher.iter_custom(|iterations| {
            let mut measured = Duration::ZERO;
            for _ in 0..iterations {
                let (mut controller, mut renderer, mut destination) = mixed_pool(Arc::clone(&asset));
                let started_at = Instant::now();
                for _ in 0..BLOCKS_PER_SAMPLE {
                    renderer.render(&mut destination, BLOCK_FRAMES).unwrap();
                    std::hint::black_box(destination.sample(0, 0));
                }
                measured += started_at.elapsed();
                controller.reclaim(|_| {});
                renderer.into_retirement().reclaim();
            }
            measured
        });
    });
}

fn decoder_throughput(criterion: &mut Criterion) {
    let file = sine_wave_file(2, 48_000, 48_000);
    criterion.bench_function("playback/decode_wave_1s_stereo", |bencher| {
        bencher.iter(|| {
            let asset = decode_audio_file(file.path(), SampleRate::default()).unwrap();
            std::hint::black_box(asset.frames());
        });
    });
}

fn resident_pool(
    voice_count: u16,
    asset: Arc<ResidentAudioAsset>,
) -> (PlaybackVoiceController, PlaybackVoiceRenderer, PlanarBuffer) {
    let (mut controller, renderer) = playback_voice_pool(voice_count, 2, BLOCK_FRAMES).unwrap();
    for index in 0..voice_count {
        controller
            .try_activate(voice(
                index,
                PlaybackVoiceSource::Resident(Arc::clone(&asset)),
                default_playback_routes(1, 2),
            ))
            .unwrap();
    }
    (controller, renderer, PlanarBuffer::new(2, BLOCK_FRAMES).unwrap())
}

fn mixed_pool(asset: Arc<ResidentAudioAsset>) -> (PlaybackVoiceController, PlaybackVoiceRenderer, PlanarBuffer) {
    let (mut controller, renderer) = playback_voice_pool(16, 2, BLOCK_FRAMES).unwrap();
    for index in 0..8 {
        controller
            .try_activate(voice(
                index,
                PlaybackVoiceSource::Resident(Arc::clone(&asset)),
                default_playback_routes(1, 2),
            ))
            .unwrap();
    }
    let stream_frames = BLOCK_FRAMES * BLOCKS_PER_SAMPLE;
    for index in 8..16 {
        let (mut writer, reader) = streaming_playback_ring(1, stream_frames).unwrap();
        writer.write_interleaved(&vec![0.25; stream_frames]).unwrap();
        writer.finish();
        controller
            .try_activate(voice(
                index,
                PlaybackVoiceSource::Stream {
                    reader,
                    channels: 1,
                    scratch: vec![0.0; BLOCK_FRAMES].into_boxed_slice(),
                },
                default_playback_routes(1, 2),
            ))
            .unwrap();
    }
    (controller, renderer, PlanarBuffer::new(2, BLOCK_FRAMES).unwrap())
}

fn voice(index: u16, source: PlaybackVoiceSource, routes: Vec<DefaultPlaybackRoute>) -> PlaybackVoice {
    PlaybackVoice::new(
        PlaybackId::new(format!("voice-{index}")).unwrap(),
        PathBuf::from("benchmark.wav"),
        source,
        GainDb::UNITY,
        routes,
        480,
        BLOCK_FRAMES,
    )
    .unwrap()
}

fn resident_asset(channels: u16, frames: usize) -> Arc<ResidentAudioAsset> {
    Arc::new(
        ResidentAudioAsset::new(
            ResidentAssetKey {
                source: AudioSourceFingerprint {
                    canonical_path: PathBuf::from("benchmark.wav"),
                    length_bytes: 1,
                    modified_nanos: 1,
                },
                track: 0,
                engine_sample_rate: SampleRate::default(),
            },
            channels,
            frames,
            vec![0.25; usize::from(channels) * frames],
        )
        .unwrap(),
    )
}

fn sine_wave_file(channels: u16, sample_rate: u32, frames: u32) -> NamedTempFile {
    let mut file = Builder::new().suffix(".wav").tempfile().unwrap();
    let data_bytes = frames.saturating_mul(u32::from(channels)).saturating_mul(2);
    file.write_all(b"RIFF").unwrap();
    file.write_all(&(36 + data_bytes).to_le_bytes()).unwrap();
    file.write_all(b"WAVEfmt ").unwrap();
    file.write_all(&16_u32.to_le_bytes()).unwrap();
    file.write_all(&1_u16.to_le_bytes()).unwrap();
    file.write_all(&channels.to_le_bytes()).unwrap();
    file.write_all(&sample_rate.to_le_bytes()).unwrap();
    file.write_all(&(sample_rate * u32::from(channels) * 2).to_le_bytes())
        .unwrap();
    file.write_all(&(channels * 2).to_le_bytes()).unwrap();
    file.write_all(&16_u16.to_le_bytes()).unwrap();
    file.write_all(b"data").unwrap();
    file.write_all(&data_bytes.to_le_bytes()).unwrap();
    for frame in 0..frames {
        let sample = (((frame as f32 * 440.0 * std::f32::consts::TAU / sample_rate as f32).sin()) * 12_000.0) as i16;
        for _ in 0..channels {
            file.write_all(&sample.to_le_bytes()).unwrap();
        }
    }
    file.flush().unwrap();
    file
}

criterion_group!(benches, playback_benchmarks);
criterion_main!(benches);
