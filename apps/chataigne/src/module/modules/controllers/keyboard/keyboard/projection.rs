use super::*;
use golden_core::edit::{Edit, EditPreconditions, NodeReadGuard};

impl KeyboardModule {
    pub(super) fn guard_input(&self, ctx: &mut ProcessCtx, publication: &keyboard_runtime::Publication) -> bool {
        if !publication.current() {
            return false;
        }
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            return false;
        };
        let catalog = &publication.prepared.catalog;
        if !snapshot.node(self.id()).is_some_and(|node| node.enabled)
            || snapshot
                .node(self.receive_enabled.id())
                .and_then(|node| node.param_value.as_ref())
                != Some(&ParamValue::Bool(true))
        {
            return false;
        }
        let Some(ParamValue::Enum(selection)) = snapshot
            .node(self.device.id())
            .and_then(|node| node.param_value.as_ref())
        else {
            return false;
        };
        if selection != &catalog.requested && selection != &catalog.selection {
            return false;
        }
        let mut ids = Vec::new();
        let mut collect =
            |path: &str, id: Option<NodeId>, name: &str| collect_target(&snapshot, self.id(), path, id, name, &mut ids);
        macro_rules! group {
            ($path:literal, $($field:ident),+ $(,)?) => {
                $(if !collect($path, self.$field.is_bound().then(|| self.$field.id()), stringify!($field)) { return false; })+
            };
        }
        group!("connection", device, receive_enabled, send_enabled);
        if !collect("connection", self.base.connection_status_parameter(), "connected") {
            return false;
        }
        group!(
            "values/info",
            input_active,
            output_active,
            connected_devices,
            device_id,
            device_name,
            last_event,
            last_key,
            last_pressed,
            held_keys,
            held_key_count
        );
        group!(
            "values/modifiers",
            left_shift,
            right_shift,
            left_control,
            right_control,
            left_alt,
            right_alt,
            left_meta,
            right_meta,
            caps_lock
        );
        ids.sort_unstable_by_key(|id| id.0);
        ids.dedup();
        let mut reads = Vec::with_capacity(32);
        let Some(root) = NodeReadGuard::capture_identity(&snapshot, self.id()) else {
            return false;
        };
        reads.push(root);
        for id in ids {
            let Some(guard) = NodeReadGuard::capture_structure(&snapshot, id) else {
                return false;
            };
            reads.push(guard);
        }
        for id in [self.device.id(), self.receive_enabled.id(), self.send_enabled.id()] {
            let Some(guard) = NodeReadGuard::capture(&snapshot, id) else {
                return false;
            };
            reads.push(guard);
        }
        // Capture the whole turn before catalogs, values or callbacks can be queued.
        for chunk in reads.chunks(32) {
            ctx.edits.push(Edit::CheckPreconditions {
                preconditions: EditPreconditions::new(chunk.to_vec(), publication.guards.to_vec()).unwrap(),
            });
        }
        true
    }
}

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
impl KeyboardModule {
    pub(super) fn guard_output(&self, ctx: &mut ProcessCtx) -> bool {
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
    pub(super) fn guard_output_settings(&self, ctx: &mut ProcessCtx) -> bool {
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            return false;
        };
        let ids = [
            self.device.is_bound().then(|| self.device.id()),
            self.receive_enabled.is_bound().then(|| self.receive_enabled.id()),
            self.send_enabled.is_bound().then(|| self.send_enabled.id()),
        ];
        let mut reads = Vec::with_capacity(3);
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
