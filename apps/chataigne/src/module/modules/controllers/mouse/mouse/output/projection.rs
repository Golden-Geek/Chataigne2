use super::*;
use golden_core::edit::{Edit, EditPreconditions, NodeReadGuard};
fn collect_target(
    snapshot: &golden_core::process_ctx::ProcessTreeSnapshot,
    root: NodeId,
    path: &str,
    id: Option<NodeId>,
    name: &str,
    ids: &mut Vec<NodeId>,
) -> bool {
    let Some(id) = id else {
        return false;
    };
    let Some(node) = snapshot.node(id) else {
        return false;
    };
    if node.decl_id.rsplit('/').next() != Some(name) || node.param_value.is_none() {
        return false;
    }
    ids.push(id);
    let mut parent = node.parent;
    for part in path.rsplit('/') {
        let Some(id) = parent else {
            return false;
        };
        let Some(node) = snapshot.node(id) else {
            return false;
        };
        if node.decl_id.rsplit('/').next() != Some(part) {
            return false;
        }
        ids.push(id);
        parent = node.parent;
    }
    parent == Some(root)
}
impl MouseModule {
    pub(in super::super) fn guard_output(&self, ctx: &mut ProcessCtx) -> bool {
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            return false;
        };
        let mut ids = Vec::new();
        macro_rules! field {
            ($path:literal, $field:ident) => {
                if !collect_target(
                    &snapshot,
                    self.id(),
                    $path,
                    self.$field.is_bound().then(|| self.$field.id()),
                    stringify!($field),
                    &mut ids,
                ) {
                    return false;
                }
            };
        }
        field!("values/info", input_active);
        field!("values/info", output_active);
        field!("values/info", connected_devices);
        field!("values/info", device_id);
        field!("values/info", device_name);
        field!("values/info", last_event);
        if !collect_target(
            &snapshot,
            self.id(),
            "connection",
            self.base.connection_status_parameter(),
            "connected",
            &mut ids,
        ) {
            return false;
        }
        ids.sort_unstable_by_key(|id| id.0);
        ids.dedup();
        let Some(root) = NodeReadGuard::capture_identity(&snapshot, self.id()) else {
            return false;
        };
        let mut reads = vec![root];
        for id in ids {
            let Some(guard) = NodeReadGuard::capture_structure(&snapshot, id) else {
                return false;
            };
            reads.push(guard);
        }
        ctx.edits.push(Edit::CheckPreconditions {
            preconditions: EditPreconditions::new(reads, vec![]).unwrap(),
        });
        true
    }
    pub(in super::super) fn guard_output_settings(&self, ctx: &mut ProcessCtx) -> bool {
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            return false;
        };
        let ids = [
            self.device.is_bound().then(|| self.device.id()),
            self.receive_enabled.is_bound().then(|| self.receive_enabled.id()),
            self.send_enabled.is_bound().then(|| self.send_enabled.id()),
            self.capture_os_input.is_bound().then(|| self.capture_os_input.id()),
        ];
        let mut reads = Vec::with_capacity(4);
        for id in ids {
            let Some(guard) = id.and_then(|id| NodeReadGuard::capture(&snapshot, id)) else {
                return false;
            };
            reads.push(guard);
        }
        ctx.edits.push(Edit::CheckPreconditions {
            preconditions: EditPreconditions::new(reads, vec![]).unwrap(),
        });
        true
    }
}
