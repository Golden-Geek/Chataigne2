use super::*;
use golden_core::edit::{Edit, EditPreconditions, NodeReadGuard};
impl GamepadModule {
    pub(super) fn calibration_ids(&self) -> [NodeId; 16] {
        [
            self.left_stick_x_dead_zone.id(),
            self.left_stick_x_offset.id(),
            self.left_stick_y_dead_zone.id(),
            self.left_stick_y_offset.id(),
            self.left_z_dead_zone.id(),
            self.left_z_offset.id(),
            self.right_stick_x_dead_zone.id(),
            self.right_stick_x_offset.id(),
            self.right_stick_y_dead_zone.id(),
            self.right_stick_y_offset.id(),
            self.right_z_dead_zone.id(),
            self.right_z_offset.id(),
            self.dpad_x_dead_zone.id(),
            self.dpad_x_offset.id(),
            self.dpad_y_dead_zone.id(),
            self.dpad_y_offset.id(),
        ]
    }
    pub(super) fn guard_input(&self, ctx: &mut ProcessCtx, row: &gamepad_runtime::Publication) -> bool {
        if !row.current() {
            return false;
        }
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            return false;
        };
        let catalog = &row.prepared.catalog;
        let value = snapshot.node(self.device.id()).and_then(|n| n.param_value.as_ref());
        if value != Some(&ParamValue::Enum(catalog.settings.requested.clone()))
            && value != Some(&ParamValue::Enum(catalog.selection.clone()))
        {
            return false;
        }
        for (id, value) in self
            .calibration_ids()
            .into_iter()
            .zip(catalog.settings.axes.into_iter().flat_map(|a| [a.dead_zone, a.offset]))
        {
            if snapshot.node(id).and_then(|n| n.param_value.as_ref()) != Some(&ParamValue::Float(value)) {
                return false;
            }
        }
        self.guard_targets(ctx)
    }
    pub(super) fn guard_targets(&self, ctx: &mut ProcessCtx) -> bool {
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            return false;
        };
        let mut ids = Vec::with_capacity(128);
        let mut collect = |path: &str, id: Option<NodeId>, name: &str| {
            let Some(id) = id else {
                return false;
            };
            let Some(node) = snapshot.node(id) else {
                return false;
            };
            if node.param_value.is_none() || node.decl_id.rsplit('/').next() != Some(name) {
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
            parent == Some(self.id())
        };
        macro_rules! group {($path:literal,$($field:ident),+)=>{$(if !collect($path,self.$field.is_bound().then(||self.$field.id()),stringify!($field)) {return false;})+};}
        group!("connection", device);
        if !collect("connection", self.base.connection_status_parameter(), "connected") {
            return false;
        }
        group!(
            "values/info",
            active,
            connected_devices,
            device_index,
            device_id,
            device_name,
            last_event
        );
        group!("values/axes", left_stick, left_z, right_stick, right_z, dpad);
        group!(
            "values/buttons",
            south,
            east,
            north,
            west,
            c,
            z,
            left_trigger,
            left_trigger_2,
            right_trigger,
            right_trigger_2,
            select,
            start,
            mode,
            left_thumb,
            right_thumb,
            dpad_up,
            dpad_down,
            dpad_left,
            dpad_right
        );
        group!(
            "parameters/axis_processing/left_stick_x",
            left_stick_x_dead_zone,
            left_stick_x_offset
        );
        group!(
            "parameters/axis_processing/left_stick_y",
            left_stick_y_dead_zone,
            left_stick_y_offset
        );
        group!("parameters/axis_processing/left_z", left_z_dead_zone, left_z_offset);
        group!(
            "parameters/axis_processing/right_stick_x",
            right_stick_x_dead_zone,
            right_stick_x_offset
        );
        group!(
            "parameters/axis_processing/right_stick_y",
            right_stick_y_dead_zone,
            right_stick_y_offset
        );
        group!("parameters/axis_processing/right_z", right_z_dead_zone, right_z_offset);
        group!("parameters/axis_processing/dpad_x", dpad_x_dead_zone, dpad_x_offset);
        group!("parameters/axis_processing/dpad_y", dpad_y_dead_zone, dpad_y_offset);
        ids.sort_unstable_by_key(|id| id.0);
        ids.dedup();
        let Some(root) = NodeReadGuard::capture_identity(&snapshot, self.id()) else {
            return false;
        };
        let mut reads = vec![root];
        for id in ids {
            let Some(read) = NodeReadGuard::capture_structure(&snapshot, id) else {
                return false;
            };
            reads.push(read);
        }
        for id in std::iter::once(self.device.id()).chain(self.calibration_ids()) {
            let Some(read) = NodeReadGuard::capture(&snapshot, id) else {
                return false;
            };
            reads.push(read);
        }
        for chunk in reads.chunks(32) {
            ctx.edits.push(Edit::CheckPreconditions {
                preconditions: EditPreconditions::new(chunk.to_vec(), vec![]).unwrap(),
            });
        }
        true
    }
}
