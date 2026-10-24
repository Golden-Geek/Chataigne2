//! One-way migration of DMX's former JSON-text frame parameters.

use golden_core::{engine::ProjectNodeRecord, parameter::ParamValue};
use std::borrow::Cow;

pub(super) fn migrate_module_children(children: &[ProjectNodeRecord]) -> Result<Cow<'_, [ProjectNodeRecord]>, String> {
    let mut migrated = Cow::Borrowed(children);
    for (folder, parameter) in [("parameters", "output_frame"), ("values", "received_frame")] {
        let Some(index) = children.iter().position(|child| decl(child) == folder) else {
            continue;
        };
        let frames = migrate_frames(&children[index].children, parameter)?;
        if let Cow::Owned(frames) = frames {
            migrated.to_mut()[index].children = frames;
        }
    }
    Ok(migrated)
}

pub(crate) fn migrate_command_children(children: &[ProjectNodeRecord]) -> Result<Cow<'_, [ProjectNodeRecord]>, String> {
    migrate_frames(children, "slots")
}

fn decl(record: &ProjectNodeRecord) -> &str {
    record
        .meta
        .decl_id
        .as_ref()
        .and_then(|id| id.0.rsplit('/').next())
        .unwrap_or("")
}

fn migrate_frames<'a>(
    children: &'a [ProjectNodeRecord],
    parameter: &str,
) -> Result<Cow<'a, [ProjectNodeRecord]>, String> {
    let mut migrated = Cow::Borrowed(children);
    for (index, child) in children.iter().enumerate() {
        if decl(child) != parameter || child.node_type != "str" {
            continue;
        }
        let child = &mut migrated.to_mut()[index];
        if let Some(data) = child.data.as_mut().and_then(serde_json::Value::as_object_mut) {
            for field in ["value", "default_value"] {
                let Some(value) = data.get_mut(field) else {
                    continue;
                };
                let ParamValue::Str(text) = serde_json::from_value(value.clone())
                    .map_err(|error| format!("invalid legacy DMX {field}: {error}"))?
                else {
                    return Err(format!("legacy DMX {field} must be a string parameter"));
                };
                let slots = super::parse_slots_json(&text)?;
                *value = serde_json::to_value(ParamValue::Bytes(slots)).map_err(|error| error.to_string())?;
            }
        }
        child.node_type = "bytes".into();
    }
    Ok(migrated)
}
