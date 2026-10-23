mod projection;
mod service;
mod ultraleap_runtime;

use golden_core::{
    engine::NodeExecutionRule,
    node,
    node::{Node, NodeCreationContext, NodeHandle, NodeId, NodeScriptDescriptor},
    parameter::{ParamValue, Vec3},
    process_ctx::ProcessCtx,
};

use self::ultraleap_runtime::{UltraleapFrameSnapshot, UltraleapHandSnapshot, UltraleapRuntime, UltraleapVec3};

const ULTRALEAP_RUNTIME_WARNING_ID: &str = "ultraleap_runtime";

const ULTRALEAP_SCRIPT_METHODS: &[&str] = &[];
const ULTRALEAP_MODULE_COMMAND_TYPES: &[&str] = &[];

#[node("ultraleap_module", label = "Ultraleap")]
#[children(
    folder(connection) {
        [base_children];
    }
    folder(parameters) {
        [base_children];
    }
    folder(values) {
        folder(info, label = "Info") {
            service_connected: bool = false (
                label = "Service Connected",
                description = "Whether the local Ultraleap Tracking Service connection is active.",
                read_only = true
            );
            device_available: bool = false (
                label = "Device Available",
                description = "Whether an Ultraleap device is currently available to the runtime.",
                read_only = true
            );
            tracking_active: bool = false (
                label = "Tracking Active",
                description = "Whether at least one hand is visible in the latest tracking frame.",
                read_only = true
            );
            connected_devices: i32 = 0 (
                label = "Connected Devices",
                description = "Number of Ultraleap devices visible to the runtime.",
                read_only = true
            );
            visible_hands: i32 = 0 [0..2] (
                label = "Visible Hands",
                description = "Number of hands visible in the latest tracking frame.",
                read_only = true
            );
            last_event: String = String::new() (
                label = "Last Event",
                description = "Last Ultraleap runtime or tracking event handled by this module.",
                read_only = true
            );
        }
        folder(metrics, label = "Metrics") {
            hands_distance: f64 = 0.0 [0.0..2.0] (
                label = "Hands Distance",
                description = "Distance in meters between left and right palm positions when both hands are visible.",
                read_only = true
            );
        }
        folder(left_hand, label = "Left Hand", collapsed = true) {
            left_active: bool = false (label = "Active", read_only = true);
            left_grab_strength: f64 = 0.0 [0.0..1.0] (label = "Grab Strength", read_only = true);
            left_pinch_strength: f64 = 0.0 [0.0..1.0] (label = "Pinch Strength", read_only = true);
            left_pinch_distance: f64 = 0.0 [0.0..0.2] (label = "Pinch Distance", read_only = true);
            left_thumb_extended: bool = false (label = "Thumb Extended", read_only = true);
            left_index_extended: bool = false (label = "Index Extended", read_only = true);
            left_middle_extended: bool = false (label = "Middle Extended", read_only = true);
            left_ring_extended: bool = false (label = "Ring Extended", read_only = true);
            left_pinky_extended: bool = false (label = "Pinky Extended", read_only = true);
            left_palm_position: Vec3 = (0.0, 0.0, 0.0) [(-1.0,-1.0,-1.0)..(1.0,1.0,1.0)] (label = "Palm Position", read_only = true);
            left_palm_stabilized_position: Vec3 = (0.0, 0.0, 0.0) [(-1.0,-1.0,-1.0)..(1.0,1.0,1.0)] (label = "Stabilized Position", read_only = true);
            left_palm_velocity: Vec3 = (0.0, 0.0, 0.0) [(-3.0,-3.0,-3.0)..(3.0,3.0,3.0)] (label = "Palm Velocity", read_only = true);
            left_palm_direction: Vec3 = (0.0, 0.0, 0.0) [(-1.0,-1.0,-1.0)..(1.0,1.0,1.0)] (label = "Palm Direction", read_only = true);
            left_palm_normal: Vec3 = (0.0, 0.0, 0.0) [(-1.0,-1.0,-1.0)..(1.0,1.0,1.0)] (label = "Palm Normal", read_only = true);
        }
        folder(right_hand, label = "Right Hand", collapsed = true) {
            right_active: bool = false (label = "Active", read_only = true);
            right_grab_strength: f64 = 0.0 [0.0..1.0] (label = "Grab Strength", read_only = true);
            right_pinch_strength: f64 = 0.0 [0.0..1.0] (label = "Pinch Strength", read_only = true);
            right_pinch_distance: f64 = 0.0 [0.0..0.2] (label = "Pinch Distance", read_only = true);
            right_thumb_extended: bool = false (label = "Thumb Extended", read_only = true);
            right_index_extended: bool = false (label = "Index Extended", read_only = true);
            right_middle_extended: bool = false (label = "Middle Extended", read_only = true);
            right_ring_extended: bool = false (label = "Ring Extended", read_only = true);
            right_pinky_extended: bool = false (label = "Pinky Extended", read_only = true);
            right_palm_position: Vec3 = (0.0, 0.0, 0.0) [(-1.0,-1.0,-1.0)..(1.0,1.0,1.0)] (label = "Palm Position", read_only = true);
            right_palm_stabilized_position: Vec3 = (0.0, 0.0, 0.0) [(-1.0,-1.0,-1.0)..(1.0,1.0,1.0)] (label = "Stabilized Position", read_only = true);
            right_palm_velocity: Vec3 = (0.0, 0.0, 0.0) [(-3.0,-3.0,-3.0)..(3.0,3.0,3.0)] (label = "Palm Velocity", read_only = true);
            right_palm_direction: Vec3 = (0.0, 0.0, 0.0) [(-1.0,-1.0,-1.0)..(1.0,1.0,1.0)] (label = "Palm Direction", read_only = true);
            right_palm_normal: Vec3 = (0.0, 0.0, 0.0) [(-1.0,-1.0,-1.0)..(1.0,1.0,1.0)] (label = "Palm Normal", read_only = true);
        }
        [base_children];
    }
)]
pub struct UltraleapModule {
    base: crate::app::ModuleBase,
    service: service::Service,
}

