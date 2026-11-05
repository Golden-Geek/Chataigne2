//! Bounded direct geometry edits; never load the Matrix or enumerate a Layer's Cells per sample.
use super::protocol::LayerId;
use crate::{
    app::{AppEngine, AppNode},
    media::{
        protocol::{MediaEditResult, MediaId, MediaKind},
        spatial::protocol::RectGeometryEdit,
    },
};
use golden_core::node::Node;

pub(super) fn layer(
    engine: &mut AppEngine,
    media: MediaId,
    layer: LayerId,
    edit: RectGeometryEdit,
) -> Result<MediaEditResult, String> {
    let id = engine.node_id_by_uuid(layer.0).ok_or("Layer is missing")?;
    let Some(AppNode::MatrixElement(node)) = engine.nodes.get(id) else {
        return Err("geometry target is not a Layer".into());
    };
    let mut settings = node.layer_settings()?.ok_or("geometry target is not a Layer")?;
    let mut next = node.node_data().parent;
    let mut owned = false;
    for _ in 0..64 {
        let Some(parent) = next.and_then(|id| engine.nodes.get(id)) else {
            break;
        };
        if let AppNode::Media(parent) = parent {
            owned = parent.kind() == MediaKind::Matrix && parent.node_data().meta.uuid == media.0;
            break;
        }
        next = parent.node_data().parent;
    }
    if !owned {
        return Err("Layer belongs to another Matrix".into());
    }
    let rect = edit.apply()?;
    settings.position = rect.position;
    settings.size = rect.size;
    settings.validate()?;
    engine.edits.push(node.spatial_edit(settings));
    Ok(MediaEditResult::MatrixLayerGeometryChanged { media, layer, settings })
}
