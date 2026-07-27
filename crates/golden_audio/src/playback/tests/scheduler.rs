use std::{
    thread,
    time::{Duration, Instant},
};

use crate::{CommandSequence, GainDb, PlayFileRequest, PlaybackId, SampleRate};

use super::{
    super::{
        PlaybackPreparation, PlaybackPreparationResult, PlaybackScheduler, PlaybackSchedulerConfig,
        PlaybackSchedulerRequest,
    },
    fixtures::{rewrite_sine_wave, sine_wave_file},
};

#[test]
fn scheduler_rejects_zero_workers() {
    let mut config = config();
    config.worker_count = 0;
    assert!(config.validate().is_err());
}

#[test]
fn scheduler_requires_a_bounded_inbox_for_every_worker() {
    let mut config = config();
    config.worker_count = 3;
    config.job_capacity = 2;
    assert!(config.validate().is_err());
}

#[test]
fn same_id_replacement_discards_stale_decode_completion() {
    let first = sine_wave_file(1, 48_000, 4_096);
    let second = sine_wave_file(2, 48_000, 128);
    let id = PlaybackId::new("replace").unwrap();
    let mut scheduler = PlaybackScheduler::new(config()).unwrap();
    scheduler.try_schedule(request(1, id.clone(), first.path())).unwrap();
    scheduler.try_schedule(request(2, id.clone(), second.path())).unwrap();

    let result = wait_result(&mut scheduler);
    assert_eq!(result.sequence(), CommandSequence::new(2).unwrap());
    match result {
        PlaybackPreparationResult::Prepared {
            preparation: PlaybackPreparation::Resident(asset),
            ..
        } => assert_eq!(asset.channels(), 2),
        _ => panic!("replacement should prepare the newest resident source"),
    }
    scheduler.shutdown().unwrap();
}

#[test]
fn stop_before_load_and_stop_all_cancel_without_stale_results() {
    let file = sine_wave_file(2, 48_000, 32_768);
    let mut scheduler = PlaybackScheduler::new(config()).unwrap();
    let stopped = PlaybackId::new("stopped").unwrap();
    scheduler
        .try_schedule(request(1, stopped.clone(), file.path()))
        .unwrap();
    assert!(scheduler.stop(&stopped));

    for sequence in 2..=12 {
        scheduler
            .try_schedule(request(
                sequence,
                PlaybackId::new(format!("voice-{sequence}")).unwrap(),
                file.path(),
            ))
            .unwrap();
    }
    assert_eq!(scheduler.stop_all(), 11);
    assert_eq!(scheduler.active_request_count(), 0);
    thread::sleep(Duration::from_millis(40));
    assert!(scheduler.try_recv().is_none());
    scheduler.shutdown().unwrap();
}

#[test]
fn resident_cache_hits_invalidates_and_enforces_the_configured_budget() {
    let file = sine_wave_file(1, 48_000, 64);
    let mut scheduler_config = config();
    scheduler_config.resident_asset_threshold_bytes = 4_096;
    scheduler_config.resident_cache_budget_bytes = 4_096;
    let mut scheduler = PlaybackScheduler::new(scheduler_config).unwrap();
    let first_id = PlaybackId::new("cache-first").unwrap();
    scheduler
        .try_schedule(request(1, first_id.clone(), file.path()))
        .unwrap();
    assert!(matches!(
        wait_result(&mut scheduler),
        PlaybackPreparationResult::Prepared {
            preparation: PlaybackPreparation::Resident(_),
            ..
        }
    ));
    assert!(scheduler.complete(&first_id, CommandSequence::new(1).unwrap()));

    let hit_id = PlaybackId::new("cache-hit").unwrap();
    scheduler.try_schedule(request(2, hit_id.clone(), file.path())).unwrap();
    let _ = wait_result(&mut scheduler);
    assert!(scheduler.cache_observation().hits >= 1);
    assert!(scheduler.complete(&hit_id, CommandSequence::new(2).unwrap()));

    rewrite_sine_wave(file.path(), 1, 48_000, 96);
    let changed_id = PlaybackId::new("cache-changed").unwrap();
    scheduler.try_schedule(request(3, changed_id, file.path())).unwrap();
    let result = wait_result(&mut scheduler);
    match result {
        PlaybackPreparationResult::Prepared {
            preparation: PlaybackPreparation::Resident(asset),
            ..
        } => assert_eq!(asset.frames(), 96),
        _ => panic!("changed file should decode a new resident generation"),
    }
    assert!(scheduler.cache_observation().invalidations >= 1);
    scheduler.shutdown().unwrap();
}

