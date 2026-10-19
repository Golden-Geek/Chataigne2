use super::{
    bounds::MAX_SOURCE_BYTES, bytes_request, hex_string_request, string_request, value_weight, values_json_request,
    values_request, StreamingSendRequest, MAX_COMMAND_VALUES,
};
use golden_core::{node::NodeId, parameter::ParamValue, process_ctx::ProcessTreeSnapshot};
use std::borrow::Cow;

#[derive(Clone, Copy, Debug)]
pub(crate) enum CommandCapture {
    String {
        text: NodeId,
        ending: NodeId,
    },
    Bytes(NodeId),
    Hex(NodeId),
    Values {
        root: NodeId,
        prefix: NodeId,
        suffix: NodeId,
        separator: NodeId,
    },
    Json(NodeId),
}
impl CommandCapture {
    pub fn materialize(
        self,
        snapshot: &ProcessTreeSnapshot,
        cancelled: &impl Fn() -> bool,
    ) -> Result<StreamingSendRequest, String> {
        match self {
            Self::String { text, ending } => {
                string_request(&text_value(snapshot, text)?, &text_value(snapshot, ending)?)
            }
            Self::Bytes(value) => bytes_request(&text_value(snapshot, value)?),
            Self::Hex(value) => hex_string_request(&text_value(snapshot, value)?),
            Self::Values {
                root,
                prefix,
                suffix,
                separator,
            } => {
                let mut values = Vec::new();
                let mut budget = Budget::default();
                collect(snapshot, root, 0, &mut budget, cancelled, &mut values)?;
                values_request(
                    values,
                    &text_value(snapshot, prefix)?,
                    &text_value(snapshot, suffix)?,
                    &text_value(snapshot, separator)?,
                )
            }
            Self::Json(root) => {
                let value = json(snapshot, root, 0, &mut Budget::default(), cancelled)?;
                values_json_request(&value)
            }
        }
    }
}
fn text_value(snapshot: &ProcessTreeSnapshot, id: NodeId) -> Result<Cow<'_, str>, String> {
    let Some(value) = snapshot.node(id).and_then(|node| node.param_value.as_ref()) else {
        return Ok(Cow::Borrowed(""));
    };
    if value_weight(value) > MAX_SOURCE_BYTES {
        return Err("Stream command parameter exceeds 4 MiB".into());
    }
    Ok(match value {
        ParamValue::Str(text) | ParamValue::File(text) | ParamValue::Enum(text) => Cow::Borrowed(text),
        _ => Cow::Owned(value.as_str().unwrap_or_default()),
    })
}
#[derive(Default)]
struct Budget {
    nodes: usize,
    values: usize,
    bytes: usize,
}
impl Budget {
    fn visit(&mut self, depth: usize, cancelled: &impl Fn() -> bool) -> Result<(), String> {
        if cancelled() {
            return Err("Stream command preparation cancelled".into());
        }
        self.nodes += 1;
        if depth > 64 || self.nodes > 16_384 {
            return Err("Stream command tree exceeds 64 levels or 16384 nodes".into());
        }
        Ok(())
    }
    fn claim(&mut self, value: Option<&ParamValue>, label: &str) -> Result<(), String> {
        if let Some(value) = value {
            self.values += 1;
            self.bytes = self.bytes.saturating_add(value_weight(value));
        }
        self.bytes = self.bytes.saturating_add(label.len());
        if self.values > MAX_COMMAND_VALUES || self.bytes > MAX_SOURCE_BYTES {
            return Err("Stream command tree exceeds 4096 values or 4 MiB".into());
        }
        Ok(())
    }
}
fn folder(node_type: &str) -> bool {
    matches!(node_type, "folder" | "streaming_command_values")
}
fn collect<'a>(
    snapshot: &'a ProcessTreeSnapshot,
    root: NodeId,
    depth: usize,
    budget: &mut Budget,
    cancelled: &impl Fn() -> bool,
    values: &mut Vec<&'a ParamValue>,
) -> Result<(), String> {
    for id in snapshot.child_ids_slice(root) {
        budget.visit(depth, cancelled)?;
        let node = snapshot.node(*id).ok_or("Stream command tree has a missing node")?;
        if let Some(value) = &node.param_value {
            budget.claim(Some(value), "")?;
            values.push(value);
        } else if folder(&node.node_type) {
            collect(snapshot, *id, depth + 1, budget, cancelled, values)?;
        }
    }
    Ok(())
}
fn json(
    snapshot: &ProcessTreeSnapshot,
    root: NodeId,
    depth: usize,
    budget: &mut Budget,
    cancelled: &impl Fn() -> bool,
) -> Result<serde_json::Value, String> {
    let mut object = serde_json::Map::new();
    for id in snapshot.child_ids_slice(root) {
        budget.visit(depth, cancelled)?;
        let node = snapshot.node(*id).ok_or("Stream command tree has a missing node")?;
        let key = node.label.trim();
        if key.is_empty() {
            continue;
        }
        budget.claim(node.param_value.as_ref(), key)?;
        if let Some(value) = &node.param_value {
            object.insert(key.to_string(), value.to_script_json());
        } else if folder(&node.node_type) {
            object.insert(key.to_string(), json(snapshot, *id, depth + 1, budget, cancelled)?);
        }
    }
    Ok(serde_json::Value::Object(object))
}
