//! Bounded request-to-Matrix correlation; reliable outcomes authorize installation metadata.
use super::{ChannelRenderer, PreparedChannel};
use crate::{
    matrix::protocol::*,
    media::{capture::CapturedChannels, protocol::ChannelReference},
};
use golden_video::runtime::{OutcomeKind, PlanIdentity, RequestId, RuntimeOutcome, RuntimeSnapshot, RuntimeState};
use std::collections::HashMap;

#[derive(Clone)]
struct Binding {
    reference: ChannelReference,
    stamp: MatrixPlanStamp,
    availability: crate::media::resolution::ChannelAvailability,
}
struct Publication {
    request: RequestId,
    project: uuid::Uuid,
    bindings: Vec<Binding>,
}

#[derive(Default)]
pub(super) struct Feedback {
    // Admission credits also bound all undrained outcomes, hence this metadata map.
    pending: HashMap<RequestId, Publication>,
    installed: Option<(PlanIdentity, Publication)>,
    rejected: Option<(RequestId, String)>,
    latest: Option<RequestId>,
}
impl Feedback {
    pub fn publish(&mut self, request: RequestId, capture: &CapturedChannels, channels: &[PreparedChannel]) {
        let bindings = capture
            .channels
            .iter()
            .filter_map(|channel| {
                let matrix = channel.matrix.as_ref()?;
                let prepared = channels
                    .iter()
                    .find(|prepared| prepared.reference == channel.reference)?;
                Some(Binding {
                    reference: channel.reference,
                    stamp: matrix.observation.clone(),
                    availability: prepared.resolution.availability,
                })
            })
            .collect();
        self.pending.insert(
            request,
            Publication {
                request,
                project: capture.project,
                bindings,
            },
        );
        self.rejected = None;
        self.latest = Some(request);
    }
    pub fn complete(&mut self, outcome: &RuntimeOutcome) {
        let Some(publication) = self.pending.remove(&outcome.request) else {
            return;
        };
        match &outcome.result {
            OutcomeKind::Installed { generation, .. } => self.installed = Some((generation.identity, publication)),
            OutcomeKind::Rejected(failure) if Some(outcome.request) == self.latest => {
                self.rejected = Some((outcome.request, failure.to_string()))
            }
            _ => {}
        }
    }
    fn installed(&self, snapshot: &RuntimeSnapshot, project: uuid::Uuid) -> Option<&Publication> {
        let (identity, publication) = self.installed.as_ref()?;
        let resources = snapshot.resources?;
        (publication.project == project
            && snapshot.state == RuntimeState::Running
            && resources.initialized
            && !resources.device_lost
            && resources
                .active
                .is_some_and(|active| active.identity == *identity && identity.project == snapshot.project))
        .then_some(publication)
    }
}
impl ChannelRenderer {
    /// Worker-side sparse metadata only. Frame counters never enter coalesced UI event equality.
    pub fn matrix_statuses(&self) -> Vec<(ChannelReference, MatrixRenderStatus)> {
        self.matrix_statuses_from(self.snapshot())
    }
    fn matrix_statuses_from(&self, snapshot: RuntimeSnapshot) -> Vec<(ChannelReference, MatrixRenderStatus)> {
        let Some(desired) = &self.desired else {
            return vec![];
        };
        let feedback = self.feedback.lock().expect("Matrix feedback lock poisoned");
        let installed = feedback.installed(&snapshot, desired.capture.project);
        let error = feedback
            .rejected
            .as_ref()
            .filter(|(request, _)| Some(*request) == self.latest_request)
            .map(|(_, error)| error.clone())
            .or_else(|| {
                (snapshot.state == RuntimeState::Faulted)
                    .then(|| snapshot.failure.as_ref().map(ToString::to_string))
                    .flatten()
            });
        let stage = if !desired.capture.is_current() {
            MatrixPreparationStage::Changed
        } else if matches!(snapshot.state, RuntimeState::Stopping | RuntimeState::Stopped) {
            MatrixPreparationStage::Stopped
        } else if snapshot.resources.is_some_and(|resources| resources.device_lost) {
            MatrixPreparationStage::Recovering
        } else if snapshot.state == RuntimeState::Faulted {
            MatrixPreparationStage::Faulted
        } else if error.is_some() {
            MatrixPreparationStage::Rejected
        } else if installed.is_some_and(|publication| Some(publication.request) == self.latest_request) {
            MatrixPreparationStage::Installed
        } else if snapshot.prepared {
            MatrixPreparationStage::Prepared
        } else if snapshot.preparing {
            MatrixPreparationStage::Preparing
        } else {
            MatrixPreparationStage::Published
        };
        self.channels
            .iter()
            .filter_map(|channel| {
                let captured = desired
                    .capture
                    .channels
                    .iter()
                    .find(|captured| captured.reference == channel.reference)?;
                let matrix = captured.matrix.as_ref()?;
                let installed_stamp = installed.and_then(|publication| {
                    publication.bindings.iter().find(|binding| {
                        binding.reference == channel.reference
                            && binding.availability == crate::media::resolution::ChannelAvailability::Ready
                    })
                });
                let submitted = installed_stamp.filter(|_| {
                    snapshot.last_frame.is_some_and(|frame| {
                        snapshot
                            .resources
                            .and_then(|resources| resources.active)
                            .is_some_and(|active| {
                                frame.identity == active.identity
                                    && frame.generation == active.generation
                                    && frame.completion.device == active.device
                                    && frame.time.epoch() == snapshot.time.epoch()
                            })
                    })
                });
                Some((
                    channel.reference,
                    MatrixRenderStatus {
                        project: desired.capture.project,
                        desired: matrix.observation.clone(),
                        stage,
                        availability: channel.resolution.availability,
                        installed: installed_stamp.map(|binding| binding.stamp.clone()),
                        submitted: submitted.map(|binding| binding.stamp.clone()),
                        error: error.clone(),
                    },
                ))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests;
