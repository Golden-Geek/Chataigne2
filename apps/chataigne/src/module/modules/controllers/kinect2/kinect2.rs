mod kinect2_runtime;

use std::collections::VecDeque;

use golden_core::{
    engine::NodeExecutionRule,
    logerror, node,
    node::{Node, NodeCreationContext, NodeHandle, NodeId, NodeScriptDescriptor},
    parameter::{Enum, ParamValue, Vec2, Vec3},
    process_ctx::ProcessCtx,
};

use self::kinect2_runtime::{
    Kinect2Runtime, KinectBodySnapshot, KinectFrameSnapshot, KinectJoint, KinectRuntimePoll,
    KinectVec3,
};

const KINECT2_MODULE_UPDATE_RATE_HZ: u32 = 30;
const KINECT2_RUNTIME_RETRY_INTERVAL_SECS: f64 = 2.0;
const KINECT2_RUNTIME_WARNING_ID: &str = "kinect2_runtime";
const KINECT2_SPACE_ABSOLUTE: &str = "absolute";
const KINECT2_SPACE_TORSO: &str = "torso";
const KINECT2_SPACE_HEAD: &str = "head";
const KINECT2_HAND_SPEED_RESET_SECS: f64 = 0.5;

const KINECT2_SCRIPT_METHODS: &[&str] = &[];
const KINECT2_MODULE_COMMAND_TYPES: &[&str] = &[];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum KinectReferenceSpace {
    Absolute,
    Torso,
    Head,
}

impl KinectReferenceSpace {
    fn from_variant(value: &str) -> Self {
        match value.trim() {
            KINECT2_SPACE_TORSO => Self::Torso,
            KINECT2_SPACE_HEAD => Self::Head,
            _ => Self::Absolute,
        }
    }
}

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
    runtime: Option<Kinect2Runtime>,
    runtime_retry_elapsed: f64,
    last_runtime_error: Option<String>,
    pending_frames: VecDeque<KinectFrameSnapshot>,
    runtime_start_suppressed: bool,
    last_hand_midpoint: Option<KinectVec3>,
    last_hand_midpoint_timestamp_100ns: Option<u64>,
    last_hand_midpoint_tracking_id: Option<u64>,
    last_hand_midpoint_space: Option<KinectReferenceSpace>,
    last_selected_tracking_id: Option<u64>,
}

impl Kinect2Module {
    pub fn create() -> Self {
        Self::new(
            crate::app::ModuleBase::new(),
            None,
            KINECT2_RUNTIME_RETRY_INTERVAL_SECS,
            None,
            VecDeque::new(),
            false,
            None,
            None,
            None,
            None,
            None,
        )
    }

    #[cfg(test)]
    pub(crate) fn disable_runtime_for_test(&mut self) {
        self.runtime = None;
        self.runtime_start_suppressed = true;
    }

    #[cfg(test)]
    pub(crate) fn enqueue_frame_for_test(&mut self, frame: KinectFrameSnapshot) {
        self.pending_frames.push_back(frame);
    }

    fn ensure_runtime(&mut self, ctx: &mut ProcessCtx) {
        if self.runtime_start_suppressed || self.runtime.is_some() {
            return;
        }
        if self.runtime_retry_elapsed < KINECT2_RUNTIME_RETRY_INTERVAL_SECS {
            return;
        }

        self.runtime_retry_elapsed = 0.0;
        match Kinect2Runtime::create() {
            Ok(runtime) => {
                golden_core::log!(origin = self.id(); "Started Kinect 2 input runtime.");
                self.runtime = Some(runtime);
                self.last_runtime_error = None;
                self.clear_runtime_warning(ctx);
            }
            Err(error) => {
                if self.last_runtime_error.as_deref() != Some(error.as_str()) {
                    logerror!(origin = self.id(); format!("Failed to start Kinect 2 input runtime: {error}"));
                }
                self.runtime = None;
                self.last_runtime_error = Some(error.clone());
                self.set_runtime_warning(ctx, error.as_str());
            }
        }
    }

    fn stop_runtime(&mut self) {
        self.runtime = None;
        self.clear_speed_history();
        self.last_selected_tracking_id = None;
    }

