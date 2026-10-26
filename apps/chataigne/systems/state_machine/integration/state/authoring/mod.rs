//! State-owned graph mutations. No caller writes private layout parameters or constructs transitions.

use crate::app::{AppEngine, AppNode};
use chataigne_state_machine::authoring::*;
use golden_core::{
    edit::{Edit, NodeTree},
    node::{DeclId, Node, NodeId, NodeMetaPatch, NodeReference},
    parameter::{ParamValue, Parameter, ParameterChangeCheck},
};
use std::collections::HashSet;

#[cfg(test)]
mod tests;

fn state(engine: &AppEngine, manager: StateGraphIdentity, identity: StateGraphIdentity) -> Result<&AppNode, String> {
    engine
        .nodes
        .get(identity.node)
        .filter(|node| {
            node.get_type() == crate::app::StateMachineState::NODE_TYPE
                && node.node_data().meta.uuid == identity.uuid
                && node.node_data().parent == Some(manager.node)
        })
        .ok_or_else(|| "State identity changed or belongs to another manager".into())
}

fn child(engine: &AppEngine, owner: NodeId, declaration: &str) -> Result<NodeId, String> {
    let mut next = engine.nodes.get(owner).and_then(|node| node.node_data().first_child);
    while let Some(id) = next {
        let node = engine.nodes.get(id).ok_or("Declared child was retired")?;
        if node.node_data().meta.decl_id.0 == declaration {
            return Ok(id);
        }
        next = node.node_data().next_sibling;
    }
    Err(format!("State declaration '{declaration}' is unavailable"))
}

fn vector_edit(engine: &AppEngine, owner: NodeId, declaration: &str, value: [f64; 2]) -> Result<Edit, String> {
    if !value.into_iter().all(f64::is_finite) {
        return Err("Graph coordinates must be finite".into());
    }
    let node = child(engine, owner, declaration)?;
    let parameter = engine
        .nodes
        .get(node)
        .and_then(Node::engine_param_snapshot)
        .ok_or("Layout field is not a parameter")?;
    if parameter.read_only || !matches!(parameter.value, ParamValue::Vec2(..)) {
        return Err("State layout field is unavailable or read-only".into());
    }
    Ok(Edit::SetParam {
        node,
        value: ParamValue::Vec2(value[0], value[1]),
        behaviour: parameter.event_behaviour,
    })
}

pub(crate) fn apply_graph_operation(
    engine: &mut AppEngine,
    operation: &str,
    payload: serde_json::Value,
) -> Result<serde_json::Value, String> {
    if operation != STATE_GRAPH_EDIT_OPERATION {
        return Err("Unsupported state graph operation".into());
    }
    crate::app::module::common::json::retained_weight(&payload, 4 * 1024 * 1024, 200_000, 16)?;
    let request: StateGraphEditRequest = serde_json::from_value(payload).map_err(|error| error.to_string())?;
    let manager = request.manager;
    engine
        .nodes
        .get(manager.node)
        .filter(|node| {
            node.get_type() == crate::app::StateMachineManager::NODE_TYPE && node.node_data().meta.uuid == manager.uuid
        })
        .ok_or("State graph manager identity changed or is unavailable")?;
    let mut edits = Vec::new();
    let mut result = StateGraphEditResult {
        states: Vec::new(),
        connection: None,
    };
    match request.edit {
        StateGraphEdit::Move { moves } => {
            if moves.is_empty() || moves.len() > 16_384 {
                return Err("Move requires 1..16384 states".into());
            }
            let mut seen = HashSet::new();
            for item in moves {
                state(engine, manager, item.state)?;
                if !seen.insert(item.state.node) {
                    return Err("Move state identities must be unique".into());
                }
                edits.push(vector_edit(engine, item.state.node, "position", item.position)?);
                result.states.push(item.state);
            }
        }
        StateGraphEdit::Resize { state: identity, size } => {
            state(engine, manager, identity)?;
            let parameter = child(engine, identity.node, "size")?;
            let node = engine.nodes.get(parameter).ok_or("State size is unavailable")?;
            if !node.node_data().meta.can_be_disabled
                || node.engine_param_snapshot().is_none_or(|value| value.read_only)
            {
                return Err("State size is unavailable or read-only".into());
            }
            if let Some(size) = size {
                if !size.into_iter().all(|value| value > 0.0 && value.is_finite()) {
                    return Err("State size must be finite and positive".into());
                }
                edits.push(vector_edit(engine, identity.node, "size", size)?);
            }
            edits.push(Edit::PatchMeta {
                node: parameter,
                patch: NodeMetaPatch {
                    enabled: Some(size.is_some()),
                    ..Default::default()
                },
            });
            result.states.push(identity);
        }
        StateGraphEdit::Rename { state: identity, label } => {
            let node = state(engine, manager, identity)?;
            let label = label.trim();
            if !node.node_data().meta.user_permissions.can_edit_name || label.is_empty() || label.len() > 1024 {
                return Err("State name is unavailable or invalid".into());
            }
            edits.push(Edit::PatchMeta {
                node: identity.node,
                patch: NodeMetaPatch {
                    label: Some(label.into()),
                    ..Default::default()
                },
            });
            result.states.push(identity);
        }
        StateGraphEdit::Collapse {
            state: identity,
            collapsed,
        } => {
            let mut presentation = state(engine, manager, identity)?.node_data().meta.presentation.clone();
            presentation.collapsed = collapsed;
            edits.push(Edit::PatchMeta {
                node: identity.node,
                patch: NodeMetaPatch {
                    presentation: Some(presentation),
                    ..Default::default()
                },
            });
            result.states.push(identity);
        }
        StateGraphEdit::Enable {
            state: identity,
            enabled,
        } => {
            if !state(engine, manager, identity)?.node_data().meta.can_be_disabled {
                return Err("State cannot be disabled".into());
            }
            edits.push(Edit::PatchMeta {
                node: identity.node,
                patch: NodeMetaPatch {
                    enabled: Some(enabled),
                    ..Default::default()
                },
            });
            result.states.push(identity);
        }
        StateGraphEdit::Connect { from, to } => {
            if from.socket != STATE_GRAPH_OUTPUT || to.socket != STATE_GRAPH_INPUT {
                return Err("Invalid state transition sockets".into());
            }
            state(engine, manager, from.state)?;
            state(engine, manager, to.state)?;
            let parent = child(engine, from.state.node, "transitions")?;
            if engine
                .nodes
                .get(parent)
                .is_none_or(|node| node.get_type() != crate::app::StateTransitionManager::NODE_TYPE)
            {
                return Err("State transition manager is unavailable".into());
            }
            let transition = crate::app::StateTransition::new();
            let uuid = transition.node_data().meta.uuid;
            let mut target = Parameter::new(
                "Target State",
                ParamValue::Reference(NodeReference::new(to.state.uuid)),
                ParameterChangeCheck::ValueChange,
            );
            target.node_data_mut().meta.decl_id = DeclId("target".into());
            let mut tree = NodeTree::new(transition).as_user_item();
            tree.push_child(NodeTree::new(target));
            engine.add_user_item_tree(tree, Some(parent));
            engine.apply_edits().map_err(|error| error.to_string())?;
            let node = engine.node_id_by_uuid(uuid).ok_or("Transition identity was retired")?;
            result.states.extend([from.state, to.state]);
            result.connection = Some(StateGraphIdentity { node, uuid });
        }
    }
    for edit in edits {
        engine.edits.push(edit);
    }
    engine.apply_edits().map_err(|error| error.to_string())?;
    serde_json::to_value(result).map_err(|error| error.to_string())
}
