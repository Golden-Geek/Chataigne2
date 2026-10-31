//! App-owned channel preparation/publication. Construct and drive this owner on a host or
//! preparation worker, never the engine/UI thread. Golden owns frame cadence and GPU cleanup.

mod derived;
mod feedback;
mod plan;
mod sources;
pub use crate::preview::layout::PreviewLayout;

use super::capture::CapturedChannels;
use super::protocol::{ChannelReference, ChannelSource};
use super::resolution::ChannelResolution;
use crate::screens::protocol::ScreenId;
pub use crate::screens::rendering::ScreenRoute;
use golden_video::plan::PlanLimits;
use golden_video::runtime::{LatePolicy, PlanActivation, RequestId, RuntimeOutcome, RuntimeSnapshot, VideoRuntime};
use golden_video::{OutputId, TextureId};
pub use sources::SourceObservations;
use std::collections::HashMap;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug)]
pub struct ChannelRoute {
    pub output: OutputId,
    pub channel: ChannelReference,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreparedChannel {
    pub reference: ChannelReference,
    pub resolution: ChannelResolution,
    pub target: TextureId,
    pub source: Option<TextureId>,
}

#[derive(Clone)]
struct Entry {
    target: TextureId,
    source: Option<(TextureId, ChannelSource)>,
    scratch: Option<TextureId>,
}

pub struct ChannelRenderer {
    applications: Option<crate::applications::runtime::PlanRetainer>,
    feedback: std::sync::Mutex<feedback::Feedback>,
    runtime: VideoRuntime,
    limits: PlanLimits,
    project: Option<uuid::Uuid>,
    next_texture: u64,
    entries: HashMap<ChannelReference, Entry>,
    screen_entries: HashMap<ScreenId, TextureId>,
    desired: Option<Desired>,
    observations: Option<SourceObservations>,
    channels: Vec<PreparedChannel>,
    latest_request: Option<RequestId>,
    pictures: Option<super::picture::Pictures>,
}

#[derive(Clone)]
struct Desired {
    capture: CapturedChannels,
    routes: Vec<ChannelRoute>,
    preview: Option<PreviewLayout>,
    screens: Vec<ScreenRoute>,
}

impl ChannelRenderer {
    pub fn new(runtime: VideoRuntime, limits: PlanLimits) -> Self {
        Self {
            applications: None,
            feedback: Default::default(),
            runtime,
            limits,
            project: None,
            // Textures 1..=3 belong to this owner's window compositor.
            next_texture: 3,
            entries: HashMap::new(),
            screen_entries: HashMap::new(),
            desired: None,
            observations: None,
            channels: Vec::new(),
            latest_request: None,
            pictures: None,
        }
    }

    /// Bind process final-use ownership to this renderer's actual queued/live/retiring plans.
    pub fn bind_applications(&mut self, retainer: crate::applications::runtime::PlanRetainer) {
        self.applications = Some(retainer);
    }
    /// Latest captured preparation/clip demand; installation and final-use proof remain separate.
    pub fn application_demand(&self) -> Vec<crate::applications::demand::ApplicationDemand> {
        self.desired
            .as_ref()
            .map(|desired| desired.capture.application_demand())
            .unwrap_or_default()
    }

    /// Configure one bounded picture worker from a host/preparation owner. No file work occurs
    /// on this caller. Pass the owner's notifier for completion/coarse missing-file retry wakes.
    pub fn enable_pictures(
        &mut self,
        root: &std::path::Path,
        limits: golden_assets::PreparationLimits,
        notifier: Option<golden_io::WorkerNotifier>,
    ) -> Result<(), String> {
        if self.pictures.is_some() {
            return Err("picture preparation is already configured".into());
        }
        self.pictures = Some(super::picture::Pictures::new(root, limits, notifier)?);
        Ok(())
    }
    pub fn retry_pictures(&mut self) -> Result<(), String> {
        self.pictures
            .as_mut()
            .ok_or("picture preparation is not configured")?
            .retry()
    }
    /// File-change/relink recovery for one source; healthy prepared sources remain admitted.
    pub fn retry_picture(&mut self, asset: super::protocol::AssetId) -> Result<(), String> {
        self.pictures
            .as_mut()
            .ok_or("picture preparation is not configured")?
            .retry_asset(asset)
    }
    pub fn relocate_pictures(&mut self, root: &std::path::Path) -> Result<(), String> {
        self.pictures
            .as_mut()
            .ok_or("picture preparation is not configured")?
            .relocate(root)
    }
    pub fn picture_retirement(&self) -> Option<golden_assets::AssetRetirement> {
        self.pictures.as_ref().map(|pictures| pictures.retirement())
    }
    pub fn next_picture_retry_deadline(&self) -> Option<std::time::Instant> {
        self.pictures.as_ref().and_then(|pictures| pictures.next_retry())
    }