    fn poll_runtime(&mut self) -> Result<KinectRuntimePoll, String> {
        if let Some(frame) = self.pending_frames.pop_front() {
            return Ok(KinectRuntimePoll {
                sensor_available: frame.sensor_available,
                frame: Some(frame),
            });
        }

        let Some(runtime) = self.runtime.as_mut() else {
            return Ok(KinectRuntimePoll {
                sensor_available: false,
                frame: None,
            });
        };

        runtime.poll()
    }

    fn handle_runtime_error(&mut self, ctx: &mut ProcessCtx, error: String) {
        if self.last_runtime_error.as_deref() != Some(error.as_str()) {
            logerror!(origin = self.id(); format!("Kinect 2 runtime stopped: {error}"));
        }
        self.stop_runtime();
        self.last_runtime_error = Some(error.clone());
        self.set_runtime_warning(ctx, error.as_str());
        self.reset_all_values(ctx);
    }

    fn apply_frame(&mut self, ctx: &mut ProcessCtx, frame: KinectFrameSnapshot) {
        let tracked_body_count = clamp_usize_to_i32(frame.tracked_bodies.len());
        let selected_body = select_primary_body(frame.tracked_bodies.as_slice());

        self.refresh_connection_state(
            ctx,
            frame.sensor_available,
            tracked_body_count,
            selected_body.as_ref().map(|body| body.tracking_id),
        );

        let Some(selected_body) = selected_body else {
            if self.last_selected_tracking_id.take().is_some() {
                self.set_last_event(ctx, "Lost tracked body".to_string());
            }
            self.clear_speed_history();
            self.reset_tracking_values(ctx);
            return;
        };

        let reference_space = KinectReferenceSpace::from_variant(self.reference_space.get_ref().as_str());
        let origin = reference_origin(selected_body, reference_space);
        let mut transformed = [KinectVec3::ZERO; KinectJoint::COUNT];
        for joint in KinectJoint::ALL {
            transformed[joint.index()] = selected_body.joint(joint).available_position().subtract(origin);
            self.set_joint_value(ctx, joint, transformed[joint.index()]);
        }

        let left_hand = transformed[KinectJoint::HandLeft.index()];
        let right_hand = transformed[KinectJoint::HandRight.index()];
        let distance = left_hand.distance(right_hand);
        let hands_rotation = planar_direction(left_hand, right_hand);
        let hands_midpoint = left_hand.midpoint(right_hand);
        let hands_speed = self.compute_hand_speed(
            selected_body.tracking_id,
            reference_space,
            frame.timestamp_100ns,
            hands_midpoint,
        );

        self.set_hands_distance(ctx, distance);
        self.set_hands_rotation(ctx, hands_rotation.0, hands_rotation.1);
        self.set_hands_speed(ctx, hands_speed);

        if self.last_selected_tracking_id != Some(selected_body.tracking_id) {
            self.set_last_event(ctx, format!("Tracking body {}", selected_body.tracking_id));
        }
        self.last_selected_tracking_id = Some(selected_body.tracking_id);
        self.base.emit_incoming_traffic(ctx);
    }

    fn compute_hand_speed(
        &mut self,
        tracking_id: u64,
        reference_space: KinectReferenceSpace,
        timestamp_100ns: u64,
        midpoint: KinectVec3,
    ) -> KinectVec3 {
        let previous_midpoint = self.last_hand_midpoint;
        let previous_timestamp = self.last_hand_midpoint_timestamp_100ns;
        let previous_tracking_id = self.last_hand_midpoint_tracking_id;
        let previous_space = self.last_hand_midpoint_space;

        self.last_hand_midpoint = Some(midpoint);
        self.last_hand_midpoint_timestamp_100ns = Some(timestamp_100ns);
        self.last_hand_midpoint_tracking_id = Some(tracking_id);
        self.last_hand_midpoint_space = Some(reference_space);

        let Some(previous_midpoint) = previous_midpoint else {
            return KinectVec3::ZERO;
        };
        let Some(previous_timestamp) = previous_timestamp else {
            return KinectVec3::ZERO;
        };
        if previous_tracking_id != Some(tracking_id) || previous_space != Some(reference_space) {
            return KinectVec3::ZERO;
        }
        if timestamp_100ns <= previous_timestamp {
            return KinectVec3::ZERO;
        }

        let delta_seconds = (timestamp_100ns - previous_timestamp) as f64 / 10_000_000.0;
        if !(0.0..=KINECT2_HAND_SPEED_RESET_SECS).contains(&delta_seconds) || delta_seconds <= f64::EPSILON {
            return KinectVec3::ZERO;
        }

        midpoint.subtract(previous_midpoint).scale(1.0 / delta_seconds)
    }

