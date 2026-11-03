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
    for id in crate::media::folders::entries(engine)? {
        let node = engine.nodes.get(id).ok_or("media catalog changed")?;
        if let AppNode::MediaFolder(folder) = node {
            guards.push(folder.guard());
        }
        let AppNode::Media(media) = node else {
            continue;
        };
        if media.kind() != MediaKind::Matrix {
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
        if playback.launch_id.is_none() && playback.layers.is_empty() {
            continue;
        }
        if matrices.len() == 128 {
            return Err("logical Matrix scope capacity exceeded".into());
        }
        guards.push(root.guard());
        guards.push(control.application_guard());
        let columns = crate::matrix::capture_application_columns(engine, root_id, control, columns_left)?;
        columns_left -= columns.len();
        let cursors = playback
            .desired_column
            .zip(playback.launch_id)
            .map(|(column, scope)| DemandCursor {
                scope,
                column,
                reason: DemandReason::Preparation,
            })
            .into_iter()
            .collect::<Vec<_>>();
        let configuration = crate::matrix::configuration::index(
            engine,
            crate::media::protocol::MediaId(media.node_data().meta.uuid),
            control,
        )?;
        let holds = playback
            .layers
            .iter()
            .flat_map(|request| {
                let cell = request
                    .column
                    .and_then(|column| configuration.cell_apps.get(&(request.layer, column)));
                // Disabled destinations retain their previous Layer request, including a
                // Column-origin control. Individual triggers never activate Column controls.
                let column = request
                    .column
                    .filter(|_| !request.individual)
                    .and_then(|column| configuration.column_apps.get(&column));
                cell.into_iter().chain(column).copied()
            })
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .map(|application| DemandHold {
                application,
                reason: DemandReason::Preparation,
            })
            .collect::<Vec<_>>();
        matrices.push((columns, cursors, holds));
    }
    let inputs = matrices
        .iter()
        .map(|(columns, cursors, holds)| MatrixDemand {
            columns,
            cursors,
            holds,
        })
        .collect::<Vec<_>>();
    let mut demand = resolve(&[], &inputs, DemandLimits::default())?;
    for entry in &mut demand {
        if let Some(node) = super::control_node(engine, entry.application) {
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
