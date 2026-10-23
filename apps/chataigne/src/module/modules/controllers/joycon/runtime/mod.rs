use crate::app::module::common::joycon::{
    JoyConControllerTarget, JoyConSetLedRequest, JoyConVibrateRequest, JOYCON_LED_STATE_FLASH, JOYCON_LED_STATE_OFF,
    JOYCON_LED_STATE_ON,
};
use golden_io::{
    BoundedWorker, PayloadPool, PendingDrainState, RetainedPayload, RetirementPool, RevisionGuard, RevisionSource,
    WorkerContext, WorkerLimits, WorkerNotifier, WorkerReceive,
};
use joycon_rs::joycon::{device::JoyConDeviceType, Buttons};
use std::{
    num::NonZeroUsize,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, OnceLock,
    },
    time::{Duration, Instant},
};

mod native;
mod receipts;
#[cfg(test)]
mod tests;
#[cfg(test)]
use native::{
    low_pass_vec2, low_pass_vec3, manager_scan_interval_for_connected_slots, report_is_stale, should_attempt_attach,
    JOYCON_MANAGER_DISCOVERY_SCAN_INTERVAL, JOYCON_MANAGER_FULL_SCAN_INTERVAL, JOYCON_MANAGER_PARTIAL_SCAN_INTERVAL,
    JOYCON_REPORT_STALE_DISCONNECT_TIMEOUT,
};
use receipts::AdmittedCommand;
pub(super) use receipts::{Outcome, Receipt};

const COMMAND_WEIGHT: usize = 4096;
const EVENT_WEIGHT: usize = 8192;
const COMMAND_TURN: usize = 8;

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
    Error(String),
}
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum JoyConWorkerCommand {
    Vibrate(JoyConVibrateRequest),
    SetPlayerLights(JoyConSetLedRequest),
}
pub(super) struct Publication {
    pub(super) event: JoyConWorkerEvent,
    pub(super) guards: Vec<RevisionGuard>,
}

fn lifetimes() -> &'static RetirementPool {
    static POOL: OnceLock<RetirementPool> = OnceLock::new();
    POOL.get_or_init(|| RetirementPool::new(NonZeroUsize::new(4).unwrap()))
}
fn payloads(incoming: bool) -> Result<PayloadPool, String> {
    static INPUT: OnceLock<Mutex<Option<PayloadPool>>> = OnceLock::new();
    static OUTPUT: OnceLock<Mutex<Option<PayloadPool>>> = OnceLock::new();
    let mut slot = if incoming { &INPUT } else { &OUTPUT }
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    if slot.is_none() {
        *slot = Some(
            PayloadPool::new(
                if incoming { "joycon-input" } else { "joycon-output" },
                if incoming { 128 } else { 512 },
                if incoming {
                    128 * EVENT_WEIGHT
                } else {
                    512 * COMMAND_WEIGHT
                },
            )
            .map_err(|error| error.to_string())?,
        );
    }
    Ok(slot.as_ref().unwrap().clone())
}
pub(super) trait Provider: Send + 'static {
    fn execute(&mut self, command: &JoyConWorkerCommand) -> Result<String, String>;
    fn poll(&mut self) -> Vec<JoyConWorkerEvent>;
    fn maintain(&mut self) {}
    fn idle_interval(&self) -> Duration {
        Duration::from_millis(1)
    }
}
struct Exclusive<P> {
    provider: P,
    _permit: golden_io::RetirementPermit,
}
impl<P: Provider> Provider for Exclusive<P> {
    fn maintain(&mut self) {
        self.provider.maintain();
    }
    fn execute(&mut self, command: &JoyConWorkerCommand) -> Result<String, String> {
        self.provider.execute(command)
    }
    fn poll(&mut self) -> Vec<JoyConWorkerEvent> {
        self.provider.poll()
    }
    fn idle_interval(&self) -> Duration {
        self.provider.idle_interval()
    }
}
struct Shared {
    running: AtomicBool,
    pressure: AtomicBool,
    notifier: WorkerNotifier,
}
struct Stopped {
    shared: Arc<Shared>,
    native: Option<RevisionSource>,
}
impl Drop for Stopped {
    fn drop(&mut self) {
        self.native.take();
        self.shared.running.store(false, Ordering::Release);
        self.shared.notifier.notify();
    }
}
type Context = WorkerContext<AdmittedCommand, RetainedPayload<Publication>>;
struct Owned<P> {
    // Termination and queued-command disposition precede potentially blocked native cleanup.
    _stopped: Stopped,
    context: Context,
    provider: P,
}