    fn clear_speed_history(&mut self) {
        self.last_hand_midpoint = None;
        self.last_hand_midpoint_timestamp_100ns = None;
        self.last_hand_midpoint_tracking_id = None;
        self.last_hand_midpoint_space = None;
    }

    fn refresh_connection_state(
        &mut self,
        ctx: &mut ProcessCtx,
        sensor_available: bool,
        tracked_bodies: i32,
        tracking_id: Option<u64>,
    ) {
        self.base.set_connected(ctx, sensor_available);
        self.set_bool_info(ctx, KinectBoolInfoParam::SensorAvailable, sensor_available);
        self.set_bool_info(ctx, KinectBoolInfoParam::TrackingActive, tracking_id.is_some());
        self.set_int_info(ctx, KinectIntInfoParam::TrackedBodies, tracked_bodies);

        match tracking_id {
            Some(tracking_id) => {
                let tracking_id = tracking_id.to_string();
                self.set_string_info(ctx, KinectStringInfoParam::TrackingId, tracking_id.as_str());
            }
            None => self.set_string_info(ctx, KinectStringInfoParam::TrackingId, ""),
        }
    }

    fn refresh_data_capabilities(&mut self, ctx: &mut ProcessCtx) {
        self.base.set_data_capabilities(
            ctx,
            crate::app::module::ModuleDataCapabilities::new(cfg!(windows), false),
        );
    }

    fn reset_tracking_values(&mut self, ctx: &mut ProcessCtx) {
        self.set_bool_info(ctx, KinectBoolInfoParam::TrackingActive, false);
        self.set_string_info(ctx, KinectStringInfoParam::TrackingId, "");
        self.set_hands_distance(ctx, 0.0);
        self.set_hands_rotation(ctx, 0.0, 0.0);
        self.set_hands_speed(ctx, KinectVec3::ZERO);
        for joint in KinectJoint::ALL {
            self.set_joint_value(ctx, joint, KinectVec3::ZERO);
        }
    }

    fn reset_all_values(&mut self, ctx: &mut ProcessCtx) {
        self.refresh_connection_state(ctx, false, 0, None);
        self.reset_tracking_values(ctx);
    }

    fn set_runtime_warning(&self, ctx: &mut ProcessCtx, message: &str) {
        NodeHandle::new(self.id()).set_warning_with(ctx, Some(KINECT2_RUNTIME_WARNING_ID), message, None);
    }

    fn clear_runtime_warning(&self, ctx: &mut ProcessCtx) {
        NodeHandle::new(self.id()).clear_warning(ctx, Some(KINECT2_RUNTIME_WARNING_ID));
    }

    fn set_bool_info(&mut self, ctx: &mut ProcessCtx, param: KinectBoolInfoParam, value: bool) {
        match param {
            KinectBoolInfoParam::SensorAvailable
                if self.sensor_available.is_bound() && self.sensor_available.get() != value =>
            {
                self.sensor_available.set(ctx, value);
            }
            KinectBoolInfoParam::TrackingActive
                if self.tracking_active.is_bound() && self.tracking_active.get() != value =>
            {
                self.tracking_active.set(ctx, value);
            }
            _ => {}
        }
    }

    fn set_int_info(&mut self, ctx: &mut ProcessCtx, param: KinectIntInfoParam, value: i32) {
        match param {
            KinectIntInfoParam::TrackedBodies
                if self.tracked_bodies.is_bound() && self.tracked_bodies.get() != value =>
            {
                self.tracked_bodies.set(ctx, value);
            }
            _ => {}
        }
    }

    fn set_string_info(&mut self, ctx: &mut ProcessCtx, param: KinectStringInfoParam, value: &str) {
        match param {
            KinectStringInfoParam::TrackingId
                if self.tracking_id.is_bound() && self.tracking_id.get_ref() != value =>
            {
                self.tracking_id.set(ctx, value.to_string());
            }
            KinectStringInfoParam::LastEvent
                if self.last_event.is_bound() && self.last_event.get_ref() != value =>
            {
                self.last_event.set(ctx, value.to_string());
            }
            _ => {}
        }
    }

