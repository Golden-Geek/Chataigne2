use super::{projection::RenderIndex, protocol::*, timeline::Frame};
use crate::applications::{demand::*, protocol::ApplicationId};
use golden_core::node::{Node, NodeId};
use std::collections::HashMap;
use std::sync::Arc;

pub(crate) fn capture_application_columns(
    engine: &crate::app::AppEngine,
    root: NodeId,
    control: &super::playback::PlaybackControl,
    maximum: usize,
) -> Result<Arc<[DemandColumn]>, String> {
    if let Some(columns) = control.application_columns() {
        if columns.len() > maximum {
            return Err("logical Matrix aggregate column capacity exceeded".into());
        }
        return Ok(columns);
    }
    let owner = engine
        .nodes
        .get(root)
        .and_then(|node| node.node_data().parent)
        .and_then(|id| engine.nodes.get(id))
        .ok_or("Matrix media is missing")?;
    let document =
        super::document::Document::load(engine, crate::media::protocol::MediaId(owner.node_data().meta.uuid))?;
    if document.snapshot.columns.len() > maximum {
        return Err("logical Matrix column capacity exceeded".into());
    }
    for id in document.columns.values() {
        if let Some(crate::app::AppNode::MatrixColumn(node)) = engine.nodes.get(*id) {
            node.bind_runtime(control.binding());
        }
    }
    for (group, id) in &document.column_groups {
        let Some(crate::app::AppNode::MatrixColumnGroup(node)) = engine.nodes.get(*id) else {
            unreachable!()
        };
        node.bind_runtime(control.binding());
        if let Some(settings) =
            crate::applications::control_node(engine, crate::applications::protocol::ApplicationId(group.0))
        {
            super::cell_applications::bind(settings, control);
        }
    }
    let mut columns = document
        .snapshot
        .columns
        .iter()
        .map(|column| DemandColumn {
            id: column.id,
            bindings: Vec::new(),
        })
        .collect::<Vec<_>>();
    super::cell_applications::append(engine, &document, control, &mut columns)?;
    let columns: Arc<[DemandColumn]> = columns.into();
    control.cache_application_columns(columns.clone());
    Ok(columns)
}

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
        self.clips.keys().filter_map(|(_, layer)| *layer)
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
                references.extend(index.column_apps.get(&entry.clip.column).copied());
                references.sort_by_key(|id| id.0.0);
                references.dedup();
                holds.extend(references.iter().map(|application| DemandHold {
                    application: *application,
                    reason,
                }));
            }
            if let Some(layer) = layer {
                let references = self.clips.entry((entry.clip.id, Some(layer))).or_default();
                references.extend(index.cell_apps.get(&(layer, entry.clip.column)).copied());
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
                columns: if scope.is_none() { &index.columns } else { &[] },
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
