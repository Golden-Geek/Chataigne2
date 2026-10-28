use std::time::{Duration, Instant};

use serde::Serialize;

use crate::ProbeResult;
use crate::d3d::{AdapterInfo, Gpu};
use crate::fixture::{self, FRAME_COUNT};
use crate::spout::Endpoint;
use mapgyver_native_probe::workload::Scenario;

#[derive(Serialize)]
pub struct SmokeReport {
    evidence_kind: &'static str,
    sdk_version: String,
    feature_level: &'static str,
    adapter: AdapterInfo,
    channel_observations: [u64; 2],
    phase_markers_bgra: [[u8; 4]; 3],
    unaffected_channel_marker_bgra: [u8; 4],
    b_observations_during_a_recovery: u64,
    diagnostic_readbacks: u32,
    elapsed_ms: f64,
    sources: [fixture::protocol::Report; 2],
    unqualified: [&'static str; 4],
}

pub fn run(adapter: u32) -> ProbeResult<SmokeReport> {
    let sdk_version = crate::spout::sdk_version()?;
    if sdk_version != spout2_sys::SPOUT_SDK_VERSION {
        return Err("linked SDK version differs from binding declaration".into());
    }
    let gpu = Gpu::new(adapter)?;
    let suffix = format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    );
    let name_a = format!("MapGyver-probe-a-{suffix}");
    let name_b = format!("MapGyver-probe-b-{suffix}");
    let mut receiver_a = Endpoint::receiver(&gpu, &name_a)?;
    let mut receiver_b = Endpoint::receiver(&gpu, &name_b)?;
    let mut child_a = fixture::spawn_ready(
        adapter,
        &name_a,
        "a",
        FRAME_COUNT,
        Scenario::Diagnostic,
        &gpu.adapter.luid,
    )?;
    let mut child_b = fixture::spawn_ready(
        adapter,
        &name_b,
        "b",
        FRAME_COUNT,
        Scenario::Diagnostic,
        &gpu.adapter.luid,
    )?;
    let start = Instant::now();
    let deadline = start + Duration::from_secs(15);
    let mut counts = [0_u64; 2];
    let mut phase_markers = [None; 3];
    let mut b_marker = None;
    let mut b_during_absence = 0;
    let mut saw_a = false;
    let mut saw_absence = false;
    while Instant::now() < deadline {
        let a = receiver_a.receive()?;
        let b = receiver_b.receive()?;
        if let Some(frame) = a {
            counts[0] += 1;
            saw_a = true;
            let phase = if frame.width == 960 && frame.height == 540 {
                1
            } else if frame.width == 1920 && frame.height == 1080 {
                if saw_absence { 2 } else { 0 }
            } else {
                return Err("unexpected channel A extent".into());
            };
            if phase_markers[phase].is_none() {
                // A connected texture may precede its first copied frame during initialization.
                let marker = gpu.read_marker(&frame.texture)?;
                let expected = [[0, 0, 255, 255], [0, 255, 0, 255], [255, 0, 0, 255]][phase];
                if marker != expected {
                    return Err(format!("channel A phase {phase}: marker {marker:?} != {expected:?}").into());
                }
                phase_markers[phase] = Some(marker);
            }
        } else if saw_a && phase_markers[1].is_some() {
            // New-frame absence alone is not disconnection. Confirm registry/connection below.
            if !receiver_a.connected() {
                saw_absence = true;
            }
        }
        if let Some(frame) = b {
            counts[1] += 1;
            if frame.width != 1280 || frame.height != 720 {
                return Err("channel B changed extent".into());
            }
            if saw_absence && phase_markers[2].is_none() {
                b_during_absence += 1;
            }
            if b_marker.is_none() {
                let marker = gpu.read_marker(&frame.texture)?;
                if marker != [0, 255, 255, 255] {
                    return Err(format!("channel B marker invalid: {marker:?}").into());
                }
                b_marker = Some(marker);
            }
        }
        let a_done = child_a.child_mut().try_wait()?;
        let b_done = child_b.child_mut().try_wait()?;
        for status in [a_done, b_done].into_iter().flatten() {
            if !status.success() {
                return Err(format!("fixture exited with {status}").into());
            }
        }
        if a_done.is_some() && b_done.is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    if child_a.child_mut().try_wait()?.is_none() || child_b.child_mut().try_wait()?.is_none() {
        return Err("fixtures exceeded deadline".into());
    }
    if receiver_a.sender_exists(&name_a)? || receiver_b.sender_exists(&name_b)? {
        return Err("fixture sender registration survived process exit".into());
    }
    if counts.iter().any(|count| *count < 100) || b_during_absence < 5 {
        return Err("insufficient independent channel/recovery observations".into());
    }
    let markers = [
        phase_markers[0].ok_or("no initial frame")?,
        phase_markers[1].ok_or("no resized frame")?,
        phase_markers[2].ok_or("no restarted frame")?,
    ];
    drop(receiver_a);
    drop(receiver_b);
    let sources = [child_a.finish()?, child_b.finish()?];
    // SAFETY: a live device can report removal without changing the desktop driver.
    unsafe { gpu.device.GetDeviceRemovedReason()? };
    Ok(SmokeReport {
        evidence_kind: "native_cross_process_gpu_lifecycle_smoke_not_q01_qualification",
        sdk_version,
        feature_level: "11_0",
        adapter: gpu.adapter,
        // SDK frame counting can be disabled in local Spout settings. Poll observations are
        // deliberately not reported as distinct source frames or a cadence measurement.
        channel_observations: counts,
        phase_markers_bgra: markers,
        unaffected_channel_marker_bgra: b_marker.ok_or("no channel B marker")?,
        b_observations_during_a_recovery: b_during_absence,
        diagnostic_readbacks: 4,
        elapsed_ms: start.elapsed().as_secs_f64() * 1000.0,
        sources,
        unqualified: [
            "WebView underlay/DOM/input/DPI",
            "native and composited Spout output",
            "Q01/Q02 cadence/memory budgets",
            "device reset and sustained retirement",
        ],
    })
}
