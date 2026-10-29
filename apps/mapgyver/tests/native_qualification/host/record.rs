//! Sparse process/status recording and bounded telemetry draining on the outer host worker.

use std::{
    collections::BTreeMap,
    fs::File,
    io::{self, BufWriter, Error, Write},
    num::NonZeroUsize,
    time::Duration,
};

use golden_video::native::{
    D3d11Control, DiagnosticDelivery, DiagnosticEvent, query_adapter_memory, query_process_resources,
};
use golden_video::runtime::VideoRuntime;
use serde::Serialize;
use serde_json::{Value, json};
use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};

use super::{HostConfig, Observations};

pub struct Clock {
    pub frequency: u64,
}
impl Clock {
    pub fn new() -> io::Result<Self> {
        let mut frequency = 0;
        // SAFETY: the API writes only the initialized local frequency coordinate.
        unsafe { QueryPerformanceFrequency(&mut frequency) }.map_err(Error::other)?;
        if frequency <= 0 {
            return Err(Error::other("QPC frequency is unavailable"));
        }
        Ok(Self {
            frequency: frequency as u64,
        })
    }
    pub fn now(&self) -> io::Result<u64> {
        let mut now = 0;
        // SAFETY: the API writes only the initialized local QPC coordinate.
        unsafe { QueryPerformanceCounter(&mut now) }.map_err(Error::other)?;
        if now <= 0 {
            return Err(Error::other("QPC coordinate is unavailable"));
        }
        Ok(now as u64)
    }
    pub fn ticks(&self, duration: Duration) -> io::Result<u64> {
        let ticks = duration
            .as_nanos()
            .checked_mul(u128::from(self.frequency))
            .ok_or_else(|| Error::other("phase QPC duration overflow"))?
            / 1_000_000_000;
        u64::try_from(ticks).map_err(Error::other)
    }
}

#[derive(Serialize)]
pub struct PhaseWindow {
    pub name: String,
    pub begin_qpc: u64,
    pub end_qpc: Option<u64>,
}

#[derive(Default, Serialize)]
struct OutputCounts {
    accepted: u64,
    busy: u64,
    unavailable: u64,
    failed: u64,
    device_lost: u64,
}

pub struct Recorder {
    pub clock: Clock,
    pub windows: Vec<PhaseWindow>,
    output_counts: BTreeMap<String, BTreeMap<u64, OutputCounts>>,
    writer: BufWriter<File>,
    rows: Vec<DiagnosticEvent>,
}