pub(crate) struct JoyConRuntimeHandle {
    worker: Option<BoundedWorker<AdmittedCommand, RetainedPayload<Publication>>>,
    actor: Option<RevisionSource>,
    shared: Arc<Shared>,
    commands: PayloadPool,
    admission: RetirementPool,
}
impl JoyConRuntimeHandle {
    pub(super) fn spawn(notifier: WorkerNotifier, owner: &RetirementPool) -> Result<Self, String> {
        let permit = owner
            .try_reserve()
            .map_err(|_| "Previous Joy-Con runtime is still retiring")?;
        Self::spawn_using(notifier, lifetimes(), payloads(false)?, payloads(true)?, move || {
            native::Native::open().map(|provider| Exclusive {
                provider,
                _permit: permit,
            })
        })
    }
    fn spawn_using<P: Provider>(
        notifier: WorkerNotifier,
        slots: &RetirementPool,
        commands: PayloadPool,
        events: PayloadPool,
        open: impl FnOnce() -> Result<P, String> + Send + 'static,
    ) -> Result<Self, String> {
        let actor = RevisionSource::default();
        let actor_guard = actor.reader().guard_at(0);
        let native = RevisionSource::default();
        let native_guard = native.reader().guard_at(0);
        let shared = Arc::new(Shared {
            running: AtomicBool::new(true),
            pressure: AtomicBool::new(false),
            notifier: notifier.clone(),
        });
        let state = shared.clone();
        let worker = BoundedWorker::spawn_notified(
            "joycon-runtime",
            slots,
            WorkerLimits {
                command_items: 64,
                command_weight: 64 * COMMAND_WEIGHT,
                event_items: 32,
                event_weight: 32 * EVENT_WEIGHT,
            },
            notifier,
            move |ctx| {
                let stopped = Stopped {
                    shared: state,
                    native: Some(native),
                };
                let guards = [actor_guard, native_guard];
                let provider = match open() {
                    Ok(provider) => provider,
                    Err(error) => {
                        publish(
                            &ctx,
                            &events,
                            std::slice::from_ref(&guards[0]),
                            JoyConWorkerEvent::Error(error),
                        );
                        return;
                    }
                };
                run(
                    Owned {
                        _stopped: stopped,
                        context: ctx,
                        provider,
                    },
                    events,
                    guards,
                );
            },
        )
        .map_err(|error| error.to_string())?;
        Ok(Self {
            worker: Some(worker),
            actor: Some(actor),
            shared,
            commands,
            admission: RetirementPool::new(NonZeroUsize::new(64).unwrap()),
        })
    }
    pub(super) fn send(&self, command: JoyConWorkerCommand) -> Result<Receipt, String> {
        validate_command(&command)?;
        if !self.is_running() {
            return Err("Joy-Con worker is no longer running".into());
        }
        let worker = self.worker.as_ref().ok_or("Joy-Con runtime is stopped")?;
        let permit = self
            .admission
            .try_reserve()
            .map_err(|_| "Joy-Con has 64 retained command lifetimes")?;
        let source = self
            .commands
            .try_retain(
                receipts::Source {
                    command,
                    _admission: permit,
                },
                COMMAND_WEIGHT,
            )
            .map_err(|_| "Joy-Con command lifetime capacity is full")?;
        let actor = self
            .actor
            .as_ref()
            .ok_or("Joy-Con runtime is stopped")?
            .reader()
            .guard_at(0);
        let (command, receipt) = AdmittedCommand::new(source, actor, self.shared.notifier.clone());
        worker.send_weighted(command, COMMAND_WEIGHT).map_err(|error| {
            let (mut command, message) = match error {
                golden_io::PendingSendError::Full(command) => (command, "Joy-Con command mailbox is full"),
                golden_io::PendingSendError::Disconnected(command) => (command, "Joy-Con worker is no longer running"),
            };
            command.armed = false;
            message
        })?;
        Ok(receipt)
    }
    pub(super) fn try_recv(&self) -> Result<RetainedPayload<Publication>, std::sync::mpsc::TryRecvError> {
        let Some(worker) = &self.worker else {
            return Err(std::sync::mpsc::TryRecvError::Disconnected);
        };
        let mut events = Vec::with_capacity(1);
        let drain = worker.drain_events(&mut events, NonZeroUsize::MIN);
        events.pop().ok_or(if drain.state == PendingDrainState::Disconnected {
            std::sync::mpsc::TryRecvError::Disconnected
        } else {
            std::sync::mpsc::TryRecvError::Empty
        })
    }
    pub(super) fn is_running(&self) -> bool {
        self.shared.running.load(Ordering::Acquire)
    }
    pub(super) fn pressure(&self) -> bool {
        self.shared.pressure.load(Ordering::Acquire)
    }
    pub(super) fn has_pending(&self) -> bool {
        self.worker.as_ref().is_some_and(BoundedWorker::has_pending)
    }
    pub(crate) fn stop(&mut self) {
        self.actor.take();
        self.worker.take();
    }
}
impl Drop for JoyConRuntimeHandle {
    fn drop(&mut self) {
        self.stop();
    }
}
fn run<P: Provider>(mut owned: Owned<P>, events: PayloadPool, guards: [RevisionGuard; 2]) {
    let ctx = &owned.context;
    let mut waiting = None;
    let mut pending = std::collections::VecDeque::with_capacity(8);
    let mut prepared = None;
    while !ctx.cancellation().is_cancelled() {
        // Input pressure must not delay native rumble stop deadlines or admitted command outcomes.
        owned.provider.maintain();
        let mut commands = Vec::with_capacity(COMMAND_TURN);
        if let Some(command) = waiting.take() {
            commands.push(command);
        }
        let remaining = COMMAND_TURN - commands.len();
        let drain = ctx.drain_commands(&mut commands, NonZeroUsize::new(remaining).unwrap());
        for command in commands {
            if ctx.cancellation().is_cancelled() || !command.current() {
                command.finish(Outcome::NotSent);
                continue;
            }
            command.started();
            command.finish(Outcome::Completed(
                owned
                    .provider
                    .execute(command.command())
                    .map(bounded_text)
                    .map_err(bounded_text),
            ));
        }
        if drain.state == PendingDrainState::Disconnected || ctx.cancellation().is_cancelled() {
            break;
        }
        if pending.is_empty() && prepared.is_none() {
            let input = owned.provider.poll();
            if input.len() > 8 {
                publish(
                    ctx,
                    &events,
                    &guards,
                    JoyConWorkerEvent::Error("Joy-Con provider event turn exceeds eight records".into()),
                );
                break;
            }
            pending.extend(input);
        }
        loop {
            if prepared.is_none() {
                let Some(event) = pending.pop_front() else {
                    break;
                };
                match try_prepare(&events, &guards, event) {
                    Ok(event) => prepared = Some(event),
                    Err(event) => {
                        pending.push_front(event);
                        break;
                    }
                }
            }
            match ctx.try_publish_weighted(prepared.take().unwrap(), EVENT_WEIGHT) {
                Ok(()) => {}
                Err(golden_io::PendingSendError::Full(event)) => {
                    prepared = Some(event);
                    break;
                }
                Err(golden_io::PendingSendError::Disconnected(_)) => return,
            }
        }
        let pressure = !pending.is_empty() || prepared.is_some();
        if owned._stopped.shared.pressure.swap(pressure, Ordering::AcqRel) != pressure {
            owned._stopped.shared.notifier.notify();
        }
        let idle = if pressure {
            Duration::from_millis(10)
        } else {
            owned.provider.idle_interval()
        };
        if !idle.is_zero() {
            match ctx.receive_until(Instant::now() + idle) {
                WorkerReceive::Command(command) => waiting = Some(command),
                WorkerReceive::Deadline => {}
                WorkerReceive::Disconnected => break,
            }
        }
    }
}
fn try_prepare(
    pool: &PayloadPool,
    guards: &[RevisionGuard],
    event: JoyConWorkerEvent,
) -> Result<RetainedPayload<Publication>, JoyConWorkerEvent> {
    let event = match event {
        JoyConWorkerEvent::Error(message) => JoyConWorkerEvent::Error(bounded_text(message)),
        JoyConWorkerEvent::State(state)
            if [
                state.left.left_buttons.len(),
                state.left.right_buttons.len(),
                state.left.shared_buttons.len(),
                state.right.left_buttons.len(),
                state.right.right_buttons.len(),
                state.right.shared_buttons.len(),
            ]
            .iter()
            .any(|len| *len > 16) =>
        {
            JoyConWorkerEvent::Error("Joy-Con report exceeds the fixed button vocabulary".into())
        }
        other => other,
    };
    pool.try_retain(
        Publication {
            event,
            guards: guards.to_vec(),
        },
        EVENT_WEIGHT,
    )
    .map_err(|error| error.into_value().event)
}
fn publish(ctx: &Context, pool: &PayloadPool, guards: &[RevisionGuard], mut event: JoyConWorkerEvent) -> bool {
    loop {
        match try_prepare(pool, guards, event) {
            Ok(event) => return ctx.publish_weighted(event, EVENT_WEIGHT).is_ok(),
            Err(value) => event = value,
        }
        if ctx.cancellation().wait(Duration::from_millis(10)) {
            return false;
        }
    }
}
fn bounded_text(mut text: String) -> String {
    if text.len() > 1024 {
        let mut end = 1024;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
    }
    text
}
fn validate_command(command: &JoyConWorkerCommand) -> Result<(), String> {
    let fields: Vec<&str> = match command {
        JoyConWorkerCommand::Vibrate(request) => vec![&request.target, &request.description],
        JoyConWorkerCommand::SetPlayerLights(request) => vec![
            &request.target,
            &request.description,
            &request.led_1,
            &request.led_2,
            &request.led_3,
            &request.led_4,
        ],
    };
    if fields.iter().any(|text| text.len() > 256) {
        return Err("Joy-Con command text exceeds 256 retained bytes".into());
    }
    if let JoyConWorkerCommand::Vibrate(request) = command {
        if !request.frequency_hz.is_finite() || !request.amplitude.is_finite() {
            return Err("Joy-Con rumble requires finite frequency and amplitude".into());
        }
    }
    Ok(())
}
