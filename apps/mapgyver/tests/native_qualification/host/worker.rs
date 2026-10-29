//! Owns the production runtime, reporting intervals and actual native retirement.

use std::{
    io::{self, Error},
    num::NonZeroUsize,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

use golden_core::app::NativePresentationStop;
use golden_io::WorkerNotifier;
use golden_video::Extent2D;
use golden_video::native::{
    D3d11Config, D3d11Control, DiagnosticsConfig, InputLimits, OutputBinding, SourceState, WindowTarget,
    query_adapter_memory,
};
use golden_video::preview::{PreviewController, PreviewReceiver};
use golden_video::qualification::protocol::{QualificationAction, QualificationPhase};
use golden_video::runtime::{CompletionToken, RuntimeState, VideoRuntime};
use serde_json::{Value, json};
use windows::Win32::{Foundation::HWND, UI::WindowsAndMessaging::IsWindow};

use super::super::scene;
use super::{
    HostConfig, Observations,
    preview::Preview,
    record::{self, Recorder},
};

pub struct Input {
    pub config: HostConfig,
    pub editor: tauri::WebviewWindow,
    pub output: tauri::Window,
    pub editor_hwnd: isize,
    pub output_hwnd: isize,
    pub output_extent: Extent2D,
    pub editor_backing: Extent2D,
    pub window_metadata: Value,
    pub retired: Arc<AtomicBool>,
    pub controller: PreviewController,
    pub receiver: PreviewReceiver,
    pub observations: Arc<Observations>,
    pub notifier: WorkerNotifier,
    pub signal: mpsc::Receiver<()>,
}

struct Lease {
    runtime: VideoRuntime,
    controller: PreviewController,
    retired: bool,
}
impl Drop for Lease {
    fn drop(&mut self) {
        self.controller.close();
        self.runtime.close();
        if !self.retired {
            // The external owned-process watchdog can fail a stuck run. A timeout here
            // cannot release the HWND or turn incomplete GPU cleanup into retirement.
            while !self.runtime.retirement().wait(Duration::from_millis(20)) {}
        }
    }
}

struct Capture {
    lease: Lease,
    input: Input,
    native: D3d11Control,
    preview: Preview,
    recorder: Recorder,
    next_drain: Instant,
    next_resources: Instant,
    admission: Value,
}

pub fn run(input: Input, stop: NativePresentationStop) -> io::Result<()> {
    let summary = input.config.directory.join("renderer-summary.json");
    let result = run_capture(input, stop);
    if let Err(error) = &result
        && !summary.exists()
    {
        let mut writer = std::io::BufWriter::new(std::fs::File::create(summary)?);
        serde_json::to_writer_pretty(
            &mut writer,
            &json!({"functional_completed":false,"process_id":std::process::id(),"error":error.to_string(),
            "qualification_budgets_evaluated":false,"capture_summary_unavailable":true}),
        )?;
        std::io::Write::flush(&mut writer)?;
    }
    result
}

fn run_capture(mut input: Input, stop: NativePresentationStop) -> io::Result<()> {
    let mut recorder = Recorder::new(&input.config)?;
    settle_output_client(&mut input, &stop)?;
    recorder.write(
        json!({"kind":"windows", "qpc":recorder.clock.now()?, "output":input.window_metadata,
        "editor_backing":[input.editor_backing.width(),input.editor_backing.height()]}),
    )?;
    let memory = query_adapter_memory(input.config.adapter).map_err(Error::other)?;
    let ceiling = memory
        .admission_limit(input.config.memory_ceiling)
        .map_err(Error::other)?;
    let admission =
        json!({"adapter_memory":memory,"declared_ceiling":input.config.memory_ceiling,"admitted_ceiling":ceiling});
    recorder.write(json!({"kind":"admission", "qpc":recorder.clock.now()?, "admission":admission}))?;
    let runtime = VideoRuntime::spawn_d3d11(
        scene::config(ceiling, input.notifier.clone()),
        D3d11Config {
            diagnostics: Some(DiagnosticsConfig {
                capacity: NonZeroUsize::new(input.config.trace_capacity)
                    .ok_or_else(|| Error::other("trace capacity is zero"))?,
            }),
            adapter: input.config.adapter,
            inputs: InputLimits {
                max_sources: 2,
                max_storage_bytes: scene::input_bytes(input.output_extent),
            },
            outputs: vec![
                OutputBinding::Window {
                    id: scene::output(scene::MAIN_OUTPUT),
                    // SAFETY: the task retains this owned output and prevents close until actual retirement.
                    window: unsafe { WindowTarget::from_raw(input.output_hwnd) }.map_err(Error::other)?,
                    extent: input.output_extent,
                },
                OutputBinding::Spout {
                    id: scene::output(scene::SPOUT_OUTPUT),
                    name: input.config.output_name.clone(),
                    descriptor: scene::source_descriptor(input.output_extent),
                },
                OutputBinding::Window {
                    id: scene::output(scene::EDITOR_OUTPUT),
                    // SAFETY: shared-host NativePresentation retains its editor HWND until this task returns.
                    window: unsafe { WindowTarget::from_raw(input.editor_hwnd) }.map_err(Error::other)?,
                    extent: input.editor_backing,
                },
            ],
        },
    )
    .map_err(Error::other)?;
    let lease = Lease {
        runtime,
        controller: input.controller.clone(),
        retired: false,
    };
    let native = lease
        .runtime
        .d3d11_control()
        .ok_or_else(|| Error::other("D3D11 control is unavailable"))?;
    native.set_source_notifier(input.notifier.clone());
    native.set_output_notifier(input.notifier.clone());
    let preview = Preview::new(
        input.receiver.clone(),
        input.observations.clone(),
        input.config.source_names.clone(),
        input.output_extent,
        input.editor_backing,
    );
    let mut capture = Capture {
        input,
        lease,
        native,
        preview,
        recorder,
        next_drain: Instant::now(),
        next_resources: Instant::now(),
        admission,
    };
    let result = capture.capture(&stop);
    capture.finish(result)
}

impl Capture {
    fn service(&mut self, stop: &NativePresentationStop) -> io::Result<()> {
        if stop.is_requested() {
            return Err(Error::other("host exit interrupted the functional capture"));
        }
        if self.lease.runtime.snapshot().state == RuntimeState::Stopped {
            return Err(Error::other("renderer stopped before capture completion"));
        }
        self.preview.service(
            &self.input.editor,
            &mut self.lease.runtime,
            &self.native,
            &mut self.recorder,
        )?;
        let now = Instant::now();
        if now >= self.next_drain {
            self.recorder.drain(&self.native)?;
            self.recorder
                .snapshot(&self.lease.runtime, &self.native, &self.input.observations)?;
            self.next_drain = now + Duration::from_millis(100);
        }
        if now >= self.next_resources {
            self.recorder.resources(&self.input.config)?;
            self.next_resources = now + Duration::from_secs(1);
        }
        let _ = self.input.signal.recv_timeout(Duration::from_millis(20));
        Ok(())
    }

    fn wait(
        &mut self,
        label: &str,
        stop: &NativePresentationStop,
        mut ready: impl FnMut(&Self) -> bool,
    ) -> io::Result<()> {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            self.service(stop)?;
            if ready(self) {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(Error::other(format!(
                    "timed out waiting for {label}; runtime={:?}",
                    self.lease.runtime.snapshot()
                )));
            }
        }
    }

    fn capture(&mut self, stop: &NativePresentationStop) -> io::Result<()> {
        self.wait(
            "two source streams and installed physical preview geometry",
            stop,
            |capture| {
                let sources = capture.native.sources();
                let extents = scene::source_extents(capture.input.output_extent);
                capture.preview.ready()
                    && capture.preview.geometry.is_some()
                    && capture.lease.runtime.snapshot().state == RuntimeState::Running
                    && sources.len() == 2
                    && sources.iter().zip(extents).all(|(source, extent)| {
                        source.state == SourceState::Streaming
                            && source.observed_descriptor == Some(scene::source_descriptor(extent))
                    })
                    && capture
                        .native
                        .output_snapshots()
                        .iter()
                        .all(|output| output.last_accepted.is_some())
            },
        )?;
        self.recorder
            .write(json!({"kind":"geometry", "qpc":self.recorder.clock.now()?, "observed":self.preview.geometry}))?;
        self.interval("warmup", self.input.config.warmup, stop)?;
        self.interval("sample", self.input.config.duration, stop)?;
        if !self.input.config.independence_duration.is_zero() {
            let editor = self.input.editor.clone();
            on_ui(&self.input.editor, move || {
                editor.minimize().map_err(|error| error.to_string())
            })?;
            self.interval("editor_minimized", self.input.config.independence_duration, stop)?;
            let editor = self.input.editor.clone();
            on_ui(&self.input.editor, move || {
                editor.unminimize().map_err(|error| error.to_string())
            })?;
            self.preview
                .command(&self.input.editor, 1, QualificationAction::Ready)?;
            self.wait("restored editor installation", stop, |capture| {
                capture.preview.acknowledged(1, QualificationPhase::Ready) && capture.preview.ready()
            })?;
            self.preview
                .command(&self.input.editor, 2, QualificationAction::Disconnect)?;
            self.wait("acknowledged editor disconnection", stop, |capture| {
                capture.preview.acknowledged(2, QualificationPhase::Disconnected)
                    && capture.preview.receiver.snapshot().views.is_empty()
            })?;
            self.interval("editor_disconnected", self.input.config.independence_duration, stop)?;
        }
        Ok(())
    }

    fn interval(&mut self, phase: &str, duration: Duration, stop: &NativePresentationStop) -> io::Result<()> {
        let before = main_tokens(&self.native);
        let end = self.recorder.phase(phase, Some(duration))?.unwrap();
        while self.recorder.clock.now()? < end {
            self.service(stop)?;
        }
        self.recorder.drain(&self.native)?;
        let after = main_tokens(&self.native);
        self.recorder
            .write(json!({"kind":"phase_outputs", "name":phase,"end_qpc":end,"before":before,"after":after}))?;
        if !duration.is_zero()
            && !before.iter().zip(after).all(|(before, after)| match (before, after) {
                (Some(before), Some(after)) => after.device == before.device && after.serial > before.serial,
                _ => false,
            })
        {
            return Err(Error::other(format!(
                "main native/Spout outputs did not advance during {phase}"
            )));
        }
        Ok(())
    }

    fn finish(&mut self, result: io::Result<()>) -> io::Result<()> {
        let active = record::runtime_snapshot(&self.lease.runtime);
        let active_native = record::native_snapshot(&self.native);
        let retirement_begin = self.recorder.clock.now().ok();
        let started = Instant::now();
        self.input.controller.close();
        self.lease.runtime.close();
        let mut cleanup_error = None;
        while !self.lease.runtime.retirement().wait(Duration::from_millis(20)) {
            if Instant::now() >= self.next_drain {
                if let Err(error) = self.recorder.drain(&self.native) {
                    cleanup_error.get_or_insert(error);
                }
                self.next_drain = Instant::now() + Duration::from_millis(100);
            }
        }
        self.lease.retired = true;
        self.input.retired.store(true, Ordering::Release);
        let retirement_end = self.recorder.clock.now().ok();
        let retained_windows = window_live(self.input.editor_hwnd) && window_live(self.input.output_hwnd);
        for _ in 0..=self.input.config.trace_capacity / 4096 {
            self.recorder.drain(&self.native)?;
            if self.native.diagnostics_snapshot().queued == 0 {
                break;
            }
        }
        self.recorder
            .snapshot(&self.lease.runtime, &self.native, &self.input.observations)?;
        self.recorder.resources(&self.input.config)?;
        let output = self.input.output.clone();
        let destroyed = on_ui(&self.input.editor, move || {
            output.destroy().map_err(|error| error.to_string())
        });
        // Tauri queues native destruction after this UI callback returns. Observe the actual
        // HWND disappearing; successful request dispatch alone is not destruction evidence.
        let output_destroyed =
            destroyed.is_ok() && wait_for_window_destruction(self.input.output_hwnd, Duration::from_secs(5));
        let diagnostics = self.native.diagnostics_snapshot();
        let complete = result.is_ok() && cleanup_error.is_none() && retained_windows && output_destroyed;
        self.recorder.finish(&self.input.config, json!({"functional_completed":complete,"process_id":std::process::id(),
            "error":result.as_ref().err().map(ToString::to_string),"cleanup_error":cleanup_error.as_ref().map(ToString::to_string),
            "output_destroy_error":destroyed.as_ref().err().map(ToString::to_string),
            "scenario":self.input.config.scenario,"admission":self.admission,"window":self.input.window_metadata,
            "geometry":self.preview.geometry,"active_runtime":active,"active_native":active_native,
            "final_runtime":record::runtime_snapshot(&self.lease.runtime),"final_native":record::native_snapshot(&self.native),
            "diagnostics":diagnostics,"fixture_report_loss":self.input.observations.report_loss.load(Ordering::Relaxed),
            "retirement":{"begin_qpc":retirement_begin,"end_qpc":retirement_end,"elapsed_seconds":started.elapsed().as_secs_f64(),
                "actual_retirement":true,"both_hwnds_retained_until_retirement":retained_windows,"output_destroyed_after_retirement":output_destroyed},
            "qualification_budgets_evaluated":false,"physical_display_qualification":false}))?;
        result?;
        if let Some(error) = cleanup_error {
            return Err(error);
        }
        destroyed?;
        if !retained_windows || !output_destroyed {
            return Err(Error::other("native window lifetime verification failed"));
        }
        Ok(())
    }
}

