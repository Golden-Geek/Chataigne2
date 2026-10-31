use super::{projection::RenderIndex, protocol::*, timeline::Frame};
use crate::applications::{demand::*, protocol::ApplicationId};
use std::collections::HashMap;

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
            let references = self.clips.entry(entry.clip.id).or_insert_with(|| {
                index
                    .columns
                    .iter()
                    .find(|column| column.id == entry.clip.column)
                    .map(|column| {
                        column
                            .definition
                            .applications
                            .iter()
                            .map(|entry| entry.application)
                            .collect()
                    })
                    .unwrap_or_default()
            });
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
