use super::{projection::RenderIndex, protocol::*, timeline::Frame};
use crate::applications::{demand::*, protocol::ApplicationId};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone, Default)]
pub(crate) struct CapturedApplications {
    pub demand: Arc<[ApplicationDemand]>,
    pub references: Arc<[ApplicationId]>,
}
#[derive(Default)]
pub(crate) struct Applications {
    clips: HashMap<(uuid::Uuid, Option<LayerId>), Vec<ApplicationId>>,
    key: Option<(u64, u64)>,
    cached: HashMap<Option<LayerId>, CapturedApplications>,
}
impl Applications {
    pub fn layers(&self) -> impl Iterator<Item = LayerId> + '_ {
        self.clips
            .iter()
            .filter_map(|((_, layer), applications)| (!applications.is_empty()).then_some(*layer).flatten())
    }
    pub fn retain(&mut self, frame: &Frame) {
        self.clips.retain(|(id, layer), _| match layer {
            Some(layer) => frame.layer(*layer).clips.iter().any(|entry| entry.clip.id == *id),
            None => frame
                .clips
                .iter()
                .chain(frame.layers.values().flat_map(|frame| frame.clips.iter()))
                .any(|entry| entry.clip.id == *id && !entry.clip.individual),
        });
        self.cached.clear();
    }
    pub fn capture(
        &mut self,
        index: &RenderIndex,
        frame: &Frame,
        playback: &MatrixPlaybackSnapshot,
        revision: u64,
        scope: Option<LayerId>,
    ) -> Result<CapturedApplications, String> {
        let key = (revision, frame.generation);
        if self.key != Some(key) {
            self.retain(frame);
            self.key = Some(key);
        }
        if let Some(captured) = self.cached.get(&scope) {
            return Ok(captured.clone());
        }
        let mut layers = index
            .applications
            .cell_apps
            .keys()
            .map(|(layer, _)| *layer)
            .collect::<std::collections::HashSet<_>>();
        layers.extend(self.clips.keys().filter_map(|(_, layer)| *layer));
        layers.extend(frame.layers.keys().copied());
        let mut holds = Vec::new();
        let mut observe = |entry: &super::timeline::Contribution, layer: Option<LayerId>| {
            let reason = if entry.to > 0.0 {
                DemandReason::Active
            } else {
                DemandReason::Outgoing
            };
            if !entry.clip.individual {
                let references = self.clips.entry((entry.clip.id, None)).or_default();
                references.extend(index.applications.column_applications(entry.clip.column));
                references.sort_by_key(|id| id.0.0);
                references.dedup();
                holds.extend(references.iter().map(|application| DemandHold {
                    application: *application,
                    reason,
                }));
            }
            if let Some(layer) = layer {
                let references = self.clips.entry((entry.clip.id, Some(layer))).or_default();
                references.extend(index.applications.cell_apps.get(&(layer, entry.clip.column)).copied());
                references.sort_by_key(|id| id.0.0);
                references.dedup();
                holds.extend(references.iter().map(|application| DemandHold {
                    application: *application,
                    reason,
                }));
            }
        };
        if let Some(layer) = scope {
            for entry in &frame.layer(layer).clips {
                observe(entry, Some(layer));
            }
        } else {
            for entry in frame
                .clips
                .iter()
                .chain(frame.layers.values().flat_map(|frame| frame.clips.iter()))
                .filter(|entry| !entry.clip.individual)
            {
                observe(entry, None);
            }
            for layer in layers {
                for entry in &frame.layer(layer).clips {
                    observe(entry, Some(layer));
                }
            }
        }
        holds.sort_by_key(|hold| (hold.application.0.0, hold.reason));
        holds.dedup_by_key(|hold| (hold.application, hold.reason));
        let mut cursors = Vec::new();
        if scope.is_none() {
            if let (Some(column), Some(scope)) = (playback.desired_column, playback.launch_id) {
                cursors.push(DemandCursor {
                    scope,
                    column,
                    reason: DemandReason::Preparation,
                });
            }
            for entry in frame
                .clips
                .iter()
                .chain(frame.layers.values().flat_map(|frame| frame.clips.iter()))
                .filter(|entry| !entry.clip.individual)
            {
                cursors.push(DemandCursor {
                    scope: entry.clip.id,
                    column: entry.clip.column,
                    reason: if entry.to > 0.0 {
                        DemandReason::Active
                    } else {
                        DemandReason::Outgoing
                    },
                });
            }
            cursors.sort_by_key(|cursor| (cursor.scope, cursor.reason));
            cursors.dedup_by_key(|cursor| (cursor.scope, cursor.reason));
        }
        let demand = resolve(
            &[],
            &[MatrixDemand {
                columns: if scope.is_none() {
                    &index.applications.columns
                } else {
                    &[]
                },
                spans: if scope.is_none() {
                    &index.applications.spans
                } else {
                    &[]
                },
                cursors: &cursors,
                holds: &holds,
            }],
            DemandLimits::default(),
        )?;
        let mut references = holds.iter().map(|hold| hold.application).collect::<Vec<_>>();
        references.sort_by_key(|id| id.0.0);
        references.dedup();
        let captured = CapturedApplications {
            demand: demand.into(),
            references: references.into(),
        };
        self.cached.insert(scope, captured.clone());
        Ok(captured)
    }
}