    /// Rebase relative source preparation after save-as/project relocation on the owner worker.
    pub fn set_picture_root(&mut self, root: &std::path::Path) -> Result<bool, String> {
        self.pictures
            .as_mut()
            .map(|pictures| pictures.set_root(root))
            .transpose()
            .map(|changed| changed.unwrap_or(false))
    }

    /// Captures metadata only. Its project/device stamps are rechecked before use.
    pub fn source_observations(&self) -> SourceObservations {
        sources::capture(&self.runtime)
    }
    /// Read-only observation of the video runtime's shared monotonic authority.
    pub fn read_time(&self) -> Result<golden_time::TimeSnapshot, golden_time::TimeError> {
        self.runtime.read_time()
    }

    pub fn publish(&mut self, capture: CapturedChannels, routes: Vec<ChannelRoute>) -> Result<RequestId, String> {
        self.publish_targets(capture, routes, None)
    }

    /// Publishes native output routes and an optional clipped editor-window composition.
    pub fn publish_targets(
        &mut self,
        capture: CapturedChannels,
        routes: Vec<ChannelRoute>,
        preview: Option<PreviewLayout>,
    ) -> Result<RequestId, String> {
        self.publish_screen_targets(capture, routes, Vec::new(), preview)
    }

    /// Publish media routes and composed Screen textures through one shared renderer generation.
    /// Callers capture the union of demanded channel references, so shared sources render once.
    pub fn publish_screen_targets(
        &mut self,
        capture: CapturedChannels,
        routes: Vec<ChannelRoute>,
        screens: Vec<ScreenRoute>,
        preview: Option<PreviewLayout>,
    ) -> Result<RequestId, String> {
        if !capture.is_current() {
            return Err("captured channels are stale".into());
        }
        self.validate_screens(&capture, &screens)?;
        if self.project.is_some_and(|project| project != capture.project) {
            self.runtime.replace_project().map_err(|error| error.to_string())?;
            self.entries.clear();
            self.screen_entries.clear();
            self.observations = None;
            self.desired = None;
            self.latest_request = None;
            self.channels.clear();
            *self.feedback.lock().expect("Matrix feedback lock poisoned") = Default::default();
        }
        self.project = Some(capture.project);
        let observations = self.source_observations();
        self.publish_observed_targets(capture, routes, observations, preview, screens)
    }

    pub fn publish_observed(
        &mut self,
        capture: CapturedChannels,
        routes: Vec<ChannelRoute>,
        observations: SourceObservations,
    ) -> Result<RequestId, String> {
        self.publish_observed_targets(capture, routes, observations, None, Vec::new())
    }

    fn publish_observed_targets(
        &mut self,
        capture: CapturedChannels,
        routes: Vec<ChannelRoute>,
        observations: SourceObservations,
        preview: Option<PreviewLayout>,
        screens: Vec<ScreenRoute>,
    ) -> Result<RequestId, String> {
        if !capture.is_current() || self.project.is_some_and(|project| project != capture.project) {
            return Err("captured channels belong to a stale project/revision".into());
        }
        self.validate_screens(&capture, &screens)?;
        let current = self.runtime.snapshot();
        if observations.project != current.project
            || (observations.device.is_some()
                && observations.device != current.resources.map(|resources| resources.last_submitted.device))
            || !observations.equivalent(&self.source_observations())
        {
            return Err("source observations belong to a stale renderer project/device".into());
        }
        if let Some(pictures) = &mut self.pictures {
            let mut demanded = routes.iter().map(|route| route.channel).collect::<Vec<_>>();
            if let Some(preview) = &preview {
                demanded.extend(preview.references()?);
            }
            for screen in &screens {
                demanded.extend(screen.screen.references());
            }
            demanded = derived::ordered(
                &capture,
                &demanded,
                self.limits.max_dependencies,
                self.limits.max_dependencies,
                self.limits.max_passes.saturating_sub(capture.channels.len()),
            )?
            .0;
            pictures.sync(&capture, &demanded, self.limits.max_external_textures)?;
        }
        let mut prepared = plan::build(
            &capture,
            plan::Targets {
                routes: &routes,
                preview: preview.as_ref(),
                screens: &screens,
            },
            plan::Previous {
                channels: &self.entries,
                screens: &self.screen_entries,
            },
            &mut self.next_texture,
            self.limits,
            &observations,
            self.pictures.as_ref(),
        )?;
        let applications = capture.plan_applications();
        if !applications.is_empty() {
            let retainer = self
                .applications
                .as_ref()
                .ok_or("Matrix applications require a bound lifecycle owner before rendering")?;
            if retainer.project() != capture.project {
                return Err("Matrix application owner belongs to another project".into());
            }
            let lifetime = golden_video::runtime::PlanLifetime::default();
            retainer.retain(&applications, lifetime.observer())?;
            prepared.submission.lifetime = Some(lifetime);
        }
        let activation =
            PlanActivation::NextFrame(self.runtime.read_time().map_err(|error| error.to_string())?.epoch());
        let request = self
            .runtime
            .submit(current.project, prepared.submission, activation, LatePolicy::Activate)
            .map_err(|error| error.to_string())?;
        self.feedback
            .lock()
            .expect("Matrix feedback lock poisoned")
            .publish(request, &capture, &prepared.channels);
        self.project = Some(capture.project);
        self.entries = prepared.entries;
        self.screen_entries = prepared.screens;
        self.channels = prepared.channels;
        self.desired = Some(Desired {
            capture,
            routes,
            preview,
            screens,
        });
        self.observations = Some(observations);
        self.latest_request = Some(request);
        Ok(request)
    }

