use crate::parameter::ParamValue;
use crate::process_ctx::{ProcessCtx, ProcessTreeNodeSnapshot};

use super::{NodeData, NodeId, NodeReference, NodeScriptDescriptor};

pub(crate) fn parameter_node_type_from_value(value: &ParamValue) -> &'static str {
    match value {
        ParamValue::Trigger() => "trigger",
        ParamValue::Int(_) => "int",
        ParamValue::Float(_) => "float",
        ParamValue::Str(_) => "str",
        ParamValue::File(_) => "file",
        ParamValue::Enum(_) => "enum",
        ParamValue::Bool(_) => "bool",
        ParamValue::CssValue(_) => "css_value",
        ParamValue::Vec2(_, _) => "vec2",
        ParamValue::Vec3(_, _, _) => "vec3",
        ParamValue::Color(_, _, _, _) => "color",
        ParamValue::Reference(_) => "reference",
    }
}

pub(crate) fn default_parameter_value_for_node_type(node_type: &str) -> Option<ParamValue> {
    match node_type {
        "trigger" => Some(ParamValue::Trigger()),
        "int" => Some(ParamValue::Int(0)),
        "float" => Some(ParamValue::Float(0.0)),
        "str" => Some(ParamValue::Str(String::new())),
        "file" => Some(ParamValue::File(String::new())),
        "enum" => Some(ParamValue::Enum(String::new())),
        "bool" => Some(ParamValue::Bool(false)),
        "css_value" => Some(ParamValue::CssValue(crate::parameter::CssValue::default())),
        "vec2" => Some(ParamValue::Vec2(0.0, 0.0)),
        "vec3" => Some(ParamValue::Vec3(0.0, 0.0, 0.0)),
        "color" => Some(ParamValue::Color(1.0, 1.0, 1.0, 1.0)),
        "reference" => Some(ParamValue::Reference(NodeReference::default())),
        _ => None,
    }
}

pub(crate) struct ScriptChildLookup {
    pub primary: Option<NodeId>,
    pub primary_matches_type: bool,
    pub duplicates: Vec<NodeId>,
}

fn script_child_matches_key(node: &ProcessTreeNodeSnapshot, key: &str) -> bool {
    let key = key.trim();
    if key.is_empty() {
        return false;
    }

    node.decl_id.eq_ignore_ascii_case(key)
        || node.short_name.eq_ignore_ascii_case(key)
        || node.label.eq_ignore_ascii_case(key)
}

pub(crate) fn lookup_script_child_by_key_and_type(
    ctx: &ProcessCtx,
    parent: NodeId,
    key: &str,
    expected_node_type: &str,
) -> ScriptChildLookup {
    let mut matches = Vec::new();
    let mut same_type_matches = Vec::new();

    if let Some(snapshot) = ctx.tree_snapshot() {
        let mut child = snapshot.node(parent).and_then(|node| node.first_child);
        while let Some(child_id) = child {
            let Some(child_snapshot) = snapshot.node(child_id) else {
                break;
            };

            if script_child_matches_key(child_snapshot, key) {
                matches.push(child_id);
                if child_snapshot.node_type.eq_ignore_ascii_case(expected_node_type) {
                    same_type_matches.push(child_id);
                }
            }

            child = child_snapshot.next_sibling;
        }
    }

    let (primary, primary_matches_type) = if let Some(node) = same_type_matches.first().copied() {
        (Some(node), true)
    } else if let Some(node) = matches.first().copied() {
        (Some(node), false)
    } else {
        (None, false)
    };

    let duplicates = matches
        .into_iter()
        .filter(|candidate| Some(*candidate) != primary)
        .collect::<Vec<_>>();
    ScriptChildLookup {
        primary,
        primary_matches_type,
        duplicates,
    }
}

pub(crate) fn core_node_script_descriptor(node_data: &NodeData, node_type: &str) -> NodeScriptDescriptor {
    NodeScriptDescriptor::for_node(node_data, node_type)
}
