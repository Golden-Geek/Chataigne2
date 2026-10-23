mod output;
mod runtime;
pub(crate) use output::dispatch as queue_joycon_command;
mod projection;
mod service;
mod state;
use state::*;

use std::sync::mpsc::TryRecvError;
use std::time::Duration;

use golden_core::{
    engine::NodeExecutionRule,
    events::{CustomEvent, Event},
    node,
    node::{Node, NodeCreationContext, NodeId, NodeScriptDescriptor},
    parameter::{Enum, ParamValue, Vec2, Vec3},
    process_ctx::ProcessCtx,
};
use joycon_rs::joycon::Buttons;

use self::runtime::{JoyConRuntimeHandle, JoyConRuntimeState, JoyConSide, JoyConWorkerEvent};
use crate::app::module::common::joycon::{
    joycon_motion_data_enum_options, JoyConMotionDataMode, JOYCON_MOTION_DATA_ALL, JOYCON_MOTION_DATA_NONE,
    JOYCON_MOTION_DATA_ORIENTATION, JOYCON_SET_LED_COMMAND_NODE_TYPE, JOYCON_VIBRATE_COMMAND_NODE_TYPE,
};

const JOYCON_MODULE_UPDATE_RATE_HZ: u32 = 120;
const JOYCON_RUNTIME_WARNING_ID: &str = "joycon_runtime";
const JOYCON_DEFAULT_PROCESSING_FPS_CAP: i32 = JOYCON_MODULE_UPDATE_RATE_HZ as i32;
const JOYCON_DEFAULT_STICK_DEAD_ZONE: f64 = 0.1;
const JOYCON_ACTIVITY_HOLD_DURATION: Duration = Duration::from_millis(150);
const JOYCON_INCOMING_LOG_INTERVAL: Duration = Duration::from_millis(250);
const JOYCON_RUNTIME_STALL_TIMEOUT: Duration = Duration::from_secs(2);

const JOYCON_CONNECTED_CALLBACK: &str = "joyConConnected";
const JOYCON_DISCONNECTED_CALLBACK: &str = "joyConDisconnected";
const JOYCON_BUTTON_PRESSED_CALLBACK: &str = "joyConButtonPressed";
const JOYCON_BUTTON_RELEASED_CALLBACK: &str = "joyConButtonReleased";
const JOYCON_BUTTON_CHANGED_CALLBACK: &str = "joyConButtonChanged";
const JOYCON_STICK_CHANGED_CALLBACK: &str = "joyConStickChanged";
const JOYCON_MOTION_CHANGED_CALLBACK: &str = "joyConMotionChanged";

const JOYCON_SCRIPT_METHODS: &[&str] = &["vibrate", "setPlayerLights"];
const JOYCON_MODULE_COMMAND_TYPES: &[&str] = &[JOYCON_VIBRATE_COMMAND_NODE_TYPE, JOYCON_SET_LED_COMMAND_NODE_TYPE];

