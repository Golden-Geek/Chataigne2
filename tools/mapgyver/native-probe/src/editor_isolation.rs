//! Owned WebView interruption experiment. The supervisor never owns a GPU context.

use crate::{
    ProbeResult, clock,
    presentation::{BusyCommand, EditorActivity},
    render_worker::Shared,
};
use serde::Serialize;
use std::sync::{Arc, atomic::Ordering};
use std::time::{Duration, Instant};
use tauri::WebviewWindow;

#[cfg(test)]
mod tests;

#[derive(Default)]
pub struct Signals {
    started: Option<(Instant, u64)>,
    finished: Option<(Instant, u64, f64)>,
}

impl Signals {
    pub fn record(&mut self, event: EditorActivity, current_session: u32) -> ProbeResult<()> {
        match event {
            EditorActivity::BusyStarted { session } if session == current_session && self.started.is_none() => {
                self.started = Some((Instant::now(), clock::now()?));
            }
            EditorActivity::BusyFinished { session, elapsed_ms }
                if session == current_session
                    && self.started.is_some()
                    && self.finished.is_none()
                    && elapsed_ms.is_finite()
                    && (65_000.0..120_000.0).contains(&elapsed_ms) =>
            {
                self.finished = Some((Instant::now(), clock::now()?, elapsed_ms));
            }
            _ => return Err("invalid, duplicate or stale editor activity".into()),
        }
        Ok(())
    }
}

#[derive(Serialize)]
pub struct Snapshot {
    pub qpc: u64,
    pub editor_requests: u64,
    pub layout_session: u32,
    pub layout_revision: u32,
    pub rendered_revision: u32,
    pub preview_regions: usize,
    pub preview_presents: u32,
    pub output_presents: u32,
    pub spout_sends: u32,
}

impl Snapshot {
    fn read(shared: &Shared) -> ProbeResult<Self> {
        let status = shared.status.lock().map_err(|_| "probe status poisoned")?;
        Ok(Self {
            qpc: clock::now()?,
            editor_requests: shared.editor_requests.load(Ordering::Relaxed),
            layout_session: status.layout_session,
            layout_revision: status.layout_revision,
            rendered_revision: status.rendered_revision,
            preview_regions: status.rects.len(),
            preview_presents: status.preview_presents,
            output_presents: status.output_presents,
            spout_sends: status.spout_sends,
        })
    }
}

#[derive(Serialize)]
pub struct Window {
    pub kind: &'static str,
    pub snapshots: Vec<Snapshot>,
}

#[derive(Serialize)]
pub struct Report {
    pub qpc_frequency: u64,
    pub busy_started_qpc: u64,
    pub busy_finished_qpc: u64,
    pub frontend_busy_elapsed_ms: f64,
    pub windows: [Window; 2],
    pub reconnected: Snapshot,
    pub scope: &'static str,
}