impl Recorder {
    pub fn new(config: &HostConfig) -> io::Result<Self> {
        let mut result = Self {
            clock: Clock::new()?,
            windows: Vec::new(),
            output_counts: BTreeMap::new(),
            writer: BufWriter::new(File::create(config.directory.join("renderer.jsonl"))?),
            rows: Vec::with_capacity(4096),
        };
        result.write(json!({"kind":"capture", "process_id":std::process::id(), "qpc_frequency":result.clock.frequency,
            "scenario":config.scenario, "adapter":config.adapter, "source_names":config.source_names,
            "output_name":config.output_name, "requested_warmup_seconds":config.warmup.as_secs_f64(),
            "requested_sample_seconds":config.duration.as_secs_f64(), "requested_independence_seconds":config.independence_duration.as_secs_f64(),
            "fixture_pids":config.fixture_pids, "webview_process_ids":null, "qualification_budgets_evaluated":false}))?;
        result.phase("startup", None)?;
        Ok(result)
    }
    pub fn write(&mut self, value: Value) -> io::Result<()> {
        serde_json::to_writer(&mut self.writer, &value)?;
        self.writer.write_all(b"\n")
    }
    pub fn phase(&mut self, name: &str, duration: Option<Duration>) -> io::Result<Option<u64>> {
        let now = self.clock.now()?;
        if let Some(previous) = self.windows.last_mut()
            && previous.end_qpc.is_none()
        {
            previous.end_qpc = Some(now);
        }
        let end = duration
            .map(|duration| {
                self.clock
                    .ticks(duration)
                    .and_then(|ticks| now.checked_add(ticks).ok_or_else(|| Error::other("phase QPC overflow")))
            })
            .transpose()?;
        self.windows.push(PhaseWindow {
            name: name.into(),
            begin_qpc: now,
            end_qpc: end,
        });
        self.write(json!({"kind":"phase", "name":name, "begin_qpc":now, "end_qpc":end}))?;
        self.writer.flush()?;
        Ok(end)
    }
    pub fn drain(&mut self, native: &D3d11Control) -> io::Result<()> {
        self.rows.clear();
        native.drain_diagnostics(&mut self.rows, NonZeroUsize::new(4096).unwrap());
        let rows = std::mem::take(&mut self.rows);
        for event in &rows {
            if let DiagnosticEvent::Output(output) = event {
                let stamp = output.delivery.map(|interval| interval.end);
                let phase = self
                    .windows
                    .iter()
                    .find(|window| {
                        stamp.is_some_and(|stamp| {
                            stamp >= window.begin_qpc && window.end_qpc.is_none_or(|end| stamp < end)
                        })
                    })
                    .map_or("unassigned", |window| window.name.as_str());
                let counts = self
                    .output_counts
                    .entry(phase.into())
                    .or_default()
                    .entry(output.output.get())
                    .or_default();
                match output.outcome {
                    DiagnosticDelivery::Accepted => counts.accepted += 1,
                    DiagnosticDelivery::Busy => counts.busy += 1,
                    DiagnosticDelivery::Unavailable => counts.unavailable += 1,
                    DiagnosticDelivery::Failed { .. } => counts.failed += 1,
                    DiagnosticDelivery::DeviceLost { .. } => counts.device_lost += 1,
                }
            }
            self.write(json!({"kind":"diagnostic", "event":event}))?;
        }
        self.rows = rows;
        self.rows.clear();
        Ok(())
    }
    pub fn snapshot(
        &mut self,
        runtime: &VideoRuntime,
        native: &D3d11Control,
        observations: &Observations,
    ) -> io::Result<()> {
        self.write(
            json!({"kind":"snapshot", "qpc":self.clock.now()?, "runtime":runtime_snapshot(runtime),
            "native":native_snapshot(native), "diagnostics":native.diagnostics_snapshot(),
            "host":viewport(*observations.viewport.lock().unwrap()),
            "control_requests":observations.control_requests.load(std::sync::atomic::Ordering::Relaxed),
            "fixture_report_loss":observations.report_loss.load(std::sync::atomic::Ordering::Relaxed)}),
        )
    }
    pub fn resources(&mut self, config: &HostConfig) -> io::Result<()> {
        let memory = query_adapter_memory(config.adapter);
        self.write(json!({"kind":"adapter_memory", "qpc":self.clock.now()?, "snapshot":memory.as_ref().ok(), "error":memory.as_ref().err()}))?;
        for pid in std::iter::once(std::process::id()).chain(config.fixture_pids.iter().copied()) {
            let process = query_process_resources(pid);
            self.write(
                json!({"kind":"process_resources", "snapshot":process.as_ref().ok(), "error":process.as_ref().err()}),
            )?;
        }
        self.writer.flush()
    }
    pub fn finish(&mut self, config: &HostConfig, summary: Value) -> io::Result<()> {
        let mut summary = summary;
        summary["phase_windows"] = serde_json::to_value(&self.windows)?;
        summary["per_output_counts"] = serde_json::to_value(&self.output_counts)?;
        summary["qpc_frequency"] = json!(self.clock.frequency);
        self.writer.flush()?;
        let file = File::create(config.directory.join("renderer-summary.json"))?;
        let mut writer = BufWriter::new(file);
        serde_json::to_writer_pretty(&mut writer, &summary)?;
        writer.flush()
    }
}