#[node("joycon_module", label = "Joy-Con")]
#[children(
    folder(connection) {
        processing_fps_cap: i32 = JOYCON_DEFAULT_PROCESSING_FPS_CAP [1..JOYCON_MODULE_UPDATE_RATE_HZ as i32] (
            label = "Processing FPS Cap",
            description = "Maximum rate for applying analog Joy-Con state. The worker samples continuously; button and connection transitions retain their order independently of this cap.",
            widget = "text"
        );
        motion_data_mode: Enum = JOYCON_MOTION_DATA_NONE (
            label = "Motion Data",
            description = "Choose how much motion data the module publishes. Orientation derives pitch and roll from the latest IMU frame.",
            enum_options = joycon_motion_data_enum_options()
        );
        [base_children];
    }
    folder(parameters) {
        folder(stick_processing, label = "Stick Processing", collapsed = true) {
            stick_dead_zone: f64 = JOYCON_DEFAULT_STICK_DEAD_ZONE [0.0..1.0] (
                label = "Dead Zone",
                description = "Centered range ignored for both Joy-Con sticks after calibration normalization."
            );
        }
        [base_children];
    }
    folder(values) {
        folder(info, label = "Info", collapsed = true) {
            activity: bool = false (
                label = "Activity",
                description = "Pulses when Joy-Con input data is being received and applied.",
                read_only = true
            );
            is_connected: bool = false (
                label = "Is Connected",
                description = "Whether at least one Joy-Con controller slot is connected.",
                read_only = true
            );
            left_controller_connected: bool = false (
                label = "Left Controller Connected",
                description = "Whether the left Joy-Con slot is currently connected.",
                read_only = true
            );
            right_controller_connected: bool = false (
                label = "Right Controller Connected",
                description = "Whether the right Joy-Con slot is currently connected.",
                read_only = true
            );
            last_event: String = String::new() (
                label = "Last Event",
                description = "Last Joy-Con runtime or command event processed by this module.",
                read_only = true
            );
        }
        folder(left_controller, label = "Left Controller") {
            left_stick: Vec2 = (0.0, 0.0) [(-1.0, -1.0)..(1.0, 1.0)] (
                label = "Stick",
                description = "Left Joy-Con stick values.",
                read_only = true
            );
            folder(left_buttons, label = "Buttons", collapsed = true) {
                left_dpad_down: bool = false (label = "Down", read_only = true);
                left_dpad_up: bool = false (label = "Up", read_only = true);
                left_dpad_right: bool = false (label = "Right", read_only = true);
                left_dpad_left: bool = false (label = "Left", read_only = true);
                left_sl: bool = false (label = "SL", read_only = true);
                left_sr: bool = false (label = "SR", read_only = true);
                left_l: bool = false (label = "L", read_only = true);
                left_zl: bool = false (label = "ZL", read_only = true);
                left_minus: bool = false (label = "Minus", read_only = true);
                left_stick_button: bool = false (label = "Stick", read_only = true);
                left_capture: bool = false (label = "Capture", read_only = true);
            }
            folder(left_motion, label = "Motion", collapsed = true) {
                left_orientation: Vec2 = (0.0, 0.0) [(-90.0, -180.0)..(90.0, 180.0)] (
                    label = "Orientation",
                    description = "Derived pitch and roll from the latest accelerometer frame.",
                    read_only = true
                );
                left_accelerometer: Vec3 = (0.0, 0.0, 0.0) (
                    label = "Accelerometer",
                    description = "Latest raw accelerometer frame.",
                    read_only = true
                );
                left_gyroscope: Vec3 = (0.0, 0.0, 0.0) (
                    label = "Gyroscope",
                    description = "Latest raw gyroscope frame.",
                    read_only = true
                );
            }
        }
        folder(right_controller, label = "Right Controller") {
            right_stick: Vec2 = (0.0, 0.0) [(-1.0, -1.0)..(1.0, 1.0)] (
                label = "Stick",
                description = "Right Joy-Con stick values.",
                read_only = true
            );
            folder(right_buttons, label = "Buttons", collapsed = true) {
                right_y: bool = false (label = "Y", read_only = true);
                right_x: bool = false (label = "X", read_only = true);
                right_b: bool = false (label = "B", read_only = true);
                right_a: bool = false (label = "A", read_only = true);
                right_sr: bool = false (label = "SR", read_only = true);
                right_sl: bool = false (label = "SL", read_only = true);
                right_r: bool = false (label = "R", read_only = true);
                right_zr: bool = false (label = "ZR", read_only = true);
                right_plus: bool = false (label = "Plus", read_only = true);
                right_stick_button: bool = false (label = "Stick", read_only = true);
                right_home: bool = false (label = "Home", read_only = true);
            }
            folder(right_motion, label = "Motion", collapsed = true) {
                right_orientation: Vec2 = (0.0, 0.0) [(-90.0, -180.0)..(90.0, 180.0)] (
                    label = "Orientation",
                    description = "Derived pitch and roll from the latest accelerometer frame.",
                    read_only = true
                );
                right_accelerometer: Vec3 = (0.0, 0.0, 0.0) (
                    label = "Accelerometer",
                    description = "Latest raw accelerometer frame.",
                    read_only = true
                );
                right_gyroscope: Vec3 = (0.0, 0.0, 0.0) (
                    label = "Gyroscope",
                    description = "Latest raw gyroscope frame.",
                    read_only = true
                );
            }
        }
        [base_children];
    }
)]
pub struct JoyConModule {
    base: crate::app::ModuleBase,
    runtime: Option<Box<JoyConRuntimeHandle>>,
    service: service::Service,
    last_state: JoyConRuntimeState,
    runtime_dirty: bool,
    pending_state: Option<golden_io::RetainedPayload<runtime::Publication>>,
    processing_elapsed: Duration,
    incoming_log_elapsed: Duration,
    runtime_contact_elapsed: Duration,
}