    fn set_last_event(&mut self, ctx: &mut ProcessCtx, value: String) {
        self.set_string_info(ctx, KinectStringInfoParam::LastEvent, value.as_str());
    }

    fn set_hands_distance(&mut self, ctx: &mut ProcessCtx, value: f64) {
        if self.hands_distance.is_bound() && float_changed(self.hands_distance.get(), value) {
            self.hands_distance.set(ctx, value);
        }
    }

    fn set_hands_rotation(&mut self, ctx: &mut ProcessCtx, x: f64, y: f64) {
        if self.hands_rotation.is_bound() {
            let current = self.hands_rotation.get();
            if vec2_changed(current, x, y) {
                self.hands_rotation.set(ctx, Vec2::new(x, y));
            }
        }
    }

    fn set_hands_speed(&mut self, ctx: &mut ProcessCtx, value: KinectVec3) {
        if self.hands_speed.is_bound() {
            let current = self.hands_speed.get();
            if vec3_changed(current, value) {
                self.hands_speed
                    .set(ctx, Vec3::new(value.x, value.y, value.z));
            }
        }
    }

    fn set_joint_value(&mut self, ctx: &mut ProcessCtx, joint: KinectJoint, value: KinectVec3) {
        match joint {
            KinectJoint::SpineBase if self.spine_base.is_bound() && vec3_changed(self.spine_base.get(), value) => {
                self.spine_base.set(ctx, Vec3::new(value.x, value.y, value.z));
            }
            KinectJoint::SpineMid if self.spine_mid.is_bound() && vec3_changed(self.spine_mid.get(), value) => {
                self.spine_mid.set(ctx, Vec3::new(value.x, value.y, value.z));
            }
            KinectJoint::Neck if self.neck.is_bound() && vec3_changed(self.neck.get(), value) => {
                self.neck.set(ctx, Vec3::new(value.x, value.y, value.z));
            }
            KinectJoint::Head if self.head.is_bound() && vec3_changed(self.head.get(), value) => {
                self.head.set(ctx, Vec3::new(value.x, value.y, value.z));
            }
            KinectJoint::ShoulderLeft
                if self.shoulder_left.is_bound() && vec3_changed(self.shoulder_left.get(), value) =>
            {
                self.shoulder_left.set(ctx, Vec3::new(value.x, value.y, value.z));
            }
            KinectJoint::ElbowLeft
                if self.elbow_left.is_bound() && vec3_changed(self.elbow_left.get(), value) =>
            {
                self.elbow_left.set(ctx, Vec3::new(value.x, value.y, value.z));
            }
            KinectJoint::WristLeft
                if self.wrist_left.is_bound() && vec3_changed(self.wrist_left.get(), value) =>
            {
                self.wrist_left.set(ctx, Vec3::new(value.x, value.y, value.z));
            }
            KinectJoint::HandLeft
                if self.hand_left.is_bound() && vec3_changed(self.hand_left.get(), value) =>
            {
                self.hand_left.set(ctx, Vec3::new(value.x, value.y, value.z));
            }
            KinectJoint::ShoulderRight
                if self.shoulder_right.is_bound() && vec3_changed(self.shoulder_right.get(), value) =>
            {
                self.shoulder_right.set(ctx, Vec3::new(value.x, value.y, value.z));
            }
            KinectJoint::ElbowRight
                if self.elbow_right.is_bound() && vec3_changed(self.elbow_right.get(), value) =>
            {
                self.elbow_right.set(ctx, Vec3::new(value.x, value.y, value.z));
            }
            KinectJoint::WristRight
                if self.wrist_right.is_bound() && vec3_changed(self.wrist_right.get(), value) =>
            {
                self.wrist_right.set(ctx, Vec3::new(value.x, value.y, value.z));
            }
            KinectJoint::HandRight
                if self.hand_right.is_bound() && vec3_changed(self.hand_right.get(), value) =>
            {
                self.hand_right.set(ctx, Vec3::new(value.x, value.y, value.z));
            }
            KinectJoint::HipLeft if self.hip_left.is_bound() && vec3_changed(self.hip_left.get(), value) => {
                self.hip_left.set(ctx, Vec3::new(value.x, value.y, value.z));
            }
            KinectJoint::KneeLeft if self.knee_left.is_bound() && vec3_changed(self.knee_left.get(), value) => {
                self.knee_left.set(ctx, Vec3::new(value.x, value.y, value.z));
            }
            KinectJoint::AnkleLeft
                if self.ankle_left.is_bound() && vec3_changed(self.ankle_left.get(), value) =>
            {
                self.ankle_left.set(ctx, Vec3::new(value.x, value.y, value.z));
            }
            KinectJoint::FootLeft if self.foot_left.is_bound() && vec3_changed(self.foot_left.get(), value) => {
                self.foot_left.set(ctx, Vec3::new(value.x, value.y, value.z));
            }
            KinectJoint::HipRight
                if self.hip_right.is_bound() && vec3_changed(self.hip_right.get(), value) =>
            {
                self.hip_right.set(ctx, Vec3::new(value.x, value.y, value.z));
            }
            KinectJoint::KneeRight
                if self.knee_right.is_bound() && vec3_changed(self.knee_right.get(), value) =>
            {
                self.knee_right.set(ctx, Vec3::new(value.x, value.y, value.z));
            }
            KinectJoint::AnkleRight
                if self.ankle_right.is_bound() && vec3_changed(self.ankle_right.get(), value) =>
            {
                self.ankle_right.set(ctx, Vec3::new(value.x, value.y, value.z));
            }
            KinectJoint::FootRight
                if self.foot_right.is_bound() && vec3_changed(self.foot_right.get(), value) =>
            {
                self.foot_right.set(ctx, Vec3::new(value.x, value.y, value.z));
            }
            KinectJoint::SpineShoulder
                if self.spine_shoulder.is_bound() && vec3_changed(self.spine_shoulder.get(), value) =>
            {
                self.spine_shoulder.set(ctx, Vec3::new(value.x, value.y, value.z));
            }
            KinectJoint::HandTipLeft
                if self.hand_tip_left.is_bound() && vec3_changed(self.hand_tip_left.get(), value) =>
            {
                self.hand_tip_left.set(ctx, Vec3::new(value.x, value.y, value.z));
            }
            KinectJoint::ThumbLeft
                if self.thumb_left.is_bound() && vec3_changed(self.thumb_left.get(), value) =>
            {
                self.thumb_left.set(ctx, Vec3::new(value.x, value.y, value.z));
            }
            KinectJoint::HandTipRight
                if self.hand_tip_right.is_bound() && vec3_changed(self.hand_tip_right.get(), value) =>
            {
                self.hand_tip_right.set(ctx, Vec3::new(value.x, value.y, value.z));
            }
            KinectJoint::ThumbRight
                if self.thumb_right.is_bound() && vec3_changed(self.thumb_right.get(), value) =>
            {
                self.thumb_right.set(ctx, Vec3::new(value.x, value.y, value.z));
            }
            _ => {}
        }
    }

