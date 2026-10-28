use super::{RunState, Session};
use crate::cadence::{Cadence, Submission};
use crate::{
    ProbeResult,
    d3d::Gpu,
    frame_admission::{FinalReport, FrameAdmission},
    observer::protocol::{Checkpoint, CompositePhase, generation_marker},
    presentation::PhysicalRect,
    presenter::{AdmissionReport, Presenter, Target},
    quad::Quad,
    resources::{ProcessSnapshot, ResourceSample, SceneCheckpoint, Stage},
    source_worker::{self, Source},
    spout::Endpoint,
    timing::{Timing, TimingReport},
};
use mapgyver_native_probe::operation_timing::{self, FrameTimer, GpuWork, Operation, Stamp};
use mapgyver_native_probe::pipeline_timing::{Dependencies, DependencyTrace};
use mapgyver_native_probe::workload::GenerationEnd;
use serde::Serialize;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

#[derive(Serialize)]
pub struct GenerationReport {
    pub generation: u32,
    pub end: GenerationEnd,
    pub adapter_luid: String,
    pub elapsed_seconds: f64,
    pub setup_ms: f64,
    pub consumer_ready_ms: Option<f64>,
    pub native_output_extent: [u32; 2],
    pub fixed_geometry_observed: bool,
    pub sampled_geometry_mismatches: u32,
    pub output_presents: u32,
    pub preview_presents: u32,
    pub spout_sends: u32,
    pub timing: TimingReport,
    pub dependency_trace: DependencyTrace,
    pub operation_timing: operation_timing::Report,
    pub gpu_admission: FinalReport,
    pub source_workers: Vec<source_worker::Report>,
    pub consumer_checkpoint: Option<Checkpoint>,
    pub retirement_fence_ms: f64,
    pub after_release: Option<ProcessSnapshot>,
    pub allocations_after_resource_drop: mapgyver_native_probe::allocation::Snapshot,
    pub allocations_after_fence: mapgyver_native_probe::allocation::Snapshot,
    pub operations: mapgyver_native_probe::allocation::Operations,
    pub submission_editor_output_spout: [crate::cadence::Report; 3],
    pub admission_editor_output: [AdmissionReport; 2],
    pub steady_scene: Option<SceneCheckpoint>,
    pub settling: Option<SettlingReport>,
    #[serde(skip)]
    pub retirement_started: Instant,
}

#[derive(Serialize)]
pub struct SettlingReport {
    pub elapsed_seconds: f64,
    pub scene: SceneCheckpoint,
}