impl JoyConModule {
    pub fn create() -> Self {
        Self::new(
            crate::app::ModuleBase::new(),
            None,
            service::Service::new(),
            JoyConRuntimeState::disconnected(),
            true,
            None,
            Duration::ZERO,
            JOYCON_INCOMING_LOG_INTERVAL,
            Duration::ZERO,
        )
    }

    fn refresh_data_capabilities(&mut self, ctx: &mut ProcessCtx) {
        self.base
            .set_data_capabilities(ctx, crate::app::module::ModuleDataCapabilities::new(true, true));
    }

    fn apply_runtime_state(&mut self, ctx: &mut ProcessCtx, state: JoyConRuntimeState) {
        if !self.guard_projection(ctx, &[]) {
            return;
        }
        self.base.set_connected(ctx, state.any_connected());
        self.set_bool_handle(ctx, JoyConBoolParam::IsConnected, state.any_connected());
        self.set_bool_handle(ctx, JoyConBoolParam::LeftControllerConnected, state.left.connected);
        self.set_bool_handle(ctx, JoyConBoolParam::RightControllerConnected, state.right.connected);

        let motion_mode = self.motion_data_mode();
        self.apply_controller_state(ctx, JoyConSide::Left, state.left.clone(), motion_mode);
        self.apply_controller_state(ctx, JoyConSide::Right, state.right.clone(), motion_mode);
        self.last_state = state;
    }

    fn apply_controller_state(
        &mut self,
        ctx: &mut ProcessCtx,
        side: JoyConSide,
        state: runtime::JoyConControllerStateSnapshot,
        motion_mode: JoyConMotionDataMode,
    ) {
        let (stick_x, stick_y) = self.process_stick_values(state.stick_x, state.stick_y);

        match side {
            JoyConSide::Left => {
                self.set_vec2_handle(ctx, JoyConVec2Param::LeftStick, stick_x, stick_y);
                self.set_bool_button(
                    ctx,
                    JoyConButtonParam::LeftDown,
                    state.left_buttons.contains(&Buttons::Down),
                );
                self.set_bool_button(
                    ctx,
                    JoyConButtonParam::LeftUp,
                    state.left_buttons.contains(&Buttons::Up),
                );
                self.set_bool_button(
                    ctx,
                    JoyConButtonParam::LeftRight,
                    state.left_buttons.contains(&Buttons::Right),
                );
                self.set_bool_button(
                    ctx,
                    JoyConButtonParam::LeftLeft,
                    state.left_buttons.contains(&Buttons::Left),
                );
                self.set_bool_button(
                    ctx,
                    JoyConButtonParam::LeftSL,
                    state.left_buttons.contains(&Buttons::SL),
                );
                self.set_bool_button(
                    ctx,
                    JoyConButtonParam::LeftSR,
                    state.left_buttons.contains(&Buttons::SR),
                );
                self.set_bool_button(ctx, JoyConButtonParam::LeftL, state.left_buttons.contains(&Buttons::L));
                self.set_bool_button(
                    ctx,
                    JoyConButtonParam::LeftZL,
                    state.left_buttons.contains(&Buttons::ZL),
                );
                self.set_bool_button(
                    ctx,
                    JoyConButtonParam::LeftMinus,
                    state.shared_buttons.contains(&Buttons::Minus),
                );
                self.set_bool_button(
                    ctx,
                    JoyConButtonParam::LeftStickButton,
                    state.shared_buttons.contains(&Buttons::LStick),
                );
                self.set_bool_button(
                    ctx,
                    JoyConButtonParam::LeftCapture,
                    state.shared_buttons.contains(&Buttons::Capture),
                );

                self.apply_motion_values(
                    ctx,
                    JoyConMotionTargets {
                        orientation: JoyConMotionParam::LeftOrientation,
                        accelerometer: JoyConMotionParam::LeftAccelerometer,
                        gyroscope: JoyConMotionParam::LeftGyroscope,
                    },
                    JoyConMotionSample::from(&state),
                    motion_mode,
                );
            }
            JoyConSide::Right => {
                self.set_vec2_handle(ctx, JoyConVec2Param::RightStick, stick_x, stick_y);
                self.set_bool_button(
                    ctx,
                    JoyConButtonParam::RightY,
                    state.right_buttons.contains(&Buttons::Y),
                );
                self.set_bool_button(
                    ctx,
                    JoyConButtonParam::RightX,
                    state.right_buttons.contains(&Buttons::X),
                );
                self.set_bool_button(
                    ctx,
                    JoyConButtonParam::RightB,
                    state.right_buttons.contains(&Buttons::B),
                );
                self.set_bool_button(
                    ctx,
                    JoyConButtonParam::RightA,
                    state.right_buttons.contains(&Buttons::A),
                );
                self.set_bool_button(
                    ctx,
                    JoyConButtonParam::RightSR,
                    state.right_buttons.contains(&Buttons::SR),
                );
                self.set_bool_button(
                    ctx,
                    JoyConButtonParam::RightSL,
                    state.right_buttons.contains(&Buttons::SL),
                );
                self.set_bool_button(
                    ctx,
                    JoyConButtonParam::RightR,
                    state.right_buttons.contains(&Buttons::R),
                );
                self.set_bool_button(
                    ctx,
                    JoyConButtonParam::RightZR,
                    state.right_buttons.contains(&Buttons::ZR),
                );
                self.set_bool_button(
                    ctx,
                    JoyConButtonParam::RightPlus,
                    state.shared_buttons.contains(&Buttons::Plus),
                );
                self.set_bool_button(
                    ctx,
                    JoyConButtonParam::RightStickButton,
                    state.shared_buttons.contains(&Buttons::RStick),
                );
                self.set_bool_button(
                    ctx,
                    JoyConButtonParam::RightHome,
                    state.shared_buttons.contains(&Buttons::Home),
                );

                self.apply_motion_values(
                    ctx,
                    JoyConMotionTargets {
                        orientation: JoyConMotionParam::RightOrientation,
                        accelerometer: JoyConMotionParam::RightAccelerometer,
                        gyroscope: JoyConMotionParam::RightGyroscope,
                    },
                    JoyConMotionSample::from(&state),
                    motion_mode,
                );
            }
        }
    }