    fn validate_screens(&self, capture: &CapturedChannels, screens: &[ScreenRoute]) -> Result<(), String> {
        if screens.len() > self.limits.max_passes
            || screens
                .iter()
                .any(|route| route.screen.project != capture.project || !route.screen.is_current())
        {
            return Err("Screen routes exceed limits or belong to a stale/different project".into());
        }
        Ok(())
    }

    /// Invoke on a worker notification. Frame counters are excluded from observation equality;
    /// unchanged metadata produces no plan construction/publication.
    pub fn refresh(&mut self) -> Result<Option<RequestId>, String> {
        let time = self.runtime.read_time().map_err(|error| error.to_string())?;
        let transitions_finished = self
            .desired
            .as_ref()
            .is_some_and(|desired| desired.capture.transitions_finished(time));
        let pictures_changed = self
            .pictures
            .as_mut()
            .map(|pictures| pictures.service())
            .transpose()?
            .unwrap_or(false);
        let observations = self.source_observations();
        if !pictures_changed
            && !transitions_finished
            && self
                .observations
                .as_ref()
                .is_some_and(|previous| previous.equivalent(&observations))
        {
            return Ok(None);
        }
        let Some(mut desired) = self.desired.clone() else {
            return Ok(None);
        };
        if transitions_finished {
            desired.capture.settle_transitions(time)?;
        }
        self.publish_observed_targets(
            desired.capture,
            desired.routes,
            observations,
            desired.preview,
            desired.screens,
        )
        .map(Some)
    }

    /// Latest admitted desired metadata; reliable Installed outcomes identify the live generation.
    pub fn channels(&self) -> &[PreparedChannel] {
        &self.channels
    }
    pub fn latest_request(&self) -> Option<RequestId> {
        self.latest_request
    }
    pub fn snapshot(&self) -> RuntimeSnapshot {
        self.runtime.snapshot()
    }
    pub fn drain_outcomes(&self, outcomes: &mut Vec<RuntimeOutcome>, budget: std::num::NonZeroUsize) -> usize {
        let start = outcomes.len();
        let drained = self.runtime.drain_outcomes(outcomes, budget);
        let mut feedback = self.feedback.lock().expect("Matrix feedback lock poisoned");
        for outcome in &outcomes[start..] {
            feedback.complete(outcome);
        }
        drained
    }
    pub fn null_control(&self) -> Option<golden_video::runtime::NullControl> {
        self.runtime.null_control()
    }
    pub fn retirement(&self) -> golden_video::runtime::RuntimeRetirement {
        self.runtime.retirement()
    }
    pub fn close(&mut self) {
        self.runtime.close();
        self.desired = None;
        *self.feedback.lock().expect("Matrix feedback lock poisoned") = Default::default();
        if let Some(pictures) = self.pictures.take() {
            pictures.close();
        }
    }

    #[cfg(all(windows, feature = "native-video"))]
    pub fn native_control(&self) -> Option<golden_video::native::D3d11Control> {
        self.runtime.d3d11_control()
    }
}
