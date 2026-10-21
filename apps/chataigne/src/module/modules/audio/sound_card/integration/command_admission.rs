use super::*;

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
        return Err(command_error("Sound Card channel gain target is not an output channel"));
    }
    Ok(target)
}

pub(crate) fn authored_gain_change(
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

pub(crate) fn command_error(message: impl Into<String>) -> AudioError {
    AudioError::new(AudioErrorCategory::InvalidConfiguration, message)
}
