//! Counts and wall-time distributions belong to the renderer, not the ingest device.

use mapgyver_native_probe::statistics::{Distribution, Series};
use serde::Serialize;
use std::time::Instant;

const STAGES: [&str; 10] = [
    "bind_device",
    "metadata_snapshot",
    "prune_imports",
    "cache_lookup",
    "open_resource",
    "cast_mutex",
    "cache_insert",
    "acquire_key",
    "release_reader",
    "clear_imports",
];

#[derive(Clone, Copy)]
pub(in crate::source_worker) enum Stage {
    BindDevice,
    Metadata,
    Prune,
    Lookup,
    OpenResource,
    CastMutex,
    Insert,
    AcquireKey,
    ReleaseReader,
    Clear,
}

#[derive(Default, Serialize)]
pub struct Counts {
    pub polls: u64,
    pub metadata_busy: u64,
    pub no_publication: u64,
    pub same_surface: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub opens: u64,
    pub evictions: u64,
    pub pruned: u64,
    pub cleared: u64,
    pub acquire_timeouts: u64,
    pub reader_changes: u64,
    pub reader_clears: u64,
}

#[derive(Serialize)]
pub struct StageReport {
    stage: &'static str,
    all: Option<Distribution>,
    sampled: Option<Distribution>,
    truncated_all: u32,
    truncated_sampled: u32,
}

#[derive(Serialize)]
pub struct ImportReport {
    scope: &'static str,
    pub capacity: usize,
    pub peak_cached: usize,
    pub cached_after_clear: usize,
    pub live_imports_after_clear: usize,
    pub peak_live_imports: usize,
    pub all: Counts,
    pub sampled: Counts,
    pub stages: Vec<StageReport>,
}

#[derive(Default)]
struct StageSeries {
    all: Series,
    sampled: Series,
}

#[derive(Default)]
pub(super) struct Metrics {
    pub all: Counts,
    sampled: Counts,
    stages: [StageSeries; STAGES.len()],
}

impl Metrics {
    pub(super) fn count(&mut self, collecting: bool, update: impl Fn(&mut Counts)) {
        update(&mut self.all);
        if collecting {
            update(&mut self.sampled);
        }
    }

    pub(super) fn record(&mut self, stage: Stage, start: Instant, collecting: bool) {
        let elapsed = start.elapsed().as_secs_f64() * 1000.0;
        let series = &mut self.stages[stage as usize];
        series.all.push(elapsed);
        if collecting {
            series.sampled.push(elapsed);
        }
    }

    pub(super) fn finish(
        self,
        capacity: usize,
        peak_cached: usize,
        cached_after_clear: usize,
        live_imports_after_clear: usize,
        peak_live_imports: usize,
    ) -> ImportReport {
        ImportReport {
            scope: "renderer-device COM aliases only; cache capacity excludes the active reader; miss-before-eviction can transiently retain capacity+1 aliases; all includes startup/warmup/cleanup; sampled follows renderer attempts; wall time includes preemption; counts do not prove physical residency",
            capacity,
            peak_cached,
            cached_after_clear,
            live_imports_after_clear,
            peak_live_imports,
            all: self.all,
            sampled: self.sampled,
            stages: self
                .stages
                .into_iter()
                .enumerate()
                .map(|(index, mut s)| StageReport {
                    stage: STAGES[index],
                    all: s.all.distribution(),
                    sampled: s.sampled.distribution(),
                    truncated_all: s.all.truncated,
                    truncated_sampled: s.sampled.truncated,
                })
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests;