    fn apply_motion_values(
        &mut self,
        ctx: &mut ProcessCtx,
        targets: JoyConMotionTargets,
        sample: JoyConMotionSample,
        motion_mode: JoyConMotionDataMode,
    ) {
        match motion_mode {
            JoyConMotionDataMode::None => {
                self.set_motion_vec2(ctx, targets.orientation, 0.0, 0.0);
                self.set_motion_vec3(ctx, targets.accelerometer, 0.0, 0.0, 0.0);
                self.set_motion_vec3(ctx, targets.gyroscope, 0.0, 0.0, 0.0);
            }
            JoyConMotionDataMode::Orientation => {
                self.set_motion_vec2(ctx, targets.orientation, sample.pitch, sample.roll);
                self.set_motion_vec3(ctx, targets.accelerometer, 0.0, 0.0, 0.0);
                self.set_motion_vec3(ctx, targets.gyroscope, 0.0, 0.0, 0.0);
            }
            JoyConMotionDataMode::All => {
                self.set_motion_vec2(ctx, targets.orientation, sample.pitch, sample.roll);
                self.set_motion_vec3(
                    ctx,
                    targets.accelerometer,
                    sample.accelerometer.0,
                    sample.accelerometer.1,
                    sample.accelerometer.2,
                );
                self.set_motion_vec3(
                    ctx,
                    targets.gyroscope,
                    sample.gyroscope.0,
                    sample.gyroscope.1,
                    sample.gyroscope.2,
                );
            }
        }
    }

    fn motion_data_mode(&self) -> JoyConMotionDataMode {
        JoyConMotionDataMode::from_variant_id(self.motion_data_mode.get_ref().as_str())
            .unwrap_or(JoyConMotionDataMode::None)
    }

    fn process_stick_values(&self, x: f64, y: f64) -> (f64, f64) {
        processed_stick_values(x, y, self.stick_dead_zone.get())
    }

    fn on_custom_event_inner(&mut self, ctx: &mut ProcessCtx, event: CustomEvent) {
        self.accept_output(ctx, event);
    }

    fn set_runtime_warning(&self, ctx: &mut ProcessCtx, message: &str) {
        if self.motion_data_mode.is_bound() && self.guard_warning(ctx) {
            self.motion_data_mode
                .set_warning_with(ctx, Some(JOYCON_RUNTIME_WARNING_ID), message, None);
        }
    }

