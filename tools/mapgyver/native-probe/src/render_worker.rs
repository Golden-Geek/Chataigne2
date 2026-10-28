mod generation;
use crate::{
    ProbeResult, d3d, fixture,
    observer::{ObserverProcess, protocol::ObserverReport},
    presentation::ProbeStatus,
    resources::{ProcessSnapshot, ResourceReport},
};
use generation::GenerationReport;
use mapgyver_native_probe::{
    sample_window::Window,
    workload::{ConsumerMode, GenerationEnd, RunSpec},
};
use serde::Serialize;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64},
};
use std::time::{Duration, Instant};

#[derive(Default)]
pub struct Shared {
    pub stop: AtomicBool,
    pub finished: AtomicBool,
    pub status: Mutex<ProbeStatus>,
    pub editor_requests: AtomicU64,
    pub editor_test_enabled: AtomicBool,
    pub editor_test_complete: AtomicBool,
    pub editor_signals: Mutex<crate::editor_isolation::Signals>,
}

#[derive(Serialize)]
pub struct RenderReport {
    pub editor_isolation: Option<crate::editor_isolation::Report>,
    pub workload: RunSpec,
    pub sample_window: Option<Window>,
    pub evidence_kind: &'static str,
    pub render_process_id: u32,
    pub adapter_luid: String,
    pub compiler: &'static str,
    pub windows_sdk_include: &'static str,
    pub elapsed_seconds: f64,
    pub maximum_session_seconds: u32,
    pub status: ProbeStatus,
    pub observer: ObserverReport,
    pub sources: Vec<fixture::protocol::Report>,
    pub generations: Vec<GenerationReport>,
    pub resources: ResourceReport,
    pub shutdown_seconds: Option<f64>,
    #[serde(skip)]
    pub retirement_started: Instant,
    pub unqualified: [&'static str; 3],
}

struct Session {
    adapter: u32,
    adapter_luid: String,
    spec: RunSpec,
    editor: usize,
    output: usize,
    source_names: [String; 2],
    output_name: String,
}

struct RunState {
    shared: Arc<Shared>,
    start: Instant,
    next_resource: Duration,
    next_checkpoint: usize,
    observer: ObserverProcess,
    fixtures: Option<[fixture::FixtureProcess; 2]>,
    sources: Vec<fixture::protocol::Report>,
    resources: ResourceReport,
}

pub fn run(
    adapter: u32,
    spec: RunSpec,
    editor: usize,
    output: usize,
    shared: Arc<Shared>,
) -> ProbeResult<RenderReport> {
    let adapters = d3d::inventory()?;
    let info = adapters.get(adapter as usize).ok_or("selected adapter unavailable")?;
    if info.software {
        return Err("software adapter cannot qualify hardware recreation".into());
    }
    let suffix = std::process::id();
    let session = Session {
        adapter,
        adapter_luid: info.luid.clone(),
        spec,
        editor,
        output,
        source_names: [
            format!("MapGyver-underlay-a-{suffix}"),
            format!("MapGyver-underlay-b-{suffix}"),
        ],
        output_name: format!("MapGyver-underlay-out-{suffix}"),
    };
    let resources = ResourceReport::new()?;
    let observer = ObserverProcess::spawn(
        adapter,
        &session.output_name,
        &session.adapter_luid,
        spec.scenario,
        spec.consumer_mode(),
    )?;
    let fixtures = [
        fixture::spawn_ready(
            adapter,
            &session.source_names[0],
            "a",
            (spec.total_seconds() + 2) * 60,
            spec.scenario,
            &session.adapter_luid,
        )?,
        fixture::spawn_ready(
            adapter,
            &session.source_names[1],
            "b",
            (spec.total_seconds() + 2) * 60,
            spec.scenario,
            &session.adapter_luid,
        )?,
    ];
    let mut state = RunState {
        shared: shared.clone(),
        start: Instant::now(),
        next_resource: Duration::ZERO,
        next_checkpoint: 0,
        observer,
        fixtures: Some(fixtures),
        sources: Vec::with_capacity(2),
        resources,
    };
    let sample_window = if spec.scenario.cards() {
        let window = Window {
            start_qpc: crate::clock::now()?,
            frequency: crate::clock::frequency()?,
            warmup_seconds: spec.warmup_seconds(),
            maximum_seconds: spec.total_seconds(),
        };
        state.observer.begin_observation(window)?;
        Some(window)
    } else {
        None
    };
    let mut generations = Vec::new();
    for generation in 0..=spec.device_recreations() {
        let generation_start = Instant::now();
        let mut report = generation::run(&session, &mut state, generation)?;
        // The generation function has returned, so its device/context COM references are gone.
        report.after_release = Some(ProcessSnapshot::current()?);
        report.elapsed_seconds = generation_start.elapsed().as_secs_f64();
        let recreate = report.end == GenerationEnd::Recreated;
        generations.push(report);
        if !recreate {
            break;
        }
    }
    if generations.len() != spec.device_recreations() as usize + 1
        || state.observer.verified_generation() != spec.device_recreations()
    {
        return Err(format!(
            "device sequence incomplete: {} scopes, {} independently verified rebuilds of {}, {}-second watchdog",
            generations.len(),
            state.observer.verified_generation(),
            spec.device_recreations(),
            spec.total_seconds()
        )
        .into());
    }
    let observer = state.observer.finish()?;
    let retirement_started = generations
        .last()
        .ok_or("missing final device scope")?
        .retirement_started;
    state.resources.after_gpu_release = Some(ProcessSnapshot::current()?);
    let status = shared.status.lock().map_err(|_| "probe status poisoned")?.clone();
    if let Some(error) = &status.error {
        return Err(format!("WebView geometry failed: {error}").into());
    }
    if status.layout_revision == 0
        || status.preview_presents == 0
        || status.output_presents == 0
        || status.source_observations.contains(&0)
    {
        return Err("underlay probe did not observe layout, both inputs and both presentations".into());
    }
    Ok(RenderReport {
        editor_isolation: None,
        workload: spec,
        sample_window,
        evidence_kind: if spec.consumer_mode() == ConsumerMode::Paused {
            "consumer_paused_gpu_sharing_diagnostic_not_qualification"
        } else {
            "ordinary_tauri_d3d11_probe_not_visual_or_hardware_reset_qualification"
        },
        render_process_id: std::process::id(),
        adapter_luid: session.adapter_luid,
        compiler: env!("PROBE_CXX"),
        windows_sdk_include: env!("PROBE_WINDOWS_SDK_INCLUDE"),
        elapsed_seconds: state.start.elapsed().as_secs_f64(),
        maximum_session_seconds: spec.total_seconds(),
        status,
        observer,
        sources: state.sources,
        generations,
        resources: state.resources,
        shutdown_seconds: None,
        retirement_started,
        unqualified: [
            "manual visual/input/clipping/DPI",
            "texture residency, displayed cadence and latency",
            "hardware device removal and sustained retirement",
        ],
    })
}