impl UltraleapModule {
    pub fn create() -> Self {
        Self::new(crate::app::ModuleBase::new(), service::Service::new())
    }

    #[cfg(test)]
    pub(crate) fn disable_runtime_for_test(&mut self) {
        self.stop_runtime();
        self.service.suppressed = true;
    }

    fn refresh_connection_state(&mut self, ctx: &mut ProcessCtx, service_connected: bool, connected_devices: usize) {
        let device_available = connected_devices > 0;
        self.base.set_connected(ctx, device_available);
        self.set_bool_info(ctx, UltraleapBoolInfoParam::ServiceConnected, service_connected);
        self.set_bool_info(ctx, UltraleapBoolInfoParam::DeviceAvailable, device_available);
        self.set_int_info(
            ctx,
            UltraleapIntInfoParam::ConnectedDevices,
            clamp_usize_to_i32(connected_devices),
        );
    }

    fn refresh_data_capabilities(&mut self, ctx: &mut ProcessCtx) {
        self.base.set_data_capabilities(
            ctx,
            crate::app::module::ModuleDataCapabilities::new(
                cfg!(any(windows, target_os = "linux", target_os = "macos")),
                false,
            ),
        );
    }

    fn apply_frame(&mut self, ctx: &mut ProcessCtx, frame: UltraleapFrameSnapshot, distance: f64) {
        self.set_bool_info(ctx, UltraleapBoolInfoParam::TrackingActive, frame.hand_count > 0);
        self.set_int_info(
            ctx,
            UltraleapIntInfoParam::VisibleHands,
            clamp_usize_to_i32(frame.hand_count),
        );
        self.set_hands_distance(ctx, distance);
        self.set_left_hand_snapshot(ctx, &frame.left);
        self.set_right_hand_snapshot(ctx, &frame.right);
        if frame.hand_count > 0 {
            self.base.emit_incoming_traffic(ctx);
        }
    }

