use super::{MAX_SAMPLES, Series};

#[test]
fn empty_and_capped_measurements_do_not_invent_samples() {
    let mut series = Series::default();
    assert!(series.distribution().is_none());
    for _ in 0..MAX_SAMPLES + 3 {
        series.push(1.0);
    }
    assert_eq!(series.truncated, 3);
    assert_eq!(series.distribution().unwrap().samples, MAX_SAMPLES);
}

#[test]
fn nearest_rank_tail_keeps_an_outlier_instead_of_averaging_it_away() {
    let mut series = Series::default();
    series.push(300.0);
    for _ in 0..99 {
        series.push(1.0);
    }
    let distribution = series.distribution().unwrap();
    assert_eq!(distribution.p95_ms, 1.0);
    assert_eq!(distribution.p99_ms, 1.0);
    assert_eq!(distribution.maximum_ms, 300.0);
    assert_eq!(distribution.mean_ms, 3.99);
}
