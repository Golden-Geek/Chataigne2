use super::*;

fn config() -> Config {
    Config {
        start_qpc: 1000,
        frequency: 1000,
        warmup_seconds: 1,
        maximum_seconds: 10,
    }
}

fn sample(frame: u32, qpc: u64, completed: u64) -> Sample {
    Sample {
        submitted_qpc: completed - 1,
        completed_qpc: completed,
        words: [marker_words(frame, 0, qpc), marker_words(frame, 1, qpc)],
    }
}

#[test]
fn completion_time_can_follow_a_source_stamp_newer_than_cpu_enqueue() {
    let mut value = sample(3, 2005, 2010);
    value.submitted_qpc = 2001;
    let result = analyze(config(), &[value]).unwrap();
    assert_eq!(result[0].frame_age.as_ref().unwrap().p95_ms, 5.0);
    assert_eq!(result[0].invalid, 0);
}

#[test]
fn repetitions_absence_corruption_and_future_or_regressing_frames_remain_visible() {
    let mut absent = sample(2, 2010, 2020);
    absent.words[0] = [BACKGROUND; WORDS_PER_CHANNEL];
    let mut corrupt = sample(3, 2020, 2030);
    corrupt.words[0][4] ^= 1;
    let values = [
        sample(1, 2000, 2010),
        sample(1, 2000, 2011),
        absent,
        corrupt,
        sample(4, 2100, 2040),
        sample(0, 1990, 2050),
    ];
    let result = analyze(config(), &values).unwrap();
    assert_eq!(
        (result[0].valid, result[0].repeated, result[0].absent, result[0].invalid),
        (2, 1, 1, 3)
    );
    assert_eq!((result[1].valid, result[1].unique_frames, result[1].invalid), (4, 3, 2));
    assert!(analyze(config(), &[sample(1, 990, 999)]).is_err());
    assert!(analyze(config(), &[sample(1, 2000, 2010), sample(2, 2001, 2009)]).is_err());
}

#[test]
fn clock_admission_bounds_the_gpu_log_before_allocating_it() {
    let mut c = config();
    c.maximum_seconds = 3600;
    c.warmup_seconds = 0;
    assert_eq!(c.capacity(), MAX_SAMPLES);
    c.frequency = u64::MAX;
    assert!(c.validate().is_err());
    c = config();
    c.maximum_seconds = c.warmup_seconds;
    assert!(c.validate().is_err());
    let high_stamp = 0xabcd_1234_5678_9012;
    let words = marker_words(216_000, 1, high_stamp);
    assert_eq!(u64::from(words[2]) | (u64::from(words[3]) << 32), high_stamp);
}

#[test]
fn derived_summary_survives_the_observer_json_boundary_exactly() {
    let mut config = config();
    config.frequency = 10_000_000;
    config.start_qpc = 1;
    config.warmup_seconds = 0;
    for count in 1..100 {
        let values: Vec<_> = (0..count)
            .map(|index| {
                let start = 20_000_000 + index * 100_000;
                sample(index as u32, start, start + 123_457 + index * 31)
            })
            .collect();
        let channels = analyze(config, &values).unwrap();
        let bytes = serde_json::to_vec(&channels).unwrap();
        let parsed: [Channel; 2] = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(channels, parsed, "{count} samples");
    }
}
