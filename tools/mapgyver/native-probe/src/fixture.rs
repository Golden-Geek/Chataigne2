use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

use crate::ProbeResult;
use crate::child_resources::Recorder;
use crate::d3d::Gpu;
use crate::spout::Endpoint;
use crate::wire;
use mapgyver_native_probe::workload::Scenario;
mod client;
pub mod protocol;
#[cfg(feature = "desktop")]
pub use client::FixtureProcess;
pub use client::spawn_ready;
use protocol::{Request, Response};

pub const FRAME_COUNT: u32 = 360;

/// A changes size, disappears, and restarts under the same name. B remains live throughout.
pub fn run_for(adapter: u32, name: &str, channel: &str, frame_count: u32, scenario: Scenario) -> ProbeResult<()> {
    if !(1..=216_120).contains(&frame_count) {
        return Err("invalid fixture frame budget".into());
    }
    if !matches!(channel, "a" | "b") {
        return Err("fixture channel must be a or b".into());
    }
    let gpu = Gpu::new(adapter)?;
    let mut resources = Recorder::new(&gpu)?;
    let card = if scenario.cards() {
        Some(crate::card::Card::new(&gpu)?)
    } else {
        None
    };
    let [source_width, source_height] = scenario.extent();
    let mut color = [0.0; 4];
    let stop = Arc::new(AtomicBool::new(false));
    let control_stop = stop.clone();
    std::thread::Builder::new()
        .name("probe-fixture-control".into())
        .spawn(move || {
            // The only admitted command is Stop. EOF or invalid control also cancels.
            let _ = wire::read_message::<Request>(&mut std::io::stdin().lock());
            // Parent exit/pipe loss must cancel this owned fixture, including abnormal exit.
            control_stop.store(true, Ordering::Release);
        })?;
    let mut sender = None;
    let mut texture = None;
    let mut last_phase = u32::MAX;
    let start = Instant::now();
    let mut frames_sent = 0;
    for frame in 0..frame_count {
        if stop.load(Ordering::Acquire) {
            break;
        }
        let phase = if channel == "b" || frame < 90 {
            0
        } else if frame < 180 {
            1
        } else if frame < 210 {
            2
        } else {
            3
        };
        let phase_changed = phase != last_phase;
        if phase_changed {
            // Resize updates the live sender. Only the absence/restart phases replace it.
            if phase != 1 {
                sender.take();
            }
            texture.take();
            if phase != 2 {
                let (width, height, next_color) = match (channel, phase) {
                    ("b", _) => (1280, 720, [1.0, 1.0, 0.0, 1.0]),
                    (_, 0) => (source_width, source_height, [1.0, 0.0, 0.0, 1.0]),
                    (_, 1) => (source_width / 2, source_height / 2, [0.0, 1.0, 0.0, 1.0]),
                    _ => (source_width, source_height, [0.0, 0.0, 1.0, 1.0]),
                };
                color = next_color;
                let image = gpu.texture(width, height)?;
                gpu.clear(&image, color)?;
                texture = Some(image);
                if sender.is_none() {
                    let endpoint = Endpoint::sender(&gpu, name)?;
                    if endpoint.name() != name {
                        return Err("fixture sender name collision".into());
                    }
                    sender = Some(endpoint);
                }
            }
            last_phase = phase;
        }
        if let (Some(sender), Some(texture)) = (&mut sender, &texture) {
            if let Some(card) = &card {
                card.draw(&gpu, texture, color, frame, u32::from(channel == "b"))?;
            }
            sender.send(texture)?;
            frames_sent += 1;
            if frame == 0 {
                gpu.wait_submitted()?;
                wire::respond(&Response::Ready {
                    process_id: std::process::id(),
                    adapter_luid: gpu.adapter.luid.clone(),
                    channel: channel.into(),
                })?;
            }
        }
        if phase_changed {
            resources.record(&gpu, Some(phase))?;
        }
        resources.tick(&gpu)?;
        let next = start + Duration::from_secs_f64(f64::from(frame + 1) / 60.0);
        if let Some(delay) = next.checked_duration_since(Instant::now()) {
            std::thread::sleep(delay);
        }
    }
    resources.record(&gpu, None)?;
    let before_drop = gpu.allocations.snapshot();
    // Drop SDK instances before device, and unregister fixture names before process exit.
    drop(sender);
    drop(texture);
    drop(card);
    for message in mapgyver_native_probe::resource_log::messages(resources.finish(&gpu, before_drop)?) {
        wire::respond(&Response::Resources { message })?;
    }
    wire::respond(&Response::Finished {
        report: Box::new(protocol::Report {
            process_id: std::process::id(),
            adapter_luid: gpu.adapter.luid.clone(),
            channel: channel.into(),
            frames_sent,
            resources: None,
        }),
    })?;
    Ok(())
}
