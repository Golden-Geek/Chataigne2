use super::*;
use golden_core::edit::{Edit, EditPreconditions, NodeReadGuard};
impl UltraleapModule {
    pub(super) fn guard_projection(&self, ctx: &mut ProcessCtx) -> bool {
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            return false;
        };
        let mut ids = Vec::with_capacity(64);
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
        macro_rules! group {
            ($path:literal,$($field:ident),+ $(,)?)=>{
                $(if !collect($path,self.$field.is_bound().then(||self.$field.id()),stringify!($field)) {return false;})+
            };
        }
        if !collect("connection", self.base.connection_status_parameter(), "connected") {
            return false;
        }
        group!(
            "values/info",
            service_connected,
            device_available,
            tracking_active,
            connected_devices,
            visible_hands,
            last_event
        );
        group!("values/metrics", hands_distance);
        group!(
            "values/left_hand",
            left_active,
            left_grab_strength,
            left_pinch_strength,
            left_pinch_distance,
            left_thumb_extended,
            left_index_extended,
            left_middle_extended,
            left_ring_extended,
            left_pinky_extended,
            left_palm_position,
            left_palm_stabilized_position,
            left_palm_velocity,
            left_palm_direction,
            left_palm_normal
        );
        group!(
            "values/right_hand",
            right_active,
            right_grab_strength,
            right_pinch_strength,
            right_pinch_distance,
            right_thumb_extended,
            right_index_extended,
            right_middle_extended,
            right_ring_extended,
            right_pinky_extended,
            right_palm_position,
            right_palm_stabilized_position,
            right_palm_velocity,
            right_palm_direction,
            right_palm_normal
        );
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
        for chunk in reads.chunks(32) {
            ctx.edits.push(Edit::CheckPreconditions {
                preconditions: EditPreconditions::new(chunk.to_vec(), vec![]).unwrap(),
            });
        }
        true
    }
}
