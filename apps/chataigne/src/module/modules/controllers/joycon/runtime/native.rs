use super::*;
use joycon_rs::joycon::{
    device::calibration::stick::StickCalibration,
    device::JoyConDevice,
    input_report_mode::{standard_full_mode::IMUData, InputReportMode, StandardFullMode, StandardInputReport},
    lights::{Flash, LightUp, Lights},
    JoyConDriver, JoyConManager, Rumble, SimpleJoyConDriver,
};
use joycon_rs::result::JoyConError;
use std::{convert::TryFrom, panic};
const JOYCON_ATTACH_RETRY_INTERVAL: Duration = Duration::from_millis(250);
const JOYCON_HEARTBEAT_INTERVAL: Duration = Duration::from_millis(250);
const JOYCON_REPORT_READ_TIMEOUT_MS: i32 = 1;
pub(super) const JOYCON_REPORT_STALE_DISCONNECT_TIMEOUT: Duration = Duration::from_secs(2);
pub(super) const JOYCON_MANAGER_DISCOVERY_SCAN_INTERVAL: Duration = Duration::from_millis(250);
pub(super) const JOYCON_MANAGER_PARTIAL_SCAN_INTERVAL: Duration = Duration::from_secs(2);
pub(super) const JOYCON_MANAGER_FULL_SCAN_INTERVAL: Duration = Duration::from_secs(5);
const JOYCON_STICK_NOISE_FLOOR: f64 = 0.02;
const JOYCON_STICK_QUANTUM: f64 = 0.001;
const JOYCON_ORIENTATION_QUANTUM_DEGREES: f64 = 0.25;
const JOYCON_RAW_IMU_QUANTUM: f64 = 4.0;
const JOYCON_ACCELEROMETER_FILTER_ALPHA: f64 = 0.2;
const JOYCON_ORIENTATION_FILTER_ALPHA: f64 = 0.8;
type SharedJoyConDevice = Arc<Mutex<JoyConDevice>>;
type JoyConMode = StandardFullMode<SimpleJoyConDriver>;
type JoyConReport = StandardInputReport<IMUData>;

