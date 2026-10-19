use super::{DeclId, Node, NodeId, NodeMetaPatch};
use crate::{
    events::{CustomEvent, EventKind},
    parameter::{ParamValue, ParameterConstraints, ParameterControlState},
    process_ctx::ProcessCtx,
};

enum InboxDispatchEvent {
    ParamChanged {
        param: NodeId,
        old_value: ParamValue,
    },
    ParamControlChanged {
        param: NodeId,
        old_state: ParameterControlState,
        new_state: ParameterControlState,
    },
    ParamConstraintsChanged {
        param: NodeId,
        old_constraints: ParameterConstraints,
        new_constraints: Box<ParameterConstraints>,
    },
    ChildAdded {
        parent: NodeId,
        child: NodeId,
        decl_id: DeclId,
    },
    ChildRemoved {
        parent: NodeId,
        child: NodeId,
    },
    ChildReplaced {
        parent: NodeId,
        old: NodeId,
        new: NodeId,
        decl_id: DeclId,
    },
    ChildMoved {
        child: NodeId,
        old_parent: NodeId,
        new_parent: NodeId,
    },
    ChildReordered {
        parent: NodeId,
        child: NodeId,
    },
    NodeCreated {
        node: NodeId,
    },
    NodeDeleted {
        node: NodeId,
    },
    MetaChanged {
        node: NodeId,
        patch: NodeMetaPatch,
    },
    GraphTransaction,
    Custom(CustomEvent),
}

pub(super) fn dispatch_inbox<N: Node + ?Sized>(node: &mut N, ctx: &mut ProcessCtx) {
    if ctx.has_structure_changed() {
        node.on_structure_changed(ctx);
    }

    for index in 0..ctx.events.len() {
        let event = match &ctx.events[index].kind {
            EventKind::ParamChanged { param, old_value, .. } => InboxDispatchEvent::ParamChanged {
                param: *param,
                old_value: old_value.clone(),
            },
            EventKind::ParamControlChanged {
                param,
                old_state,
                new_state,
            } => InboxDispatchEvent::ParamControlChanged {
                param: *param,
                old_state: old_state.clone(),
                new_state: new_state.clone(),
            },
            EventKind::ParamConstraintsChanged {
                param,
                old_constraints,
                new_constraints,
            } => InboxDispatchEvent::ParamConstraintsChanged {
                param: *param,
                old_constraints: old_constraints.clone(),
                new_constraints: new_constraints.clone(),
            },
            EventKind::ChildAdded { parent, child, decl_id } => InboxDispatchEvent::ChildAdded {
                parent: *parent,
                child: *child,
                decl_id: decl_id.clone(),
            },
            EventKind::ChildRemoved { parent, child } => InboxDispatchEvent::ChildRemoved {
                parent: *parent,
                child: *child,
            },
            EventKind::ChildReplaced {
                parent,
                old,
                new,
                decl_id,
            } => InboxDispatchEvent::ChildReplaced {
                parent: *parent,
                old: *old,
                new: *new,
                decl_id: decl_id.clone(),
            },
            EventKind::ChildMoved {
                child,
                old_parent,
                new_parent,
            } => InboxDispatchEvent::ChildMoved {
                child: *child,
                old_parent: *old_parent,
                new_parent: *new_parent,
            },
            EventKind::ChildReordered { parent, child } => InboxDispatchEvent::ChildReordered {
                parent: *parent,
                child: *child,
            },
            EventKind::NodeCreated { node } => InboxDispatchEvent::NodeCreated { node: *node },
            EventKind::NodeDeleted { node } => InboxDispatchEvent::NodeDeleted { node: *node },
            EventKind::MetaChanged { node, patch } => InboxDispatchEvent::MetaChanged {
                node: *node,
                patch: patch.clone(),
            },
            EventKind::GraphTransaction { .. } => {
                // Handled implicitly by on_structure_changed or individual ops if destructured later.
                InboxDispatchEvent::GraphTransaction
            }
            EventKind::Custom(event) => InboxDispatchEvent::Custom(event.clone()),
        };
        match event {
            InboxDispatchEvent::ParamChanged { param, old_value } => {
                node.on_param_change(ctx, param, old_value);
            }
            InboxDispatchEvent::ParamControlChanged {
                param,
                old_state,
                new_state,
            } => {
                node.on_param_control_changed(ctx, param, old_state, new_state);
            }
            InboxDispatchEvent::ParamConstraintsChanged {
                param,
                old_constraints,
                new_constraints,
            } => {
                node.on_param_constraints_changed(ctx, param, old_constraints, *new_constraints);
            }
            InboxDispatchEvent::ChildAdded { parent, child, decl_id } => {
                node.on_child_added_decl(ctx, parent, child, &decl_id);
            }
            InboxDispatchEvent::ChildRemoved { parent, child } => {
                node.on_child_removed(ctx, parent, child);
            }
            InboxDispatchEvent::ChildReplaced {
                parent,
                old,
                new,
                decl_id,
            } => {
                node.on_child_replaced_decl(ctx, parent, old, new, &decl_id);
            }
            InboxDispatchEvent::ChildMoved {
                child,
                old_parent,
                new_parent,
            } => {
                node.on_child_moved(ctx, child, old_parent, new_parent);
            }
            InboxDispatchEvent::ChildReordered { parent, child } => {
                node.on_child_reordered(ctx, parent, child);
            }
            InboxDispatchEvent::NodeCreated { node: created } => {
                node.on_node_created(ctx, created);
            }
            InboxDispatchEvent::NodeDeleted { node: deleted } => {
                node.on_node_deleted(ctx, deleted);
            }
            InboxDispatchEvent::MetaChanged { node: changed, patch } => {
                node.on_meta_changed(ctx, changed, patch);
            }
            InboxDispatchEvent::GraphTransaction => {}
            InboxDispatchEvent::Custom(event) => {
                node.on_custom_event(ctx, event);
            }
        }
    }
}