    fn reset_tracking_values(&mut self, ctx: &mut ProcessCtx) {
        self.set_bool_info(ctx, UltraleapBoolInfoParam::TrackingActive, false);
        self.set_int_info(ctx, UltraleapIntInfoParam::VisibleHands, 0);
        self.set_hands_distance(ctx, 0.0);
        self.set_left_hand_snapshot(ctx, &UltraleapHandSnapshot::default());
        self.set_right_hand_snapshot(ctx, &UltraleapHandSnapshot::default());
    }

    fn reset_all_values(&mut self, ctx: &mut ProcessCtx) {
        self.refresh_connection_state(ctx, false, 0);
        self.reset_tracking_values(ctx);
    }

    fn set_runtime_warning(&self, ctx: &mut ProcessCtx, message: &str) {
        NodeHandle::new(self.id()).set_warning_with(ctx, Some(ULTRALEAP_RUNTIME_WARNING_ID), message, None);
    }

    fn clear_runtime_warning(&self, ctx: &mut ProcessCtx) {
        NodeHandle::new(self.id()).clear_warning(ctx, Some(ULTRALEAP_RUNTIME_WARNING_ID));
    }

    fn set_bool_info(&mut self, ctx: &mut ProcessCtx, param: UltraleapBoolInfoParam, value: bool) {
        match param {
            UltraleapBoolInfoParam::ServiceConnected
                if self.service_connected.is_bound() && self.service_connected.get() != value =>
            {
                self.service_connected.set(ctx, value);
            }
            UltraleapBoolInfoParam::DeviceAvailable
                if self.device_available.is_bound() && self.device_available.get() != value =>
            {
                self.device_available.set(ctx, value);
            }
            UltraleapBoolInfoParam::TrackingActive
                if self.tracking_active.is_bound() && self.tracking_active.get() != value =>
            {
                self.tracking_active.set(ctx, value);
            }
            _ => {}
        }
    }

    fn set_int_info(&mut self, ctx: &mut ProcessCtx, param: UltraleapIntInfoParam, value: i32) {
        match param {
            UltraleapIntInfoParam::ConnectedDevices
                if self.connected_devices.is_bound() && self.connected_devices.get() != value =>
            {
                self.connected_devices.set(ctx, value);
            }
            UltraleapIntInfoParam::VisibleHands
                if self.visible_hands.is_bound() && self.visible_hands.get() != value =>
            {
                self.visible_hands.set(ctx, value);
            }
            _ => {}
        }
    }

    fn set_string_info(&mut self, ctx: &mut ProcessCtx, param: UltraleapStringInfoParam, value: &str) {
        match param {
            UltraleapStringInfoParam::LastEvent if self.last_event.is_bound() && self.last_event.get_ref() != value => {
                self.last_event.set(ctx, value.to_string());
            }
            _ => {}
        }
    }

    fn set_last_event(&mut self, ctx: &mut ProcessCtx, value: String) {
        self.set_string_info(ctx, UltraleapStringInfoParam::LastEvent, value.as_str());
    }

    fn set_hands_distance(&mut self, ctx: &mut ProcessCtx, value: f64) {
        if self.hands_distance.is_bound() && float_changed(self.hands_distance.get(), value) {
            self.hands_distance.set(ctx, value);
            if self.base.log_incoming_enabled() {
                golden_core::log!(origin = self.id(); format!("Hands distance: {value}"));
            }
        }
    }

