use golden_core::{
    engine::Engine,
    node::{Node, NodeId},
    process_ctx::ProcessTreeSnapshot,
};

use super::constants::{MODULE_VALUES_DECL_ID, MODULE_VALUES_REFERENCE_FILTER_KEY};
use crate::app::descriptors::{MODULE_ITEM_KIND, MODULE_MANAGER};

pub(crate) fn resolve_enclosing_module_root(snapshot: &ProcessTreeSnapshot, start: NodeId) -> Option<NodeId> {
    let mut current = Some(start);

    while let Some(node_id) = current {
        if snapshot.find_child(node_id, super::constants::MODULE_CONNECTION_DECL_ID).is_some()
            && snapshot
                .find_child(node_id, super::constants::MODULE_PARAMETERS_DECL_ID)
                .is_some()
            && snapshot.find_child(node_id, MODULE_VALUES_DECL_ID).is_some()
        {
            return Some(node_id);
        }

        current = snapshot.node(node_id).and_then(|node| node.parent);
    }

    None
}

pub(crate) fn register_module_reference_filters<T: Node>(engine: &mut Engine<T>) {
    engine.register_reference_filter(
        MODULE_VALUES_REFERENCE_FILTER_KEY,
        |engine, _param_node, _root, candidate| candidate_is_module_values_parameter(engine, candidate),
    );
}

fn candidate_is_module_values_parameter<T: Node>(engine: &Engine<T>, candidate: NodeId) -> bool {
    let Some(candidate_node) = engine.nodes.get(candidate) else {
        return false;
    };
    if candidate_node.engine_param_snapshot().is_none() {
        return false;
    }

    let Some(mut current) = candidate_node.node_data().parent else {
        return false;
    };

    let mut has_values_ancestor = false;
    let mut has_module_ancestor = false;
    let mut has_module_manager_ancestor = false;
    loop {
        let Some(node) = engine.nodes.get(current) else {
            return false;
        };
        if node.node_data().meta.decl_id.0 == MODULE_VALUES_DECL_ID {
            has_values_ancestor = true;
        }
        if node.user_item_kind() == MODULE_ITEM_KIND {
            has_module_ancestor = true;
        }
        if node.get_type() == MODULE_MANAGER.type_id {
            has_module_manager_ancestor = true;
        }
        let Some(parent) = node.node_data().parent else {
            break;
        };
        current = parent;
    }

    has_values_ancestor && has_module_ancestor && has_module_manager_ancestor
}
