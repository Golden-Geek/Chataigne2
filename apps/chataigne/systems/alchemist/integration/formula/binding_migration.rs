//! One-time migration of the first authored Output Command binding document.

use chataigne_state_machine::{MappingOutputBindingsDto, OutputBindingConfig};
use golden_core::{
    edit::Edit,
    node::{Node, NodeId, NodeMetaPatch},
    parameter::{ParamValue, ParameterEventBehaviour},
    process_ctx::ProcessTreeSnapshot,
};

use crate::app::AppEngine;

use super::{ANODE_NODE_TYPE, FORMULA_EXTERNAL_READ_ONLY_TAG};

pub(crate) const OUTPUT_BINDINGS_V2_TAG: &str = "chataigne.output_bindings.authoring.v2";
const OUTPUT_TARGET_TYPE_TAG: &str = "alchemist.anode.type:chataigne.output_target";

pub(crate) fn migrate_output_binding_documents(engine: &mut AppEngine) -> Result<(), String> {
    let snapshot = engine.process_tree_snapshot();
    let root = snapshot.root();
    let root_node = snapshot.node(root).ok_or("project root is missing")?;
    if root_node.tags.iter().any(|tag| tag == OUTPUT_BINDINGS_V2_TAG) {
        return Ok(());
    }

    let mut replacements = Vec::new();
    for (id, node) in engine.nodes.iter() {
        if node.get_type() != ANODE_NODE_TYPE || !node.node_data().meta.tags.iter().any(|tag| tag == OUTPUT_TARGET_TYPE_TAG) {
            continue;
        }
        let Some(config) = snapshot.find_child_by_decl_id(id, "config") else {
            return Err(format!("Output Command {id:?} has no Config folder"));
        };
        let Some(bindings) = snapshot.find_child_by_decl_id(config, "config/bindings") else {
            return Err(format!("Output Command {id:?} has no Bindings field"));
        };
        let Some(ParamValue::Str(document)) = snapshot.node(bindings).and_then(|node| node.param_value.as_ref()) else {
            return Err(format!("Output Command {id:?} has a non-string Bindings field"));
        };
        if serde_json::from_str::<MappingOutputBindingsDto>(document).is_ok() {
            OutputBindingConfig::from_authoring_json(document)
                .map_err(|error| format!("Output Command {id:?} has invalid bindings: {error}"))?;
            continue;
        }
        let legacy: OutputBindingConfig = serde_json::from_str(document)
            .map_err(|error| format!("Output Command {id:?} has an unknown binding schema: {error}"))?;
        if is_external_descendant(&snapshot, id) {
            return Err(format!(
                "shared Output Command {id:?} has historical bindings; re-export it from a migrated project"
            ));
        }
        replacements.push((bindings, legacy.to_authoring_json()?));
    }

    for (bindings, document) in replacements {
        engine.edits.push(Edit::SetParam {
            node: bindings,
            value: ParamValue::Str(document),
            behaviour: ParameterEventBehaviour::Coalesce,
        });
    }
    let mut tags = root_node.tags.clone();
    tags.push(OUTPUT_BINDINGS_V2_TAG.to_owned());
    engine.edits.push(Edit::PatchMeta {
        node: root,
        patch: NodeMetaPatch {
            tags: Some(tags),
            ..NodeMetaPatch::default()
        },
    });
    engine
        .apply_project_load_edits()
        .map_err(|error| format!("Output Command binding migration failed: {error}"))
}

fn is_external_descendant(snapshot: &ProcessTreeSnapshot, node: NodeId) -> bool {
    let mut ancestor = Some(node);
    while let Some(id) = ancestor {
        let Some(current) = snapshot.node(id) else {
            break;
        };
        if current.tags.iter().any(|tag| tag == FORMULA_EXTERNAL_READ_ONLY_TAG) {
            return true;
        }
        ancestor = current.parent;
    }
    false
}