pub fn run(session: &Session, state: &mut RunState, generation: u32) -> ProbeResult<GenerationReport> {
    let spec = session.spec;
    let shared = &state.shared;
    let generation_start = Instant::now();
    let gpu = Gpu::new(session.adapter)?;
    if gpu.adapter.luid != session.adapter_luid {
        return Err("selected adapter changed during recreation".into());
    }
    let quad = Quad::new(&gpu)?;
    let mut editor = Presenter::new(&gpu, session.editor)?;
    let mut output = Presenter::new(&gpu, session.output)?;
    let sample = (state.start + Duration::from_secs(u64::from(spec.warmup_seconds())))
        ..(state.start + Duration::from_secs(u64::from(spec.total_seconds())));
    let mut inputs = [
        Source::spawn(
            session.adapter,
            &session.adapter_luid,
            &session.source_names[0],
            sample.clone(),
        )?,
        Source::spawn(session.adapter, &session.adapter_luid, &session.source_names[1], sample)?,
    ];
    let mut sender = Endpoint::sender(&gpu, &session.output_name)?;
    if sender.name() != session.output_name {
        return Err("composite sender name collision".into());
    }
    let [width, height] = spec.scenario.extent();
    let composed = gpu.texture(width, height)?;
    let marker = gpu.texture(1, 1)?;
    let [b, g, r, a] = generation_marker(generation);
    gpu.clear(
        &marker,
        [
            f32::from(r) / 255.0,
            f32::from(g) / 255.0,
            f32::from(b) / 255.0,
            f32::from(a) / 255.0,
        ],
    )?;
    let mut timing = Timing::traced(&gpu)?;
    let mut dependency_trace = DependencyTrace::new(crate::clock::frequency()?);
    let mut admission = FrameAdmission::new(&gpu)?;
    let mut operation_timing = operation_timing::Recorder::new(crate::clock::frequency()?);
    let mut frame_index = 0;
    let mut next_frame = Instant::now();
    let mut cadence: [Cadence; 3] = std::array::from_fn(|_| Cadence::default());
    let mut generation_requested = false;
    let mut fixed_geometry_observed = false;
    let mut sampled_geometry_mismatches = 0;
    let mut consumer_ready_ms = None;
    let mut steady_scene = None;
    let mut settling_start: Option<Instant> = None;
    let mut settling = None;
    let initial = shared.status.lock().map_err(|_| "probe status poisoned")?.clone();
    let setup_ms = generation_start.elapsed().as_secs_f64() * 1000.0;
    let end = loop {
        if state.start.elapsed() >= Duration::from_secs(u64::from(spec.total_seconds())) {
            break GenerationEnd::DeadlineReached;
        }
        if shared.stop.load(Ordering::Acquire) {
            break GenerationEnd::WindowClosed;
        }
        if spec.isolates_editor() && shared.editor_test_complete.load(Ordering::Acquire) {
            break GenerationEnd::EditorIsolationComplete;
        }
        let collecting = state.start.elapsed() >= Duration::from_secs(u64::from(spec.warmup_seconds()));
        let deadline = next_frame + Duration::from_nanos(16_666_667);
        let stamp = Stamp {
            frame: frame_index,
            qpc: crate::clock::now()?,
        };
        frame_index += 1;
        let mut operations = FrameTimer::new(Instant::now(), next_frame, stamp);
        if state.start.elapsed() >= state.next_resource {
            let fixtures = state.fixtures.as_ref().ok_or("source fixtures retired early")?;
            let sample = operations.measure(Operation::Resources, || {
                ResourceSample::read(
                    state.start.elapsed().as_secs_f64(),
                    collecting,
                    &gpu,
                    [fixtures[0].child(), fixtures[1].child()],
                    state.observer.child(),
                    [&editor, &output],
                    Stage {
                        generation,
                        settling: settling_start.is_some(),
                        gpu_pipeline: admission.snapshot(),
                    },
                )
            })?;
            state.resources.samples.push(sample);
            // Preserve sparse cadence without a catch-up burst after a stalled GPU call.
            state.next_resource = Duration::from_secs(state.start.elapsed().as_secs() + 1);
        }
        let reservation = operations.measure(Operation::GpuAdmission, || admission.reserve(collecting))?;
        let Some(reservation) = reservation else {
            operations.measure(Operation::ObserverControl, || state.observer.poll())?;
            for path in &mut cadence {
                path.record(Instant::now(), deadline, collecting, Submission::Busy);
            }
            operation_timing.record(operations.finish(GpuWork::Coalesced), collecting);
            pace(&mut next_frame);
            continue;
        };
        let work_start = Instant::now();
        let query_recorded = operations.measure(Operation::QueryBegin, || {
            timing.begin(&gpu, work_start, collecting, stamp)
        })?;
        let (rects, revision, layout_session) = operations.measure(Operation::Layout, || -> ProbeResult<_> {
            let status = shared.status.lock().map_err(|_| "probe status poisoned")?;
            Ok((status.rects.clone(), status.layout_revision, status.layout_session))
        })?;
        let mut observations = [0; 2];
        for channel in 0..2 {
            let operation = if channel == 0 {
                Operation::PollSourceA
            } else {
                Operation::PollSourceB
            };
            let received = operations.measure(operation, || inputs[channel].poll(&gpu))?;
            if received {
                observations[channel] = 1;
            }
        }
        if collecting {
            dependency_trace.record(Dependencies {
                renderer_frame: stamp.frame,
                sources: std::array::from_fn(|channel| inputs[channel].frame().map(|frame| frame.batch)),
            });
        }
        operations.measure(Operation::Compose, || -> ProbeResult<()> {
            gpu.clear(&composed, [0.05, 0.05, 0.05, 1.0])?;
            for (index, input) in inputs.iter().enumerate() {
                if let Some(frame) = input.frame() {
                    quad.draw(
                        &gpu,
                        &composed,
                        &frame.texture,
                        PhysicalRect {
                            left: index as i32 * (width / 2) as i32,
                            top: 0,
                            width: width / 2,
                            height,
                        },
                    )?;
                }
            }
            if generation > 0 {
                quad.draw(
                    &gpu,
                    &composed,
                    &marker,
                    PhysicalRect {
                        left: 0,
                        top: 0,
                        width: 16,
                        height: 16,
                    },
                )?;
            }
            Ok(())
        })?;
        operations.measure(Operation::SendSpout, || sender.send(&composed))?;
        cadence[2].record(Instant::now(), deadline, collecting, Submission::Accepted);
        operations.measure(Operation::ObserverControl, || -> ProbeResult<()> {
            if generation > 0 && !generation_requested && inputs.iter().all(|input| input.frame().is_some()) {
                state.observer.verify_generation(generation)?;
                generation_requested = true;
            }
            if let Some(phase) = CompositePhase::ALL.get(state.next_checkpoint).copied()
                && inputs[1].frame().is_some()
                && phase.matches_source(inputs[0].frame().map(|frame| frame.surface.extent), spec.scenario)
            {
                state.observer.sample(phase)?;
                state.next_checkpoint += 1;
            }
            state.observer.poll()?;
            Ok(())
        })?;
        if generation > 0 && consumer_ready_ms.is_none() && state.observer.verified_generation() == generation {
            consumer_ready_ms = Some(generation_start.elapsed().as_secs_f64() * 1000.0);
        }
        let output_presented = match operations.measure(Operation::OutputTarget, || output.target(&gpu))? {
            Target::Busy => Submission::Busy,
            Target::Unavailable => Submission::NoTarget,
            Target::Ready(target) => {
                let [width, height] = output.size();
                operations.measure(Operation::OutputDraw, || {
                    quad.draw(
                        &gpu,
                        &target,
                        &composed,
                        PhysicalRect {
                            left: 0,
                            top: 0,
                            width,
                            height,
                        },
                    )
                })?;
                drop(target);
                operations.measure(Operation::OutputPresent, || output.present())?
            }
        };
        cadence[1].record(Instant::now(), deadline, collecting, output_presented);
        let fixed_geometry = rects.len() == 2
            && output.size() == spec.scenario.extent()
            && rects.iter().zip([[640, 360], [480, 270]]).all(|(rect, expected)| {
                [rect.bounds.width, rect.bounds.height] == expected
                    && rect.clip.width == rect.bounds.width
                    && rect.clip.height == rect.bounds.height
            });
        fixed_geometry_observed |= fixed_geometry;
        if collecting && spec.scenario.cards() && !fixed_geometry {
            sampled_geometry_mismatches += 1;
        }
        let editor_presented = match operations.measure(Operation::EditorTarget, || editor.target(&gpu))? {
            Target::Busy => Submission::Busy,
            Target::Unavailable => Submission::NoTarget,
            Target::Ready(target) => {
                operations.measure(Operation::EditorDraw, || -> ProbeResult<()> {
                    gpu.clear(&target, [0.08, 0.08, 0.08, 1.0])?;
                    for (index, rect) in rects.iter().enumerate() {
                        if let Some(frame) = inputs[index].frame() {
                            quad.draw_clipped(&gpu, &target, &frame.texture, rect.bounds, rect.clip)?;
                        }
                    }
                    Ok(())
                })?;
                drop(target);
                operations.measure(Operation::EditorPresent, || editor.present())?
            }
        };
        cadence[0].record(Instant::now(), deadline, collecting, editor_presented);
        operations.measure(Operation::Status, || -> ProbeResult<()> {
            let mut status = shared.status.lock().map_err(|_| "probe status poisoned")?;
            status.generation = generation;
            status.settling = settling_start.is_some();
            if editor_presented == Submission::Accepted && status.layout_session == layout_session {
                status.rendered_revision = revision;
            }
            status.preview_presents = status
                .preview_presents
                .saturating_add(u32::from(editor_presented == Submission::Accepted));
            status.output_presents = status
                .output_presents
                .saturating_add(u32::from(output_presented == Submission::Accepted));
            status.spout_sends = status.spout_sends.saturating_add(1);
            for (count, delta) in status.source_observations.iter_mut().zip(observations) {
                *count = count.saturating_add(delta);
            }
            Ok(())
        })?;
        operations.measure(Operation::QueryEnd, || timing.end_gpu(&gpu));
        let signaled = operations.measure(Operation::GpuSignal, || admission.signal(reservation))?;
        operations.measure(Operation::GpuFlush, || signaled.flush());
        timing.end_cpu(work_start);
        operation_timing.record(
            operations.finish(GpuWork::Submitted {
                profiling_skipped: !query_recorded,
            }),
            collecting,
        );
        // Classify interruption before accepting a rebuild/final acknowledgement. Teardown
        // time must never turn an early close or expired watchdog into a complete exercise.
        if state.start.elapsed() >= Duration::from_secs(u64::from(spec.total_seconds())) {
            break GenerationEnd::DeadlineReached;
        }
        if shared.stop.load(Ordering::Acquire) {
            break GenerationEnd::WindowClosed;
        }
        let ready = spec.device_recreations() > 0
            && state.start.elapsed() >= Duration::from_secs(6)
            && generation_start.elapsed() >= Duration::from_secs(2)
            && state.observer.recovered()
            && state.observer.verified_generation() == generation;
        if ready && steady_scene.is_none() {
            steady_scene = Some(SceneCheckpoint::read(state.start.elapsed().as_secs_f64(), &gpu)?);
        }
        if ready && generation < spec.device_recreations() {
            break GenerationEnd::Recreated;
        }
        if ready && generation == spec.device_recreations() {
            let start = *settling_start.get_or_insert_with(Instant::now);
            if start.elapsed() >= Duration::from_secs(u64::from(spec.settling_seconds())) {
                settling = Some(SettlingReport {
                    elapsed_seconds: start.elapsed().as_secs_f64(),
                    scene: SceneCheckpoint::read(state.start.elapsed().as_secs_f64(), &gpu)?,
                });
                break GenerationEnd::FinalGenerationComplete;
            }
        }
        pace(&mut next_frame);
    };
    let recreate = end == GenerationEnd::Recreated;
    let retirement_started = Instant::now();

    // Retirement and all COM destruction stay on this worker; both HWNDs remain alive.
    let timing = timing.finish(&gpu)?;
    let native_output_extent = output.size();
    let status = shared.status.lock().map_err(|_| "probe status poisoned")?.clone();
    let consumer_checkpoint = state.observer.generation_result(generation);
    let admission_editor_output = [editor.admission(), output.admission()];
    drop(sender);
    drop(editor);
    drop(output);
    drop(quad);
    drop(composed);
    drop(marker);
    // Complete all reads on the render context before releasing bridge readers and joining
    // source workers. Their SDK/context destruction never runs on this render context.
    gpu.retire()?;
    let source_workers = inputs
        .into_iter()
        .map(Source::finish)
        .collect::<ProbeResult<Vec<_>>>()?;
    if !recreate {
        for fixture in state.fixtures.take().ok_or("source fixtures retired early")? {
            state.sources.push(fixture.finish()?);
        }
    }
    let directory = Endpoint::new(&gpu)?;
    if directory.sender_exists(&session.output_name)? {
        return Err("retired output registration survived".into());
    }
    for name in &session.source_names {
        if directory.sender_exists(name)? != recreate {
            return Err("source lifecycle changed across device retirement".into());
        }
    }
    drop(directory);
    let allocations_after_resource_drop = gpu.allocations.snapshot();
    let retirement_fence_ms = gpu.retire()?;
    let gpu_admission = admission.finish_after_fence()?;
    let allocations_after_fence = gpu.allocations.snapshot();
    // Cancellation still follows normal owned-fixture/SDK/swapchain retirement before erroring.
    spec.validate_generation_end(generation, end)?;
    if generation > 0 && consumer_checkpoint.is_none() {
        return Err("device generation was not independently observed before retirement".into());
    }
    Ok(GenerationReport {
        generation,
        end,
        adapter_luid: gpu.adapter.luid.clone(),
        elapsed_seconds: generation_start.elapsed().as_secs_f64(),
        setup_ms,
        consumer_ready_ms,
        native_output_extent,
        fixed_geometry_observed,
        sampled_geometry_mismatches,
        output_presents: status.output_presents - initial.output_presents,
        preview_presents: status.preview_presents - initial.preview_presents,
        spout_sends: status.spout_sends - initial.spout_sends,
        timing,
        dependency_trace,
        operation_timing: operation_timing.finish(),
        gpu_admission,
        source_workers,
        consumer_checkpoint,
        retirement_fence_ms,
        after_release: None,
        allocations_after_resource_drop,
        allocations_after_fence,
        operations: gpu.allocations.operations(),
        submission_editor_output_spout: cadence.map(Cadence::finish),
        admission_editor_output,
        steady_scene,
        settling,
        retirement_started,
    })
}

fn pace(next_frame: &mut Instant) {
    *next_frame += Duration::from_nanos(16_666_667);
    if let Some(delay) = next_frame.checked_duration_since(Instant::now()) {
        std::thread::sleep(delay);
    } else {
        *next_frame = Instant::now();
    }
}
