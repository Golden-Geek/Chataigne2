use super::*;

pub(super) fn validate_request_type(command_type: &str, request: &SoundCardCommandRequest) -> Result<(), AudioError> {
    let matches = matches!(
        (command_type, request),
        (
            SOUND_CARD_PLAY_FILE_COMMAND_NODE_TYPE,
            SoundCardCommandRequest::PlayFile { .. }
        ) | (
            SOUND_CARD_STOP_FILE_COMMAND_NODE_TYPE,
            SoundCardCommandRequest::StopFile { .. }
        ) | (
            SOUND_CARD_STOP_ALL_FILES_COMMAND_NODE_TYPE,
            SoundCardCommandRequest::StopAllFiles
        ) | (
            SOUND_CARD_SET_MASTER_VOLUME_COMMAND_NODE_TYPE,
            SoundCardCommandRequest::SetMasterVolume { .. }
        ) | (
            SOUND_CARD_SET_CHANNEL_VOLUME_COMMAND_NODE_TYPE,
            SoundCardCommandRequest::SetChannelVolume { .. }
        )
    );
    matches.then_some(()).ok_or_else(|| {
        command_error(format!(
            "Sound Card command payload does not match node type '{command_type}'"
        ))
    })
}

pub(super) fn resolve_output_channel(
    snapshot: &ProcessTreeSnapshot,
    module: NodeId,
    reference: &NodeReference,
) -> Result<golden_audio::AudioChannelId, AudioError> {
    let target = resolve_output_channel_node(snapshot, module, reference)?;
    let target_state = snapshot
        .node(target)
        .ok_or_else(|| command_error("Sound Card output channel disappeared"))?;
    Ok(golden_audio::AudioChannelId::from_uuid(target_state.uuid.0))
}

fn resolve_output_channel_node(
    snapshot: &ProcessTreeSnapshot,
    module: NodeId,
    reference: &NodeReference,
) -> Result<NodeId, AudioError> {
    let target = reference
        .cached_id()
        .filter(|target| snapshot.node(*target).is_some_and(|node| node.uuid == reference.uuid()))
        .or_else(|| snapshot.node_id_by_uuid(reference.uuid()))
        .ok_or_else(|| command_error(format!("Sound Card output channel {} is missing", reference.uuid().0)))?;
    let target_state = snapshot
        .node(target)
        .ok_or_else(|| command_error("Sound Card output channel disappeared"))?;
    if !matches!(target_state.param_value, Some(ParamValue::Float(_)))
        || target_state.parent != find_path(snapshot, module, OUTPUT_CHANNELS_PATH)
    {
        return Err(command_error(
            "Sound Card channel gain target is not an output channel",
        ));
    }
    Ok(target)
}

pub(super) fn authored_gain_change(
    snapshot: &ProcessTreeSnapshot,
    module: NodeId,
    request: &SoundCardCommandRequest,
) -> Result<Option<(NodeId, ParamValue)>, AudioError> {
    match request {
        SoundCardCommandRequest::SetMasterVolume { gain } => {
            let parameter = find_path(snapshot, module, "parameters/output/master_gain_db")
                .ok_or_else(|| command_error("Sound Card master-gain parameter is missing"))?;
            Ok(Some((parameter, ParamValue::Float(f64::from(gain.get())))))
        }
        SoundCardCommandRequest::SetChannelVolume { output_channel, gain } => {
            let output = resolve_output_channel_node(snapshot, module, output_channel)?;
            Ok(Some((output, ParamValue::Float(f64::from(gain.get())))))
        }
        _ => Ok(None),
    }
}

pub(super) fn route_path(direction: golden_audio::AudioDirection) -> &'static str {
    match direction {
        golden_audio::AudioDirection::Input => INPUT_ROUTES_PATH,
        golden_audio::AudioDirection::Output => OUTPUT_ROUTES_PATH,
    }
}

pub(super) fn find_route(
    snapshot: &ProcessTreeSnapshot,
    parent: NodeId,
    physical_channel: &str,
    channel_uuid: NodeUuid,
) -> Option<NodeId> {
    snapshot.child_ids_slice(parent).iter().copied().find(|route| {
        let physical = find_path(snapshot, *route, "physical_channel")
            .and_then(|node| snapshot.node(node))
            .and_then(|node| node.param_value.as_ref())
            .and_then(ParamValue::as_str);
        let channel = find_path(snapshot, *route, "channel")
            .and_then(|node| snapshot.node(node))
            .and_then(|node| node.param_value.as_ref())
            .and_then(|value| match value {
                ParamValue::Reference(reference) => Some(reference.uuid()),
                _ => None,
            });
        physical.as_deref() == Some(physical_channel) && channel == Some(channel_uuid)
    })
}

pub(super) fn command_error(message: impl Into<String>) -> AudioError {
    AudioError::new(AudioErrorCategory::InvalidConfiguration, message)
}