    fn set_left_hand_snapshot(&mut self, ctx: &mut ProcessCtx, hand: &UltraleapHandSnapshot) {
        if self.left_active.is_bound() && self.left_active.get() != hand.active {
            self.left_active.set(ctx, hand.active);
        }
        if self.left_grab_strength.is_bound() && float_changed(self.left_grab_strength.get(), hand.grab_strength) {
            self.left_grab_strength.set(ctx, hand.grab_strength);
        }
        if self.left_pinch_strength.is_bound() && float_changed(self.left_pinch_strength.get(), hand.pinch_strength) {
            self.left_pinch_strength.set(ctx, hand.pinch_strength);
        }
        if self.left_pinch_distance.is_bound() && float_changed(self.left_pinch_distance.get(), hand.pinch_distance) {
            self.left_pinch_distance.set(ctx, hand.pinch_distance);
        }
        if self.left_thumb_extended.is_bound() && self.left_thumb_extended.get() != hand.thumb_extended {
            self.left_thumb_extended.set(ctx, hand.thumb_extended);
        }
        if self.left_index_extended.is_bound() && self.left_index_extended.get() != hand.index_extended {
            self.left_index_extended.set(ctx, hand.index_extended);
        }
        if self.left_middle_extended.is_bound() && self.left_middle_extended.get() != hand.middle_extended {
            self.left_middle_extended.set(ctx, hand.middle_extended);
        }
        if self.left_ring_extended.is_bound() && self.left_ring_extended.get() != hand.ring_extended {
            self.left_ring_extended.set(ctx, hand.ring_extended);
        }
        if self.left_pinky_extended.is_bound() && self.left_pinky_extended.get() != hand.pinky_extended {
            self.left_pinky_extended.set(ctx, hand.pinky_extended);
        }
        if self.left_palm_position.is_bound() && vec3_changed(self.left_palm_position.get(), hand.palm_position) {
            self.left_palm_position.set(
                ctx,
                Vec3::new(hand.palm_position.x, hand.palm_position.y, hand.palm_position.z),
            );
        }
        if self.left_palm_stabilized_position.is_bound()
            && vec3_changed(self.left_palm_stabilized_position.get(), hand.palm_stabilized_position)
        {
            self.left_palm_stabilized_position.set(
                ctx,
                Vec3::new(
                    hand.palm_stabilized_position.x,
                    hand.palm_stabilized_position.y,
                    hand.palm_stabilized_position.z,
                ),
            );
        }
        if self.left_palm_velocity.is_bound() && vec3_changed(self.left_palm_velocity.get(), hand.palm_velocity) {
            self.left_palm_velocity.set(
                ctx,
                Vec3::new(hand.palm_velocity.x, hand.palm_velocity.y, hand.palm_velocity.z),
            );
        }
        if self.left_palm_direction.is_bound() && vec3_changed(self.left_palm_direction.get(), hand.palm_direction) {
            self.left_palm_direction.set(
                ctx,
                Vec3::new(hand.palm_direction.x, hand.palm_direction.y, hand.palm_direction.z),
            );
        }
        if self.left_palm_normal.is_bound() && vec3_changed(self.left_palm_normal.get(), hand.palm_normal) {
            self.left_palm_normal.set(
                ctx,
                Vec3::new(hand.palm_normal.x, hand.palm_normal.y, hand.palm_normal.z),
            );
        }
    }

