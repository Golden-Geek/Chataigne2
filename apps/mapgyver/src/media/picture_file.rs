//! File authoring edits only the selected Picture channel's own typed Parameter.
use super::protocol::*;
use crate::app::{AppEngine, AppNode};
use golden_core::{
    edit::Edit,
    node::Node,
    parameter::{ParamValue, ParameterEventBehaviour},
};

fn channel(engine: &AppEngine, reference: ChannelReference) -> Result<&super::ChannelNode, String> {
    let snapshot = super::authoring::channel(engine, reference)?;
    if snapshot.definition.media_kind() != MediaKind::Picture {
        return Err("File editing requires a Picture channel".into());
    }
    let id = super::authoring::channel_id(engine, reference)?;
    let AppNode::MediaChannel(channel) = engine.nodes.get(id).unwrap() else {
        unreachable!()
    };
    Ok(channel)
}

pub(super) fn inspect(engine: &AppEngine, reference: ChannelReference) -> Result<Option<PictureFileBinding>, String> {
    if super::queries::inspect_channel(engine, reference)?.is_none() {
        return Ok(None);
    }
    let channel = channel(engine, reference)?;
    let node = channel.file_parameter();
    let parameter = engine.nodes.get(node).ok_or("Picture File Parameter is missing")?;
    Ok(Some(PictureFileBinding {
        reference,
        parameter: FileParameterAddress {
            node,
            uuid: parameter.node_data().meta.uuid,
        },
    }))
}

pub(super) fn configure(engine: &mut AppEngine, reference: ChannelReference, file: &str) -> Result<(), String> {
    super::assets::parse_file(file)?;
    let node = channel(engine, reference)?.file_parameter();
    engine.edits.push(Edit::SetParam {
        node,
        value: ParamValue::File(file.to_owned()),
        behaviour: ParameterEventBehaviour::Append,
    });
    Ok(())
}

pub(super) fn reload(engine: &mut AppEngine, reference: ChannelReference) -> Result<(), String> {
    channel(engine, reference)?;
    let id = super::authoring::channel_id(engine, reference)?;
    let Some(AppNode::MediaChannel(channel)) = engine.nodes.get_mut(id) else {
        unreachable!("validated Picture channel")
    };
    channel.reload_picture();
    Ok(())
}
