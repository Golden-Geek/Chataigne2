//! Formula-owned layout validation. All targets are checked before publishing any edit.

use super::*;
use golden_core::node::NodeMetaPatch;

fn anode<'a>(
    engine: &'a crate::app::AppEngine,
    formula: &FormulaOwnerDto,
    identity: &FormulaOwnerDto,
) -> Result<&'a crate::app::AppNode, String> {
    engine
        .nodes
        .get(identity.node)
        .filter(|node| {
            node.get_type() == ANODE_NODE_TYPE
                && node.node_data().meta.uuid == identity.uuid
                && node.node_data().parent == Some(formula.node)
        })
        .ok_or_else(|| "ANode identity changed or belongs to another Formula".into())
}

fn parameter(engine: &crate::app::AppEngine, owner: NodeId, declaration: &str) -> Result<NodeId, String> {
    let mut next = engine.nodes.get(owner).and_then(|node| node.node_data().first_child);
    while let Some(id) = next {
        let node = engine.nodes.get(id).ok_or("Layout field was retired")?;
        if node.node_data().meta.decl_id.0 == declaration {
            let value = node.engine_param_snapshot().ok_or("Layout field is not a parameter")?;
            if value.read_only || !matches!(value.value, ParamValue::Vec2(..)) {
                return Err("ANode layout field is unavailable or read-only".into());
            }
            return Ok(id);
        }
        next = node.node_data().next_sibling;
    }
    Err(format!("ANode layout field '{declaration}' is unavailable"))
}

fn vector(engine: &crate::app::AppEngine, node: NodeId, value: [f64; 2]) -> Result<Edit, String> {
    if !value.into_iter().all(f64::is_finite) {
        return Err("ANode coordinates must be finite".into());
    }
    let parameter = engine
        .nodes
        .get(node)
        .and_then(Node::engine_param_snapshot)
        .ok_or("Layout parameter was retired")?;
    Ok(Edit::SetParam {
        node,
        value: ParamValue::Vec2(value[0], value[1]),
        behaviour: parameter.event_behaviour,
    })
}

