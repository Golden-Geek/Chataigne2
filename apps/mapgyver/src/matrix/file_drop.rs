//! Native and workbench files use the same typed Matrix destinations.
use super::{document::Document, protocol::*};
use crate::{app::AppEngine, applications::protocol::ApplicationId, media::protocol::MediaEditResult};
use golden_assets::AbsoluteAssetPath;
use golden_core::node::Node;
use std::path::Path;

pub(super) fn apply(
    engine: &mut AppEngine,
    document: &Document,
    target: MatrixFileTarget,
    path: AbsoluteAssetPath,
) -> Result<MediaEditResult, String> {
    let executable = Path::new(path.as_str())
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"));
    match target {
        MatrixFileTarget::Cell { layer, column } if executable => {
            super::cell_applications::set_file(engine, document, layer, column, path.as_str().to_owned())
        }
        MatrixFileTarget::Cell { layer, column } => {
            super::cells::drop_media_file(engine, document, layer, Some(column), path)
        }
        MatrixFileTarget::NewColumn { layer } if !executable => {
            super::cells::drop_media_file(engine, document, layer, None, path)
        }
        MatrixFileTarget::NewColumn { .. } => Err("Drop the .exe file on a Column, Column Group or Cell".into()),
        MatrixFileTarget::ColumnItem { item } if executable => {
            let node = engine
                .nodes
                .get(document.column_item(item)?)
                .ok_or("Column item is missing")?;
            let id = ApplicationId(node.node_data().meta.uuid);
            crate::applications::set_file(engine, id, Some(path.as_str().to_owned()))?;
            Ok(MediaEditResult::Changed)
        }
        MatrixFileTarget::ColumnItem { .. } => Err("Drop an .exe file to set App Control".into()),
    }
}
