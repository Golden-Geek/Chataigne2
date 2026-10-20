use super::{OscValuePayload, OSC_OUTPUT_NODE_TYPE, VALUE_LABEL_PREFIX};
use golden_core::{node::NodeId, parameter::ParamValue, process_ctx::ProcessTreeSnapshot};

pub(super) fn collect_outputs_recursive(snapshot: &ProcessTreeSnapshot, parent: NodeId, output: &mut Vec<NodeId>) {
    for child_id in snapshot.child_ids(parent) {
        let Some(child_snapshot) = snapshot.node(child_id) else {
            continue;
        };

        if child_snapshot.node_type == OSC_OUTPUT_NODE_TYPE {
            output.push(child_id);
        } else if child_snapshot.node_type == "folder" {
            collect_outputs_recursive(snapshot, child_id, output);
        }
    }
}

pub(super) fn resolve_outgoing_target(snapshot: &ProcessTreeSnapshot, node_id: NodeId) -> Option<NodeId> {
    let node = snapshot.node(node_id)?;
    node.param_value.as_ref()?;

    let parent_id = node.parent?;
    let parent = snapshot.node(parent_id)?;
    if parent.node_type == "folder"
        && indexed_value_label_index(node.label.as_str()).is_some()
        && has_indexed_value_children(snapshot, parent_id)
    {
        return Some(parent_id);
    }

    Some(node_id)
}

pub(super) fn outgoing_address_for_values_node(
    snapshot: &ProcessTreeSnapshot,
    values_id: NodeId,
    target_id: NodeId,
) -> Option<String> {
    let mut segments = Vec::new();
    let mut current = Some(target_id);

    while let Some(node_id) = current {
        if node_id == values_id {
            break;
        }

        let node = snapshot.node(node_id)?;
        let segment = node.label.trim().trim_matches('/');
        if !segment.is_empty() {
            segments.push(segment.to_string());
        }
        current = node.parent;
    }

    if current != Some(values_id) || segments.is_empty() {
        return None;
    }

    segments.reverse();
    Some(format!("/{}", segments.join("/")))
}

pub(super) fn outgoing_payload_for_target(
    snapshot: &ProcessTreeSnapshot,
    target_id: NodeId,
) -> Option<OscValuePayload> {
    let target = snapshot.node(target_id)?;
    if let Some(value) = target.param_value.clone() {
        return Some(OscValuePayload::Single(value));
    }

    if target.node_type != "folder" {
        return None;
    }

    let mut indexed_values = snapshot
        .child_ids(target_id)
        .into_iter()
        .filter_map(|child_id| {
            let child = snapshot.node(child_id)?;
            let index = indexed_value_label_index(child.label.as_str())?;
            let value = child.param_value.clone()?;
            Some((index, value))
        })
        .collect::<Vec<_>>();
    indexed_values.sort_by_key(|(index, _)| *index);

    if indexed_values.is_empty() {
        return None;
    }

    Some(OscValuePayload::Multi(
        indexed_values.into_iter().map(|(_, value)| value).collect(),
    ))
}

pub(super) fn child_string_param(snapshot: &ProcessTreeSnapshot, parent: NodeId, child_name: &str) -> Option<String> {
    snapshot.find_child_by_decl_id(parent, child_name).and_then(|child_id| {
        snapshot
            .node(child_id)
            .and_then(|node| node.param_value.as_ref())
            .and_then(ParamValue::as_str)
    })
}

pub(super) fn child_int_param(snapshot: &ProcessTreeSnapshot, parent: NodeId, child_name: &str) -> Option<i32> {
    snapshot.find_child_by_decl_id(parent, child_name).and_then(|child_id| {
        snapshot
            .node(child_id)
            .and_then(|node| node.param_value.as_ref())
            .and_then(ParamValue::as_int)
    })
}

pub(super) fn format_osc_payload(payload: &OscValuePayload) -> String {
    match payload {
        OscValuePayload::Single(value) => format_param_value(value),
        OscValuePayload::Multi(values) => format!(
            "[{}]",
            values.iter().map(format_param_value).collect::<Vec<_>>().join(", ")
        ),
        OscValuePayload::Arguments(values) => format!(
            "[{}]",
            values.iter().map(format_param_value).collect::<Vec<_>>().join(", ")
        ),
    }
}

pub(super) fn format_param_value(value: &ParamValue) -> String {
    match value {
        ParamValue::Trigger() => "trigger".to_string(),
        ParamValue::Int(value) => value.to_string(),
        ParamValue::Float(value) => value.to_string(),
        ParamValue::Str(value) | ParamValue::File(value) | ParamValue::Enum(value) => value.clone(),
        ParamValue::Bool(value) => value.to_string(),
        ParamValue::Vec2(x, y) => format!("({}, {})", x, y),
        ParamValue::Vec3(x, y, z) => format!("({}, {}, {})", x, y, z),
        ParamValue::Color(r, g, b, a) => format!("rgba({}, {}, {}, {})", r, g, b, a),
        ParamValue::CssValue(value) => format!("{value:?}"),
        ParamValue::Reference(reference) => format!("{reference:?}"),
    }
}

pub(super) fn indexed_value_label_index(label: &str) -> Option<usize> {
    let suffix = label.strip_prefix(VALUE_LABEL_PREFIX)?;
    suffix.parse::<usize>().ok()?.checked_sub(1)
}

pub(super) fn has_indexed_value_children(snapshot: &ProcessTreeSnapshot, folder_id: NodeId) -> bool {
    snapshot.child_ids(folder_id).into_iter().any(|child_id| {
        snapshot
            .node(child_id)
            .is_some_and(|child| indexed_value_label_index(child.label.as_str()).is_some())
    })
}

pub(super) fn is_descendant_of_node(snapshot: &ProcessTreeSnapshot, start: NodeId, ancestor: NodeId) -> bool {
    let mut current = Some(start);
    while let Some(node_id) = current {
        if node_id == ancestor {
            return true;
        }
        current = snapshot.node(node_id).and_then(|node| node.parent);
    }

    false
}