    fn clear_runtime_warning(&self, ctx: &mut ProcessCtx) {
        if self.motion_data_mode.is_bound() && self.guard_warning(ctx) {
            self.motion_data_mode
                .clear_warning(ctx, Some(JOYCON_RUNTIME_WARNING_ID));
        }
    }

    fn set_last_event(&mut self, ctx: &mut ProcessCtx, value: String) {
        if self.last_event.is_bound() && self.last_event.get_ref() != value.as_str() {
            self.last_event.set(ctx, value);
        }
    }

    fn set_bool_handle(&mut self, ctx: &mut ProcessCtx, param: JoyConBoolParam, value: bool) {
        match param {
            JoyConBoolParam::Activity if self.activity.is_bound() && self.activity.get() != value => {
                self.activity.set(ctx, value);
            }
            JoyConBoolParam::IsConnected if self.is_connected.is_bound() && self.is_connected.get() != value => {
                self.is_connected.set(ctx, value);
            }
            JoyConBoolParam::LeftControllerConnected
                if self.left_controller_connected.is_bound() && self.left_controller_connected.get() != value =>
            {
                self.left_controller_connected.set(ctx, value);
            }
            JoyConBoolParam::RightControllerConnected
                if self.right_controller_connected.is_bound() && self.right_controller_connected.get() != value =>
            {
                self.right_controller_connected.set(ctx, value);
            }
            _ => {}
        }
    }

    fn set_bool_button(&mut self, ctx: &mut ProcessCtx, param: JoyConButtonParam, value: bool) {
        match param {
            JoyConButtonParam::LeftDown if self.left_dpad_down.is_bound() && self.left_dpad_down.get() != value => {
                self.left_dpad_down.set(ctx, value);
            }
            JoyConButtonParam::LeftUp if self.left_dpad_up.is_bound() && self.left_dpad_up.get() != value => {
                self.left_dpad_up.set(ctx, value);
            }
            JoyConButtonParam::LeftRight if self.left_dpad_right.is_bound() && self.left_dpad_right.get() != value => {
                self.left_dpad_right.set(ctx, value);
            }
            JoyConButtonParam::LeftLeft if self.left_dpad_left.is_bound() && self.left_dpad_left.get() != value => {
                self.left_dpad_left.set(ctx, value);
            }
            JoyConButtonParam::LeftSL if self.left_sl.is_bound() && self.left_sl.get() != value => {
                self.left_sl.set(ctx, value);
            }
            JoyConButtonParam::LeftSR if self.left_sr.is_bound() && self.left_sr.get() != value => {
                self.left_sr.set(ctx, value);
            }
            JoyConButtonParam::LeftL if self.left_l.is_bound() && self.left_l.get() != value => {
                self.left_l.set(ctx, value);
            }
            JoyConButtonParam::LeftZL if self.left_zl.is_bound() && self.left_zl.get() != value => {
                self.left_zl.set(ctx, value);
            }
            JoyConButtonParam::LeftMinus if self.left_minus.is_bound() && self.left_minus.get() != value => {
                self.left_minus.set(ctx, value);
            }
            JoyConButtonParam::LeftStickButton
                if self.left_stick_button.is_bound() && self.left_stick_button.get() != value =>
            {
                self.left_stick_button.set(ctx, value);
            }
            JoyConButtonParam::LeftCapture if self.left_capture.is_bound() && self.left_capture.get() != value => {
                self.left_capture.set(ctx, value);
            }
            JoyConButtonParam::RightY if self.right_y.is_bound() && self.right_y.get() != value => {
                self.right_y.set(ctx, value);
            }
            JoyConButtonParam::RightX if self.right_x.is_bound() && self.right_x.get() != value => {
                self.right_x.set(ctx, value);
            }
            JoyConButtonParam::RightB if self.right_b.is_bound() && self.right_b.get() != value => {
                self.right_b.set(ctx, value);
            }
            JoyConButtonParam::RightA if self.right_a.is_bound() && self.right_a.get() != value => {
                self.right_a.set(ctx, value);
            }
            JoyConButtonParam::RightSR if self.right_sr.is_bound() && self.right_sr.get() != value => {
                self.right_sr.set(ctx, value);
            }
            JoyConButtonParam::RightSL if self.right_sl.is_bound() && self.right_sl.get() != value => {
                self.right_sl.set(ctx, value);
            }
            JoyConButtonParam::RightR if self.right_r.is_bound() && self.right_r.get() != value => {
                self.right_r.set(ctx, value);
            }
            JoyConButtonParam::RightZR if self.right_zr.is_bound() && self.right_zr.get() != value => {
                self.right_zr.set(ctx, value);
            }
            JoyConButtonParam::RightPlus if self.right_plus.is_bound() && self.right_plus.get() != value => {
                self.right_plus.set(ctx, value);
            }
            JoyConButtonParam::RightStickButton
                if self.right_stick_button.is_bound() && self.right_stick_button.get() != value =>
            {
                self.right_stick_button.set(ctx, value);
            }
            JoyConButtonParam::RightHome if self.right_home.is_bound() && self.right_home.get() != value => {
                self.right_home.set(ctx, value);
            }
            _ => {}
        }
    }

