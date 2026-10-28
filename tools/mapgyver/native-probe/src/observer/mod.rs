//! Independent diagnostic consumer. Never participates in the render worker's frame cadence.

#[cfg(feature = "desktop")]
mod client;
mod frame_age;
pub mod protocol;
#[cfg(test)]
mod tests;

#[cfg(feature = "desktop")]
pub use client::ObserverProcess;

use std::time::{Duration, Instant};

use crate::{ProbeResult, d3d::Gpu, spout::Endpoint};
use crate::{
    child_resources::Recorder,
    wire::{read_message, respond},
};
use mapgyver_native_probe::{
    sample_window::Window,
    workload::{ConsumerMode, Scenario},
};
use protocol::{Checkpoint, CompositePhase, ObservationReady, ObserverReport, Request, Response};

pub fn run(adapter: u32, sender: &str, scenario: Scenario, consumer_mode: ConsumerMode) -> ProbeResult<()> {
    let gpu = Gpu::new(adapter)?;
    let mut receiver = Endpoint::receiver(&gpu, sender)?;
    let mut resources = Recorder::new(&gpu)?;
    resources.record(&gpu, None)?;
    let process_id = std::process::id();
    respond(&Response::Ready {
        process_id,
        adapter_luid: gpu.adapter.luid.clone(),
        consumer_mode,
    })?;
    let mut report = ObserverReport {
        process_id,
        adapter_luid: gpu.adapter.luid.clone(),
        consumer_mode,
        observation_ready: None,
        observation_ready_received_qpc: None,
        observation_finish_qpc: None,
        checkpoints: Vec::with_capacity(4),
        received_observations: 0,
        diagnostic_readbacks: 0,
        verified_generations: 0,
        frame_age: None,
        resources: None,
    };
    let (input_sender, input) = std::sync::mpsc::sync_channel(8);
    std::thread::Builder::new()
        .name("probe-observer-control".into())
        .spawn(move || {
            let mut stdin = std::io::stdin().lock();
            loop {
                let message = read_message::<Request>(&mut stdin).map_err(|error| error.to_string());
                let terminal = !matches!(&message, Ok(Some(_)));
                if input_sender.send(message).is_err() || terminal {
                    break;
                }
            }
        })?;
    let mut age = None;
    let mut window = None;
    loop {
        let request = match input.recv_timeout(Duration::from_millis(1)) {
            Ok(Ok(Some(request))) => Some(request),
            Ok(Ok(None)) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                return Err("observer control closed without a finish request".into());
            }
            Ok(Err(error)) => return Err(error.into()),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => None,
        };
        match request {
            Some(Request::BeginObservation { window: requested }) => {
                protocol::validate_observation_window(requested, consumer_mode)?;
                if window.is_some() || !report.checkpoints.is_empty() || !scenario.cards() {
                    return Err("invalid observation start sequence".into());
                }
                age = Some(frame_age::Collector::new(&gpu, requested)?);
                window = Some(requested);
            }
            Some(Request::Sample { phase }) => {
                if CompositePhase::ALL.get(report.checkpoints.len()) != Some(&phase) {
                    return Err("out-of-order or duplicate observer checkpoint".into());
                }
                if consumer_mode == ConsumerMode::Paused && report.observation_ready.is_some() {
                    return Err("paused observer cannot read after its warmup fence".into());
                }
                let result = sample(&gpu, &mut receiver, phase, &mut report, scenario, None, window)?;
                respond(&Response::Checkpoint { result: result.clone() })?;
                report.checkpoints.push(result);
                if report.checkpoints.len() == 4
                    && let Some(window) = window.filter(|window| window.warmup_seconds == 30)
                {
                    gpu.wait_submitted()?;
                    resources.tick_passive(&gpu)?;
                    let completed_qpc = crate::clock::now()?;
                    let (completion_markers, skipped_completion_markers) = resources.completion_counts();
                    let ready = ObservationReady {
                        window,
                        consumer_mode,
                        completed_qpc,
                        operations_at_fence: gpu.allocations.operations(),
                        completion_markers,
                        skipped_completion_markers,
                    };
                    ready.validate(window, consumer_mode, completed_qpc)?;
                    respond(&Response::ObservationReady { ready: ready.clone() })?;
                    report.observation_ready = Some(ready);
                }
            }
            Some(Request::VerifyGeneration { generation }) => {
                if consumer_mode == ConsumerMode::Paused
                    || report.checkpoints.len() != 4
                    || generation != report.verified_generations + 1
                    || generation > 100
                {
                    return Err("invalid recreation generation sequence".into());
                }
                let result = sample(
                    &gpu,
                    &mut receiver,
                    CompositePhase::Recovered,
                    &mut report,
                    scenario,
                    Some(generation),
                    window,
                )?;
                report.verified_generations = generation;
                respond(&Response::Generation { generation, result })?;
            }
            Some(Request::Finish) => {
                if report.checkpoints.len() != 4 {
                    return Err("observer stopped before all content checkpoints".into());
                }
                if consumer_mode == ConsumerMode::Paused && report.observation_ready.is_none() {
                    return Err("paused observer never completed its pre-sample warmup fence".into());
                }
                report.observation_finish_qpc = window.map(|_| crate::clock::now()).transpose()?;
                protocol::validate_observation_finish(window, consumer_mode, report.observation_finish_qpc)?;
                resources.record(&gpu, None)?;
                let before_drop = gpu.allocations.snapshot();
                if consumer_mode == ConsumerMode::Active
                    && let Some(age) = age.take()
                {
                    let mut result = age.finish(&gpu)?;
                    let samples = std::mem::take(&mut result.samples);
                    for (index, chunk) in samples
                        .chunks(mapgyver_native_probe::frame_age::CHUNK_SAMPLES)
                        .enumerate()
                    {
                        respond(&Response::FrameAgeChunk {
                            offset: index * mapgyver_native_probe::frame_age::CHUNK_SAMPLES,
                            samples: chunk.to_vec(),
                        })?;
                    }
                    report.frame_age = Some(result);
                }
                // Paused retains the identical log allocation through measurement but never
                // submits marker copies or creates a final readback pretending to be age data.
                drop(age);
                drop(receiver);
                let resources = resources.finish(&gpu, before_drop)?;
                if consumer_mode == ConsumerMode::Paused
                    && !report.observation_ready.as_ref().is_some_and(|ready| {
                        ready.resource_activity_unchanged(
                            &resources.operations,
                            resources.completion_markers,
                            resources.skipped_completion_markers,
                        )
                    })
                {
                    return Err("paused observer performed work after its warmup fence".into());
                }
                for message in mapgyver_native_probe::resource_log::messages(resources) {
                    respond(&Response::Resources { message })?;
                }
                respond(&Response::Finished {
                    report: Box::new(report),
                })?;
                return Ok(());
            }
            None => {}
        }
        if report.observation_ready.is_none() {
            protocol::checkpoint_before_sample(window, crate::clock::now()?)?;
        }
        if consumer_mode == ConsumerMode::Active
            && let Some(age) = &mut age
        {
            age.tick(&gpu, &mut receiver, scenario.extent())?;
        }
        if consumer_mode == ConsumerMode::Paused && report.observation_ready.is_some() {
            resources.tick_passive(&gpu)?;
        } else {
            resources.tick(&gpu)?;
        }
    }
}

