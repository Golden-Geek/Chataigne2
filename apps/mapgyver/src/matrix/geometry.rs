//! Bounded direct geometry edits; never load the Matrix or enumerate a Layer's Cells per sample.
use super::protocol::{CellGeometryEdit, CellId, LayerId};
use crate::{
    app::{AppEngine, AppNode},
    media::{
        protocol::{MediaEditResult, MediaId, MediaKind},
        spatial::protocol::RectGeometryEdit,
    },
};
use golden_core::node::{Node, NodeId};

fn owns(engine: &AppEngine, media: MediaId, mut next: Option<NodeId>) -> bool {
    for _ in 0..64 {
        let Some(parent) = next.and_then(|id| engine.nodes.get(id)) else {
            return false;
        };
        if let AppNode::Media(parent) = parent {
            return parent.kind() == MediaKind::Matrix && parent.node_data().meta.uuid == media.0;
        }
        next = parent.node_data().parent;
    }
    false
}

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
    if !owns(engine, media, node.node_data().parent) {
        return Err("Layer belongs to another Matrix".into());
    }
    let rect = edit.apply()?;
    settings.position = rect.position;
    settings.size = rect.size;
    settings.validate()?;
    engine.edits.push(node.spatial_edit(settings));
    Ok(MediaEditResult::MatrixLayerGeometryChanged { media, layer, settings })
}

pub(super) fn cell(
    engine: &mut AppEngine,
    media: MediaId,
    cell: CellId,
    edit: CellGeometryEdit,
) -> Result<MediaEditResult, String> {
    let id = engine.node_id_by_uuid(cell.0).ok_or("Cell is missing")?;
    let Some(AppNode::MatrixCell(node)) = engine.nodes.get(id) else {
        return Err("geometry target is not a Cell".into());
    };
    if !owns(engine, media, node.node_data().parent) {
        return Err("Cell belongs to another Matrix".into());
    }
    let mut settings = node.settings(engine);
    let original = settings;
    match edit {
        CellGeometryEdit::Translate { origin, start, point } => {
            settings.position = crate::media::spatial::translate_position(origin, start, point)?;
        }
        CellGeometryEdit::Resize {
            origin,
            handle,
            start,
            point,
        } => {
            let current = match settings.size {
                Some(size) => size,
                None => {
                    let source = node
                        .selection(engine)?
                        .map(|source| source.output(engine))
                        .transpose()?
                        .flatten()
                        .ok_or("source dimensions are unavailable for resize")?;
                    let extent = crate::media::resolution::channel_extent(engine, source)?;
                    if extent != origin.size {
                        return Err("source dimensions changed; start a new resize gesture".into());
                    }
                    extent
                }
            };
            let rect = crate::media::spatial::protocol::RectGeometryEdit {
                origin,
                mode: crate::media::spatial::protocol::RectGeometryMode::Resize { handle },
                start,
                point,
            }
            .apply()?;
            // A click without a resolved geometry change must not freeze natural source sizing.
            if settings.size.is_some() || rect.size != current || rect.position != settings.position {
                settings.size = Some(rect.size);
            }
            settings.position = rect.position;
        }
    }
    settings.validate()?;
    if settings == original {
        return Ok(MediaEditResult::MatrixCellGeometryChanged { media, cell, settings });
    }
    let root = super::playback::root(engine, media)?;
    let Some(AppNode::MediaChannel(root)) = engine.nodes.get(root) else {
        unreachable!()
    };
    node.bind_runtime(
        root.matrix_playback()
            .ok_or("Matrix runtime owner is missing")?
            .binding(),
    );
    let edits = node.configuration_edits(settings);
    for edit in edits {
        engine.edits.push(edit);
    }
    Ok(MediaEditResult::MatrixCellGeometryChanged { media, cell, settings })
}
