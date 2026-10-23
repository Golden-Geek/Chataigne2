mod kinect2_runtime;
mod projection;
mod service;
use golden_core::{
    engine::NodeExecutionRule,
    node,
    node::{Node, NodeCreationContext, NodeHandle, NodeId, NodeScriptDescriptor},
    parameter::{Enum, ParamValue, Vec2, Vec3},
    process_ctx::ProcessCtx,
};
use kinect2_runtime::{Kinect2Runtime, KinectReferenceSpace};
const KINECT2_RUNTIME_WARNING_ID: &str = "kinect2_runtime";
const KINECT2_SPACE_ABSOLUTE: &str = "absolute";
const KINECT2_SPACE_TORSO: &str = "torso";
const KINECT2_SPACE_HEAD: &str = "head";
const KINECT2_HAND_SPEED_RESET_SECS: f64 = 0.5;
const KINECT2_SCRIPT_METHODS: &[&str] = &[];
const KINECT2_MODULE_COMMAND_TYPES: &[&str] = &[];
#[node("kinect2_module", label = "Kinect 2")]
#[children(
    folder(connection) {
        [base_children];
    }
    folder(parameters) {
        reference_space: Enum = KINECT2_SPACE_ABSOLUTE (
            label = "Reference Space",
            description = "How skeleton joint positions are reported.",
            enum_options = ["absolute", "torso", "head"]
        );
        [base_children];
    }
    folder(values) {
        folder(info, label = "Info") {
            sensor_available: bool = false (
                label = "Sensor Available",
                description = "Whether the local Kinect 2 sensor is currently available.",
                read_only = true
            );
            tracking_active: bool = false (
                label = "Tracking Active",
                description = "Whether a primary tracked body is currently selected.",
                read_only = true
            );
            tracked_bodies: i32 = 0 [0..2147483647] (
                label = "Tracked Bodies",
                description = "Number of tracked bodies in the latest frame.",
                read_only = true
            );
            tracking_id: String = String::new() (
                label = "Tracking ID",
                description = "Tracking id of the selected body.",
                read_only = true
            );
            last_event: String = String::new() (
                label = "Last Event",
                description = "Last Kinect runtime or tracking event handled by this module.",
                read_only = true
            );
        }
        folder(hands, label = "Hands") {
            hands_distance: f64 = 0.0 [0.0..20.0] (
                label = "Distance",
                description = "Distance between left and right hands in the selected reference space.",
                read_only = true
            );
            hands_rotation: Vec2 = (0.0, 0.0) [(-1.0,-1.0)..(1.0,1.0)] (
                label = "Rotation XY",
                description = "Normalized XY direction from left hand to right hand.",
                read_only = true
            );
            hands_speed: Vec3 = (0.0, 0.0, 0.0) [(-50.0,-50.0,-50.0)..(50.0,50.0,50.0)] (
                label = "Speed",
                description = "Midpoint hand speed in units per second for the selected reference space.",
                read_only = true
            );
        }
        folder(joints, label = "Joints", collapsed = true) {
            spine_base: Vec3 = (0.0, 0.0, 0.0) [(-10.0,-10.0,-10.0)..(10.0,10.0,10.0)] (label = "Spine Base", read_only = true);
            spine_mid: Vec3 = (0.0, 0.0, 0.0) [(-10.0,-10.0,-10.0)..(10.0,10.0,10.0)] (label = "Spine Mid", read_only = true);
            neck: Vec3 = (0.0, 0.0, 0.0) [(-10.0,-10.0,-10.0)..(10.0,10.0,10.0)] (label = "Neck", read_only = true);
            head: Vec3 = (0.0, 0.0, 0.0) [(-10.0,-10.0,-10.0)..(10.0,10.0,10.0)] (label = "Head", read_only = true);
            shoulder_left: Vec3 = (0.0, 0.0, 0.0) [(-10.0,-10.0,-10.0)..(10.0,10.0,10.0)] (label = "Shoulder Left", read_only = true);
            elbow_left: Vec3 = (0.0, 0.0, 0.0) [(-10.0,-10.0,-10.0)..(10.0,10.0,10.0)] (label = "Elbow Left", read_only = true);
            wrist_left: Vec3 = (0.0, 0.0, 0.0) [(-10.0,-10.0,-10.0)..(10.0,10.0,10.0)] (label = "Wrist Left", read_only = true);
            hand_left: Vec3 = (0.0, 0.0, 0.0) [(-10.0,-10.0,-10.0)..(10.0,10.0,10.0)] (label = "Hand Left", read_only = true);
            shoulder_right: Vec3 = (0.0, 0.0, 0.0) [(-10.0,-10.0,-10.0)..(10.0,10.0,10.0)] (label = "Shoulder Right", read_only = true);
            elbow_right: Vec3 = (0.0, 0.0, 0.0) [(-10.0,-10.0,-10.0)..(10.0,10.0,10.0)] (label = "Elbow Right", read_only = true);
            wrist_right: Vec3 = (0.0, 0.0, 0.0) [(-10.0,-10.0,-10.0)..(10.0,10.0,10.0)] (label = "Wrist Right", read_only = true);
            hand_right: Vec3 = (0.0, 0.0, 0.0) [(-10.0,-10.0,-10.0)..(10.0,10.0,10.0)] (label = "Hand Right", read_only = true);
            hip_left: Vec3 = (0.0, 0.0, 0.0) [(-10.0,-10.0,-10.0)..(10.0,10.0,10.0)] (label = "Hip Left", read_only = true);
            knee_left: Vec3 = (0.0, 0.0, 0.0) [(-10.0,-10.0,-10.0)..(10.0,10.0,10.0)] (label = "Knee Left", read_only = true);
            ankle_left: Vec3 = (0.0, 0.0, 0.0) [(-10.0,-10.0,-10.0)..(10.0,10.0,10.0)] (label = "Ankle Left", read_only = true);
            foot_left: Vec3 = (0.0, 0.0, 0.0) [(-10.0,-10.0,-10.0)..(10.0,10.0,10.0)] (label = "Foot Left", read_only = true);
            hip_right: Vec3 = (0.0, 0.0, 0.0) [(-10.0,-10.0,-10.0)..(10.0,10.0,10.0)] (label = "Hip Right", read_only = true);
            knee_right: Vec3 = (0.0, 0.0, 0.0) [(-10.0,-10.0,-10.0)..(10.0,10.0,10.0)] (label = "Knee Right", read_only = true);
            ankle_right: Vec3 = (0.0, 0.0, 0.0) [(-10.0,-10.0,-10.0)..(10.0,10.0,10.0)] (label = "Ankle Right", read_only = true);
            foot_right: Vec3 = (0.0, 0.0, 0.0) [(-10.0,-10.0,-10.0)..(10.0,10.0,10.0)] (label = "Foot Right", read_only = true);
            spine_shoulder: Vec3 = (0.0, 0.0, 0.0) [(-10.0,-10.0,-10.0)..(10.0,10.0,10.0)] (label = "Spine Shoulder", read_only = true);
            hand_tip_left: Vec3 = (0.0, 0.0, 0.0) [(-10.0,-10.0,-10.0)..(10.0,10.0,10.0)] (label = "Hand Tip Left", read_only = true);
            thumb_left: Vec3 = (0.0, 0.0, 0.0) [(-10.0,-10.0,-10.0)..(10.0,10.0,10.0)] (label = "Thumb Left", read_only = true);
            hand_tip_right: Vec3 = (0.0, 0.0, 0.0) [(-10.0,-10.0,-10.0)..(10.0,10.0,10.0)] (label = "Hand Tip Right", read_only = true);
            thumb_right: Vec3 = (0.0, 0.0, 0.0) [(-10.0,-10.0,-10.0)..(10.0,10.0,10.0)] (label = "Thumb Right", read_only = true);
        }
        [base_children];
    }
)]
pub struct Kinect2Module {
    base: crate::app::ModuleBase,
    service: service::Service,
}
impl Kinect2Module {
    pub fn create() -> Self {
        Self::new(crate::app::ModuleBase::new(), service::Service::new())
    }
    #[cfg(test)]
    pub(crate) fn disable_runtime_for_test(&mut self) {
        self.stop_runtime();
        self.service.suppressed = true;
    }
    fn space(&self) -> KinectReferenceSpace {
        KinectReferenceSpace::from_variant(self.reference_space.get_ref().as_str())
    }
    fn project(&mut self, ctx: &mut ProcessCtx, row: &kinect2_runtime::Prepared) {
        self.base.set_connected(ctx, row.sensor_available);
        self.sensor_available.set(ctx, row.sensor_available);
        self.tracking_active.set(ctx, !row.tracking_id.is_empty());
        self.tracked_bodies.set(ctx, row.tracked_bodies);
        self.tracking_id.set(ctx, row.tracking_id.clone());
        if let Some(message) = &row.last_event {
            self.last_event.set(ctx, message.clone());
        }
        self.hands_distance.set(ctx, row.distance);
        self.hands_rotation.set(ctx, Vec2::new(row.rotation.0, row.rotation.1));
        self.hands_speed
            .set(ctx, Vec3::new(row.speed.x, row.speed.y, row.speed.z));
        let value = row.joints[0];
        self.spine_base.set(ctx, Vec3::new(value.x, value.y, value.z));
        let value = row.joints[1];
        self.spine_mid.set(ctx, Vec3::new(value.x, value.y, value.z));
        let value = row.joints[2];
        self.neck.set(ctx, Vec3::new(value.x, value.y, value.z));
        let value = row.joints[3];
        self.head.set(ctx, Vec3::new(value.x, value.y, value.z));
        let value = row.joints[4];
        self.shoulder_left.set(ctx, Vec3::new(value.x, value.y, value.z));
        let value = row.joints[5];
        self.elbow_left.set(ctx, Vec3::new(value.x, value.y, value.z));
        let value = row.joints[6];
        self.wrist_left.set(ctx, Vec3::new(value.x, value.y, value.z));
        let value = row.joints[7];
        self.hand_left.set(ctx, Vec3::new(value.x, value.y, value.z));
        let value = row.joints[8];
        self.shoulder_right.set(ctx, Vec3::new(value.x, value.y, value.z));
        let value = row.joints[9];
        self.elbow_right.set(ctx, Vec3::new(value.x, value.y, value.z));
        let value = row.joints[10];
        self.wrist_right.set(ctx, Vec3::new(value.x, value.y, value.z));
        let value = row.joints[11];
        self.hand_right.set(ctx, Vec3::new(value.x, value.y, value.z));
        let value = row.joints[12];
        self.hip_left.set(ctx, Vec3::new(value.x, value.y, value.z));
        let value = row.joints[13];
        self.knee_left.set(ctx, Vec3::new(value.x, value.y, value.z));
        let value = row.joints[14];
        self.ankle_left.set(ctx, Vec3::new(value.x, value.y, value.z));
        let value = row.joints[15];
        self.foot_left.set(ctx, Vec3::new(value.x, value.y, value.z));
        let value = row.joints[16];
        self.hip_right.set(ctx, Vec3::new(value.x, value.y, value.z));
        let value = row.joints[17];
        self.knee_right.set(ctx, Vec3::new(value.x, value.y, value.z));
        let value = row.joints[18];
        self.ankle_right.set(ctx, Vec3::new(value.x, value.y, value.z));
        let value = row.joints[19];
        self.foot_right.set(ctx, Vec3::new(value.x, value.y, value.z));
        let value = row.joints[20];
        self.spine_shoulder.set(ctx, Vec3::new(value.x, value.y, value.z));
        let value = row.joints[21];
        self.hand_tip_left.set(ctx, Vec3::new(value.x, value.y, value.z));
        let value = row.joints[22];
        self.thumb_left.set(ctx, Vec3::new(value.x, value.y, value.z));
        let value = row.joints[23];
        self.hand_tip_right.set(ctx, Vec3::new(value.x, value.y, value.z));
        let value = row.joints[24];
        self.thumb_right.set(ctx, Vec3::new(value.x, value.y, value.z));
        if row.incoming {
            self.base.emit_incoming_traffic(ctx);
        }
    }
    fn reset_all_values(&mut self, ctx: &mut ProcessCtx) {
        self.project(ctx, &kinect2_runtime::Prepared::empty(self.space()));
    }
    fn refresh_data_capabilities(&mut self, ctx: &mut ProcessCtx) {
        self.base.set_data_capabilities(
            ctx,
            crate::app::module::ModuleDataCapabilities::new(cfg!(windows), false),
        );
    }
    fn set_runtime_warning(&self, ctx: &mut ProcessCtx, error: &str) {
        NodeHandle::new(self.id()).set_warning_with(ctx, Some(KINECT2_RUNTIME_WARNING_ID), error, None);
    }
    fn clear_runtime_warning(&self, ctx: &mut ProcessCtx) {
        NodeHandle::new(self.id()).clear_warning(ctx, Some(KINECT2_RUNTIME_WARNING_ID));
    }
}
#[golden_core::item("module",node="kinect2_module",via=base,from_struct,menu_path=["Controllers"])]
impl Node for Kinect2Module {
    fn init(&mut self, ctx: &mut ProcessCtx) {
        self.base.init(ctx);
        self.base.configure_command_tester(ctx, KINECT2_MODULE_COMMAND_TYPES);
        self.refresh_data_capabilities(ctx);
        self.reset_all_values(ctx);
    }
    fn on_node_ready(&mut self, ctx: &mut ProcessCtx, _context: NodeCreationContext) {
        if let Err(error) = self.register_service(ctx) {
            self.set_runtime_warning(ctx, &error);
        }
        self.request_service();
    }
    fn on_worker_wake(&mut self, ctx: &mut ProcessCtx) {
        self.service_runtime(ctx);
    }
    fn worker_wake_requires_tree_snapshot(&self) -> bool {
        true
    }
    fn destroy(&mut self, _ctx: &mut ProcessCtx) {
        self.stop_runtime();
    }
    fn execution_rule(&self) -> NodeExecutionRule {
        NodeExecutionRule::passive().with_compiled_kernel("chataigne.runtime.kinect2")
    }
    fn update_requires_tree_snapshot(&self) -> bool {
        false
    }
    fn engine_script_descriptor(&self) -> NodeScriptDescriptor {
        crate::app::module::script_api::descriptor_for_node(self.node_data(), self.get_type(), KINECT2_SCRIPT_METHODS)
    }
    fn engine_call_script_method(
        &mut self,
        ctx: &mut ProcessCtx,
        method: &str,
        args: &[ParamValue],
    ) -> Result<bool, String> {
        self.base.engine_call_script_method(ctx, method, args)
    }
    fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, old_value: ParamValue) {
        if let Some(snapshot) = ctx.tree_snapshot_arc() {
            self.base.emit_script_param_callback(ctx, &snapshot, param, &old_value);
        }
        if self.reference_space.is_bound() && self.reference_space.id() == param {
            self.configure_runtime();
            self.request_service();
        }
    }
    fn on_effective_enabled_changed(&mut self, ctx: &mut ProcessCtx, enabled: bool) {
        if enabled {
            self.service.retry_at = None;
            self.refresh_data_capabilities(ctx);
            self.request_service();
        } else {
            self.stop_runtime();
            self.clear_runtime_warning(ctx);
            NodeHandle::new(self.id()).clear_warning(ctx, Some("kinect2_pressure"));
            self.reset_all_values(ctx);
        }
    }
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::create)
    }
}
#[cfg(test)]
mod tests;
