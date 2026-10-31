//! Committed column demand, independent of render consumers, cells, devices and video clocks.
use super::demand::*;
use crate::{
    app::{AppEngine, AppNode},
    media::protocol::MediaKind,
};
use golden_core::node::Node;
use golden_io::RevisionGuard;
use std::sync::Arc;

#[derive(Clone)]
pub struct LogicalApplicationCapture {
    pub project: uuid::Uuid,
    pub demand: Vec<ApplicationDemand>,
    pub(crate) guards: Arc<[RevisionGuard]>,
}
impl LogicalApplicationCapture {
    pub fn is_current(&self) -> bool {
        self.guards.iter().all(RevisionGuard::is_current)
    }
}

/// Capture after transaction settlement. This cannot launch, sample a frame or advance a transition.
pub fn capture_logical_applications(engine: &AppEngine) -> Result<LogicalApplicationCapture, String> {
    let id = crate::media::authoring::manager(engine)?;
    let Some(AppNode::MediaManager(manager)) = engine.nodes.get(id) else {
        unreachable!()
    };
    let revision = manager.application_revision.reader();
    let mut guards = vec![
        manager.project_guard(),
        manager.membership_guard(),
        revision.guard_at(revision.current()),
    ];
    let mut matrices = Vec::new();
    let mut columns_left = DemandLimits::default().columns;
    let mut next = manager.node_data().first_child;
    while let Some(id) = next {
        let node = engine.nodes.get(id).ok_or("media catalog changed")?;
        next = node.node_data().next_sibling;
        let AppNode::Media(media) = node else {
            continue;
        };
        if media.kind != MediaKind::Matrix {
            continue;
        }
        guards.push(media.guard());
        let root_id =
            crate::matrix::playback::root(engine, crate::media::protocol::MediaId(media.node_data().meta.uuid))?;
        let Some(AppNode::MediaChannel(root)) = engine.nodes.get(root_id) else {
            unreachable!()
        };
        let control = root.matrix_playback().ok_or("Matrix playback is missing")?;
        control.bind_application_parent(&manager.application_revision);
        let playback = control.snapshot();
        if !root.node_data().effective_enabled {
            continue;
        }
        let (Some(column), Some(scope)) = (playback.desired_column, playback.launch_id) else {
            continue;
        };
        if matrices.len() == 128 {
            return Err("logical Matrix scope capacity exceeded".into());
        }
        guards.push(root.guard());
        guards.push(control.application_guard());
        let columns = crate::matrix::capture_application_columns(engine, root_id, control, columns_left)?;
        columns_left -= columns.len();
        matrices.push((
            columns,
            [DemandCursor {
                scope,
                column,
                reason: DemandReason::Preparation,
            }],
        ));
    }
    let inputs = matrices
        .iter()
        .map(|(columns, cursors)| MatrixDemand {
            columns,
            cursors,
            holds: &[],
        })
        .collect::<Vec<_>>();
    let mut demand = resolve(&[], &inputs, DemandLimits::default())?;
    for entry in &mut demand {
        if let Some(AppNode::Application(node)) = engine
            .node_id_by_uuid(entry.application.0)
            .and_then(|id| engine.nodes.get(id))
        {
            guards.push(node.guard());
            entry.availability = node.availability();
        }
    }
    Ok(LogicalApplicationCapture {
        project: manager.instance,
        demand,
        guards: guards.into(),
    })
}