fn main_tokens(native: &D3d11Control) -> [Option<CompletionToken>; 2] {
    let outputs = native.output_snapshots();
    [scene::MAIN_OUTPUT, scene::SPOUT_OUTPUT].map(|id| {
        outputs
            .iter()
            .find(|output| output.id == scene::output(id))
            .and_then(|output| output.last_accepted)
    })
}

fn settle_output_client(input: &mut Input, stop: &NativePresentationStop) -> io::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(3);
    let target = tauri::PhysicalSize::new(input.output_extent.width(), input.output_extent.height());
    let expected_scale = input.window_metadata["selected_monitor"]["scale"].as_f64();
    input.window_metadata["client_before_settle"] = input.window_metadata["client"].clone();
    let mut attempts = 0;
    loop {
        if stop.is_requested() {
            return Err(Error::other("output sizing interrupted by host exit"));
        }
        attempts += 1;
        let output = input.output.clone();
        on_ui(&input.editor, move || {
            output.set_size(target).map_err(|error| error.to_string())
        })?;
        // The monitor move can queue WM_DPICHANGED after preparation. Let the UI process it,
        // then observe the real client in a separate dispatch instead of trusting set_size.
        if stop.wait_timeout(Duration::from_millis(100)) {
            return Err(Error::other("output sizing interrupted by host exit"));
        }
        let output = input.output.clone();
        let (client, scale, position) = on_ui(&input.editor, move || {
            Ok((
                output.inner_size().map_err(|error| error.to_string())?,
                output.scale_factor().map_err(|error| error.to_string())?,
                output.outer_position().map_err(|error| error.to_string())?,
            ))
        })?;
        input.window_metadata["client"] = json!([client.width, client.height]);
        input.window_metadata["scale"] = json!(scale);
        input.window_metadata["position"] = json!([position.x, position.y]);
        input.window_metadata["client_settle_attempts"] = json!(attempts);
        if client == target && expected_scale.is_none_or(|expected| (scale - expected).abs() < 0.0001) {
            input.window_metadata["client_extent_verified"] = json!(true);
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(Error::other(format!(
                "output client did not settle at {}x{}: observed {}x{} scale={scale}",
                target.width, target.height, client.width, client.height
            )));
        }
    }
}

fn on_ui<T: Send + 'static>(
    editor: &tauri::WebviewWindow,
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> io::Result<T> {
    let (result, done) = mpsc::sync_channel(1);
    editor
        .run_on_main_thread(move || {
            let _ = result.send(work());
        })
        .map_err(Error::other)?;
    done.recv_timeout(Duration::from_secs(5))
        .map_err(Error::other)?
        .map_err(Error::other)
}

fn window_live(hwnd: isize) -> bool {
    // SAFETY: inspection only; the task retains the corresponding host-owned window.
    unsafe { IsWindow(Some(HWND(hwnd as *mut _))).as_bool() }
}

fn wait_for_window_destruction(hwnd: isize, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while window_live(hwnd) {
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    true
}
