//! Eight-second, headless query-progress controls; never a Q01/Q02 performance pass.

use crate::{
    ProbeResult,
    d3d::Gpu,
    frame_admission::{FinalReport, FrameAdmission},
    spout::Endpoint,
    timing::{Timing, TimingReport},
};
use mapgyver_native_probe::{
    allocation::{Operations, Snapshot},
    operation_timing::Stamp,
    statistics::{Distribution, Series},
};
use serde::Serialize;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Event,
    Warmup,
    Timestamp,
    ReceiverWarmup,
    ReceiverTimestamp,
    ClearWarmup,
    Fence,
    FenceWarmup,
    FenceNoDrain,
}

impl std::str::FromStr for Mode {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "event" => Ok(Self::Event),
            "warmup" => Ok(Self::Warmup),
            "timestamp" => Ok(Self::Timestamp),
            "receiver_warmup" => Ok(Self::ReceiverWarmup),
            "receiver_timestamp" => Ok(Self::ReceiverTimestamp),
            "clear_warmup" => Ok(Self::ClearWarmup),
            "fence" => Ok(Self::Fence),
            "fence_warmup" => Ok(Self::FenceWarmup),
            "fence_no_drain" => Ok(Self::FenceNoDrain),
            _ => Err("unknown query-progress mode".into()),
        }
    }
}

#[derive(Serialize)]
pub struct Report {
    evidence_kind: &'static str,
    mode: Mode,
    adapter_luid: String,
    seconds: u32,
    target_ticks: u32,
    elapsed_seconds: f64,
    qpc_frequency: u64,
    start_qpc: u64,
    busy_ticks: Vec<Stamp>,
    tick_work: Option<Distribution>,
    timing: Option<TimingReport>,
    before_final_fence: mapgyver_native_probe::gpu_admission::Report,
    drain_clear_after_observation: bool,
    after_final_fence: FinalReport,
    allocations_after_fence: Snapshot,
    operations_including_seed: Operations,
}

pub fn run(adapter: u32, mode: Mode) -> ProbeResult<Report> {
    let gpu = Gpu::new(adapter)?;
    let image = gpu.texture(16, 16)?;
    // Every case starts after one real, completed GPU operation on the same context.
    gpu.clear(&image, [0.0, 0.0, 0.0, 1.0])?;
    gpu.wait_submitted()?;
    let missing_name = format!("MapGyver-query-missing-{}", std::process::id());
    let mut receiver = if matches!(mode, Mode::ReceiverWarmup | Mode::ReceiverTimestamp) {
        let receiver = Endpoint::receiver(&gpu, &missing_name)?;
        if receiver.sender_exists(&missing_name)? {
            return Err("missing-source name collision".into());
        }
        Some(receiver)
    } else {
        None
    };
    let mut timing = if matches!(mode, Mode::Event | Mode::Fence | Mode::FenceNoDrain) {
        None
    } else {
        Some(Timing::new(&gpu)?)
    };
    let timestamps = matches!(mode, Mode::Timestamp | Mode::ReceiverTimestamp);
    let mut admission = if matches!(mode, Mode::Fence | Mode::FenceWarmup | Mode::FenceNoDrain) {
        FrameAdmission::new(&gpu)?
    } else {
        FrameAdmission::event_queries(&gpu)?
    };
    let mut tick_work = Series::default();
    let mut busy_ticks = Vec::with_capacity(480);
    let start_qpc = crate::clock::now()?;
    let start = Instant::now();
    let mut next = start;
    for frame in 0..480 {
        if start.elapsed() >= Duration::from_secs(8) {
            break;
        }
        let stamp = Stamp {
            frame,
            qpc: crate::clock::now()?,
        };
        let tick_start = Instant::now();
        if let Some(reservation) = admission.reserve(true)? {
            if let Some(timing) = &mut timing {
                timing.begin(&gpu, tick_start, timestamps, stamp)?;
            }
            if let Some(receiver) = &mut receiver
                && (receiver.receive()?.is_some() || receiver.connected())
            {
                return Err("missing-source control unexpectedly connected".into());
            }
            if matches!(mode, Mode::ClearWarmup) {
                gpu.clear(&image, [0.0, 0.0, 0.0, 1.0])?;
            }
            if let Some(timing) = &mut timing {
                timing.end_gpu(&gpu);
            }
            admission.submit(reservation)?;
            if let Some(timing) = &mut timing {
                timing.end_cpu(tick_start);
            }
        } else {
            busy_ticks.push(stamp);
        }
        tick_work.push(tick_start.elapsed().as_secs_f64() * 1000.0);
        next += Duration::from_nanos(16_666_667);
        if let Some(delay) = next.checked_duration_since(Instant::now()) {
            std::thread::sleep(delay);
        } else {
            next = Instant::now();
        }
    }
    let elapsed_seconds = start.elapsed().as_secs_f64();
    let before_final_fence = admission.snapshot();
    // Keep the negative observation if empty/event-only work does not progress. This real
    // GPU operation is outside the measured window and is never an admission workaround.
    let drain_clear_after_observation = !matches!(mode, Mode::FenceNoDrain);
    if drain_clear_after_observation {
        gpu.clear(&image, [0.0, 0.0, 0.0, 1.0])?;
    }
    let timing = timing.map(|timing| timing.finish(&gpu)).transpose()?;
    drop(receiver);
    drop(image);
    gpu.retire()?;
    let after_final_fence = admission.finish_after_fence()?;
    Ok(Report {
        evidence_kind: "headless_query_progress_isolation_not_fixed_workload_qualification",
        mode,
        adapter_luid: gpu.adapter.luid.clone(),
        seconds: 8,
        target_ticks: 480,
        elapsed_seconds,
        qpc_frequency: crate::clock::frequency()?,
        start_qpc,
        busy_ticks,
        tick_work: tick_work.distribution(),
        timing,
        before_final_fence,
        drain_clear_after_observation,
        after_final_fence,
        allocations_after_fence: gpu.allocations.snapshot(),
        operations_including_seed: gpu.allocations.operations(),
    })
}
