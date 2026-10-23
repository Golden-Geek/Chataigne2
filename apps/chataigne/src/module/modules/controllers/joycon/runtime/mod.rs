use crate::app::module::common::joycon::{
    JoyConControllerTarget, JoyConSetLedRequest, JoyConVibrateRequest, JOYCON_LED_STATE_FLASH, JOYCON_LED_STATE_OFF,
    JOYCON_LED_STATE_ON,
};
use golden_io::{
    BoundedWorker, PayloadPool, PendingDrainState, RetainedPayload, RetirementPool, RevisionGuard, RevisionReader,
    RevisionSource, WorkerContext, WorkerLimits, WorkerNotifier, WorkerReceive,
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

mod input;
mod native;
mod publication;
mod receipts;
pub(super) use publication::Settings;
#[cfg(test)]
mod tests;
pub(super) use input::{Input, Source};
#[cfg(test)]
use native::{
    low_pass_vec2, low_pass_vec3, manager_scan_interval_for_connected_slots, report_is_stale, should_attempt_attach,
    JOYCON_MANAGER_DISCOVERY_SCAN_INTERVAL, JOYCON_MANAGER_FULL_SCAN_INTERVAL, JOYCON_MANAGER_PARTIAL_SCAN_INTERVAL,
    JOYCON_REPORT_STALE_DISCONNECT_TIMEOUT,
};
use receipts::AdmittedCommand;
pub(super) use receipts::{Outcome, Receipt};

#[cfg(test)]
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
    pub(super) rows: Option<[Arc<publication::Side>; 2]>,
    pub(super) settings: Option<Settings>,
}
impl Publication {
    pub(super) fn state(&self) -> Option<&JoyConRuntimeState> {
        if let JoyConWorkerEvent::State(state) = &self.event {
            Some(state)
        } else {
            None
        }
    }
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
                if incoming { 128 * EVENT_WEIGHT } else { 32 * 1024 * 1024 },
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
#[derive(Clone, Copy)]
struct ProjectionControl {
    settings: Settings,
    version: u64,
}
struct Shared {
    running: AtomicBool,
    pressure: AtomicBool,
    notifier: WorkerNotifier,
    projection: Mutex<ProjectionControl>,
    projection_revision: RevisionReader,
    native: RevisionReader,
    session: RevisionReader,
}
struct Stopped {
    shared: Arc<Shared>,
    native: Option<RevisionSource>,
    session: Option<RevisionSource>,
}
impl Drop for Stopped {
    fn drop(&mut self) {
        self.native.take();
        self.session.take();
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
    projection: Option<RevisionSource>,
    #[cfg(test)]
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
        Self::spawn_prepared(notifier, slots, commands, events, None, open)
    }
    fn spawn_prepared<P: Provider>(
        notifier: WorkerNotifier,
        slots: &RetirementPool,
        commands: PayloadPool,
        events: PayloadPool,
        callbacks: Option<crate::app::module::script_api::NativeCallbackPreparer>,
        open: impl FnOnce() -> Result<P, String> + Send + 'static,
    ) -> Result<Self, String> {
        let projection = RevisionSource::default();
        let actor = RevisionSource::default();
        let actor_guard = actor.reader().guard_at(0);
        let native = RevisionSource::default();
        let native_guard = native.reader().guard_at(0);
        let session = RevisionSource::default();
        let shared = Arc::new(Shared {
            running: AtomicBool::new(true),
            pressure: AtomicBool::new(false),
            notifier: notifier.clone(),
            projection: Mutex::new(ProjectionControl {
                settings: Settings::default(),
                version: 0,
            }),
            projection_revision: projection.reader(),
            native: native.reader(),
            session: session.reader(),
        });
        let state = shared.clone();
        let worker = BoundedWorker::spawn_notified(
            "joycon-runtime",
            slots,
            WorkerLimits {
                command_items: 64,
                command_weight: 4 * 1024 * 1024,
                event_items: 32,
                event_weight: 32 * EVENT_WEIGHT,
            },
            notifier,
            move |ctx| {
                let stopped = Stopped {
                    shared: state,
                    native: Some(native),
                    session: Some(session),
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
                    callbacks,
                );
            },
        )
        .map_err(|error| error.to_string())?;
        Ok(Self {
            worker: Some(worker),
            actor: Some(actor),
            shared,
            commands,
            projection: Some(projection),
            #[cfg(test)]
            admission: RetirementPool::new(NonZeroUsize::new(64).unwrap()),
        })
    }
    #[cfg(test)]
    pub(super) fn send(&self, command: JoyConWorkerCommand) -> Result<Receipt, String> {
        validate_command(&command)?;
        let permit = self
            .admission
            .try_reserve()
            .map_err(|_| "Joy-Con has 64 retained command lifetimes")?;
        let source = Source {
            input: Input::Prepared(command),
            time: golden_core::engine::EngineTime {
                tick: 0,
                micro: 0,
                seq: 0,
            },
            origin: input::Origin::Fixture,
            guards: self.source_guards()?,
            _admission: permit,
            ingress: input::Ingress::ready(),
        };
        let weight = source.weight();
        let source = self
            .commands
            .try_retain(source, weight)
            .map_err(|_| "Joy-Con command lifetime capacity is full")?;
        self.submit(source)
            .map_err(|_| "Joy-Con command mailbox is full or disconnected".into())
    }
    fn source_guards(&self) -> Result<[RevisionGuard; 3], String> {
        Ok([
            self.actor
                .as_ref()
                .ok_or("Joy-Con runtime is stopped")?
                .reader()
                .guard_at(0),
            self.shared.native.guard_at(0),
            self.shared.session.guard_at(self.shared.session.current()),
        ])
    }
    pub(super) fn capture(
        &self,
        input: Input,
        producer: golden_core::engine::NodeWaker,
        time: golden_core::engine::EngineTime,
        permit: golden_io::RetirementPermit,
        ingress: golden_io::RetirementPermit,
    ) -> Result<RetainedPayload<Source>, String> {
        if !self.is_running() {
            return Err("Joy-Con worker is stopped; request was not admitted".into());
        }
        let source = Source {
            input,
            time,
            origin: input::Origin::Engine(producer),
            guards: self.source_guards()?,
            _admission: permit,
            ingress: input::Ingress::new(ingress, self.shared.notifier.clone()),
        };
        let weight = source.weight();
        if weight > 4 * 1024 * 1024 {
            return Err("Joy-Con source exceeds 4 MiB mailbox capacity".into());
        }
        self.commands
            .try_retain(source, weight)
            .map_err(|_| "Joy-Con source lifetime capacity is full".into())
    }
    pub(super) fn submit(&self, source: RetainedPayload<Source>) -> Result<Receipt, RetainedPayload<Source>> {
        if source.pending_ingress() {
            return Err(source);
        }
        let Some(worker) = &self.worker else {
            return Err(source);
        };
        let weight = source.weight();
        let (command, receipt) = AdmittedCommand::new(source.clone(), self.shared.notifier.clone());
        worker.send_weighted(command, weight).map_err(|error| {
            let (golden_io::PendingSendError::Full(mut command)
            | golden_io::PendingSendError::Disconnected(mut command)) = error;
            command.armed = false;
            source
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
    pub(super) fn configure(&self, settings: Settings) -> Result<(), String> {
        if !settings.dead_zone.is_finite() || !(0.0..=1.0).contains(&settings.dead_zone) {
            return Err("Joy-Con dead zone must be finite within 0..1".into());
        }
        let source = self.projection.as_ref().ok_or("Joy-Con runtime is stopped")?;
        let mut control = self.shared.projection.lock().unwrap_or_else(|error| error.into_inner());
        if control.settings != settings {
            control.settings = settings;
            control.version = source.advance();
        }
        Ok(())
    }
    pub(crate) fn stop(&mut self) {
        self.actor.take();
        self.projection.take();
        self.worker.take();
    }
}
impl Drop for JoyConRuntimeHandle {
    fn drop(&mut self) {
        self.stop();
    }
}
fn run<P: Provider>(
    mut owned: Owned<P>,
    events: PayloadPool,
    guards: [RevisionGuard; 2],
    mut callbacks: Option<crate::app::module::script_api::NativeCallbackPreparer>,
) {
    let ctx = &owned.context;
    let mut waiting = None;
    let mut pending = std::collections::VecDeque::<(JoyConWorkerEvent, RevisionGuard)>::with_capacity(8);
    let mut drafting: Option<publication::Pending> = None;
    let mut prepared: Option<RetainedPayload<Publication>> = None;
    let mut cache = [None, None];
    let mut latest: Option<(Box<JoyConRuntimeState>, RevisionGuard)> = None;
    let mut version = 0;
    let mut connected = [false; 2];
    while !ctx.cancellation().is_cancelled() {
        owned.provider.maintain();
        let control = *owned
            ._stopped
            .shared
            .projection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if control.version != version {
            version = control.version;
            if drafting.as_ref().is_some_and(publication::Pending::is_state) {
                drafting = None;
            }
            if prepared.as_ref().is_some_and(|value| value.settings.is_some()) {
                prepared = None;
            }
            pending.retain(|(event, _)| !matches!(event, JoyConWorkerEvent::State(_)));
            cache = [None, None];
            if let Some((state, session)) = latest.as_ref().filter(|(_, session)| session.is_current()) {
                pending.push_back((JoyConWorkerEvent::State(state.clone()), session.clone()));
            }
        }
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
            let request = match command.prepare().map_err(bounded_text) {
                Ok(request) => request,
                Err(error) => {
                    command.finish(Outcome::Completed(Err(error)));
                    continue;
                }
            };
            if !command.current() || ctx.cancellation().is_cancelled() {
                command.finish(Outcome::NotSent);
                continue;
            }
            command.started();
            command.finish(Outcome::Completed(
                owned.provider.execute(&request).map(bounded_text).map_err(bounded_text),
            ));
        }
        if drain.state == PendingDrainState::Disconnected || ctx.cancellation().is_cancelled() {
            break;
        }
        if pending.is_empty() && prepared.is_none() && drafting.is_none() {
            let input = owned.provider.poll();
            if input.len() > 8
                || input
                    .iter()
                    .filter(|event| matches!(event, JoyConWorkerEvent::State(_)))
                    .count()
                    > 1
            {
                publish(
                    ctx,
                    &events,
                    &guards,
                    JoyConWorkerEvent::Error("Joy-Con provider turn exceeds eight records / one state".into()),
                );
                break;
            }
            for event in &input {
                if let JoyConWorkerEvent::State(state) = event {
                    let next = [state.left.connected, state.right.connected];
                    if next != connected {
                        owned._stopped.session.as_ref().unwrap().advance();
                        connected = next;
                    }
                }
            }
            let session = owned
                ._stopped
                .shared
                .session
                .guard_at(owned._stopped.shared.session.current());
            for event in input {
                let event = publication::normalize(event);
                if let JoyConWorkerEvent::State(state) = &event {
                    latest = Some((state.clone(), session.clone()));
                }
                pending.push_back((event, session.clone()));
            }
        }
        let mut blocked = false;
        if prepared.is_none() {
            if drafting.is_none() {
                if let Some((event, session)) = pending.pop_front() {
                    let mut scope = guards.to_vec();
                    scope.push(session.clone());
                    match publication::Pending::begin(
                        event,
                        &scope,
                        &control,
                        &events,
                        &mut cache,
                        &owned._stopped.shared.projection_revision,
                    ) {
                        Ok(value) => drafting = Some(value),
                        Err(event) => {
                            pending.push_front((event, session));
                            blocked = true;
                        }
                    }
                }
            }
            if let Some(draft) = &drafting {
                if draft.is_state() && callbacks.is_none() {
                    callbacks = crate::app::module::script_api::NativeCallbackPreparer::new().ok();
                }
                if let Some(rows) = draft.try_finish(callbacks.as_ref()) {
                    if let Some(rows) = &rows {
                        cache = [Some(rows[0].clone()), Some(rows[1].clone())];
                    }
                    prepared = Some(drafting.take().unwrap().finish(rows));
                }
            }
        }
        if let Some(event) = prepared.take() {
            match ctx.try_publish_weighted(event, EVENT_WEIGHT) {
                Ok(()) => {}
                Err(golden_io::PendingSendError::Full(event)) => prepared = Some(event),
                Err(golden_io::PendingSendError::Disconnected(_)) => return,
            }
        }
        let pressure = blocked || prepared.is_some() || drafting.is_some();
        if owned._stopped.shared.pressure.swap(pressure, Ordering::AcqRel) != pressure {
            owned._stopped.shared.notifier.notify();
        }
        let idle = if pressure {
            Duration::from_millis(10)
        } else if !pending.is_empty() {
            Duration::ZERO
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
    let event = publication::normalize(event);
    pool.try_retain(
        Publication {
            event,
            guards: guards.to_vec(),
            rows: None,
            settings: None,
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