    fn set_vec2_handle(&mut self, ctx: &mut ProcessCtx, param: JoyConVec2Param, x: f64, y: f64) {
        let value = Vec2::new(x, y);
        match param {
            JoyConVec2Param::LeftStick
                if self.left_stick.is_bound()
                    && (float_changed(self.left_stick.get().x, x) || float_changed(self.left_stick.get().y, y)) =>
            {
                self.left_stick.set(ctx, value);
            }
            JoyConVec2Param::RightStick
                if self.right_stick.is_bound()
                    && (float_changed(self.right_stick.get().x, x) || float_changed(self.right_stick.get().y, y)) =>
            {
                self.right_stick.set(ctx, value);
            }
            _ => {}
        }
    }

    fn set_motion_vec2(&mut self, ctx: &mut ProcessCtx, param: JoyConMotionParam, x: f64, y: f64) {
        let value = Vec2::new(x, y);
        match param {
            JoyConMotionParam::LeftOrientation
                if self.left_orientation.is_bound()
                    && (float_changed(self.left_orientation.get().x, x)
                        || float_changed(self.left_orientation.get().y, y)) =>
            {
                self.left_orientation.set(ctx, value);
            }
            JoyConMotionParam::RightOrientation
                if self.right_orientation.is_bound()
                    && (float_changed(self.right_orientation.get().x, x)
                        || float_changed(self.right_orientation.get().y, y)) =>
            {
                self.right_orientation.set(ctx, value);
            }
            _ => {}
        }
    }

    fn set_motion_vec3(&mut self, ctx: &mut ProcessCtx, param: JoyConMotionParam, x: f64, y: f64, z: f64) {
        let value = Vec3::new(x, y, z);
        match param {
            JoyConMotionParam::LeftAccelerometer
                if self.left_accelerometer.is_bound()
                    && (float_changed(self.left_accelerometer.get().x, x)
                        || float_changed(self.left_accelerometer.get().y, y)
                        || float_changed(self.left_accelerometer.get().z, z)) =>
            {
                self.left_accelerometer.set(ctx, value);
            }
            JoyConMotionParam::LeftGyroscope
                if self.left_gyroscope.is_bound()
                    && (float_changed(self.left_gyroscope.get().x, x)
                        || float_changed(self.left_gyroscope.get().y, y)
                        || float_changed(self.left_gyroscope.get().z, z)) =>
            {
                self.left_gyroscope.set(ctx, value);
            }
            JoyConMotionParam::RightAccelerometer
                if self.right_accelerometer.is_bound()
                    && (float_changed(self.right_accelerometer.get().x, x)
                        || float_changed(self.right_accelerometer.get().y, y)
                        || float_changed(self.right_accelerometer.get().z, z)) =>
            {
                self.right_accelerometer.set(ctx, value);
            }
            JoyConMotionParam::RightGyroscope
                if self.right_gyroscope.is_bound()
                    && (float_changed(self.right_gyroscope.get().x, x)
                        || float_changed(self.right_gyroscope.get().y, y)
                        || float_changed(self.right_gyroscope.get().z, z)) =>
            {
                self.right_gyroscope.set(ctx, value);
            }
            _ => {}
        }
    }
}

#[golden_core::item(
    "module",
    node = "joycon_module",
    via = base,
    from_struct,
    menu_path = ["Controllers"]
)]
impl Node for JoyConModule {
    fn init(&mut self, ctx: &mut ProcessCtx) {
        self.base.init(ctx);
        self.base.configure_command_tester(ctx, JOYCON_MODULE_COMMAND_TYPES);
        self.refresh_data_capabilities(ctx);
        crate::app::module::enable_module_authoring(self.node_data_mut());
    }