fn wait(shared: &Shared, seconds: u64, ready: impl Fn() -> bool) -> ProbeResult<()> {
    let deadline = Instant::now() + Duration::from_secs(seconds);
    while !ready() {
        if shared.stop.load(Ordering::Acquire) || shared.finished.load(Ordering::Acquire) {
            return Err("editor isolation interrupted".into());
        }
        if Instant::now() >= deadline {
            return Err("editor isolation stage timed out".into());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Ok(())
}

fn delay(shared: &Shared, duration: Duration) -> ProbeResult<()> {
    let until = Instant::now() + duration;
    wait(shared, duration.as_secs() + 2, || Instant::now() >= until)
}

fn observe(shared: &Shared, kind: &'static str, disconnected: bool) -> ProbeResult<Window> {
    let mut snapshots = Vec::with_capacity(62);
    snapshots.push(Snapshot::read(shared)?);
    let start = Instant::now();
    for second in 1..=61 {
        delay(
            shared,
            (start + Duration::from_secs(second)).saturating_duration_since(Instant::now()),
        )?;
        snapshots.push(Snapshot::read(shared)?);
    }
    for pair in snapshots.windows(2) {
        if pair[1].output_presents <= pair[0].output_presents || pair[1].spout_sends <= pair[0].spout_sends {
            return Err("output stopped during editor interruption".into());
        }
        if pair[1].editor_requests != pair[0].editor_requests
            || (disconnected && (pair[1].preview_regions != 0 || pair[1].layout_revision != 0))
        {
            return Err("editor remained active during interruption".into());
        }
    }
    Ok(Window { kind, snapshots })
}

pub fn run(window: WebviewWindow, shared: Arc<Shared>) -> Result<Report, String> {
    let result = run_inner(&window, &shared).map_err(|error| error.to_string());
    if result.is_err() {
        shared.stop.store(true, Ordering::Release);
    }
    result
}

fn run_inner(window: &WebviewWindow, shared: &Shared) -> ProbeResult<Report> {
    wait(shared, 15, || {
        shared.status.lock().is_ok_and(|s| {
            s.rects.len() == 2
                && s.layout_revision > 0
                && s.rendered_revision == s.layout_revision
                && s.output_presents > 300
                && s.spout_sends > 300
        })
    })?;
    let original_url = window.url()?;
    let command = BusyCommand {
        session: shared
            .status
            .lock()
            .map_err(|_| "probe status poisoned")?
            .layout_session,
        duration_ms: 65_000,
    };
    window.eval(format!(
        "window.dispatchEvent(new CustomEvent('probe-editor-busy', {{detail:{}}}));",
        serde_json::to_string(&command)?
    ))?;
    wait(shared, 5, || {
        shared.editor_signals.lock().is_ok_and(|s| s.started.is_some())
    })?;
    delay(shared, Duration::from_secs(2))?;
    let busy = observe(shared, "javascript_busy", false)?;
    // Busy is still in progress at the end of the >60-second observation window.
    if shared
        .editor_signals
        .lock()
        .map_err(|_| "editor signals poisoned")?
        .finished
        .is_some()
    {
        return Err("busy script ended before the observation window".into());
    }
    wait(shared, 8, || {
        shared.editor_signals.lock().is_ok_and(|s| s.finished.is_some())
    })?;
    let requests = shared.editor_requests.load(Ordering::Relaxed);
    wait(shared, 5, || shared.editor_requests.load(Ordering::Relaxed) > requests)?;
    let original_session = command.session;
    window.navigate("about:blank".parse()?)?;
    wait(shared, 5, || {
        window.url().is_ok_and(|url| url.as_str() == "about:blank")
            && shared.status.lock().is_ok_and(|s| s.layout_session > original_session)
    })?;
    delay(shared, Duration::from_secs(2))?;
    let disconnected = observe(shared, "blank_document", true)?;
    let disconnected_session = shared
        .status
        .lock()
        .map_err(|_| "probe status poisoned")?
        .layout_session;
    window.navigate(original_url)?;
    wait(shared, 10, || {
        shared.status.lock().is_ok_and(|s| {
            s.layout_session > disconnected_session
                && s.layout_revision > 0
                && s.rendered_revision == s.layout_revision
                && s.rects.len() == 2
        })
    })?;
    delay(shared, Duration::from_secs(2))?;
    let reconnected = Snapshot::read(shared)?;
    let signals = shared.editor_signals.lock().map_err(|_| "editor signals poisoned")?;
    let (_, busy_started_qpc) = signals.started.ok_or("busy start missing")?;
    let (_, busy_finished_qpc, frontend_busy_elapsed_ms) = signals.finished.ok_or("busy finish missing")?;
    let report = Report {
        qpc_frequency: clock::frequency()?,
        busy_started_qpc,
        busy_finished_qpc,
        frontend_busy_elapsed_ms,
        windows: [busy, disconnected],
        reconnected,
        scope: "owned WebView JS blocking and about:blank navigation; API output progress, not displayed cadence or browser crash recovery",
    };
    shared.editor_test_complete.store(true, Ordering::Release);
    Ok(report)
}
