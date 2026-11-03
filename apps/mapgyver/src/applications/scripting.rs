//! App-owned named script calls project to the same typed intents as other clients.
use super::protocol::{ApplicationDefinition, ApplicationEdit, ApplicationId};
use crate::{matrix::protocol::*, media::protocol::*};
use golden_core::{node::NodeUuid, parameter::ParamValue};

pub(crate) const SCRIPT_TEMPLATE: &str = include_str!("script_template.js");
pub(crate) const METHODS: &[&str] = &[
    "setApplicationFile",
    "configureApplication",
    "inspectApplication",
    "inspectApplicationRuntime",
    "inspectApplicationCallbacks",
    "launchMatrixColumn",
    "launchMatrixCell",
    "unloadMatrix",
];

fn text(args: &[ParamValue], index: usize) -> Result<&str, String> {
    match args.get(index) {
        Some(ParamValue::Str(value)) => Ok(value),
        _ => Err(format!("argument {} must be a string", index + 1)),
    }
}
fn id(args: &[ParamValue], index: usize) -> Result<NodeUuid, String> {
    uuid::Uuid::parse_str(text(args, index)?)
        .map(NodeUuid)
        .map_err(|error| error.to_string())
}

pub(crate) fn operation(method: &str, args: &[ParamValue]) -> Result<MediaEdit, String> {
    let count = match method {
        "setApplicationFile" | "configureApplication" => 2,
        "inspectApplication" | "unloadMatrix" => 1,
        "inspectApplicationRuntime" | "inspectApplicationCallbacks" => 0,
        "launchMatrixColumn" => 3,
        "launchMatrixCell" => 4,
        _ => return Err("unknown app-control script method".into()),
    };
    if args.len() != count {
        return Err(format!("{method} expects a request id and {count} argument(s)"));
    }
    if matches!(method, "launchMatrixColumn" | "launchMatrixCell" | "unloadMatrix") {
        let edit = if method == "unloadMatrix" {
            MatrixEdit::ClearPlayback
        } else {
            let mode = serde_json::from_value(serde_json::Value::String(text(args, count - 1)?.into()))
                .map_err(|error| error.to_string())?;
            if method == "launchMatrixColumn" {
                MatrixEdit::LaunchColumn {
                    column: ColumnId(id(args, 1)?),
                    mode,
                }
            } else {
                MatrixEdit::LaunchCell {
                    layer: LayerId(id(args, 1)?),
                    column: ColumnId(id(args, 2)?),
                    mode,
                }
            }
        };
        return Ok(MediaEdit::Matrix {
            media: MediaId(id(args, 0)?),
            edit,
        });
    }
    let edit = match method {
        "setApplicationFile" => ApplicationEdit::SetFile {
            application: ApplicationId(id(args, 0)?),
            executable: match text(args, 1)? {
                "" => None,
                path => Some(path.into()),
            },
        },
        "configureApplication" => {
            let json = text(args, 1)?;
            if json.len() > 64 * 1024 {
                return Err("application definition request exceeds 64 KiB".into());
            }
            let definition: ApplicationDefinition = serde_json::from_str(json).map_err(|error| error.to_string())?;
            ApplicationEdit::Configure {
                application: ApplicationId(id(args, 0)?),
                definition,
            }
        }
        "inspectApplication" => ApplicationEdit::Inspect {
            application: ApplicationId(id(args, 0)?),
        },
        "inspectApplicationRuntime" => ApplicationEdit::InspectRuntime {},
        "inspectApplicationCallbacks" => ApplicationEdit::InspectCallbacks {},
        _ => unreachable!("admitted app-control method"),
    };
    Ok(MediaEdit::Applications { edit })
}
