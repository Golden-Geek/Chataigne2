//! A direct Layer Stop fades its current mix; other scoped Stops retain Cut semantics.
use super::*;
use crate::applications::protocol::ApplicationId;

pub(crate) struct PreparedStop {
    pub layer: LayerId,
    pub applications: Vec<(ColumnId, ApplicationId)>,
}

pub(super) fn stop(engine: &mut AppEngine, media: MediaId, layer: LayerId) -> Result<MediaEditResult, String> {
    let root = super::super::playback::root(engine, media)?;
    let Some(AppNode::MediaChannel(channel)) = engine.nodes.get(root) else {
        unreachable!()
    };
    let control = channel.matrix_playback().ok_or("Matrix playback is missing")?.clone();
    let mut desired = control.snapshot();
    let index = super::super::configuration::index(engine, media, &control)?;
    if !index.layers.contains(&layer) {
        return Err("Layer belongs to another Matrix or is missing".into());
    }
    if control.layer_is_stopping(layer) {
        return Ok(MediaEditResult::MatrixPlayback { snapshot: desired });
    }
    let mut playing = HashSet::from([layer]);
    control.retain_playing_layers(&mut playing, &index);
    if playing.is_empty() {
        return Ok(MediaEditResult::MatrixPlayback { snapshot: desired });
    }
    let Some((column, individual)) = control.layer_stop_target(layer) else {
        return Ok(MediaEditResult::MatrixPlayback { snapshot: desired });
    };
    let transition = index.transition_for(layer, Some(column));
    let axis = super::super::application_axis::capture(
        engine,
        root,
        &control,
        crate::applications::demand::DemandLimits::default().columns,
    )?;
    let mut applications = control.layer_retained_applications(layer);
    applications.extend(
        axis.cell_apps
            .get(&(layer, column))
            .map(|&application| (column, application)),
    );
    if !individual {
        applications.extend(
            axis.column_applications(column)
                .map(|application| (column, application)),
        );
    }
    applications.sort_by_key(|(column, application)| (column.0.0, application.0.0));
    applications.dedup();
    let launch = uuid::Uuid::new_v4();
    let mut requests = desired.layers.to_vec();
    requests.retain(|request| request.layer != layer);
    requests.push(LayerPlaybackRequest {
        layer,
        column: None,
        launch_id: Some(launch),
        transition,
        individual,
    });
    if requests.len() > super::super::bindings::MAX_BINDINGS {
        return Err("Layer Stop exceeds4096 active Layer requests".into());
    }
    requests.sort_by_key(|request| request.layer.0.0);
    desired.layers = requests.into();
    desired.request_id = Some(launch);
    let videos = control.prepare_videos(engine, &desired, &index)?;
    let event = CustomEvent::from_latest_payload(MATRIX_PLAYBACK_EVENT, Some(root), &desired)
        .map_err(|error| error.to_string())?;
    engine.edits.push(Edit::EmitCustomEvent { event });
    control.stage_layer_stop(
        desired.clone(),
        engine.edits.completion(),
        videos,
        PreparedStop { layer, applications },
    );
    Ok(MediaEditResult::MatrixPlayback { snapshot: desired })
}
