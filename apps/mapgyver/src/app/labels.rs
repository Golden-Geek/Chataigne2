//! Backend-owned label allocation shared by authored MapGyver managers.

use std::collections::HashSet;

use super::AppEngine;
use golden_core::{
    edit::Edit,
    node::{Node, NodeId, NodeMetaPatch},
};

pub(crate) fn unique(
    engine: &AppEngine,
    parent: NodeId,
    requested: Option<String>,
    fallback: &str,
    reserved: &[&str],
) -> Result<String, String> {
    let requested = requested.unwrap_or_else(|| fallback.into());
    let requested = requested.trim();
    if requested.is_empty() || requested.len() > 255 || requested.contains('\0') {
        return Err("label must contain 1..255 UTF-8 bytes without NUL".into());
    }
    let mut labels = HashSet::new();
    labels.extend(reserved.iter().copied());
    let mut child = engine.nodes.get(parent).and_then(|node| node.node_data().first_child);
    while let Some(id) = child {
        let node = engine.nodes.get(id).ok_or("missing sibling")?;
        labels.insert(node.node_data().meta.label.as_str());
        child = node.node_data().next_sibling;
    }
    if !labels.contains(requested) {
        return Ok(requested.into());
    }
    for index in 2..=labels.len() + 2 {
        let candidate = format!("{requested} {index}");
        if !labels.contains(candidate.as_str()) && candidate.len() <= 255 {
            return Ok(candidate);
        }
    }
    Err("cannot allocate a unique label within the byte limit".into())
}

pub(crate) fn rename(engine: &mut AppEngine, node: NodeId, label: String) -> Result<(), String> {
    let data = engine.nodes.get(node).ok_or("rename target is missing")?.node_data();
    if label.trim() == data.meta.label {
        return Ok(());
    }
    let parent = data.parent.ok_or("rename target is detached")?;
    let label = unique(engine, parent, Some(label), "", &[])?;
    engine.edits.push(Edit::PatchMeta {
        node,
        patch: NodeMetaPatch {
            label: Some(label),
            ..Default::default()
        },
    });
    Ok(())
}
