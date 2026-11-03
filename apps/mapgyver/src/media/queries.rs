//! Immutable app queries share the authored DTO and result with all operation callers.
use super::protocol::*;
use crate::app::AppEngine;
use serde::Deserialize;
use serde_json::Value;

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
        MediaEdit::InspectMediaPreview { query } => {
            super::preview_selection::inspect(engine, query).map(|page| MediaEditResult::MediaPreview { page })
        }
        MediaEdit::InspectChannels { query } => {
            super::catalog::channels(engine, query).map(|page| MediaEditResult::Channels { page })
        }
        _ => return None,
    };
    Some(result.and_then(|result| serde_json::to_value(result).map_err(|error| error.to_string())))
}
