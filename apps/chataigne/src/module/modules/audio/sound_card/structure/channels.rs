use super::*;
use service::{Position, Repair, NAME_BYTES, WRITE_TURN};

fn count(snapshot: &ProcessTreeSnapshot, module: NodeId, direction: &str) -> usize {
    if direction_used(snapshot, module, direction) {
        direction_channel_count(
            snapshot,
            module,
            &format!("connection/{direction}_routing/channel_count"),
        )
    } else {
        0
    }
}

fn channel_uuid(namespace: NodeUuid, direction: &str, index: usize) -> NodeUuid {
    derived_uuid(namespace, format!("{direction}-channel-{}", index + 1).as_bytes())
}

fn constraints(min: f64, max: f64) -> ParameterConstraints {
    ParameterConstraints {
        range: RangeConstraint::uniform(Some(min), Some(max)),
        ..ParameterConstraints::default()
    }
}

fn clear_children(snapshot: &ProcessTreeSnapshot, state: &ProcessTreeNodeSnapshot) -> Option<Repair> {
    let children = snapshot.child_ids_slice(state.id);
    if children.is_empty() {
        return None;
    }
    let mut reads = vec![state.id];
    let edits = children
        .iter()
        .take(WRITE_TURN)
        .map(|&node| {
            reads.push(node);
            Edit::RemoveNode { node }
        })
        .collect();
    Some(Repair {
        edits,
        reads,
        revisit: true,
    })
}

pub(super) fn desired(
    snapshot: &ProcessTreeSnapshot,
    module: NodeId,
    direction: &str,
    pos: &mut Position,
    level: bool,
    wanted: &mut std::collections::HashSet<NodeUuid>,
) -> Result<Option<Repair>, String> {
    let index = pos.row;
    if index >= count(snapshot, module, direction) {
        advance(pos);
        return Ok(None);
    }
    let namespace = snapshot.node(module).ok_or("Sound Card retired")?.uuid;
    let gain_uuid = channel_uuid(namespace, direction, index);
    let uuid = if level {
        derived_uuid(gain_uuid, b"sound-card-channel-value")
    } else {
        gain_uuid
    };
    wanted.insert(uuid);
    let path = if level {
        format!("values/levels/{direction}/channels")
    } else {
        format!("parameters/{direction}/channels")
    };
    let Some(parent) = find_path(snapshot, module, &path) else {
        advance(pos);
        return Ok(None);
    };
    let gain = snapshot.node_id_by_uuid(gain_uuid).and_then(|id| snapshot.node(id));
    let name = gain.map(|n| n.label.strip_suffix(" Gain").unwrap_or(&n.label));
    if name.is_some_and(|n| n.len() > NAME_BYTES) {
        return Err("Sound Card channel name exceeds 65536 bytes".into());
    }
    let name = name
        .map(str::to_owned)
        .unwrap_or_else(|| format!("{} {}", title_case(direction), index + 1));
    let decl = if level {
        format!("channel_{}", gain_uuid.0.simple())
    } else {
        format!("{direction}_{}", index + 1)
    };
    let expected = if level {
        constraints(0.0, 1.0)
    } else {
        constraints(-120.0, 24.0)
    };
    let mut reads = vec![parent];
    if level {
        if let Some(gain) = gain {
            reads.push(gain.id);
        }
    }
    let edits = if let Some(state) = snapshot.node_id_by_uuid(uuid).and_then(|id| snapshot.node(id)) {
        if state.parent != Some(parent) {
            return Err("Sound Card derived channel identity belongs to another folder".into());
        }
        reads.push(state.id);
        if !matches!(state.param_value, Some(ParamValue::Float(_))) {
            let old_value = find_child_by_key(snapshot, state.id, if level { "volume" } else { "volume_db" })
                .and_then(|id| {
                    reads.push(id);
                    snapshot.node(id)
                })
                .and_then(|n| n.param_value.as_ref())
                .and_then(ParamValue::as_float)
                .unwrap_or(0.0);
            let parameter = if level {
                channel_level_parameter(uuid, &name, &decl, old_value)
            } else {
                channel_gain_parameter(uuid, &name, &decl, old_value)
            };
            // Replacement preserves topology. Capture the value first, then retire legacy
            // children in later bounded turns; a failed suffix cannot erase the only copy.
            return Ok(Some(Repair {
                edits: vec![Edit::ReplaceNode {
                    node: state.id,
                    new_node: Box::new(parameter),
                }],
                reads,
                revisit: true,
            }));
        }
        if let Some(repair) = clear_children(snapshot, state) {
            return Ok(Some(repair));
        }
        let mut edits = Vec::with_capacity(2);
        let label = if level { name.clone() } else { channel_gain_label(&name) };
        let short_name = if level {
            None
        } else {
            (state.short_name != name).then_some(name)
        };
        if state.label != label || short_name.is_some() {
            edits.push(Edit::PatchMeta {
                node: state.id,
                patch: NodeMetaPatch {
                    label: (state.label != label).then_some(label),
                    short_name,
                    ..NodeMetaPatch::default()
                },
            });
        }
        if state.param_constraints.as_ref() != Some(&expected) {
            edits.push(Edit::SetParamConstraints {
                node: state.id,
                constraints: expected,
            });
        }
        edits
    } else {
        let parameter = if level {
            channel_level_parameter(uuid, &name, &decl, 0.0)
        } else {
            channel_gain_parameter(uuid, &name, &decl, 0.0)
        };
        vec![Edit::AddNodeTree {
            parent,
            tree: NodeTree::new(parameter),
            prev_sibling: None,
        }]
    };
    pos.row += 1;
    Ok((!edits.is_empty()).then(|| Repair::new(edits, reads)))
}

pub(super) fn stale(
    source: &ProcessTreeSnapshot,
    snapshot: &ProcessTreeSnapshot,
    module: NodeId,
    direction: &str,
    pos: &mut Position,
    level: bool,
    wanted: &std::collections::HashSet<NodeUuid>,
) -> Option<Repair> {
    let path = if level {
        format!("values/levels/{direction}/channels")
    } else {
        format!("parameters/{direction}/channels")
    };
    let state = scan_child(source, snapshot, module, &path, pos)?;
    (!wanted.contains(&state.uuid)).then(|| {
        Repair::new(
            vec![Edit::RemoveNode { node: state.id }],
            vec![state.id, state.parent.unwrap()],
        )
    })
}