    fn on_param_change_inner(&mut self, ctx: &mut ProcessCtx, param: NodeId) {
        if self.reference_space.is_bound() && self.reference_space.id() == param {
            self.clear_speed_history();
            if let Some(snapshot_arc) = ctx.tree_snapshot_arc() {
                let _ = snapshot_arc;
            }
        }
    }
}

#[golden_core::item(
    "module",
    node = "kinect2_module",
    via = base,
    from_struct,
    menu_path = ["Controllers"]
)]
impl Node for Kinect2Module {
    fn init(&mut self, ctx: &mut ProcessCtx) {
        self.base.init(ctx);
        self.base.configure_command_tester(ctx, KINECT2_MODULE_COMMAND_TYPES);
        self.refresh_data_capabilities(ctx);
        self.reset_all_values(ctx);
    }

    fn on_node_ready(&mut self, ctx: &mut ProcessCtx, _context: NodeCreationContext) {
        self.ensure_runtime(ctx);
    }

    fn update(&mut self, ctx: &mut ProcessCtx) {
        self.runtime_retry_elapsed += ctx.delta_time.as_secs_f64();

        if !self.node_data().effective_enabled {
            self.stop_runtime();
            self.reset_all_values(ctx);
            return;
        }

        self.ensure_runtime(ctx);

        let poll = match self.poll_runtime() {
            Ok(poll) => poll,
            Err(error) => {
                self.handle_runtime_error(ctx, error);
                return;
            }
        };

        if !poll.sensor_available {
            self.reset_all_values(ctx);
            return;
        }

        self.base.set_connected(ctx, true);
        self.set_bool_info(ctx, KinectBoolInfoParam::SensorAvailable, true);

        if let Some(frame) = poll.frame {
            self.clear_runtime_warning(ctx);
            self.last_runtime_error = None;
            self.apply_frame(ctx, frame);
        }
    }

