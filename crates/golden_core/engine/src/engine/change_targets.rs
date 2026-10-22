use crate::events::EventKind;

pub(super) fn visit_structure_changed_nodes(kind: &EventKind, visit: &mut impl FnMut(crate::node::NodeId)) {
    match kind {
        EventKind::ParamChanged { .. }
        | EventKind::ParamControlChanged { .. }
        | EventKind::ParamConstraintsChanged { .. }
        | EventKind::Custom(_) => {}
        EventKind::MetaChanged { node, patch } => {
            if patch.label.is_some() || patch.short_name.is_some() || patch.enabled.is_some() {
                visit(*node);
            }
        }
        EventKind::GraphTransaction { transaction } => {
            use crate::ui_sync::UiGraphOp;
            for op in &transaction.ops {
                match op {
                    UiGraphOp::ParamPatched { .. }
                    | UiGraphOp::HistoryPatched { .. }
                    | UiGraphOp::LoggerPatched { .. } => {}
                    UiGraphOp::NodeMetaPatched { node, patch } => {
                        if patch.label.is_some() || patch.short_name.is_some() || patch.enabled.is_some() {
                            visit(*node);
                        }
                    }
                    _ => visit_graph_op_changed_nodes(op, visit),
                }
            }
        }
        _ => visit_changed_nodes(kind, visit),
    }
}

pub(super) fn visit_changed_nodes(kind: &EventKind, visit: &mut impl FnMut(crate::node::NodeId)) {
    match kind {
        EventKind::ParamChanged { param, .. }
        | EventKind::ParamControlChanged { param, .. }
        | EventKind::ParamConstraintsChanged { param, .. } => {
            visit(*param);
        }
        EventKind::ChildAdded { parent, child, .. } | EventKind::ChildRemoved { parent, child } => {
            for node in [*parent, *child] {
                visit(node);
            }
        }
        EventKind::ChildReplaced { parent, old, new, .. } => {
            for node in [*parent, *old, *new] {
                visit(node);
            }
        }
        EventKind::ChildMoved {
            child,
            old_parent,
            new_parent,
        } => {
            for node in [*child, *old_parent, *new_parent] {
                visit(node);
            }
        }
        EventKind::ChildReordered { parent, child } => {
            for node in [*parent, *child] {
                visit(node);
            }
        }
        EventKind::NodeCreated { node } | EventKind::NodeDeleted { node } | EventKind::MetaChanged { node, .. } => {
            visit(*node);
        }
        EventKind::GraphTransaction { transaction } => {
            for op in &transaction.ops {
                visit_graph_op_changed_nodes(op, visit);
            }
        }
        EventKind::Custom(_) => {}
    }
}

fn visit_graph_op_changed_nodes(op: &crate::ui_sync::UiGraphOp, visit: &mut impl FnMut(crate::node::NodeId)) {
    use crate::ui_sync::UiGraphOp;

    match op {
        UiGraphOp::NodeCreated { snapshot, parent, .. } => {
            visit(snapshot.node_id);
            for node in parent.iter().copied() {
                visit(node);
            }
        }
        UiGraphOp::SubtreeInserted { parent, nodes, .. } => {
            visit(*parent);
            for node in nodes.iter().map(|node| node.node_id) {
                visit(node);
            }
        }
        UiGraphOp::SubtreeRemoved {
            removed_ids,
            parent_after,
            ..
        } => {
            for node in removed_ids.iter().copied() {
                visit(node);
            }
            if let Some(parent) = parent_after {
                visit(parent.parent);
            }
        }
        UiGraphOp::NodeMoved {
            node,
            old_parent,
            new_parent,
            ..
        } => {
            visit(*node);
            for node in old_parent.iter().copied() {
                visit(node);
            }
            for node in new_parent.iter().copied() {
                visit(node);
            }
        }
        UiGraphOp::ChildrenReordered { parent, .. } => {
            visit(*parent);
        }
        UiGraphOp::ChildrenInserted { parent, children, .. } => {
            visit(*parent);
            for node in children.iter().copied() {
                visit(node);
            }
        }
        UiGraphOp::NodeMetaPatched { node, .. } => {
            visit(*node);
        }
        UiGraphOp::ParamPatched { node, param, .. } => {
            for node in [*node, *param] {
                visit(node);
            }
        }
        UiGraphOp::HistoryPatched { .. } | UiGraphOp::LoggerPatched { .. } => {}
    }
}