fn sample(
    gpu: &Gpu,
    receiver: &mut Endpoint<'_>,
    phase: CompositePhase,
    report: &mut ObserverReport,
    scenario: Scenario,
    generation: Option<u32>,
    window: Option<Window>,
) -> ProbeResult<Checkpoint> {
    let start = Instant::now();
    let deadline = start + Duration::from_secs(2);
    let mut result = Checkpoint {
        phase,
        extent: scenario.extent(),
        sampled_bgra: Vec::with_capacity(4),
        sampled_alpha: Vec::with_capacity(4),
        sampled_generation_markers: Vec::with_capacity(4),
        elapsed_ms: 0.0,
    };
    while Instant::now() < deadline {
        protocol::checkpoint_before_sample(window, crate::clock::now()?)?;
        if let Some(frame) = receiver.receive()? {
            protocol::checkpoint_before_sample(window, crate::clock::now()?)?;
            report.received_observations += 1;
            if [frame.width, frame.height] != result.extent {
                return Err("composite output changed extent".into());
            }
            let [width, height] = result.extent;
            let mut points = vec![[width / 4, height / 2], [width * 3 / 4, height / 2]];
            if scenario.cards() {
                // Sample inside the 75% alpha stripe, away from bilinear filter boundaries
                // when the 720p source is scaled to a half-width 4K composite.
                points.extend([[width * 5 / 16, height * 7 / 8], [width * 13 / 16, height * 7 / 8]]);
            }
            if generation.is_some() {
                points.push([0, 0]);
            }
            let pixels = gpu.read_samples(&frame.texture, &points)?;
            protocol::checkpoint_before_sample(window, crate::clock::now()?)?;
            let pair = [pixels[0], pixels[1]];
            report.diagnostic_readbacks += 1;
            result.sampled_bgra.push(pair);
            let alpha_matches = if scenario.cards() {
                let alpha = [pixels[2][3], pixels[3][3]];
                result.sampled_alpha.push(alpha);
                alpha == protocol::expected_alpha(phase)
            } else {
                true
            };
            let generation_matches = if let Some(generation) = generation {
                let marker = *pixels.last().ok_or("missing generation marker")?;
                result.sampled_generation_markers.push(marker);
                marker == protocol::generation_marker(generation)
            } else {
                true
            };
            if pair == phase.expected_pixels() && alpha_matches && generation_matches {
                result.elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
                return Ok(result);
            }
            if result.sampled_bgra.len() == 4 {
                return Err(format!(
                    "composite phase {phase:?}: samples {:?}, expected {:?}, alpha {:?}, expected {:?}, generation markers {:?}",
                    result.sampled_bgra,
                    phase.expected_pixels(),
                    result.sampled_alpha,
                    protocol::expected_alpha(phase),
                    result.sampled_generation_markers
                )
                .into());
            }
        }
        // Four attempts must span multiple producer periods, rather than repeatedly reading
        // one in-flight handoff before the 60 Hz renderer has had another turn.
        std::thread::sleep(Duration::from_millis(20));
    }
    Err(format!("composite phase {phase:?} timed out; samples {:?}", result.sampled_bgra).into())
}
