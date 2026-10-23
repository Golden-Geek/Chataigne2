use super::*;
use golden_core::edit::{Edit, EditPreconditions, NodeReadGuard};
impl Kinect2Module {
    pub(super) fn guard_input(&self, ctx: &mut ProcessCtx, row: &kinect2_runtime::Prepared) -> bool {
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            return false;
        };
        let Some(ParamValue::Enum(space)) = snapshot
            .node(self.reference_space.id())
            .and_then(|n| n.param_value.as_ref())
        else {
            return false;
        };
        if KinectReferenceSpace::from_variant(space) != row.space {
            return false;
        }
        self.guard_projection(ctx)
    }
    pub(super) fn guard_projection(&self, ctx: &mut ProcessCtx) -> bool {
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
        if !collect("connection", self.base.connection_status_parameter(), "connected") {
            return false;
        }
        group!("parameters", reference_space);
        group!(
            "values/info",
            sensor_available,
            tracking_active,
            tracked_bodies,
            tracking_id,
            last_event
        );
        group!("values/hands", hands_distance, hands_rotation, hands_speed);
        group!(
            "values/joints",
            spine_base,
            spine_mid,
            neck,
            head,
            shoulder_left,
            elbow_left,
            wrist_left,
            hand_left,
            shoulder_right,
            elbow_right,
            wrist_right,
            hand_right,
            hip_left,
            knee_left,
            ankle_left,
            foot_left,
            hip_right,
            knee_right,
            ankle_right,
            foot_right,
            spine_shoulder,
            hand_tip_left,
            thumb_left,
            hand_tip_right,
            thumb_right
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
        let Some(settings) = NodeReadGuard::capture(&snapshot, self.reference_space.id()) else {
            return false;
        };
        reads.push(settings);
        for chunk in reads.chunks(32) {
            ctx.edits.push(Edit::CheckPreconditions {
                preconditions: EditPreconditions::new(chunk.to_vec(), vec![]).unwrap(),
            });
        }
        true
    }
}
