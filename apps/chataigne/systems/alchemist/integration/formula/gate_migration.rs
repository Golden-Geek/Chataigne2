use golden_core::{
    edit::Edit,
    node::{Node, NodeId, NodeMetaPatch},
    parameter::{ParamValue, ParameterEventBehaviour},
    process_ctx::ProcessTreeSnapshot,
};

use crate::app::AppEngine;

use super::{ANODE_NODE_TYPE, FORMULA_EXTERNAL_BUILTIN_TAG_PREFIX};

pub(crate) const GATE_SEMANTICS_V2_TAG: &str = "chataigne.condition_gate.semantics.v2";
const GATE_TYPE_TAG: &str = "alchemist.anode.type:condition_gate";

/// Projects saved before suppressing gates were introduced used a default-valued
/// closed output. Translate their authored mode once, before runtime compilation.
pub(crate) fn migrate_legacy_gate_semantics(engine: &mut AppEngine) -> Result<(), String> {
    let snapshot = engine.process_tree_snapshot();
    let root = snapshot.root();
    let root_node = snapshot.node(root).ok_or("project root is missing")?;
    if root_node.tags.iter().any(|tag| tag == GATE_SEMANTICS_V2_TAG) {
        return Ok(());
    }

    let gates = engine
        .nodes
        .iter()
        .filter_map(|(id, node)| {
            (node.get_type() == ANODE_NODE_TYPE
                && (node.node_data().meta.tags.iter().any(|tag| tag == GATE_TYPE_TAG)
                    || snapshot.find_child_by_decl_id(id, "anode_type")
                        .and_then(|type_id| snapshot.node(type_id))
                        .and_then(|type_id| type_id.param_value.as_ref())
                        .is_some_and(|value| matches!(value, ParamValue::Str(type_id) if type_id == "condition_gate")))
                && !is_builtin_descendant(&snapshot, id))
            .then_some(id)
        })
        .collect::<Vec<_>>();
    let mut replacements = Vec::with_capacity(gates.len());
    for gate in gates {
        let Some(config) = snapshot.find_child_by_decl_id(gate, "config") else {
            return Err(format!("Condition Gate {gate:?} has no Config folder"));
        };
        let Some(mode) = snapshot.find_child_by_decl_id(config, "config/mode") else {
            return Err(format!("Condition Gate {gate:?} has no mode field"));
        };
        let current = snapshot.node(mode).and_then(|node| node.param_value.as_ref());
        let replacement = match current {
            Some(ParamValue::Enum(value)) | Some(ParamValue::Str(value)) => match value.as_str() {
                "pass_when_true" => "output_default",
                "pass_when_false" => "output_default_when_false",
                "hold_last" => "hold_last_with_default",
                "block_trigger" => "block_trigger_with_default",
                "output_default" => continue,
                other => return Err(format!("Condition Gate {gate:?} has unknown historical mode `{other}`")),
            },
            _ => return Err(format!("Condition Gate {gate:?} has an invalid historical mode")),
        };
        let value = match current {
            Some(ParamValue::Enum(_)) => ParamValue::Enum(replacement.to_owned()),
            _ => ParamValue::Str(replacement.to_owned()),
        };
        replacements.push((mode, value));
    }

    for (mode, value) in replacements {
        engine.edits.push(Edit::SetParam {
            node: mode,
            value,
            behaviour: ParameterEventBehaviour::Coalesce,
        });
    }

    let mut tags = root_node.tags.clone();
    tags.push(GATE_SEMANTICS_V2_TAG.to_owned());
    engine.edits.push(Edit::PatchMeta {
        node: root,
        patch: NodeMetaPatch {
            tags: Some(tags),
            ..NodeMetaPatch::default()
        },
    });
    engine
        .apply_project_load_edits()
        .map_err(|error| format!("Condition Gate project migration failed: {error}"))
}

fn is_builtin_descendant(snapshot: &ProcessTreeSnapshot, node: NodeId) -> bool {
    let mut ancestor = Some(node);
    while let Some(id) = ancestor {
        let Some(current) = snapshot.node(id) else {
            break;
        };
        if current.tags.iter().any(|tag| tag.starts_with(FORMULA_EXTERNAL_BUILTIN_TAG_PREFIX)) {
            return true;
        }
        ancestor = current.parent;
    }
    false
}
