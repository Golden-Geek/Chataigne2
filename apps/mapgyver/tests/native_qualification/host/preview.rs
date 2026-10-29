//! Revisioned fixture geometry and installation acknowledgment, outside UI/engine callbacks.

use std::{
    collections::HashMap,
    io::{self, Error},
    sync::Arc,
};

use golden_video::Extent2D;
use golden_video::native::D3d11Control;
use golden_video::preview::{HostViewport, PreviewReceiver};
use golden_video::qualification::protocol::{
    QUALIFICATION_COMMAND_EVENT, QUALIFICATION_LAYOUT_EVENT, QualificationAction, QualificationCommand,
    QualificationLayout, QualificationPhase, QualificationReport, QualificationView,
};
use golden_video::runtime::{
    CompletionToken, GenerationInfo, LatePolicy, OutcomeKind, PlanActivation, RequestId, VideoRuntime,
};
use serde_json::{Value, json};
use tauri::Emitter;

use super::super::scene;
use super::{
    Observations,
    record::{Recorder, viewport},
};

pub struct Preview {
    pub receiver: PreviewReceiver,
    pub latest: Option<QualificationReport>,
    pub geometry: Option<Value>,
    observations: Arc<Observations>,
    sources: [String; 2],
    output: Extent2D,
    backing: Extent2D,
    listening: bool,
    disconnected: bool,
    layout_revision: u32,
    scale: Option<f64>,
    published: Option<u64>,
    pending: HashMap<RequestId, u64>,
    installed: Option<(u64, GenerationInfo, Option<CompletionToken>)>,
}

impl Preview {
    pub fn new(
        receiver: PreviewReceiver,
        observations: Arc<Observations>,
        sources: [String; 2],
        output: Extent2D,
        backing: Extent2D,
    ) -> Self {
        Self {
            receiver,
            observations,
            sources,
            output,
            backing,
            latest: None,
            geometry: None,
            listening: false,
            disconnected: false,
            layout_revision: 0,
            scale: None,
            published: None,
            pending: HashMap::new(),
            installed: None,
        }
    }

    pub fn service(
        &mut self,
        editor: &tauri::WebviewWindow,
        runtime: &mut VideoRuntime,
        native: &D3d11Control,
        recorder: &mut Recorder,
    ) -> io::Result<()> {
        let reports = self.observations.reports.lock().unwrap().drain(..).collect::<Vec<_>>();
        let mut ready_report = false;
        for report in reports {
            recorder.write(json!({"kind":"fixture", "qpc":recorder.clock.now()?, "report":report}))?;
            if report.phase == QualificationPhase::Failed {
                return Err(Error::other(
                    report.error.unwrap_or_else(|| "held UI fixture failed".into()),
                ));
            }
            self.listening |= report.phase == QualificationPhase::Listening;
            self.disconnected |= report.phase == QualificationPhase::Disconnected;
            if self
                .latest
                .as_ref()
                .is_none_or(|previous| report.sequence > previous.sequence)
            {
                ready_report = report.phase == QualificationPhase::Ready;
                self.latest = Some(report);
            }
        }
        let host = *self.observations.viewport.lock().unwrap();
        if self.listening
            && !self.disconnected
            && let Some(host) = host
            && self.scale != Some(host.css_scale)
        {
            if !host.css_scale.is_finite() || !(0.05..=64.).contains(&host.css_scale) {
                return Err(Error::other("authenticated CSS scale is invalid"));
            }
            self.layout_revision = self
                .layout_revision
                .checked_add(1)
                .ok_or_else(|| Error::other("fixture layout identity exhausted"))?;
            let layout = QualificationLayout {
                revision: self.layout_revision,
                views: std::array::from_fn(|index| QualificationView {
                    source: scene::SOURCE_KEYS[index].into(),
                    css_width: f64::from(scene::PREVIEW_PIXELS[index][0]) / host.css_scale,
                    css_height: f64::from(scene::PREVIEW_PIXELS[index][1]) / host.css_scale,
                }),
            };
            editor.emit(QUALIFICATION_LAYOUT_EVENT, &layout).map_err(Error::other)?;
            recorder.write(
                json!({"kind":"host_layout", "qpc":recorder.clock.now()?, "layout":layout,
                "requested_physical":scene::PREVIEW_PIXELS, "host":viewport(Some(host))}),
            )?;
            self.scale = Some(host.css_scale);
        }
        let snapshot = self.receiver.snapshot();
        if self.published != Some(snapshot.generation) {
            let epoch = runtime.read_time().map_err(Error::other)?.epoch();
            let request = runtime
                .submit(
                    runtime.project_epoch(),
                    scene::plan(&self.sources, self.output, self.backing, &snapshot),
                    PlanActivation::NextFrame(epoch),
                    LatePolicy::Activate,
                )
                .map_err(Error::other)?;
            self.pending.insert(request, snapshot.generation);
            self.published = Some(snapshot.generation);
            self.installed = None;
        }
        let mut outcomes = Vec::new();
        runtime.drain_outcomes(&mut outcomes, std::num::NonZeroUsize::new(16).unwrap());
        for outcome in outcomes {
            let layout = self.pending.remove(&outcome.request);
            match outcome.result {
                OutcomeKind::Installed { generation, .. } => {
                    if let Some(layout) = layout
                        && layout == snapshot.generation
                    {
                        self.installed = Some((layout, generation, None));
                    }
                }
                OutcomeKind::Rejected(failure) => return Err(Error::other(failure.to_string())),
                _ => {}
            }
        }
        if let Some((layout, generation, first_frame)) = &mut self.installed
            && let Some(frame) = runtime.snapshot().last_frame
            && frame.identity == generation.identity
        {
            let first = first_frame.get_or_insert(frame.completion);
            let accepted = native
                .output_snapshots()
                .iter()
                .find(|output| output.id == scene::output(scene::EDITOR_OUTPUT))
                .and_then(|output| output.last_accepted)
                .is_some_and(|accepted| accepted.device == first.device && accepted.serial >= first.serial);
            if accepted {
                self.receiver.mark_installed(*layout);
            }
        }
        if ready_report && self.ready() {
            self.geometry =
                Some(self.verify_geometry(host.ok_or_else(|| Error::other("ready fixture lacks host metrics"))?)?);
        }
        Ok(())
    }

