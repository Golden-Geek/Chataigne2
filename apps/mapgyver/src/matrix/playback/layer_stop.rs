//! Receipt-owned Layer fades and logical application holds. The existing Matrix
//! deadline notifier expires headless holds; GPU plan holds retain their own fences.
use super::*;
use crate::applications::protocol::ApplicationId;
use std::time::{Duration, Instant};

pub(super) struct LayerStop {
    id: uuid::Uuid,
    deadline: Instant,
    applications: Vec<(ColumnId, ApplicationId)>,
}

impl PlaybackControl {
    pub(crate) fn layer_stop_applications(&self) -> Vec<ApplicationId> {
        let state = self.0.state.lock().expect("Matrix playback lock poisoned");
        state
            .active_layer_stops
            .iter()
            .filter_map(|layer| state.layer_stops.get(layer))
            .flat_map(|stop| stop.applications.iter().map(|(_, application)| *application))
            .collect()
    }
    pub(in crate::matrix) fn layer_is_stopping(&self, layer: LayerId) -> bool {
        self.0
            .state
            .lock()
            .expect("Matrix playback lock poisoned")
            .layer_stops
            .contains_key(&layer)
    }
    pub(in crate::matrix) fn layer_stop_target(&self, layer: LayerId) -> Option<(ColumnId, bool)> {
        let state = self.0.state.lock().expect("Matrix playback lock poisoned");
        let request = state.current.layers.iter().find(|request| request.layer == layer);
        request
            .map(|request| request.column.map(|column| (column, request.individual)))
            .unwrap_or_else(|| state.current.desired_column.map(|column| (column, false)))
            .or_else(|| {
                state
                    .timeline
                    .latest_clip(layer, &state.cuts)
                    .map(|clip| (clip.column, clip.individual))
            })
            .or_else(|| state.videos.latest_clip(layer))
    }
    pub(in crate::matrix) fn layer_retained_applications(&self, layer: LayerId) -> Vec<(ColumnId, ApplicationId)> {
        self.0
            .state
            .lock()
            .expect("Matrix playback lock poisoned")
            .timeline
            .retained_applications(layer)
    }
    pub(in crate::matrix) fn stage_layer_stop(
        &self,
        desired: MatrixPlaybackSnapshot,
        receipt: EditCompletion,
        videos: Arc<super::super::video::Prepared>,
        stop: super::super::stops::PreparedStop,
    ) {
        self.stage_request(desired, receipt, None, Some(videos), Some(stop));
    }
}

impl State {
    pub(super) fn commit_layer_stop(
        &mut self,
        desired: &MatrixPlaybackSnapshot,
        stop: Option<super::super::stops::PreparedStop>,
        cut: Option<&super::super::stops::Scope>,
    ) {
        let targets = desired
            .layers
            .iter()
            .filter(|request| request.column.is_none())
            .filter_map(|request| request.launch_id.map(|id| (request.layer, (id, request.transition))))
            .collect::<std::collections::HashMap<_, _>>();
        self.layer_stops
            .retain(|layer, stop| targets.get(layer).is_some_and(|(id, _)| *id == stop.id));
        if let Some(cut) = cut {
            for (layer, stop) in &mut self.layer_stops {
                stop.applications.retain(|(column, _)| {
                    !cut.layers.contains(layer)
                        && !cut.columns.contains(column)
                        && !cut.cells.contains(&(*layer, *column))
                });
            }
        }
        if let Some(stop) = stop {
            let &(id, transition) = targets.get(&stop.layer).expect("Layer Stop has an empty target");
            self.layer_stops.insert(
                stop.layer,
                LayerStop {
                    id,
                    deadline: Instant::now() + Duration::from_millis(transition.duration_ms.into()),
                    applications: if transition.duration_ms == 0 {
                        Vec::new()
                    } else {
                        stop.applications
                    },
                },
            );
        }
        self.active_layer_stops.retain(|layer| {
            self.layer_stops
                .get(layer)
                .is_some_and(|stop| !stop.applications.is_empty())
        });
        self.active_layer_stops.extend(
            self.layer_stops
                .iter()
                .filter(|(_, stop)| !stop.applications.is_empty())
                .map(|(&layer, _)| layer),
        );
    }
    pub(super) fn layer_stop_deadline(&self) -> Option<Instant> {
        self.active_layer_stops
            .iter()
            .filter_map(|layer| self.layer_stops.get(layer))
            .map(|stop| stop.deadline)
            .min()
    }
    pub(super) fn finish_layer_stops(&mut self, headless_now: Option<Instant>) -> bool {
        let mut changed = false;
        self.active_layer_stops.retain(|layer| {
            let stop = self.layer_stops.get_mut(layer).expect("active Layer Stop");
            if self.timeline.stop_finished(*layer, stop.id) || headless_now.is_some_and(|now| now >= stop.deadline) {
                stop.applications.clear();
                changed = true;
                false
            } else {
                true
            }
        });
        changed
    }
}
