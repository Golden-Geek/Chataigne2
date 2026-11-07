//! Immutable inspections bypass edit sessions for UI, transport and script callers alike.
use super::protocol::*;
use crate::app::{AppEngine, AppNode};
use golden_core::node::Node;
use serde::Deserialize;
use serde_json::Value;

pub(crate) fn query_operation(engine: &AppEngine, operation: &str, payload: &Value) -> Option<Result<Value, String>> {
    if operation != SCREEN_OPERATION {
        return None;
    }
    let edit = match ScreenEdit::deserialize(payload) {
        Ok(edit) => edit,
        Err(error) => return Some(Err(error.to_string())),
    };
    match read(engine, &edit) {
        Ok(None) => None,
        Ok(Some(value)) => Some(serde_json::to_value(value).map_err(|error| error.to_string())),
        Err(error) => Some(Err(error)),
    }
}

pub(super) fn read(engine: &AppEngine, edit: &ScreenEdit) -> Result<Option<ScreenEditResult>, String> {
    Ok(Some(match edit {
        ScreenEdit::InspectEditor { query } => ScreenEditResult::Editor {
            page: Box::new(super::editor::inspect(engine, *query)?),
        },
        ScreenEdit::InspectRow { node } => ScreenEditResult::Row {
            snapshot: super::rows::inspect(engine, *node)?,
        },
        ScreenEdit::InspectManager => ScreenEditResult::Manager {
            snapshot: super::authoring::inspect(engine)?,
        },
        ScreenEdit::InspectSurface { reference } => {
            let id = super::authoring::surface_id(engine, *reference)?;
            let Some(AppNode::Surface(node)) = engine.nodes.get(id) else {
                unreachable!()
            };
            ScreenEditResult::Surface {
                snapshot: SurfaceSnapshot::new(*reference, node.node_data().meta.label.clone(), node.definition()),
            }
        }
        ScreenEdit::InspectScreen { screen } => {
            let id = super::authoring::screen_id(engine, *screen)?;
            let Some(AppNode::Screen(node)) = engine.nodes.get(id) else {
                unreachable!()
            };
            ScreenEditResult::ScreenDefinition {
                screen: *screen,
                definition: node.definition(),
            }
        }
        _ => return Ok(None),
    }))
}
