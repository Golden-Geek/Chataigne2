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

use super::{reset_signal_state_if_config_changed, sample_signal, SignalConfig, SignalRuntimeState};

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

fn worker_loop(
    command_rx: Receiver<SignalWorkerCommand>,
    event_slot: SignalEventSlot,
    mut config: SignalWorkerConfig,
) {
    let mut states = HashMap::<NodeId, SignalRuntimeState>::new();
    let mut last_update = Instant::now();

    loop {
        let interval = update_interval(config.update_rate_hz);
        match command_rx.recv_timeout(interval) {
            Ok(command) => {
                if handle_command(command, &mut config, &mut states) {
                    break;
                }
                last_update = Instant::now();
                continue;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }

        while let Ok(command) = command_rx.try_recv() {
            if handle_command(command, &mut config, &mut states) {
                return;
            }
            last_update = Instant::now();
        }

        let now = Instant::now();
        let delta_seconds = now.duration_since(last_update).as_secs_f64().max(0.0);
        last_update = now;

        let event = compute_signal_update(&config, &mut states, delta_seconds);
        event_slot.publish(event);
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

fn compute_signal_update(
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
        reset_signal_state_if_config_changed(state, signal);
        state.elapsed_seconds += delta_seconds;

        let was_sampled = state.sampled_once;
        let last_cycle = state.last_cycle;
        let sample = sample_signal(signal, state);
        let cycles = if was_sampled && sample.cycle > last_cycle {
            sample.cycle.saturating_sub(last_cycle)
        } else {
            0
        };
        state.last_cycle = sample.cycle;
        state.sampled_once = true;

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
