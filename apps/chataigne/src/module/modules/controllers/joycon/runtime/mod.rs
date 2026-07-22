use std::{
    convert::TryFrom,
    panic,
    sync::{
        mpsc::{self, Receiver, Sender, TryRecvError},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use joycon_rs::joycon::{
    device::calibration::stick::StickCalibration,
    device::{JoyConDevice, JoyConDeviceType},
    input_report_mode::{standard_full_mode::IMUData, InputReportMode, StandardFullMode, StandardInputReport},
    lights::{Flash, LightUp, Lights},
    Buttons, JoyConDriver, JoyConManager, Rumble, SimpleJoyConDriver,
};
use joycon_rs::result::JoyConError;

use crate::app::module::common::joycon::{
    JoyConControllerTarget, JoyConSetLedRequest, JoyConVibrateRequest, JOYCON_LED_STATE_FLASH, JOYCON_LED_STATE_OFF,
    JOYCON_LED_STATE_ON,
};

const JOYCON_WORKER_IDLE_POLL_INTERVAL: Duration = Duration::from_millis(8);
const JOYCON_ATTACH_RETRY_INTERVAL: Duration = Duration::from_millis(250);
const JOYCON_HEARTBEAT_INTERVAL: Duration = Duration::from_millis(250);
const JOYCON_REPORT_READ_TIMEOUT_MS: i32 = 1;
const JOYCON_REPORT_STALE_DISCONNECT_TIMEOUT: Duration = Duration::from_secs(2);
const JOYCON_MANAGER_DISCOVERY_SCAN_INTERVAL: Duration = Duration::from_millis(250);
const JOYCON_MANAGER_PARTIAL_SCAN_INTERVAL: Duration = Duration::from_secs(2);
const JOYCON_MANAGER_FULL_SCAN_INTERVAL: Duration = Duration::from_secs(5);
const JOYCON_STICK_NOISE_FLOOR: f64 = 0.02;
const JOYCON_STICK_QUANTUM: f64 = 0.001;
const JOYCON_ORIENTATION_QUANTUM_DEGREES: f64 = 0.25;
const JOYCON_RAW_IMU_QUANTUM: f64 = 4.0;
const JOYCON_ACCELEROMETER_FILTER_ALPHA: f64 = 0.2;
const JOYCON_ORIENTATION_FILTER_ALPHA: f64 = 0.8;

type SharedJoyConDevice = Arc<Mutex<JoyConDevice>>;
type JoyConMode = StandardFullMode<SimpleJoyConDriver>;
type JoyConReport = StandardInputReport<IMUData>;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub(crate) enum JoyConSide {
    Left,
    Right,
}

impl JoyConSide {
    pub(crate) const ALL: [Self; 2] = [Self::Left, Self::Right];

    pub(crate) const fn index(self) -> usize {
        match self {
            Self::Left => 0,
            Self::Right => 1,
        }
    }

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Left => "Left Controller",
            Self::Right => "Right Controller",
        }
    }

    fn from_device_type(device_type: JoyConDeviceType) -> Option<Self> {
        match device_type {
            JoyConDeviceType::JoyConL => Some(Self::Left),
            JoyConDeviceType::JoyConR => Some(Self::Right),
            JoyConDeviceType::ProCon => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct JoyConControllerStateSnapshot {
    pub connected: bool,
    pub stick_x: f64,
    pub stick_y: f64,
    pub left_buttons: Vec<Buttons>,
    pub right_buttons: Vec<Buttons>,
    pub shared_buttons: Vec<Buttons>,
    pub orientation_pitch: f64,
    pub orientation_roll: f64,
    pub accelerometer: (f64, f64, f64),
    pub gyroscope: (f64, f64, f64),
}

impl JoyConControllerStateSnapshot {
    fn disconnected() -> Self {
        Self {
            connected: false,
            stick_x: 0.0,
            stick_y: 0.0,
            left_buttons: Vec::new(),
            right_buttons: Vec::new(),
            shared_buttons: Vec::new(),
            orientation_pitch: 0.0,
            orientation_roll: 0.0,
            accelerometer: (0.0, 0.0, 0.0),
            gyroscope: (0.0, 0.0, 0.0),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct JoyConRuntimeState {
    pub left: JoyConControllerStateSnapshot,
    pub right: JoyConControllerStateSnapshot,
}

impl JoyConRuntimeState {
    pub(crate) fn disconnected() -> Self {
        Self {
            left: JoyConControllerStateSnapshot::disconnected(),
            right: JoyConControllerStateSnapshot::disconnected(),
        }
    }

    pub(crate) fn any_connected(&self) -> bool {
        self.left.connected || self.right.connected
    }

    pub(crate) fn side(&self, side: JoyConSide) -> &JoyConControllerStateSnapshot {
        match side {
            JoyConSide::Left => &self.left,
            JoyConSide::Right => &self.right,
        }
    }

    pub(crate) fn side_mut(&mut self, side: JoyConSide) -> &mut JoyConControllerStateSnapshot {
        match side {
            JoyConSide::Left => &mut self.left,
            JoyConSide::Right => &mut self.right,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum JoyConWorkerEvent {
    Heartbeat,
    State(Box<JoyConRuntimeState>),
    CommandResult(String),
    Error(String),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum JoyConWorkerCommand {
    Vibrate(JoyConVibrateRequest),
    SetPlayerLights(JoyConSetLedRequest),
    Stop,
}

pub(crate) struct JoyConRuntimeHandle {
    command_tx: Sender<JoyConWorkerCommand>,
    event_rx: Receiver<JoyConWorkerEvent>,
    worker: Option<JoinHandle<()>>,
}

impl JoyConRuntimeHandle {
    pub(crate) fn spawn() -> Result<Self, String> {
        let (command_tx, command_rx) = mpsc::channel();
        let (event_tx, event_rx) = mpsc::channel();

        let worker = thread::Builder::new()
            .name("joycon-runtime".to_string())
            .spawn(move || worker_loop(command_rx, event_tx))
            .map_err(|error| format!("failed to start Joy-Con worker thread: {error}"))?;

        Ok(Self {
            command_tx,
            event_rx,
            worker: Some(worker),
        })
    }

    pub(crate) fn send(&self, command: JoyConWorkerCommand) -> Result<(), String> {
        self.command_tx
            .send(command)
            .map_err(|_| "Joy-Con worker is no longer running".to_string())
    }

    pub(crate) fn try_recv(&self) -> Result<JoyConWorkerEvent, TryRecvError> {
        self.event_rx.try_recv()
    }

    pub(crate) fn stop(&mut self) {
        let _ = self.command_tx.send(JoyConWorkerCommand::Stop);
        if let Some(worker) = self.worker.take() {
            let _ = thread::Builder::new()
                .name("joycon-runtime-stop".to_string())
                .spawn(move || {
                    let _ = worker.join();
                });
        }
    }
}

impl Drop for JoyConRuntimeHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

struct ActiveJoyConController {
    side: JoyConSide,
    mode: JoyConMode,
    stick_calibration: StickNormalization,
    rumble_stop_at: Option<Instant>,
    attached_at: Instant,
    last_report_at: Option<Instant>,
    filtered_accelerometer: Option<(f64, f64, f64)>,
    filtered_orientation: Option<(f64, f64)>,
}

impl ActiveJoyConController {
    fn attach(side: JoyConSide, device: &SharedJoyConDevice) -> Result<Self, String> {
        let stick_calibration = stick_normalization_for_device(device, side);
        let mode = StandardFullMode::new(SimpleJoyConDriver::new(device).map_err(joycon_error)?)
            .map_err(joycon_error)?;

        Ok(Self {
            side,
            mode,
            stick_calibration,
            rumble_stop_at: None,
            attached_at: Instant::now(),
            last_report_at: None,
            filtered_accelerometer: None,
            filtered_orientation: None,
        })
    }

    fn is_connected(&self) -> bool {
        self.mode.driver().joycon().is_connected()
    }

    fn note_report(&mut self, timestamp: Instant) {
        self.last_report_at = Some(timestamp);
    }

    fn activity_is_stale(&self, timestamp: Instant) -> bool {
        report_is_stale(
            Some(self.last_report_at.unwrap_or(self.attached_at)),
            timestamp,
            JOYCON_REPORT_STALE_DISCONNECT_TIMEOUT,
        )
    }
}

fn worker_loop(command_rx: Receiver<JoyConWorkerCommand>, event_tx: Sender<JoyConWorkerEvent>) {
    let manager = match panic::catch_unwind(JoyConManager::get_instance) {
        Ok(manager) => manager,
        Err(_) => {
            let _ = event_tx.send(JoyConWorkerEvent::Error(
                "failed to initialize the Joy-Con runtime".to_string(),
            ));
            return;
        }
    };

    let mut controllers: [Option<ActiveJoyConController>; 2] = [None, None];
    let mut state = JoyConRuntimeState::disconnected();
    let mut next_attach_at = Instant::now();
    let mut configured_manager_scan_interval = Duration::ZERO;
    let mut last_heartbeat_at = None;

    if event_tx
        .send(JoyConWorkerEvent::State(Box::new(state.clone())))
        .is_err()
    {
        return;
    }

    loop {
        if drain_commands(&command_rx, &event_tx, &mut controllers) {
            break;
        }

        let now = Instant::now();
        let mut state_changed = false;
        let connected_slot_count = connected_controller_count(&controllers);

        update_manager_scan_interval(&manager, connected_slot_count, &mut configured_manager_scan_interval);

        if should_attempt_attach(connected_slot_count) && now >= next_attach_at {
            match attach_available_controllers(&manager, &mut controllers) {
                Ok(changed) => {
                    state_changed |= changed;
                }
                Err(error) => {
                    if event_tx.send(JoyConWorkerEvent::Error(error)).is_err() {
                        return;
                    }
                }
            }
            next_attach_at = now + JOYCON_ATTACH_RETRY_INTERVAL;
        }

        for side in JoyConSide::ALL {
            let index = side.index();
            let Some(controller) = controllers[index].as_mut() else {
                if state.side(side).connected {
                    *state.side_mut(side) = JoyConControllerStateSnapshot::disconnected();
                    state_changed = true;
                }
                continue;
            };

            match try_read_report(&controller.mode) {
                ReadReportResult::Report(report) => {
                    controller.note_report(now);
                    if should_emit_heartbeat(&mut last_heartbeat_at, now)
                        && event_tx.send(JoyConWorkerEvent::Heartbeat).is_err()
                    {
                        return;
                    }
                    let next = snapshot_from_report(controller, &report);
                    if state.side(side) != &next {
                        *state.side_mut(side) = next;
                        state_changed = true;
                    }
                }
                ReadReportResult::NoData | ReadReportResult::TransientError => {
                    state_changed |= detach_if_inactive(
                        &mut controllers,
                        &mut state,
                        side,
                        now,
                        false,
                        None,
                        &event_tx,
                    );
                }
                ReadReportResult::Disconnected(error) => {
                    let detached = detach_if_inactive(
                        &mut controllers,
                        &mut state,
                        side,
                        now,
                        true,
                        Some(error),
                        &event_tx,
                    );
                    if detached {
                        state_changed = true;
                        next_attach_at = Instant::now();
                    }
                }
            }
        }

        if update_pending_rumble(&mut controllers).is_err() {
            next_attach_at = Instant::now();
        }

        if state_changed
            && event_tx
                .send(JoyConWorkerEvent::State(Box::new(state.clone())))
                .is_err()
        {
            return;
        }

        if controllers.iter().all(Option::is_none) {
            thread::sleep(JOYCON_WORKER_IDLE_POLL_INTERVAL);
        }
    }

    stop_all_rumble(&mut controllers);
}

fn drain_commands(
    command_rx: &Receiver<JoyConWorkerCommand>,
    event_tx: &Sender<JoyConWorkerEvent>,
    controllers: &mut [Option<ActiveJoyConController>; 2],
) -> bool {
    loop {
        match command_rx.try_recv() {
            Ok(JoyConWorkerCommand::Stop) => return true,
            Ok(command) => match handle_command(command, controllers) {
                Ok(message) => {
                    if event_tx.send(JoyConWorkerEvent::CommandResult(message)).is_err() {
                        return true;
                    }
                }
                Err(error) => {
                    if event_tx.send(JoyConWorkerEvent::Error(error)).is_err() {
                        return true;
                    }
                }
            },
            Err(TryRecvError::Empty) => return false,
            Err(TryRecvError::Disconnected) => return true,
        }
    }
}

fn handle_command(
    command: JoyConWorkerCommand,
    controllers: &mut [Option<ActiveJoyConController>; 2],
) -> Result<String, String> {
    match command {
        JoyConWorkerCommand::Vibrate(request) => handle_vibrate(request, controllers),
        JoyConWorkerCommand::SetPlayerLights(request) => handle_set_player_lights(request, controllers),
        JoyConWorkerCommand::Stop => Ok("stopped Joy-Con worker".to_string()),
    }
}

fn handle_vibrate(
    request: JoyConVibrateRequest,
    controllers: &mut [Option<ActiveJoyConController>; 2],
) -> Result<String, String> {
    let sides = resolve_target_sides(request.target.as_str(), controllers)?;
    let duration = Duration::from_millis(u64::from(request.duration_ms.max(1)));
    let deadline = Instant::now() + duration;

    for side in sides.iter().copied() {
        let controller = controller_mut(controllers, side)?;
        let rumble = Rumble::new(request.frequency_hz.clamp(0.0, 1252.0), request.amplitude.clamp(0.0, 1.799));
        controller
            .mode
            .driver_mut()
            .rumble((Some(rumble), Some(rumble)))
            .map_err(joycon_error)?;
        controller.rumble_stop_at = Some(deadline);
    }

    Ok(format!("Sent vibrate to {}", target_label(sides.as_slice())))
}

fn handle_set_player_lights(
    request: JoyConSetLedRequest,
    controllers: &mut [Option<ActiveJoyConController>; 2],
) -> Result<String, String> {
    let sides = resolve_target_sides(request.target.as_str(), controllers)?;
    let (light_up, flash) = resolve_led_states([request.led_1, request.led_2, request.led_3, request.led_4])?;

    for side in sides.iter().copied() {
        controller_mut(controllers, side)?
            .mode
            .driver_mut()
            .set_player_lights(&light_up, &flash)
            .map_err(joycon_error)?;
    }

    Ok(format!("Set player lights on {}", target_label(sides.as_slice())))
}

fn attach_available_controllers(
    manager: &Arc<Mutex<JoyConManager>>,
    controllers: &mut [Option<ActiveJoyConController>; 2],
) -> Result<bool, String> {
    let devices = {
        let manager = match manager.try_lock() {
            Ok(manager) => manager,
            Err(std::sync::TryLockError::WouldBlock) => return Ok(false),
            Err(std::sync::TryLockError::Poisoned(error)) => error.into_inner(),
        };
        manager.managed_devices()
    };

    let mut attached_controller = false;
    let mut errors = Vec::new();

    for device in devices {
        let side = match device_side(&device) {
            Some(side) => side,
            None => continue,
        };
        if controllers[side.index()].is_some() {
            continue;
        }

        match ActiveJoyConController::attach(side, &device) {
            Ok(controller) => {
                controllers[side.index()] = Some(controller);
                attached_controller = true;
            }
            Err(error) => errors.push(format!("failed to attach {}: {error}", side.label())),
        }
    }

    if errors.is_empty() {
        Ok(attached_controller)
    } else {
        Err(errors.join("; "))
    }
}

fn connected_controller_count(controllers: &[Option<ActiveJoyConController>; 2]) -> usize {
    controllers.iter().filter(|controller| controller.is_some()).count()
}

fn should_attempt_attach(connected_slot_count: usize) -> bool {
    connected_slot_count < JoyConSide::ALL.len()
}

fn manager_scan_interval_for_connected_slots(connected_slot_count: usize) -> Duration {
    match connected_slot_count {
        0 => JOYCON_MANAGER_DISCOVERY_SCAN_INTERVAL,
        1 => JOYCON_MANAGER_PARTIAL_SCAN_INTERVAL,
        _ => JOYCON_MANAGER_FULL_SCAN_INTERVAL,
    }
}

fn update_manager_scan_interval(
    manager: &Arc<Mutex<JoyConManager>>,
    connected_slot_count: usize,
    configured_scan_interval: &mut Duration,
) {
    let desired_interval = manager_scan_interval_for_connected_slots(connected_slot_count);
    if *configured_scan_interval == desired_interval {
        return;
    }

    let mut manager = match manager.try_lock() {
        Ok(manager) => manager,
        Err(std::sync::TryLockError::WouldBlock) => return,
        Err(std::sync::TryLockError::Poisoned(error)) => error.into_inner(),
    };
    manager.set_interval(desired_interval);
    *configured_scan_interval = desired_interval;
}

enum ReadReportResult {
    Report(JoyConReport),
    NoData,
    TransientError,
    Disconnected(String),
}

fn try_read_report(mode: &JoyConMode) -> ReadReportResult {
    let mut buf = [0u8; 362];
    let bytes_read = match mode.driver().read_timeout(&mut buf, JOYCON_REPORT_READ_TIMEOUT_MS) {
        Ok(bytes_read) => bytes_read,
        Err(error) => return classify_read_error(error),
    };
    if bytes_read == 0 {
        return ReadReportResult::NoData;
    }

    match StandardInputReport::try_from(buf) {
        Ok(report) => ReadReportResult::Report(report),
        Err(error) => classify_read_error(error),
    }
}

fn snapshot_from_report(controller: &mut ActiveJoyConController, report: &JoyConReport) -> JoyConControllerStateSnapshot {
    let stick = match controller.side {
        JoyConSide::Left => &report.common.left_analog_stick_data,
        JoyConSide::Right => &report.common.right_analog_stick_data,
    };
    let accel_x = quantize_float(
        average_axis_samples(report.extra.data.map(|sample| f64::from(sample.accel_x))),
        JOYCON_RAW_IMU_QUANTUM,
    );
    let accel_y = quantize_float(
        average_axis_samples(report.extra.data.map(|sample| f64::from(sample.accel_y))),
        JOYCON_RAW_IMU_QUANTUM,
    );
    let accel_z = quantize_float(
        average_axis_samples(report.extra.data.map(|sample| f64::from(sample.accel_z))),
        JOYCON_RAW_IMU_QUANTUM,
    );
    let gyro_x = quantize_float(
        average_axis_samples(report.extra.data.map(|sample| f64::from(sample.gyro_1))),
        JOYCON_RAW_IMU_QUANTUM,
    );
    let gyro_y = quantize_float(
        average_axis_samples(report.extra.data.map(|sample| f64::from(sample.gyro_2))),
        JOYCON_RAW_IMU_QUANTUM,
    );
    let gyro_z = quantize_float(
        average_axis_samples(report.extra.data.map(|sample| f64::from(sample.gyro_3))),
        JOYCON_RAW_IMU_QUANTUM,
    );
    let filtered_accelerometer = low_pass_vec3(
        controller.filtered_accelerometer,
        (accel_x, accel_y, accel_z),
        JOYCON_ACCELEROMETER_FILTER_ALPHA,
    );
    controller.filtered_accelerometer = Some(filtered_accelerometer);

    let raw_orientation = orientation_from_acceleration(
        filtered_accelerometer.0,
        filtered_accelerometer.1,
        filtered_accelerometer.2,
    );
    let filtered_orientation = low_pass_vec2(
        controller.filtered_orientation,
        raw_orientation,
        JOYCON_ORIENTATION_FILTER_ALPHA,
    );
    controller.filtered_orientation = Some(filtered_orientation);

    JoyConControllerStateSnapshot {
        connected: true,
        stick_x: stabilize_stick_value(controller.stick_calibration.normalize_x(stick.horizontal)),
        stick_y: stabilize_stick_value(controller.stick_calibration.normalize_y(stick.vertical)),
        left_buttons: report.common.pushed_buttons.left.clone(),
        right_buttons: report.common.pushed_buttons.right.clone(),
        shared_buttons: report.common.pushed_buttons.shared.clone(),
        orientation_pitch: quantize_float(filtered_orientation.0, JOYCON_ORIENTATION_QUANTUM_DEGREES),
        orientation_roll: quantize_float(filtered_orientation.1, JOYCON_ORIENTATION_QUANTUM_DEGREES),
        accelerometer: filtered_accelerometer,
        gyroscope: (gyro_x, gyro_y, gyro_z),
    }
}

fn update_pending_rumble(controllers: &mut [Option<ActiveJoyConController>; 2]) -> Result<(), String> {
    let now = Instant::now();

    for side in JoyConSide::ALL {
        let Some(controller) = controllers[side.index()].as_mut() else {
            continue;
        };
        if controller.rumble_stop_at.is_none_or(|deadline| deadline > now) {
            continue;
        }

        controller
            .mode
            .driver_mut()
            .rumble((Some(Rumble::stop()), Some(Rumble::stop())))
            .map_err(joycon_error)?;
        controller.rumble_stop_at = None;
    }

    Ok(())
}

fn stop_all_rumble(controllers: &mut [Option<ActiveJoyConController>; 2]) {
    for side in JoyConSide::ALL {
        let Some(controller) = controllers[side.index()].as_mut() else {
            continue;
        };
        let _ = controller
            .mode
            .driver_mut()
            .rumble((Some(Rumble::stop()), Some(Rumble::stop())));
        controller.rumble_stop_at = None;
    }
}

fn device_side(device: &SharedJoyConDevice) -> Option<JoyConSide> {
    let device = match device.lock() {
        Ok(device) => device,
        Err(error) => error.into_inner(),
    };
    JoyConSide::from_device_type(device.device_type())
}

fn controller_mut(
    controllers: &mut [Option<ActiveJoyConController>; 2],
    side: JoyConSide,
) -> Result<&mut ActiveJoyConController, String> {
    controllers[side.index()]
        .as_mut()
        .ok_or_else(|| format!("{} is not connected", side.label()))
}

fn resolve_target_sides(
    target: &str,
    controllers: &[Option<ActiveJoyConController>; 2],
) -> Result<Vec<JoyConSide>, String> {
    match JoyConControllerTarget::from_variant_id(target).unwrap_or(JoyConControllerTarget::Both) {
        JoyConControllerTarget::Left => {
            if controllers[JoyConSide::Left.index()].is_some() {
                Ok(vec![JoyConSide::Left])
            } else {
                Err("Left Controller is not connected".to_string())
            }
        }
        JoyConControllerTarget::Right => {
            if controllers[JoyConSide::Right.index()].is_some() {
                Ok(vec![JoyConSide::Right])
            } else {
                Err("Right Controller is not connected".to_string())
            }
        }
        JoyConControllerTarget::Both => {
            let mut sides = Vec::new();
            if controllers[JoyConSide::Left.index()].is_some() {
                sides.push(JoyConSide::Left);
            }
            if controllers[JoyConSide::Right.index()].is_some() {
                sides.push(JoyConSide::Right);
            }

            if sides.is_empty() {
                Err("No Joy-Con controllers are connected".to_string())
            } else {
                Ok(sides)
            }
        }
    }
}

fn resolve_led_states(led_states: [String; 4]) -> Result<(Vec<LightUp>, Vec<Flash>), String> {
    let mappings = [
        (LightUp::LED0, Flash::LED0),
        (LightUp::LED1, Flash::LED1),
        (LightUp::LED2, Flash::LED2),
        (LightUp::LED3, Flash::LED3),
    ];

    let mut light_up = Vec::new();
    let mut flash = Vec::new();
    for (index, state) in led_states.into_iter().enumerate() {
        match state.trim() {
            JOYCON_LED_STATE_OFF => {}
            JOYCON_LED_STATE_ON => light_up.push(mappings[index].0),
            JOYCON_LED_STATE_FLASH => flash.push(mappings[index].1),
            other => return Err(format!("invalid Joy-Con LED state '{other}' for LED {}", index + 1)),
        }
    }

    Ok((light_up, flash))
}

fn target_label(sides: &[JoyConSide]) -> &'static str {
    match sides {
        [JoyConSide::Left] => "Left Controller",
        [JoyConSide::Right] => "Right Controller",
        _ => "both controllers",
    }
}

#[derive(Clone, Copy, Debug)]
struct StickNormalization {
    center_x: f64,
    min_x: f64,
    max_x: f64,
    center_y: f64,
    min_y: f64,
    max_y: f64,
}

impl StickNormalization {
    fn default_raw() -> Self {
        Self {
            center_x: 2048.0,
            min_x: 0.0,
            max_x: 4095.0,
            center_y: 2048.0,
            min_y: 0.0,
            max_y: 4095.0,
        }
    }

    fn normalize_x(self, value: u16) -> f64 {
        normalize_axis(value, self.center_x, self.min_x, self.max_x)
    }

    fn normalize_y(self, value: u16) -> f64 {
        normalize_axis(value, self.center_y, self.min_y, self.max_y)
    }
}

fn stick_normalization_for_device(device: &SharedJoyConDevice, side: JoyConSide) -> StickNormalization {
    let device = match device.lock() {
        Ok(device) => device,
        Err(error) => error.into_inner(),
    };

    let preferred = match side {
        JoyConSide::Left => device.stick_user_calibration().left(),
        JoyConSide::Right => device.stick_user_calibration().right(),
    };
    if let Some(normalization) = stick_normalization_from_calibration(preferred) {
        return normalization;
    }

    let fallback = match side {
        JoyConSide::Left => device.stick_factory_calibration().left(),
        JoyConSide::Right => device.stick_factory_calibration().right(),
    };
    stick_normalization_from_calibration(fallback).unwrap_or_else(StickNormalization::default_raw)
}

fn stick_normalization_from_calibration(calibration: &StickCalibration) -> Option<StickNormalization> {
    let StickCalibration::Available { x, y } = calibration else {
        return None;
    };

    Some(StickNormalization {
        center_x: f64::from(x.center()),
        min_x: f64::from(x.min()),
        max_x: f64::from(x.max()),
        center_y: f64::from(y.center()),
        min_y: f64::from(y.min()),
        max_y: f64::from(y.max()),
    })
}

fn normalize_axis(value: u16, center: f64, min: f64, max: f64) -> f64 {
    let value = f64::from(value);
    if value >= center {
        let span = (max - center).max(1.0);
        ((value - center) / span).clamp(-1.0, 1.0)
    } else {
        let span = (center - min).max(1.0);
        ((value - center) / span).clamp(-1.0, 1.0)
    }
}

fn stabilize_stick_value(value: f64) -> f64 {
    let centered = if value.abs() <= JOYCON_STICK_NOISE_FLOOR { 0.0 } else { value };
    quantize_float(centered, JOYCON_STICK_QUANTUM)
}

fn average_axis_samples(samples: [f64; 3]) -> f64 {
    samples.into_iter().sum::<f64>() / 3.0
}

fn should_emit_heartbeat(last_heartbeat_at: &mut Option<Instant>, now: Instant) -> bool {
    if last_heartbeat_at
        .is_some_and(|last_heartbeat_at| now.saturating_duration_since(last_heartbeat_at) < JOYCON_HEARTBEAT_INTERVAL)
    {
        return false;
    }

    *last_heartbeat_at = Some(now);
    true
}

fn low_pass_vec2(previous: Option<(f64, f64)>, next: (f64, f64), alpha: f64) -> (f64, f64) {
    match previous {
        Some(previous) => (
            low_pass_scalar(previous.0, next.0, alpha),
            low_pass_scalar(previous.1, next.1, alpha),
        ),
        None => next,
    }
}

fn low_pass_vec3(previous: Option<(f64, f64, f64)>, next: (f64, f64, f64), alpha: f64) -> (f64, f64, f64) {
    match previous {
        Some(previous) => (
            low_pass_scalar(previous.0, next.0, alpha),
            low_pass_scalar(previous.1, next.1, alpha),
            low_pass_scalar(previous.2, next.2, alpha),
        ),
        None => next,
    }
}

fn low_pass_scalar(previous: f64, next: f64, alpha: f64) -> f64 {
    let alpha = alpha.clamp(0.0, 1.0);
    previous + (next - previous) * alpha
}

fn quantize_float(value: f64, quantum: f64) -> f64 {
    if quantum <= f64::EPSILON {
        return value;
    }

    (value / quantum).round() * quantum
}

fn report_is_stale(last_report_at: Option<Instant>, now: Instant, stale_timeout: Duration) -> bool {
    last_report_at.is_some_and(|last_report_at| now.saturating_duration_since(last_report_at) >= stale_timeout)
}

fn detach_if_inactive(
    controllers: &mut [Option<ActiveJoyConController>; 2],
    state: &mut JoyConRuntimeState,
    side: JoyConSide,
    now: Instant,
    allow_disconnect_signal_without_connection_check: bool,
    error: Option<String>,
    event_tx: &Sender<JoyConWorkerEvent>,
) -> bool {
    let should_detach = {
        let Some(controller) = controllers[side.index()].as_ref() else {
            return false;
        };
        let connection_check_failed = !controller.is_connected();
        controller.activity_is_stale(now)
            && (connection_check_failed || allow_disconnect_signal_without_connection_check)
    };

    if !should_detach {
        return false;
    }

    controllers[side.index()] = None;
    let mut state_changed = false;
    if state.side(side).connected {
        *state.side_mut(side) = JoyConControllerStateSnapshot::disconnected();
        state_changed = true;
    }

    if let Some(error) = error {
        if event_tx
            .send(JoyConWorkerEvent::Error(format!("{} disconnected: {error}", side.label())))
            .is_err()
        {
            return state_changed;
        }
    }

    state_changed
}

fn classify_read_error(error: JoyConError) -> ReadReportResult {
    match error {
        JoyConError::Disconnected => ReadReportResult::Disconnected("Joy-Con disconnected".to_string()),
        JoyConError::JoyConReportError(_) | JoyConError::HidApiError(_) => ReadReportResult::TransientError,
        JoyConError::SubCommandError(_, _) | JoyConError::JoyConDeviceError(_) => ReadReportResult::TransientError,
    }
}

fn orientation_from_acceleration(accel_x: f64, accel_y: f64, accel_z: f64) -> (f64, f64) {
    let roll = accel_y.atan2(accel_z).to_degrees();
    let pitch = (-accel_x)
        .atan2((accel_y.mul_add(accel_y, accel_z * accel_z)).sqrt())
        .to_degrees();

    (pitch, roll)
}

fn joycon_error<E: std::fmt::Debug>(error: E) -> String {
    format!("{error:?}")
}

#[cfg(test)]
mod tests;