#[test]
fn large_source_uses_bounded_stream_read_ahead_and_recovers_after_consumption() {
    let file = sine_wave_file(2, 44_100, 2_048);
    let mut scheduler_config = config();
    scheduler_config.resident_asset_threshold_bytes = 1;
    scheduler_config.stream_ring_frames = 64;
    let mut scheduler = PlaybackScheduler::new(scheduler_config).unwrap();
    let id = PlaybackId::new("stream").unwrap();
    scheduler.try_schedule(request(1, id.clone(), file.path())).unwrap();
    let result = wait_result(&mut scheduler);
    let mut reader = match result {
        PlaybackPreparationResult::Prepared {
            preparation: PlaybackPreparation::Stream { probe, reader },
            ..
        } => {
            assert_eq!(probe.sample_rate.get(), 44_100);
            reader
        }
        _ => panic!("threshold-forced source should stream"),
    };
    let mut samples = [0.0_f32; 64];
    let first = reader.read_interleaved(&mut samples);
    assert!(first > 0);
    let deadline = Instant::now() + Duration::from_secs(2);
    while reader.state().written_frames <= u64::try_from(first).unwrap() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(2));
    }
    assert!(reader.state().written_frames > u64::try_from(first).unwrap());
    reader.cancel();
    assert!(scheduler.stop(&id));
    scheduler.shutdown().unwrap();
}

#[test]
fn two_workers_sustain_forced_streaming_without_starvation() {
    const SOURCE_FRAMES: u32 = 16_384;
    const RING_FRAMES: usize = 1_024;
    const BLOCK_FRAMES: usize = 128;

    let file = sine_wave_file(2, 48_000, SOURCE_FRAMES);
    let mut scheduler_config = config();
    scheduler_config.resident_asset_threshold_bytes = 1;
    scheduler_config.stream_ring_frames = RING_FRAMES;
    assert_eq!(scheduler_config.worker_count, 2);

    let mut scheduler = PlaybackScheduler::new(scheduler_config).unwrap();
    let id = PlaybackId::new("sustained-stream").unwrap();
    scheduler.try_schedule(request(1, id.clone(), file.path())).unwrap();
    let result = wait_result(&mut scheduler);
    let mut reader = match result {
        PlaybackPreparationResult::Prepared {
            preparation: PlaybackPreparation::Stream { reader, .. },
            ..
        } => reader,
        _ => panic!("threshold-forced source should stream"),
    };

    let mut samples = [0.0_f32; BLOCK_FRAMES * 2];
    for block in 0..SOURCE_FRAMES as usize / BLOCK_FRAMES {
        let read = reader.read_interleaved(&mut samples);
        let state = reader.state();
        assert_eq!(read, BLOCK_FRAMES, "stream starved in block {block}: {state:?}");
        assert_eq!(state.read_frames, u64::try_from((block + 1) * BLOCK_FRAMES).unwrap());
        assert_eq!(state.starvation_count, 0, "stream starved in block {block}");
        assert!(
            samples.chunks_exact(2).all(|frame| frame[0] == frame[1]),
            "stereo fixture channels diverged in block {block}"
        );
        thread::sleep(Duration::from_millis(2));
    }

    let state = reader.state();
    assert_eq!(state.written_frames, u64::from(SOURCE_FRAMES));
    assert_eq!(state.read_frames, u64::from(SOURCE_FRAMES));
    assert_eq!(state.starvation_count, 0);
    reader.cancel();
    assert!(scheduler.stop(&id));
    scheduler.shutdown().unwrap();
}

#[test]
fn rapid_multiplex_like_play_stop_has_bounded_admission() {
    let file = sine_wave_file(1, 48_000, 512);
    let mut scheduler = PlaybackScheduler::new(config()).unwrap();
    let id = PlaybackId::new("rapid").unwrap();
    for sequence in 1..=100 {
        loop {
            match scheduler.try_schedule(request(sequence, id.clone(), file.path())) {
                Ok(()) => break,
                Err(error) if error.category == crate::AudioErrorCategory::QueueFull => {
                    let _ = scheduler.try_recv();
                    thread::yield_now();
                }
                Err(error) => panic!("unexpected scheduling error: {error}"),
            }
        }
        scheduler.stop(&id);
    }
    assert_eq!(scheduler.active_request_count(), 0);
    scheduler.shutdown().unwrap();
}

fn config() -> PlaybackSchedulerConfig {
    PlaybackSchedulerConfig {
        engine_sample_rate: SampleRate::default(),
        resident_asset_threshold_bytes: 1024 * 1024,
        resident_cache_budget_bytes: 2 * 1024 * 1024,
        stream_ring_frames: 256,
        worker_count: 2,
        job_capacity: 32,
        result_capacity: 32,
    }
}

fn request(sequence: u64, playback_id: PlaybackId, path: &std::path::Path) -> PlaybackSchedulerRequest {
    PlaybackSchedulerRequest {
        sequence: CommandSequence::new(sequence).unwrap(),
        request: PlayFileRequest {
            path: path.to_path_buf(),
            playback_id,
            gain: GainDb::UNITY,
        },
    }
}

fn wait_result(scheduler: &mut PlaybackScheduler) -> PlaybackPreparationResult {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(result) = scheduler.try_recv() {
            return result;
        }
        assert!(Instant::now() < deadline, "timed out waiting for playback preparation");
        thread::sleep(Duration::from_millis(2));
    }
}