    fn on_node_ready(&mut self, ctx: &mut ProcessCtx, _context: NodeCreationContext) {
        if let Err(error) = self.register_service(ctx) {
            self.set_runtime_warning(ctx, &error);
        }
        self.request_service();
    }

    fn update(&mut self, ctx: &mut ProcessCtx) {
        self.service_io(ctx);
    }
    fn on_worker_wake(&mut self, ctx: &mut ProcessCtx) {
        self.service_io(ctx);
    }
    fn needs_update(&self) -> bool {
        false
    }
    fn worker_wake_requires_tree_snapshot(&self) -> bool {
        true
    }
    fn worker_wake_when_disabled(&self) -> bool {
        self.has_pending_results()
    }

    fn destroy(&mut self, _ctx: &mut ProcessCtx) {
        self.stop_runtime();
        self.pending_state = None;
        self.runtime_dirty = false;
        self.runtime_contact_elapsed = Duration::ZERO;
    }

    fn execution_rule(&self) -> NodeExecutionRule {
        NodeExecutionRule::passive().with_compiled_kernel("chataigne.runtime.joycon")
    }

    fn child_event_interest_depth(&self, _event: &Event) -> u32 {
        u32::MAX
    }

    fn update_requires_tree_snapshot(&self) -> bool {
        true
    }

    fn engine_script_descriptor(&self) -> NodeScriptDescriptor {
        crate::app::module::script_api::descriptor_for_node(self.node_data(), self.get_type(), JOYCON_SCRIPT_METHODS)
    }

    fn engine_call_script_method(
        &mut self,
        ctx: &mut ProcessCtx,
        method: &str,
        args: &[ParamValue],
    ) -> Result<bool, String> {
        if let Some(method) = match method {
            "vibrate" => Some("vibrate"),
            "setPlayerLights" => Some("setPlayerLights"),
            _ => None,
        } {
            self.capture_script(ctx, method, args)?;
            return Ok(true);
        }

        self.base.engine_call_script_method(ctx, method, args)
    }

    fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, old_value: ParamValue) {
        self.request_service();
        if let Some(snapshot_arc) = ctx.tree_snapshot_arc() {
            self.base
                .emit_script_param_callback(ctx, snapshot_arc.as_ref(), param, &old_value);
        }
        if (self.motion_data_mode.is_bound() && self.motion_data_mode.id() == param)
            || (self.stick_dead_zone.is_bound() && self.stick_dead_zone.id() == param)
        {
            self.guard_settings(ctx);
            self.apply_runtime_state(
                ctx,
                self.pending_state
                    .as_ref()
                    .and_then(|value| value.state())
                    .cloned()
                    .unwrap_or_else(|| self.last_state.clone()),
            );
        }
        self.configure_projection(ctx);
        if self.processing_fps_cap.is_bound() && self.processing_fps_cap.id() == param {
            self.processing_elapsed = processing_interval_from_fps_cap(self.processing_fps_cap.get());
        }
    }

    fn on_effective_enabled_changed(&mut self, ctx: &mut ProcessCtx, enabled: bool) {
        self.request_service();
        if enabled {
            self.runtime_dirty = true;
            self.processing_elapsed = Duration::ZERO;
            self.service.activity_until = None;
            self.incoming_log_elapsed = JOYCON_INCOMING_LOG_INTERVAL;
            self.runtime_contact_elapsed = Duration::ZERO;
        } else {
            self.stop_runtime();
            self.runtime_dirty = true;
            self.pending_state = None;
            self.processing_elapsed = Duration::ZERO;
            self.service.activity_until = None;
            self.incoming_log_elapsed = JOYCON_INCOMING_LOG_INTERVAL;
            self.runtime_contact_elapsed = Duration::ZERO;
            self.clear_runtime_warning(ctx);
            self.clear_input_pressure(ctx);
            self.apply_runtime_state(ctx, JoyConRuntimeState::disconnected());
            self.set_bool_handle(ctx, JoyConBoolParam::Activity, false);
        }
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: CustomEvent) {
        self.on_custom_event_inner(ctx, event);
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::create)
    }
}

#[cfg(test)]
mod tests;
