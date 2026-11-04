//! Channel File presentation forwards the actual catalog Parameter; paths have one authority.
use super::protocol::*;
use crate::app::{AppEngine, AppNode};
use golden_core::{
    edit::Edit,
    node::{Node, NodeUuid},
    parameter::{ParamValue, ParameterEventBehaviour},
};

pub(super) fn inspect(engine: &AppEngine, reference: ChannelReference) -> Result<Option<PictureFileBinding>, String> {
    let Some(channel) = super::queries::inspect_channel(engine, reference)? else {
        return Ok(None);
    };
    let ChannelSource::Picture { asset } = channel.definition.source else {
        return Err("File binding requires a Picture channel".into());
    };
    let parameter = if let Some(asset) = asset
        && engine.node_id_by_uuid(NodeUuid(asset.get())).is_some()
    {
        let id = super::assets::node_id(engine, asset)?;
        let Some(AppNode::MediaAsset(node)) = engine.nodes.get(id) else {
            unreachable!("validated asset");
        };
        let node_id = node.file_parameter();
        let parameter = engine.nodes.get(node_id).ok_or("asset File Parameter is missing")?;
        Some(FileParameterAddress {
            node: node_id,
            uuid: parameter.node_data().meta.uuid,
        })
    } else {
        None
    };
    Ok(Some(PictureFileBinding {
        reference,
        asset,
        parameter,
    }))
}

pub(super) fn configure(engine: &mut AppEngine, reference: ChannelReference, file: &str) -> Result<(), String> {
    let locator = super::assets::parse_file(file)?;
    let channel = super::authoring::channel(engine, reference)?;
    let ChannelSource::Picture { asset } = channel.definition.source else {
        return Err("File editing requires a Picture channel".into());
    };
    if let Some(asset) = asset
        && engine.node_id_by_uuid(NodeUuid(asset.get())).is_some()
    {
        let id = super::assets::node_id(engine, asset)?;
        let Some(AppNode::MediaAsset(node)) = engine.nodes.get(id) else {
            unreachable!("validated asset");
        };
        engine.edits.push(Edit::SetParam {
            node: node.file_parameter(),
            value: ParamValue::File(file.to_owned()),
            behaviour: ParameterEventBehaviour::Append,
        });
    } else if let Some(locator) = locator {
        // Initial assignment and recovery from a deleted asset use the same atomic import
        // boundary. A blank unbound control does not allocate an unused catalog entry.
        super::importing::picture(engine, locator, Some(reference), None)?;
    }
    Ok(())
}
