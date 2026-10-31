//! One production runtime and two host-owned HWNDs, retained through actual GPU retirement.
use std::{
    collections::HashSet,
    io::{self, Error},
    num::NonZeroUsize,
    time::{Duration, Instant},
};

use golden_core::app::{NativeOutputBounds, NativeOutputWindow};
use golden_io::WorkerNotifier;
use golden_video::native::{
    AdapterId, D3d11Config, D3d11Control, InputLimits, OutputBinding, SourceSnapshot, SourceState, WindowTarget,
    query_process_resources,
};
use golden_video::plan::{ExternalSource, RenderOperation};
use golden_video::preview::PreviewController;
use golden_video::runtime::{
    LatePolicy, OutcomeKind, PlanActivation, PlanSubmission, RequestId, RuntimeState, VideoRuntime,
};
use golden_video::{Extent2D, TextureId};
use windows::Win32::{Foundation::HWND, UI::WindowsAndMessaging::IsWindow};

use super::super::scene;
use super::contract::StableCounters;

pub struct Owner {
    pub runtime: VideoRuntime,
    pub native: D3d11Control,
    windows: Vec<NativeOutputWindow>,
    pending: HashSet<RequestId>,
    pub superseded: u64,
    pub installed: u64,
    retired: bool,
}

pub fn plan(names: &[String; 2], source: Extent2D, edit: u32) -> io::Result<PlanSubmission> {
    let (_, receiver) = PreviewController::new(Default::default(), None).map_err(Error::other)?;
    let mut plan = scene::plan(
        names,
        Extent2D::new(1920, 1080).unwrap(),
        Extent2D::new(1280, 720).unwrap(),
        &receiver.snapshot(),
    );
    let external = &mut plan.plan.external_textures[0];
    external.descriptor = scene::source_descriptor(source);
    let Some(ExternalSource::Spout(binding)) = &mut external.source else {
        unreachable!()
    };
    binding.descriptor = external.descriptor;
    // This finite renderer test uses two fixed native viewports. DOM/session ownership is
    // qualified separately; both viewports must remain active throughout these lifecycle cycles.
    plan.plan.passes[1].operation = plan.plan.passes[0].operation.clone();
    for (index, pass) in plan.plan.passes.iter_mut().enumerate() {
        if let RenderOperation::Regions { regions, visibility } = &mut pass.operation {
            *visibility = None;
            if index == 1 {
                regions[0].destination.width = if edit.is_multiple_of(2) { 0.5 } else { 0.49 };
            }
        }
    }
    Ok(plan)
}

impl Owner {
    pub fn new(adapter: AdapterId, output_name: &str, ceiling: u64) -> io::Result<Self> {
        let extent = Extent2D::new(1920, 1080).unwrap();
        let editor = Extent2D::new(1280, 720).unwrap();
        let windows = vec![
            NativeOutputWindow::new("Golden lifecycle output", NativeOutputBounds::new(2560, 0, 1920, 1080)?)?,
            NativeOutputWindow::new(
                "Golden lifecycle native preview",
                NativeOutputBounds::new(0, 0, 1280, 720)?,
            )?,
        ];
        // SAFETY: this owner retains both HWNDs, including failed-run cleanup, until the
        // production runtime's actual retirement handle completes. No other renderer attaches.
        let targets = unsafe {
            [
                WindowTarget::from_raw(windows[0].raw_handle()),
                WindowTarget::from_raw(windows[1].raw_handle()),
            ]
        };
        let runtime = VideoRuntime::spawn_d3d11(
            scene::config(ceiling, WorkerNotifier::new(|| {})),
            D3d11Config {
                adapter,
                diagnostics: None,
                inputs: InputLimits {
                    max_sources: 4,
                    max_storage_bytes: scene::input_bytes(extent) * 2,
                },
                outputs: vec![
                    OutputBinding::Window {
                        id: scene::output(scene::MAIN_OUTPUT),
                        window: targets[0].map_err(Error::other)?,
                        extent,
                    },
                    OutputBinding::Spout {
                        id: scene::output(scene::SPOUT_OUTPUT),
                        name: output_name.into(),
                        descriptor: scene::source_descriptor(extent),
                    },
                    OutputBinding::Window {
                        id: scene::output(scene::EDITOR_OUTPUT),
                        window: targets[1].map_err(Error::other)?,
                        extent: editor,
                    },
                ],
            },
        )
        .map_err(Error::other)?;
        let native = runtime
            .d3d11_control()
            .ok_or_else(|| Error::other("missing native control"))?;
        Ok(Self {
            runtime,
            native,
            windows,
            pending: HashSet::new(),
            superseded: 0,
            installed: 0,
            retired: false,
        })
    }

    pub fn service(&mut self) -> io::Result<()> {
        let mut outcomes = Vec::new();
        self.runtime
            .drain_outcomes(&mut outcomes, NonZeroUsize::new(64).unwrap());
        for outcome in outcomes {
            if !self.pending.remove(&outcome.request) {
                return Err(Error::other("duplicate or foreign lifecycle completion"));
            }
            match outcome.result {
                OutcomeKind::Installed { .. } => self.installed += 1,
                OutcomeKind::Superseded => self.superseded += 1,
                OutcomeKind::ProjectChanged(_) => {}
                other => return Err(Error::other(format!("lifecycle request failed: {other:?}"))),
            }
        }
        if matches!(
            self.runtime.snapshot().state,
            RuntimeState::Faulted | RuntimeState::Stopped
        ) {
            return Err(Error::other(format!(
                "lifecycle runtime stopped: {:?}",
                self.runtime.snapshot().failure
            )));
        }
        Ok(())
    }