pub fn viewport(value: Option<golden_video::preview::HostViewport>) -> Value {
    value.map_or(Value::Null, |host| json!({"client":[host.client_width,host.client_height],
        "webview_origin":[host.webview_origin_x,host.webview_origin_y], "webview_extent":[host.webview_width,host.webview_height], "css_scale":host.css_scale}))
}

pub fn runtime_snapshot(runtime: &VideoRuntime) -> Value {
    let snapshot = runtime.snapshot();
    json!({
        "state": format!("{:?}", snapshot.state),
        "project": snapshot.project.generation(),
        "frames_submitted": snapshot.frames_submitted,
        "missed_frames": snapshot.missed_frames,
        "outstanding": snapshot.outstanding,
        "preparing": snapshot.preparing,
        "prepared": snapshot.prepared,
        "latest_desired": snapshot.latest_desired,
        "failure": snapshot.failure.map(|failure|failure.to_string()),
        "resources": snapshot.resources.map(|resources|json!({
            "byte_limit": resources.byte_limit,
            "bytes_used": resources.bytes_used,
            "peak_bytes": resources.peak_bytes,
            "prepared": resources.prepared_generations,
            "retiring": resources.retiring_generations,
            "in_flight": resources.in_flight,
            "initialized": resources.initialized,
            "device_lost": resources.device_lost,
            "last_submitted": resources.last_submitted,
            "last_completed": resources.last_completed,
            "active": resources.active.map(|active|json!({
                "generation": active.generation.get(),
                "revision": active.identity.revision.get(),
                "project": active.identity.project.generation(),
                "device": active.device,
                "bytes": active.bytes,
                "outputs": active.output_count
            }))
        })),
        "last_frame": snapshot.last_frame.map(|frame|json!({
            "frame_index": frame.frame_index,
            "scheduled_nanos": frame.scheduled_at.as_nanos(),
            "completion": frame.completion,
            "generation": frame.generation.get(),
            "project": frame.identity.project.generation(),
            "revision": frame.identity.revision.get()
        }))
    })
}

pub fn native_snapshot(native: &D3d11Control) -> Value {
    let state = native.snapshot();
    json!({
        "adapter": state.adapter,
        "device": state.device,
        "initialized": state.initialized,
        "initialization_attempts": state.initialization_attempts,
        "submitted": state.submitted,
        "accepted_outputs": state.accepted_outputs,
        "busy_outputs": state.busy_outputs,
        "unavailable_outputs": state.unavailable_outputs,
        "failed_outputs": state.failed_outputs,
        "hresult": state.hresult,
        "failure": state.failure.map(|failure|format!("{failure:?}")),
        "outputs": native.output_snapshots().iter().map(|output|json!({
            "id": output.id.get(),
            "state": format!("{:?}", output.state),
            "generation": output.generation,
            "attempts": output.attempts,
            "hresult": output.hresult,
            "last_accepted": output.last_accepted
        })).collect::<Vec<_>>(),
        "sources": native.sources().iter().map(|source|json!({
            "binding": source.binding,
            "device": source.device,
            "texture": source.texture.get(),
            "descriptor": source.descriptor,
            "observed_descriptor": source.observed_descriptor,
            "state": format!("{:?}", source.state),
            "generation": source.generation,
            "pool_generation": source.pool_generation,
            "bridge_slots": source.bridge_slots,
            "sdk_calls": source.sdk_calls,
            "receive_copies": source.receive_copies,
            "receive_busy": source.receive_busy,
            "unchanged": source.unchanged,
            "last_copied_frame": source.last_copied_frame,
            "publications": source.publications,
            "sequence": source.sequence,
            "bridge_busy": source.bridge_busy,
            "imports": source.imports,
            "renderer_copies": source.renderer_copies,
            "hresult": source.hresult
        })).collect::<Vec<_>>()
    })
}