pub(super) struct Native {
    manager: Arc<Mutex<JoyConManager>>,
    controllers: [Option<ActiveJoyConController>; 2],
    state: JoyConRuntimeState,
    next_attach_at: Instant,
    scan_interval: Duration,
    heartbeat: Option<Instant>,
    maintenance_error: Option<String>,
}
impl Native {
    pub(super) fn open() -> Result<Self, String> {
        if cfg!(test) {
            return Err("native Joy-Con endpoints are disabled in test builds".into());
        }
        let manager =
            panic::catch_unwind(JoyConManager::get_instance).map_err(|_| "failed to initialize the Joy-Con runtime")?;
        Ok(Self {
            manager,
            controllers: [None, None],
            state: JoyConRuntimeState::disconnected(),
            next_attach_at: Instant::now(),
            scan_interval: Duration::ZERO,
            heartbeat: None,
            maintenance_error: None,
        })
    }
}
impl Provider for Native {
    fn execute(&mut self, command: &JoyConWorkerCommand) -> Result<String, String> {
        handle_command(command.clone(), &mut self.controllers)
    }
    fn poll(&mut self) -> Vec<JoyConWorkerEvent> {
        let now = Instant::now();
        let mut events = Vec::with_capacity(4);
        let mut changed = false;
        let count = connected_controller_count(&self.controllers);
        update_manager_scan_interval(&self.manager, count, &mut self.scan_interval);
        if should_attempt_attach(count) && now >= self.next_attach_at {
            match attach_available_controllers(&self.manager, &mut self.controllers) {
                Ok(value) => changed |= value,
                Err(error) => events.push(JoyConWorkerEvent::Error(error)),
            }
            self.next_attach_at = now + JOYCON_ATTACH_RETRY_INTERVAL;
        }
        for side in JoyConSide::ALL {
            let Some(controller) = self.controllers[side.index()].as_mut() else {
                if self.state.side(side).connected {
                    *self.state.side_mut(side) = JoyConControllerStateSnapshot::disconnected();
                    changed = true;
                }
                continue;
            };
            match try_read_report(&controller.mode) {
                ReadReportResult::Report(report) => {
                    controller.note_report(now);
                    if should_emit_heartbeat(&mut self.heartbeat, now) {
                        events.push(JoyConWorkerEvent::Heartbeat);
                    }
                    let next = snapshot_from_report(controller, &report);
                    if self.state.side(side) != &next {
                        *self.state.side_mut(side) = next;
                        changed = true;
                    }
                }
                ReadReportResult::NoData | ReadReportResult::TransientError => {
                    changed |= detach_if_inactive(
                        &mut self.controllers,
                        &mut self.state,
                        side,
                        now,
                        false,
                        None,
                        &mut events,
                    );
                }
                ReadReportResult::Disconnected(error) => {
                    if detach_if_inactive(
                        &mut self.controllers,
                        &mut self.state,
                        side,
                        now,
                        true,
                        Some(error),
                        &mut events,
                    ) {
                        changed = true;
                        self.next_attach_at = now;
                    }
                }
            }
        }
        if let Some(error) = self.maintenance_error.take() {
            events.push(JoyConWorkerEvent::Error(error));
            self.next_attach_at = now;
        }
        if changed {
            events.push(JoyConWorkerEvent::State(Box::new(self.state.clone())));
        }
        events
    }
    fn idle_interval(&self) -> Duration {
        if self.controllers.iter().all(Option::is_none) {
            Duration::from_millis(8)
        } else {
            Duration::ZERO
        }
    }
    fn maintain(&mut self) {
        if let Err(error) = update_pending_rumble(&mut self.controllers) {
            if self.maintenance_error.is_none() {
                self.maintenance_error = Some(error);
            }
        }
    }
}
impl Drop for Native {
    fn drop(&mut self) {
        stop_all_rumble(&mut self.controllers);
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
        let mode =
            StandardFullMode::new(SimpleJoyConDriver::new(device).map_err(joycon_error)?).map_err(joycon_error)?;

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

fn handle_command(
    command: JoyConWorkerCommand,
    controllers: &mut [Option<ActiveJoyConController>; 2],
) -> Result<String, String> {
    match command {
        JoyConWorkerCommand::Vibrate(request) => handle_vibrate(request, controllers),
        JoyConWorkerCommand::SetPlayerLights(request) => handle_set_player_lights(request, controllers),
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
        let rumble = Rumble::new(
            request.frequency_hz.clamp(0.0, 1252.0),
            request.amplitude.clamp(0.0, 1.799),
        );
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

    if devices.len() > 64 {
        return Err("Joy-Con discovery exceeds 64 devices".into());
    }
    let mut attached_controller = false;
    let mut attempted = [false; 2];
    let mut errors = Vec::new();

    for device in devices {
        let side = match device_side(&device) {
            Some(side) => side,
            None => continue,
        };
        if controllers[side.index()].is_some() || attempted[side.index()] {
            continue;
        }
        attempted[side.index()] = true;

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

pub(super) fn should_attempt_attach(connected_slot_count: usize) -> bool {
    connected_slot_count < JoyConSide::ALL.len()
}

pub(super) fn manager_scan_interval_for_connected_slots(connected_slot_count: usize) -> Duration {
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

fn snapshot_from_report(
    controller: &mut ActiveJoyConController,
    report: &JoyConReport,
) -> JoyConControllerStateSnapshot {
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
    let mut first_error = None;
    for controller in controllers.iter_mut().flatten() {
        let mode = &mut controller.mode;
        if let Err(error) = service_rumble_stop(&mut controller.rumble_stop_at, now, || {
            mode.driver_mut()
                .rumble((Some(Rumble::stop()), Some(Rumble::stop())))
                .map(|_| ())
                .map_err(joycon_error)
        }) {
            first_error.get_or_insert(error);
        }
    }
    first_error.map_or(Ok(()), Err)
}
pub(super) fn service_rumble_stop(
    deadline: &mut Option<Instant>,
    now: Instant,
    stop: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    if deadline.is_none_or(|at| at > now) {
        return Ok(());
    }
    match stop() {
        Ok(()) => {
            *deadline = None;
            Ok(())
        }
        Err(error) => {
            *deadline = Some(now + JOYCON_ATTACH_RETRY_INTERVAL);
            Err(error)
        }
    }
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
    let centered = if value.abs() <= JOYCON_STICK_NOISE_FLOOR {
        0.0
    } else {
        value
    };
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

pub(super) fn low_pass_vec2(previous: Option<(f64, f64)>, next: (f64, f64), alpha: f64) -> (f64, f64) {
    match previous {
        Some(previous) => (
            low_pass_scalar(previous.0, next.0, alpha),
            low_pass_scalar(previous.1, next.1, alpha),
        ),
        None => next,
    }
}

pub(super) fn low_pass_vec3(previous: Option<(f64, f64, f64)>, next: (f64, f64, f64), alpha: f64) -> (f64, f64, f64) {
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

pub(super) fn report_is_stale(last_report_at: Option<Instant>, now: Instant, stale_timeout: Duration) -> bool {
    last_report_at.is_some_and(|last_report_at| now.saturating_duration_since(last_report_at) >= stale_timeout)
}

fn detach_if_inactive(
    controllers: &mut [Option<ActiveJoyConController>; 2],
    state: &mut JoyConRuntimeState,
    side: JoyConSide,
    now: Instant,
    allow_disconnect_signal_without_connection_check: bool,
    error: Option<String>,
    events: &mut Vec<JoyConWorkerEvent>,
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
        events.push(JoyConWorkerEvent::Error(format!(
            "{} disconnected: {error}",
            side.label()
        )));
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
