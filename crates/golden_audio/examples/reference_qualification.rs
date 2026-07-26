use std::time::{Duration, Instant};

use golden_audio::qualification::{ReferenceWorkload, ReferenceWorkloadHarness, ReferenceWorkloadSpec};
use serde::Serialize;

const WARMUP_BLOCKS: usize = 128;
const MEASURED_BLOCKS: usize = 10_000;
const BLOCKS_PER_FIXTURE: usize = 1_024;
const SAMPLE_RATE: u32 = 48_000;
const BLOCK_FRAMES: u32 = 128;

#[derive(Debug, Serialize)]
struct QualificationEnvironment {
    os: &'static str,
    architecture: &'static str,
    processor: String,
    logical_processors: Option<String>,
    backend: &'static str,
    sample_rate: u32,
    buffer_frames: u32,
    compiler_profile: &'static str,
    commit: String,
}

#[derive(Debug, Serialize)]
struct TimingDistribution {
    samples: usize,
    p50_micros: f64,
    p99_micros: f64,
    p99_99_micros: f64,
    maximum_micros: f64,
    block_deadline_micros: f64,
    p99_deadline_ratio: f64,
    p99_99_deadline_ratio: f64,
}

#[derive(Debug, Serialize)]
struct WorkloadQualification {
    workload: ReferenceWorkload,
    specification: ReferenceWorkloadSpec,
    timing: TimingDistribution,
    finite_output: bool,
    active_voices: u16,
    analysis_ready: bool,
    dropped_analysis_frames: u64,
    stale_analysis_frames: u64,
    estimated_resident_bytes: u64,
    realtime_target_applicable: bool,
    p99_target_passed: bool,
    p99_99_target_passed: bool,
}

#[derive(Debug, Serialize)]
struct QualificationReport {
    schema_version: u32,
    environment: QualificationEnvironment,
    workloads: Vec<WorkloadQualification>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut workloads = Vec::with_capacity(ReferenceWorkload::ALL.len());
    for workload in ReferenceWorkload::ALL {
        workloads.push(qualify(workload)?);
    }
    let report = QualificationReport {
        schema_version: 1,
        environment: QualificationEnvironment {
            os: std::env::consts::OS,
            architecture: std::env::consts::ARCH,
            processor: std::env::var("PROCESSOR_IDENTIFIER").unwrap_or_else(|_| "unreported".to_string()),
            logical_processors: std::env::var("NUMBER_OF_PROCESSORS").ok(),
            backend: "deterministic_offline",
            sample_rate: SAMPLE_RATE,
            buffer_frames: BLOCK_FRAMES,
            compiler_profile: if cfg!(debug_assertions) { "debug" } else { "release" },
            commit: std::env::var("GOLDEN_AUDIO_QUALIFICATION_COMMIT").unwrap_or_else(|_| "working-tree".to_string()),
        },
        workloads,
    };
    println!("{}", serde_json::to_string(&report)?);
    Ok(())
}

fn qualify(workload: ReferenceWorkload) -> Result<WorkloadQualification, Box<dyn std::error::Error>> {
    let specification = workload.specification();
    let mut timings = Vec::with_capacity(MEASURED_BLOCKS);
    let mut remaining = MEASURED_BLOCKS;
    let mut finite_output = true;
    let mut active_voices = 0;
    let mut analysis_ready = specification.pitch_taps + specification.spectrum_taps == 0;
    let mut dropped_analysis_frames = 0_u64;
    let mut stale_analysis_frames = 0_u64;
    let mut estimated_resident_bytes = 0_u64;
    while remaining > 0 {
        let blocks = remaining.min(BLOCKS_PER_FIXTURE);
        let mut harness = ReferenceWorkloadHarness::new(workload)?;
        harness.render_blocks(WARMUP_BLOCKS)?;
        for _ in 0..blocks {
            let started = Instant::now();
            harness.render_block()?;
            timings.push(u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX));
        }
        analysis_ready |= harness.wait_for_analysis(Duration::from_secs(2));
        let observation = harness.observation();
        finite_output &= observation.finite_output;
        active_voices = observation.active_voices;
        dropped_analysis_frames = dropped_analysis_frames.saturating_add(observation.dropped_analysis_frames);
        stale_analysis_frames = stale_analysis_frames.saturating_add(observation.stale_analysis_frames);
        estimated_resident_bytes = estimated_resident_bytes.max(observation.estimated_resident_bytes);
        remaining -= blocks;
    }
    timings.sort_unstable();
    let deadline_micros = f64::from(BLOCK_FRAMES) * 1_000_000.0 / f64::from(SAMPLE_RATE);
    let p50_micros = percentile_micros(&timings, 0.50);
    let p99_micros = percentile_micros(&timings, 0.99);
    let p99_99_micros = percentile_micros(&timings, 0.9999);
    let realtime_target_applicable = workload != ReferenceWorkload::ExtremeOffline;
    Ok(WorkloadQualification {
        workload,
        specification,
        timing: TimingDistribution {
            samples: timings.len(),
            p50_micros,
            p99_micros,
            p99_99_micros,
            maximum_micros: timings.last().copied().map_or(0.0, nanos_to_micros),
            block_deadline_micros: deadline_micros,
            p99_deadline_ratio: p99_micros / deadline_micros,
            p99_99_deadline_ratio: p99_99_micros / deadline_micros,
        },
        finite_output,
        active_voices,
        analysis_ready,
        dropped_analysis_frames,
        stale_analysis_frames,
        estimated_resident_bytes,
        realtime_target_applicable,
        p99_target_passed: !realtime_target_applicable || p99_micros < deadline_micros * 0.50,
        p99_99_target_passed: !realtime_target_applicable || p99_99_micros < deadline_micros * 0.80,
    })
}

fn percentile_micros(sorted_nanos: &[u64], percentile: f64) -> f64 {
    if sorted_nanos.is_empty() {
        return 0.0;
    }
    let rank = (percentile * sorted_nanos.len() as f64).ceil() as usize;
    nanos_to_micros(sorted_nanos[rank.saturating_sub(1).min(sorted_nanos.len() - 1)])
}

fn nanos_to_micros(nanos: u64) -> f64 {
    nanos as f64 / 1_000.0
}
