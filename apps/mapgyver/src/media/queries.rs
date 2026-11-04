//! Immutable app queries share the authored DTO and result with all operation callers.
use super::protocol::*;
use crate::app::AppEngine;
use serde::Deserialize;
use serde_json::Value;

pub(super) fn inspect_channel(
    engine: &AppEngine,
    reference: ChannelReference,
) -> Result<Option<ChannelSnapshot>, String> {
    if engine.node_id_by_uuid(reference.media.0).is_none() || engine.node_id_by_uuid(reference.channel.0).is_none() {
        return Ok(None);
    }
    super::authoring::channel(engine, reference).map(Some)
}

pub(crate) fn query_operation(engine: &AppEngine, operation: &str, payload: &Value) -> Option<Result<Value, String>> {
    if operation != MEDIA_OPERATION {
        return None;
    }
    let edit = match MediaEdit::deserialize(payload) {
        Ok(edit) => edit,
        Err(error) => return Some(Err(error.to_string())),
    };
    let edit = match edit {
        MediaEdit::ExecuteCommand { command } => match super::commands::operation(engine, command) {
            Ok(edit) => edit,
            Err(error) => return Some(Err(error)),
        },
        edit => edit,
    };
    let result = match edit {
        MediaEdit::InspectPictureFile { reference } => {
            super::picture_file::inspect(engine, reference).map(|binding| MediaEditResult::PictureFile { binding })
        }
        MediaEdit::InspectChannel { reference } => {
            inspect_channel(engine, reference).map(|snapshot| MediaEditResult::Channel { snapshot })
        }
        MediaEdit::InspectManager => {
            super::catalog::inspect(engine).map(|snapshot| MediaEditResult::Manager { snapshot })
        }
        MediaEdit::InspectChoices { query } => {
            super::catalog::choices(engine, query).map(|page| MediaEditResult::Choices { page })
        }
        MediaEdit::InspectCommandCatalog => Ok(MediaEditResult::CommandCatalog {
            commands: super::commands::catalog(),
        }),
        MediaEdit::InspectMediaPreview { query } => {
            super::preview_selection::inspect(engine, query).map(|page| MediaEditResult::MediaPreview { page })
        }
        MediaEdit::InspectCompositionEditor { query } => {
            super::composition::query::inspect(engine, query).map(|page| MediaEditResult::CompositionEditor { page })
        }
        MediaEdit::InspectChannels { query } => {
            super::catalog::channels(engine, query).map(|page| MediaEditResult::Channels { page })
        }
        _ => return None,
    };
    Some(result.and_then(|result| serde_json::to_value(result).map_err(|error| error.to_string())))
}
