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

#[derive(Clone, PartialEq, Eq)]
struct Key {
    revision: u64,
    launch: Option<uuid::Uuid>,
    clips: Vec<(uuid::Uuid, bool)>,
}
#[derive(Clone, Default)]
pub(crate) struct CapturedApplications {
    pub demand: Vec<ApplicationDemand>,
    pub references: Vec<ApplicationId>,
}
#[derive(Default)]
pub(crate) struct Applications {
    clips: HashMap<uuid::Uuid, Vec<ApplicationId>>,
    cached: Option<(Key, CapturedApplications)>,
}
impl Applications {
    pub fn retain(&mut self, frame: &Frame) {
        self.clips
            .retain(|id, _| frame.clips.iter().any(|clip| clip.clip.id == *id));
    }
    pub fn capture(
        &mut self,
        index: &RenderIndex,
        frame: &Frame,
        playback: &MatrixPlaybackSnapshot,
        revision: u64,
    ) -> Result<CapturedApplications, String> {
        let key = Key {
            revision,
            launch: playback.launch_id,
            clips: frame
                .clips
                .iter()
                .map(|entry| (entry.clip.id, entry.to > 0.0))
                .collect(),
        };
        if let Some((previous, captured)) = &self.cached
            && *previous == key
        {
            return Ok(captured.clone());
        }
        self.retain(frame);
        let mut cursors = Vec::new();
        let mut holds = Vec::new();
        if let (Some(column), Some(scope)) = (playback.desired_column, playback.launch_id) {
            cursors.push(DemandCursor {
                scope,
                column,
                reason: DemandReason::Preparation,
            });
        }
        for entry in &frame.clips {
            let reason = if entry.to > 0.0 {
                DemandReason::Active
            } else {
                DemandReason::Outgoing
            };
            cursors.push(DemandCursor {
                scope: entry.clip.id,
                column: entry.clip.column,
                reason,
            });
            let references = self.clips.entry(entry.clip.id).or_default();
            if let Some(column) = index.columns.iter().find(|column| column.id == entry.clip.column) {
                references.extend(column.bindings.iter().map(|binding| binding.application));
                references.sort_by_key(|id| id.0.0);
                references.dedup();
                if references.len() > DemandLimits::default().applications {
                    return Err("Matrix clip app-control retention capacity exceeded".into());
                }
            }
            holds.extend(references.iter().map(|application| DemandHold {
                application: *application,
                reason,
            }));
        }
        let demand = resolve(
            &[],
            &[MatrixDemand {
                columns: &index.columns,
                cursors: &cursors,
                holds: &holds,
            }],
            DemandLimits::default(),
        )?;
        let mut references = holds.iter().map(|hold| hold.application).collect::<Vec<_>>();
        references.sort_by_key(|id| id.0.0);
        references.dedup();
        let captured = CapturedApplications { demand, references };
        self.cached = Some((key, captured.clone()));
        Ok(captured)
    }
}
