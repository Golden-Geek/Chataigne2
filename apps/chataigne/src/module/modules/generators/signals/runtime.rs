use std::{
    collections::{HashMap, HashSet},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use golden_core::node::NodeId;

use super::{sample_signal, synchronize_signal_state, SignalConfig, SignalRuntimeState};

#[derive(Clone, Debug)]
pub(super) struct SignalWorkerConfig {
    pub(super) update_rate_hz: u32,
    pub(super) signals: Vec<SignalConfig>,
}

#[derive(Clone, Debug)]
pub(super) struct SignalWorkerSample {
    pub(super) item_id: NodeId,
    pub(super) label: String,
    pub(super) value: f64,
    pub(super) cycle: i64,
    pub(super) cycles: i64,
}

#[derive(Clone, Debug, Default)]
pub(super) struct SignalWorkerEvent {
    pub(super) samples: HashMap<NodeId, SignalWorkerSample>,
}

enum SignalWorkerCommand {
    Configure(SignalWorkerConfig),
    Reset(Vec<NodeId>),
    Stop,
}

#[derive(Clone)]
struct SignalEventSlot {
    event: Arc<Mutex<Option<SignalWorkerEvent>>>,
    pending: Arc<AtomicBool>,
}

impl SignalEventSlot {
    fn new() -> Self {
        Self {
            event: Arc::new(Mutex::new(None)),
            pending: Arc::new(AtomicBool::new(false)),
        }
    }

    fn publish(&self, event: SignalWorkerEvent) {
        if event.samples.is_empty() {
            return;
        }
        let Ok(mut guard) = self.event.lock() else {
            return;
        };
        if let Some(existing) = guard.as_mut() {
            merge_signal_event(existing, event);
        } else {
            *guard = Some(event);
        }
        self.pending.store(true, Ordering::Release);
    }

    fn take(&self) -> Option<SignalWorkerEvent> {
        let Ok(mut guard) = self.event.lock() else {
            return None;
        };
        let event = guard.take();
        self.pending.store(false, Ordering::Release);
        event
    }

    fn has_pending(&self) -> bool {
        self.pending.load(Ordering::Acquire)
    }
}

pub(super) struct SignalRuntimeHandle {
    command_tx: Sender<SignalWorkerCommand>,
    event_slot: SignalEventSlot,
    worker: Option<JoinHandle<()>>,
}

impl SignalRuntimeHandle {
    pub(super) fn spawn(config: SignalWorkerConfig) -> Result<Self, String> {
        let (command_tx, command_rx) = mpsc::channel();
        let event_slot = SignalEventSlot::new();
        let worker_event_slot = event_slot.clone();
        let worker = thread::Builder::new()
            .name("signals-generator-runtime".to_string())
            .spawn(move || worker_loop(command_rx, worker_event_slot, config))
            .map_err(|error| format!("failed to start Signals worker thread: {error}"))?;

        Ok(Self {
            command_tx,
            event_slot,
            worker: Some(worker),
        })
    }

    pub(super) fn configure(&self, config: SignalWorkerConfig) -> Result<(), String> {
        self.command_tx
            .send(SignalWorkerCommand::Configure(config))
            .map_err(|_| "Signals worker is no longer running".to_string())
    }

    pub(super) fn reset(&self, item_ids: Vec<NodeId>) -> Result<(), String> {
        self.command_tx
            .send(SignalWorkerCommand::Reset(item_ids))
            .map_err(|_| "Signals worker is no longer running".to_string())
    }

    pub(super) fn take_event(&self) -> Option<SignalWorkerEvent> {
        self.event_slot.take()
    }

    pub(super) fn has_pending(&self) -> bool {
        self.event_slot.has_pending()
    }

    pub(super) fn stop(&mut self) {
        let _ = self.command_tx.send(SignalWorkerCommand::Stop);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Drop for SignalRuntimeHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

struct WorkerState {
    config: SignalWorkerConfig,
    states: HashMap<NodeId, SignalRuntimeState>,
    last_update: Instant,
    next_update: Instant,
}

impl WorkerState {
    fn new(config: SignalWorkerConfig, now: Instant) -> Self {
        let next_update = now + update_interval(config.update_rate_hz);
        Self {
            config,
            states: HashMap::new(),
            last_update: now,
            next_update,
        }
    }

    fn service(&mut self, now: Instant, command: Option<SignalWorkerCommand>, event_slot: &SignalEventSlot) -> bool {
        // Settle time under the old configuration before applying a command.
        // Ordinary commands cannot discard elapsed time or postpone the deadline.
        let delta_seconds = now.saturating_duration_since(self.last_update).as_secs_f64();
        event_slot.publish(compute_signal_update(&self.config, &mut self.states, delta_seconds));
        self.last_update = now;
        let old_rate = self.config.update_rate_hz;
        if command.is_some_and(|command| handle_command(command, &mut self.config, &mut self.states)) {
            return true;
        }
        if now >= self.next_update || old_rate != self.config.update_rate_hz {
            self.next_update = now + update_interval(self.config.update_rate_hz);
        }
        false
    }
}

fn worker_loop(command_rx: Receiver<SignalWorkerCommand>, event_slot: SignalEventSlot, config: SignalWorkerConfig) {
    let mut state = WorkerState::new(config, Instant::now());
    loop {
        let wait = state.next_update.saturating_duration_since(Instant::now());
        let command = match command_rx.recv_timeout(wait) {
            Ok(command) => Some(command),
            Err(mpsc::RecvTimeoutError::Timeout) => None,
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
        };
        if state.service(Instant::now(), command, &event_slot) {
            return;
        }
    }
}

fn handle_command(
    command: SignalWorkerCommand,
    config: &mut SignalWorkerConfig,
    states: &mut HashMap<NodeId, SignalRuntimeState>,
) -> bool {
    match command {
        SignalWorkerCommand::Configure(next_config) => {
            let active_ids = next_config
                .signals
                .iter()
                .map(|signal| signal.item_id)
                .collect::<HashSet<_>>();
            states.retain(|item_id, _| active_ids.contains(item_id));
            *config = next_config;
            false
        }
        SignalWorkerCommand::Reset(item_ids) => {
            for item_id in item_ids {
                states.insert(item_id, SignalRuntimeState::default());
            }
            false
        }
        SignalWorkerCommand::Stop => true,
    }
}

pub(super) fn compute_signal_update(
    config: &SignalWorkerConfig,
    states: &mut HashMap<NodeId, SignalRuntimeState>,
    delta_seconds: f64,
) -> SignalWorkerEvent {
    let mut event = SignalWorkerEvent::default();
    for signal in &config.signals {
        if !signal.enabled {
            continue;
        }
        let state = states.entry(signal.item_id).or_default();
        synchronize_signal_state(state, signal);
        state.elapsed_seconds += delta_seconds;

        let last_cycle = state.last_cycle;
        let sample = sample_signal(signal, state);
        let cycles = if sample.cycle > last_cycle {
            sample.cycle.saturating_sub(last_cycle)
        } else {
            0
        };
        state.last_cycle = sample.cycle;

        event.samples.insert(
            signal.item_id,
            SignalWorkerSample {
                item_id: signal.item_id,
                label: signal.label.clone(),
                value: sample.value,
                cycle: sample.cycle,
                cycles,
            },
        );
    }
    event
}

fn merge_signal_event(existing: &mut SignalWorkerEvent, next: SignalWorkerEvent) {
    for (item_id, next_sample) in next.samples {
        existing
            .samples
            .entry(item_id)
            .and_modify(|sample| {
                sample.label = next_sample.label.clone();
                sample.value = next_sample.value;
                sample.cycle = next_sample.cycle;
                sample.cycles = sample.cycles.saturating_add(next_sample.cycles);
            })
            .or_insert(next_sample);
    }
}

fn update_interval(update_rate_hz: u32) -> Duration {
    Duration::from_secs_f64(1.0 / update_rate_hz.max(1) as f64)
}

#[cfg(test)]
mod tests;