    pub fn ready(&self) -> bool {
        self.receiver.snapshot().views.len() == 2
            && self.latest.as_ref().is_some_and(|report| {
                report.phase == QualificationPhase::Ready
                    && report.installed
                    && report.visible_views == 2
                    && report.retirement_masks == 0
                    && report
                        .layout
                        .as_ref()
                        .is_some_and(|layout| layout.revision == self.layout_revision)
            })
    }

    pub fn command(&self, editor: &tauri::WebviewWindow, id: u32, action: QualificationAction) -> io::Result<()> {
        editor
            .emit(QUALIFICATION_COMMAND_EVENT, QualificationCommand { id, action })
            .map_err(Error::other)
    }

    pub fn acknowledged(&self, id: u32, phase: QualificationPhase) -> bool {
        self.latest.as_ref().is_some_and(|report| {
            report.command_id == Some(id) && report.phase == phase && report.installed && report.retirement_masks == 0
        })
    }

    fn verify_geometry(&self, host: HostViewport) -> io::Result<Value> {
        let snapshot = self.receiver.snapshot();
        if snapshot.views.len() != 2 {
            return Err(Error::other("held preview did not resolve two physical regions"));
        }
        let mut views = Vec::new();
        for (index, view) in snapshot.views.iter().enumerate() {
            let width = f64::from(view.destination.width) * f64::from(host.client_width);
            let height = f64::from(view.destination.height) * f64::from(host.client_height);
            if view.source != scene::SOURCE_KEYS[index]
                || (width - f64::from(scene::PREVIEW_PIXELS[index][0])).abs() > 1.
                || (height - f64::from(scene::PREVIEW_PIXELS[index][1])).abs() > 1.
                || view.source_rect.x.abs() > 0.0001
                || view.source_rect.y.abs() > 0.0001
                || (view.source_rect.width - 1.).abs() > 0.0001
                || (view.source_rect.height - 1.).abs() > 0.0001
            {
                return Err(Error::other("preview physical size/crop differs from the fixed scene"));
            }
            views.push(json!({"source":view.source, "physical_size":[width,height],
                "destination":[view.destination.x,view.destination.y,view.destination.width,view.destination.height],
                "source_rect":[view.source_rect.x,view.source_rect.y,view.source_rect.width,view.source_rect.height]}));
        }
        Ok(
            json!({"layout_revision":self.layout_revision, "generation":snapshot.generation,"host":viewport(Some(host)),
            "views":views,"tolerance_physical_pixels":1,"full_physical_dpi_qualification":false}),
        )
    }
}