    pub fn wait(&mut self, label: &str, mut ready: impl FnMut(&Self) -> bool) -> io::Result<()> {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            self.service()?;
            if ready(self) {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(Error::other(format!("lifecycle deadline: {label}")));
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    pub fn settle(&mut self, duration: Duration) -> io::Result<()> {
        let end = Instant::now() + duration;
        while Instant::now() < end {
            self.service()?;
            std::thread::sleep(Duration::from_millis(5));
        }
        self.wait("pending/prepared/retiring generations drained", |s| {
            s.pending.is_empty()
                && s.runtime
                    .snapshot()
                    .resources
                    .is_some_and(|r| r.prepared_generations == 0 && r.retiring_generations == 0)
        })
    }

    pub fn publish(&mut self, value: PlanSubmission) -> io::Result<()> {
        self.service()?;
        let request = self
            .runtime
            .submit(
                self.runtime.project_epoch(),
                value,
                PlanActivation::NextFrame(self.runtime.read_time().map_err(Error::other)?.epoch()),
                LatePolicy::Activate,
            )
            .map_err(Error::other)?;
        self.pending.insert(request);
        Ok(())
    }

    pub fn install(&mut self, value: PlanSubmission) -> io::Result<()> {
        let installed = self.installed;
        self.publish(value)?;
        self.wait("installed plan and terminal requests", |s| {
            s.pending.is_empty() && s.installed > installed
        })
    }

    pub fn replace_project(&mut self) -> io::Result<()> {
        self.settle(Duration::ZERO)?;
        let (request, _) = self.runtime.replace_project().map_err(Error::other)?;
        self.pending.insert(request);
        self.wait("project acknowledgment after resource retirement", |s| {
            s.pending.is_empty()
                && s.runtime
                    .snapshot()
                    .resources
                    .is_some_and(|r| r.active.is_none() && r.retiring_generations == 0)
        })
    }

    pub fn source(&self, texture: u64) -> Option<SourceSnapshot> {
        self.native
            .sources()
            .into_iter()
            .find(|s| s.texture == TextureId::new(texture).unwrap() && s.state == SourceState::Streaming)
    }

    pub fn streaming(&mut self, extent: Extent2D) -> io::Result<()> {
        self.wait("both source copies and all output APIs", |s| {
            let sources = s.native.sources();
            let copied = |id| {
                sources.iter().any(|v| {
                    v.texture == TextureId::new(id).unwrap()
                        && v.state == SourceState::Streaming
                        && v.renderer_copies > 0
                        && (id != 1 || v.observed_descriptor.is_some_and(|d| d.extent == extent))
                })
            };
            copied(1)
                && copied(2)
                && s.native.output_snapshots().len() == 3
                && s.native.output_snapshots().iter().all(|v| v.last_accepted.is_some())
        })
    }

    pub fn counters(&self) -> io::Result<StableCounters> {
        let sources = self.native.sources();
        if sources.len() != 2
            || sources
                .iter()
                .any(|source| source.state != SourceState::Streaming || source.renderer_copies == 0)
        {
            return Err(Error::other(
                "stable lifecycle checkpoint lacks two active copied sources",
            ));
        }
        let process = query_process_resources(std::process::id()).map_err(Error::other)?;
        let resources = self
            .runtime
            .snapshot()
            .resources
            .ok_or_else(|| Error::other("resource snapshot missing"))?;
        let memory = process
            .memory
            .ok_or_else(|| Error::other("private-memory query unavailable"))?;
        Ok(StableCounters {
            handles: process
                .handle_count
                .ok_or_else(|| Error::other("handle query unavailable"))?,
            private_bytes: memory.private_bytes,
            working_set_bytes: memory.working_set_bytes,
            lifetime_peak_working_set_bytes: memory.lifetime_peak_working_set_bytes,
            reserved_texture_bytes: resources.bytes_used,
            active_plan_bytes: resources
                .active
                .ok_or_else(|| Error::other("active resource generation missing"))?
                .bytes,
            retiring_generations: resources.retiring_generations,
            prepared_generations: resources.prepared_generations,
            input_bindings: sources.len(),
        })
    }

    pub fn retire(&mut self) -> io::Result<Duration> {
        let began = Instant::now();
        self.runtime.close();
        let retirement = self.runtime.retirement();
        // An external owned-process watchdog bounds a stuck run. A timeout must never drop
        // HWNDs still referenced by GPU work or fabricate a retirement certificate.
        while !retirement.wait(Duration::from_millis(20)) {}
        self.retired = true;
        if self
            .windows
            .iter()
            .any(|w| !unsafe { IsWindow(Some(HWND(w.raw_handle() as *mut _))).as_bool() })
        {
            return Err(Error::other("HWND disappeared before actual renderer retirement"));
        }
        for window in self.windows.drain(..) {
            window.close()?;
        }
        let elapsed = began.elapsed();
        if elapsed > Duration::from_secs(5) {
            return Err(Error::other("renderer retirement exceeded five seconds"));
        }
        Ok(elapsed)
    }
}

impl Drop for Owner {
    fn drop(&mut self) {
        if !self.retired {
            self.runtime.close();
            while !self.runtime.retirement().wait(Duration::from_millis(20)) {}
        }
    }
}
