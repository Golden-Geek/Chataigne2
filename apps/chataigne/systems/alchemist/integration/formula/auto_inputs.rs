use super::*;

struct AutoInputTarget<'a> {
    uuid: NodeUuid,
    parameter: NodeId,
    value: Option<&'a ParamValue>,
    current: i32,
    prefix: &'static str,
}

fn target<'a>(
    snapshot: &'a ProcessTreeSnapshot,
    anode: NodeId,
    config: NodeId,
    type_id: &str,
) -> Option<AutoInputTarget<'a>> {
    let prefix = match type_id {
        "math" => "value",
        "concatenate" => "part",
        _ => return None,
    };
    let parameter = snapshot.find_child_by_decl_id(config, "config/num_inputs")?;
    let count = snapshot.node(parameter)?;
    if count.enabled {
        return None;
    }
    let current = match count.param_value.as_ref() {
        Some(ParamValue::Int(value)) => *value,
        _ => 2,
    }
    .clamp(2, 64);
    Some(AutoInputTarget {
        uuid: snapshot.node(anode)?.uuid,
        parameter,
        value: count.param_value.as_ref(),
        current,
        prefix,
    })
}

fn pending_counts(ctx: &ProcessCtx) -> HashSet<(NodeId, i32)> {
    ctx.edits
        .pending
        .iter()
        .filter_map(|request| match &request.edit {
            Edit::SetParam {
                node,
                value: ParamValue::Int(value),
                ..
            } => Some((*node, *value)),
            _ => None,
        })
        .collect()
}

fn queue_count<S: AsRef<str>>(
    ctx: &mut ProcessCtx,
    target: AutoInputTarget<'_>,
    sockets: impl IntoIterator<Item = S>,
    pending: &mut HashSet<(NodeId, i32)>,
) -> bool {
    let connected = sockets
        .into_iter()
        .filter_map(|socket| numbered_socket_index(socket.as_ref(), target.prefix))
        .collect::<HashSet<_>>();
    let all_connected = (1..=target.current).all(|index| connected.contains(&index));
    let highest = connected.iter().copied().max().unwrap_or(0);
    let desired = if all_connected {
        (target.current + 1).clamp(2, 64)
    } else if highest < target.current {
        (highest + 1).clamp(2, target.current)
    } else {
        target.current
    };
    let value = ParamValue::Int(desired);
    if target.value == Some(&value) {
        return false;
    }
    if !pending.insert((target.parameter, desired)) {
        return true;
    }
    ctx.edits.push(Edit::SetParam {
        node: target.parameter,
        value,
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    true
}

pub(super) fn sync_auto_input_count(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    anode: NodeId,
    config: NodeId,
    type_id: &str,
) {
    let Some(target) = target(snapshot, anode, config, type_id) else {
        return;
    };
    let Some(formula) = snapshot.node(anode).and_then(|node| node.parent) else {
        return;
    };
    let uuid = target.uuid;
    let sockets = snapshot.child_ids_slice(formula).iter().filter_map(|child| {
        let connection = snapshot.node(*child)?;
        if connection.node_type != CONNECTION_NODE_TYPE
            || child_reference_uuid(snapshot, *child, "target_node") != Some(uuid)
        {
            return None;
        }
        child_param(snapshot, *child, "target_socket").and_then(ParamValue::as_str)
    });
    let mut pending = pending_counts(ctx);
    queue_count(ctx, target, sockets, &mut pending);
}

/// One sibling pass collects incoming sockets; each target then reads only its own inputs.
pub(super) fn sync_auto_input_counts(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    formula: NodeId,
    skip_anode: Option<NodeId>,
) -> bool {
    let mut targets = Vec::new();
    for child in snapshot.child_ids_slice(formula) {
        if Some(*child) == skip_anode {
            continue;
        }
        let Some(anode) = snapshot.node(*child).filter(|node| node.node_type == ANODE_NODE_TYPE) else {
            continue;
        };
        let Some(type_id) = anode_type_from_tags(&anode.tags) else {
            continue;
        };
        if registry().get(&ANodeTypeId::new(&type_id)).is_none() {
            continue;
        }
        let Some(config) = snapshot.find_child_by_decl_id(*child, "config") else {
            continue;
        };
        if let Some(target) = target(snapshot, *child, config, &type_id) {
            targets.push(target);
        }
    }
    if targets.is_empty() {
        return false;
    }
    let mut incoming = HashMap::<NodeUuid, Vec<String>>::new();
    for child in snapshot.child_ids_slice(formula) {
        if !snapshot
            .node(*child)
            .is_some_and(|node| node.node_type == CONNECTION_NODE_TYPE)
        {
            continue;
        }
        let Some(uuid) = child_reference_uuid(snapshot, *child, "target_node") else {
            continue;
        };
        let Some(socket) = child_param(snapshot, *child, "target_socket").and_then(ParamValue::as_str) else {
            continue;
        };
        incoming.entry(uuid).or_default().push(socket);
    }
    let mut pending = pending_counts(ctx);
    let mut awaiting_counts = false;
    for target in targets {
        let sockets = incoming.get(&target.uuid).map(Vec::as_slice).unwrap_or_default();
        awaiting_counts |= queue_count(ctx, target, sockets.iter(), &mut pending);
    }
    awaiting_counts
}