    fn set_right_hand_snapshot(&mut self, ctx: &mut ProcessCtx, hand: &UltraleapHandSnapshot) {
        if self.right_active.is_bound() && self.right_active.get() != hand.active {
            self.right_active.set(ctx, hand.active);
        }
        if self.right_grab_strength.is_bound() && float_changed(self.right_grab_strength.get(), hand.grab_strength) {
            self.right_grab_strength.set(ctx, hand.grab_strength);
        }
        if self.right_pinch_strength.is_bound() && float_changed(self.right_pinch_strength.get(), hand.pinch_strength) {
            self.right_pinch_strength.set(ctx, hand.pinch_strength);
        }
        if self.right_pinch_distance.is_bound() && float_changed(self.right_pinch_distance.get(), hand.pinch_distance) {
            self.right_pinch_distance.set(ctx, hand.pinch_distance);
        }
        if self.right_thumb_extended.is_bound() && self.right_thumb_extended.get() != hand.thumb_extended {
            self.right_thumb_extended.set(ctx, hand.thumb_extended);
        }
        if self.right_index_extended.is_bound() && self.right_index_extended.get() != hand.index_extended {
            self.right_index_extended.set(ctx, hand.index_extended);
        }
        if self.right_middle_extended.is_bound() && self.right_middle_extended.get() != hand.middle_extended {
            self.right_middle_extended.set(ctx, hand.middle_extended);
        }
        if self.right_ring_extended.is_bound() && self.right_ring_extended.get() != hand.ring_extended {
            self.right_ring_extended.set(ctx, hand.ring_extended);
        }
        if self.right_pinky_extended.is_bound() && self.right_pinky_extended.get() != hand.pinky_extended {
            self.right_pinky_extended.set(ctx, hand.pinky_extended);
        }
        if self.right_palm_position.is_bound() && vec3_changed(self.right_palm_position.get(), hand.palm_position) {
            self.right_palm_position.set(
                ctx,
                Vec3::new(hand.palm_position.x, hand.palm_position.y, hand.palm_position.z),
            );
        }
        if self.right_palm_stabilized_position.is_bound()
            && vec3_changed(self.right_palm_stabilized_position.get(), hand.palm_stabilized_position)
        {
            self.right_palm_stabilized_position.set(
                ctx,
                Vec3::new(
                    hand.palm_stabilized_position.x,
                    hand.palm_stabilized_position.y,
                    hand.palm_stabilized_position.z,
                ),
            );
        }
        if self.right_palm_velocity.is_bound() && vec3_changed(self.right_palm_velocity.get(), hand.palm_velocity) {
            self.right_palm_velocity.set(
                ctx,
                Vec3::new(hand.palm_velocity.x, hand.palm_velocity.y, hand.palm_velocity.z),
            );
        }
        if self.right_palm_direction.is_bound() && vec3_changed(self.right_palm_direction.get(), hand.palm_direction) {
            self.right_palm_direction.set(
                ctx,
                Vec3::new(hand.palm_direction.x, hand.palm_direction.y, hand.palm_direction.z),
            );
        }
        if self.right_palm_normal.is_bound() && vec3_changed(self.right_palm_normal.get(), hand.palm_normal) {
            self.right_palm_normal.set(
                ctx,
                Vec3::new(hand.palm_normal.x, hand.palm_normal.y, hand.palm_normal.z),
            );
        }
    }
}

#[golden_core::item(
    "module",
    node = "ultraleap_module",
    via = base,
    from_struct,
    menu_path = ["Controllers"]
)]
impl Node for UltraleapModule {
    fn init(&mut self, ctx: &mut ProcessCtx) {
        self.base.init(ctx);
        self.base.configure_command_tester(ctx, ULTRALEAP_MODULE_COMMAND_TYPES);
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

    fn update_requires_tree_snapshot(&self) -> bool {
        false
    }

    fn execution_rule(&self) -> NodeExecutionRule {
        NodeExecutionRule::passive().with_compiled_kernel("chataigne.runtime.ultraleap")
    }

    fn engine_script_descriptor(&self) -> NodeScriptDescriptor {
        crate::app::module::script_api::descriptor_for_node(self.node_data(), self.get_type(), ULTRALEAP_SCRIPT_METHODS)
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
        if let Some(snapshot_arc) = ctx.tree_snapshot_arc() {
            self.base
                .emit_script_param_callback(ctx, snapshot_arc.as_ref(), param, &old_value);
        }
    }

    fn on_effective_enabled_changed(&mut self, ctx: &mut ProcessCtx, enabled: bool) {
        if enabled {
            self.service.retry_at = None;
            self.request_service();
            self.refresh_data_capabilities(ctx);
            return;
        }

        self.stop_runtime();
        NodeHandle::new(self.id()).clear_warning(ctx, Some("ultraleap_pressure"));
        self.clear_runtime_warning(ctx);
        self.reset_all_values(ctx);
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::create)
    }
}

#[derive(Clone, Copy)]
enum UltraleapBoolInfoParam {
    ServiceConnected,
    DeviceAvailable,
    TrackingActive,
}

#[derive(Clone, Copy)]
enum UltraleapIntInfoParam {
    ConnectedDevices,
    VisibleHands,
}

#[derive(Clone, Copy)]
enum UltraleapStringInfoParam {
    LastEvent,
}

fn float_changed(lhs: f64, rhs: f64) -> bool {
    (lhs - rhs).abs() > 0.000_001
}

fn vec3_changed(current: Vec3, next: UltraleapVec3) -> bool {
    float_changed(current.x, next.x) || float_changed(current.y, next.y) || float_changed(current.z, next.z)
}

fn clamp_usize_to_i32(value: usize) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}

#[cfg(test)]
mod tests;
