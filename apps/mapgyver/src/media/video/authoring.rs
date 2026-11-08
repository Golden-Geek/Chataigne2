use super::protocol::*;
use crate::{
    app::{AppEngine, AppNode},
    media::protocol::*,
};
use golden_core::{
    edit::Edit,
    parameter::{ParamValue, ParameterEventBehaviour},
};

fn owner(engine: &AppEngine, media: MediaId) -> Result<&crate::media::MediaNode, String> {
    let (id, kind) = super::super::authoring::media(engine, media)?;
    if kind != MediaKind::Video {
        return Err("Video transport belongs to Video media, not a mirrored Reference".into());
    }
    let Some(AppNode::Media(node)) = engine.nodes.get(id) else {
        unreachable!()
    };
    Ok(node)
}
pub(crate) fn inspect(engine: &AppEngine, media: MediaId) -> Result<VideoTransportSnapshot, String> {
    let node = owner(engine, media)?;
    let mut snapshot = node.video_control().unwrap().snapshot(media)?;
    snapshot.settings = node.video_settings()?;
    Ok(snapshot)
}
fn channel_file(engine: &AppEngine, reference: ChannelReference) -> Result<AssetLocator, String> {
    owner(engine, reference.media)?;
    let channel = super::super::authoring::channel(engine, reference)?;
    let ChannelSource::Video { file: Some(file) } = channel.definition.source else {
        return Err("Selected Video channel has no File".into());
    };
    Ok(file)
}
pub(crate) fn inspect_frame(
    engine: &AppEngine,
    reference: ChannelReference,
) -> Result<Option<VideoFrameState>, String> {
    let node = owner(engine, reference.media)?;
    let channel = super::super::authoring::channel(engine, reference)?;
    let ChannelSource::Video { file: Some(file) } = channel.definition.source else {
        return Ok(None);
    };
    node.video_control().unwrap().frame(reference, &file)
}
pub(crate) fn file(engine: &mut AppEngine, reference: ChannelReference, file: &str) -> Result<(), String> {
    super::super::assets::parse_file(file)?;
    owner(engine, reference.media)?;
    let id = super::super::authoring::channel_id(engine, reference)?;
    let Some(AppNode::MediaChannel(channel)) = engine.nodes.get(id) else {
        unreachable!()
    };
    engine.edits.push(Edit::SetParam {
        node: channel.file_parameter(),
        value: ParamValue::File(file.into()),
        behaviour: ParameterEventBehaviour::Append,
    });
    Ok(())
}
pub(crate) fn apply(engine: &mut AppEngine, media: MediaId, edit: VideoEdit) -> Result<MediaEditResult, String> {
    let node = owner(engine, media)?;
    let mut candidate = node.video_settings()?;
    let repeated_seek = matches!(&edit, VideoEdit::Seek { position_ms } if *position_ms == candidate.position_ms);
    match &edit {
        VideoEdit::Seek { position_ms } => candidate.position_ms = *position_ms,
        VideoEdit::Rate { rate } => candidate.rate = *rate,
        _ => {}
    }
    candidate.validate()?;
    let control = node.video_control().unwrap();
    let mut values = node.video_parameter_values(&edit);
    if let VideoEdit::Step { channel, direction } = edit {
        let file = channel_file(engine, ChannelReference { media, channel })?;
        let position_ms = control.step_position(channel, &file, direction)?;
        candidate.position_ms = position_ms;
        candidate.validate()?;
        values.extend(node.video_parameter_values(&VideoEdit::Seek { position_ms }));
    }
    engine.edits.push(Edit::SetParamsAtomic {
        values,
        behaviour: ParameterEventBehaviour::Append,
    });
    // Parameter handles and runtime callbacks settle before the reply or a worker request.
    // Merely draining edits leaves the owner's handle cache on its previous settings.
    engine
        .settle_edit_notifications(16)
        .map_err(|error| error.to_string())?;
    if matches!(edit, VideoEdit::Play) {
        control.request_play()?;
    }
    if repeated_seek {
        control.configure(inspect(engine, media)?.settings, true)?;
    }
    Ok(MediaEditResult::Video {
        snapshot: inspect(engine, media)?,
    })
}