    fn destroy(&mut self, _ctx: &mut ProcessCtx) {
        self.stop_runtime();
        self.pending_frames.clear();
    }

    fn update_requires_tree_snapshot(&self) -> bool {
        false
    }

    fn execution_rule(&self) -> NodeExecutionRule {
        NodeExecutionRule::periodic(KINECT2_MODULE_UPDATE_RATE_HZ)
    }

    fn engine_script_descriptor(&self) -> NodeScriptDescriptor {
        crate::app::module::script_api::descriptor_for_node(
            self.node_data(),
            self.get_type(),
            KINECT2_SCRIPT_METHODS,
        )
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
        self.on_param_change_inner(ctx, param);
    }

    fn on_effective_enabled_changed(&mut self, ctx: &mut ProcessCtx, enabled: bool) {
        if enabled {
            self.runtime_retry_elapsed = KINECT2_RUNTIME_RETRY_INTERVAL_SECS;
            self.refresh_data_capabilities(ctx);
            return;
        }

        self.stop_runtime();
        self.clear_runtime_warning(ctx);
        self.reset_all_values(ctx);
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::create)
    }
}

#[derive(Clone, Copy)]
enum KinectBoolInfoParam {
    SensorAvailable,
    TrackingActive,
}

#[derive(Clone, Copy)]
enum KinectIntInfoParam {
    TrackedBodies,
}

#[derive(Clone, Copy)]
enum KinectStringInfoParam {
    TrackingId,
    LastEvent,
}

fn select_primary_body(bodies: &[KinectBodySnapshot]) -> Option<&KinectBodySnapshot> {
    bodies
        .iter()
        .min_by(|left, right| left.reference_depth().total_cmp(&right.reference_depth()))
}

fn reference_origin(body: &KinectBodySnapshot, space: KinectReferenceSpace) -> KinectVec3 {
    match space {
        KinectReferenceSpace::Absolute => KinectVec3::ZERO,
        KinectReferenceSpace::Torso => {
            joint_or_zero(body, &[KinectJoint::SpineMid, KinectJoint::SpineBase, KinectJoint::SpineShoulder])
        }
        KinectReferenceSpace::Head => joint_or_zero(body, &[KinectJoint::Head]),
    }
}

fn joint_or_zero(body: &KinectBodySnapshot, joints: &[KinectJoint]) -> KinectVec3 {
    joints
        .iter()
        .map(|joint| body.joint(*joint).available_position())
        .find(|position| {
            position.x.abs() > f64::EPSILON
                || position.y.abs() > f64::EPSILON
                || position.z.abs() > f64::EPSILON
        })
        .unwrap_or(KinectVec3::ZERO)
}

fn planar_direction(left: KinectVec3, right: KinectVec3) -> (f64, f64) {
    let delta = right.subtract(left);
    let length = (delta.x * delta.x + delta.y * delta.y).sqrt();
    if length <= f64::EPSILON {
        (0.0, 0.0)
    } else {
        (delta.x / length, delta.y / length)
    }
}

fn float_changed(lhs: f64, rhs: f64) -> bool {
    (lhs - rhs).abs() > 0.000_001
}

fn vec2_changed(current: Vec2, x: f64, y: f64) -> bool {
    float_changed(current.x, x) || float_changed(current.y, y)
}

fn vec3_changed(current: Vec3, next: KinectVec3) -> bool {
    float_changed(current.x, next.x) || float_changed(current.y, next.y) || float_changed(current.z, next.z)
}

fn clamp_usize_to_i32(value: usize) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}

#[cfg(test)]
mod kinect2_tests;