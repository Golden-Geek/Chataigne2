use super::*;

#[test]
fn warmup_counts_and_timings_are_excluded_from_the_sample_window() {
    let mut metrics = Metrics::default();
    metrics.count(false, |counts| {
        counts.polls += 1;
        counts.cache_misses += 1;
        counts.opens += 1;
    });
    metrics.record(Stage::OpenResource, Instant::now(), false);
    metrics.count(true, |counts| {
        counts.polls += 2;
        counts.cache_hits += 1;
    });
    metrics.record(Stage::Lookup, Instant::now(), true);
    let report = metrics.finish(3, 1, 0, 0, 1);

    assert_eq!(report.all.polls, 3);
    assert_eq!(report.sampled.polls, 2);
    assert_eq!(report.all.opens, 1);
    assert_eq!(report.sampled.opens, 0);
    assert_eq!(report.all.cache_misses, 1);
    assert_eq!(report.sampled.cache_misses, 0);
    assert_eq!(report.all.cache_hits, 1);
    assert_eq!(report.sampled.cache_hits, 1);
    assert_eq!(stage(&report, "open_resource").all.as_ref().unwrap().samples, 1);
    assert!(stage(&report, "open_resource").sampled.is_none());
    let lookup = stage(&report, "cache_lookup");
    assert_eq!(lookup.all.as_ref().unwrap().samples, 1);
    assert_eq!(lookup.sampled.as_ref().unwrap().samples, 1);
}

#[test]
fn cleanup_counters_and_timings_stay_in_all_only() {
    let mut metrics = Metrics::default();
    metrics.count(true, |counts| counts.reader_changes += 1);
    metrics.count(false, |counts| {
        counts.cleared += 3;
        counts.reader_clears += 1;
    });
    metrics.record(Stage::ReleaseReader, Instant::now(), false);
    metrics.record(Stage::Clear, Instant::now(), false);
    let report = metrics.finish(3, 3, 0, 0, 4);

    assert_eq!(report.all.reader_changes, 1);
    assert_eq!(report.sampled.reader_changes, 1);
    assert_eq!(report.all.cleared, 3);
    assert_eq!(report.sampled.cleared, 0);
    assert_eq!(report.all.reader_clears, 1);
    assert_eq!(report.sampled.reader_clears, 0);
    assert_eq!(report.capacity, 3);
    assert_eq!(report.peak_cached, 3);
    assert_eq!(report.cached_after_clear, 0);
    assert_eq!(report.live_imports_after_clear, 0);
    assert_eq!(report.peak_live_imports, 4);
    for name in ["release_reader", "clear_imports"] {
        let cleanup = stage(&report, name);
        assert_eq!(cleanup.all.as_ref().unwrap().samples, 1);
        assert!(cleanup.sampled.is_none());
    }
}

#[test]
fn unobserved_stages_have_no_distribution() {
    let report = Metrics::default().finish(3, 0, 0, 0, 0);
    assert_eq!(report.stages.len(), STAGES.len());
    for (entry, name) in report.stages.iter().zip(STAGES) {
        assert_eq!(entry.stage, name);
        assert!(entry.all.is_none());
        assert!(entry.sampled.is_none());
        assert_eq!(entry.truncated_all, 0);
        assert_eq!(entry.truncated_sampled, 0);
    }
}

fn stage<'a>(report: &'a ImportReport, name: &str) -> &'a StageReport {
    report.stages.iter().find(|entry| entry.stage == name).unwrap()
}