pub(super) fn edit(
    engine: &mut crate::app::AppEngine,
    request: FormulaGraphEditRequest,
) -> Result<FormulaGraphEditResult, String> {
    owner(engine, &request.formula, true)?;
    let mut edits = Vec::new();
    let mut nodes = Vec::new();
    match request.edit {
        FormulaGraphEdit::Delete { nodes: selected } => {
            if selected.is_empty() || selected.len() > MAX_ITEMS {
                return Err("Delete requires 1..16384 graph items".into());
            }
            let mut remove = HashSet::new();
            let mut removed_anodes = HashSet::new();
            for identity in selected {
                let node = engine
                    .nodes
                    .get(identity.node)
                    .filter(|node| {
                        node.node_data().meta.uuid == identity.uuid
                            && node.node_data().parent == Some(request.formula.node)
                            && matches!(node.get_type(), ANODE_NODE_TYPE | CONNECTION_NODE_TYPE)
                            && node.node_data().meta.user_permissions.can_remove_and_duplicate
                    })
                    .ok_or("Graph item is stale, foreign or cannot be removed")?;
                if !remove.insert(identity.node) {
                    return Err("Delete graph identities must be unique".into());
                }
                if node.get_type() == ANODE_NODE_TYPE {
                    removed_anodes.insert(identity.uuid);
                }
                nodes.push(identity);
            }
            // One direct-sibling pass gathers incident edges; no snapshot rebuild per selected ANode.
            if !removed_anodes.is_empty() {
                let mut next = engine
                    .nodes
                    .get(request.formula.node)
                    .and_then(|node| node.node_data().first_child);
                while let Some(id) = next {
                    let node = engine.nodes.get(id).ok_or("Formula child was retired")?;
                    next = node.node_data().next_sibling;
                    if node.get_type() != CONNECTION_NODE_TYPE {
                        continue;
                    }
                    let mut child = node.node_data().first_child;
                    while let Some(parameter) = child {
                        let parameter = engine.nodes.get(parameter).ok_or("Connection field was retired")?;
                        child = parameter.node_data().next_sibling;
                        if matches!(parameter.node_data().meta.decl_id.0.as_str(), "source_node" | "target_node")
                            && parameter.engine_param_snapshot().is_some_and(|value| {
                                matches!(value.value, ParamValue::Reference(reference) if removed_anodes.contains(&reference.uuid()))
                            })
                        {
                            if !node.node_data().meta.user_permissions.can_remove_and_duplicate {
                                return Err("Incident connection cannot be removed".into());
                            }
                            remove.insert(id);
                            break;
                        }
                    }
                }
            }
            let mut remove = remove.into_iter().collect::<Vec<_>>();
            remove.sort_unstable_by_key(|node| node.0);
            edits.extend(remove.into_iter().map(|node| Edit::RemoveNode { node }));
        }
        FormulaGraphEdit::Move { moves } => {
            if moves.is_empty() || moves.len() > MAX_ITEMS {
                return Err("Move requires 1..16384 ANodes".into());
            }
            let mut seen = HashSet::new();
            for item in moves {
                anode(engine, &request.formula, &item.anode)?;
                if !seen.insert(item.anode.node) {
                    return Err("Move ANode identities must be unique".into());
                }
                let position = parameter(engine, item.anode.node, ANODE_POSITION_DECL_ID)?;
                edits.push(vector(engine, position, item.position)?);
                nodes.push(item.anode);
            }
        }
        FormulaGraphEdit::Resize { anode: identity, size } => {
            anode(engine, &request.formula, &identity)?;
            let node = parameter(engine, identity.node, "size")?;
            if !engine
                .nodes
                .get(node)
                .is_some_and(|node| node.node_data().meta.can_be_disabled)
            {
                return Err("ANode size cannot be disabled".into());
            }
            if let Some(size) = size {
                if !size.into_iter().all(|value| value > 0.0 && value.is_finite()) {
                    return Err("ANode size must be finite and positive".into());
                }
                edits.push(vector(engine, node, size)?);
            }
            edits.push(Edit::PatchMeta {
                node,
                patch: NodeMetaPatch {
                    enabled: Some(size.is_some()),
                    ..Default::default()
                },
            });
            nodes.push(identity);
        }
        FormulaGraphEdit::Rename { anode: identity, label } => {
            let node = anode(engine, &request.formula, &identity)?;
            let label = label.trim();
            if !node.node_data().meta.user_permissions.can_edit_name || label.is_empty() || label.len() > 1024 {
                return Err("ANode name is unavailable or invalid".into());
            }
            edits.push(Edit::PatchMeta {
                node: identity.node,
                patch: NodeMetaPatch {
                    label: Some(label.into()),
                    ..Default::default()
                },
            });
            nodes.push(identity);
        }
        FormulaGraphEdit::Collapse {
            anode: identity,
            collapsed,
        } => {
            let mut presentation = anode(engine, &request.formula, &identity)?
                .node_data()
                .meta
                .presentation
                .clone();
            presentation.collapsed = collapsed;
            edits.push(Edit::PatchMeta {
                node: identity.node,
                patch: NodeMetaPatch {
                    presentation: Some(presentation),
                    ..Default::default()
                },
            });
            nodes.push(identity);
        }
        FormulaGraphEdit::Enable {
            anode: identity,
            enabled,
        } => {
            if !anode(engine, &request.formula, &identity)?
                .node_data()
                .meta
                .can_be_disabled
            {
                return Err("ANode cannot be disabled".into());
            }
            edits.push(Edit::PatchMeta {
                node: identity.node,
                patch: NodeMetaPatch {
                    enabled: Some(enabled),
                    ..Default::default()
                },
            });
            nodes.push(identity);
        }
    }
    for edit in edits {
        engine.edits.push(edit);
    }
    engine.apply_edits().map_err(|error| error.to_string())?;
    Ok(FormulaGraphEditResult { nodes })
}
