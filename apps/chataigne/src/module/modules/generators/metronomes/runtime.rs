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

use super::{
    metronome_gap_seconds, reset_state_if_config_changed, MetronomeConfig, MetronomeRuntimeState,
    MAX_TICKS_PER_UPDATE,
};

#[derive(Clone, Debug)]
pub(super) struct MetronomeWorkerConfig {
    pub(super) update_rate_hz: u32,
    pub(super) metronomes: Vec<MetronomeConfig>,
}

#[derive(Clone, Debug)]
pub(super) struct MetronomeWorkerTick {
    pub(super) item_id: NodeId,
    pub(super) label: String,
    pub(super) fired: u32,
    pub(super) total_ticks: u64,
    pub(super) interval_seconds: f64,
    pub(super) last_gap_seconds: f64,
}

#[derive(Clone, Debug, Default)]
pub(super) struct MetronomeWorkerEvent {
    pub(super) ticks: HashMap<NodeId, MetronomeWorkerTick>,
}

enum MetronomeWorkerCommand {
    Configure(MetronomeWorkerConfig),
    Reset(Vec<NodeId>),
    ManualTick(NodeId),
    Stop,
}

#[derive(Clone)]
struct MetronomeEventSlot {
    event: Arc<Mutex<Option<MetronomeWorkerEvent>>>,
    pending: Arc<AtomicBool>,
}

impl MetronomeEventSlot {
    fn new() -> Self {
        Self {
            event: Arc::new(Mutex::new(None)),
            pending: Arc::new(AtomicBool::new(false)),
        }
    }

    fn publish(&self, event: MetronomeWorkerEvent) {
        if event.ticks.is_empty() {
            return;
        }
        let Ok(mut guard) = self.event.lock() else {
            return;
        };
        if let Some(existing) = guard.as_mut() {
            merge_metronome_event(existing, event);
        } else {
            *guard = Some(event);
        }
        self.pending.store(true, Ordering::Release);
    }

    fn take(&self) -> Option<MetronomeWorkerEvent> {
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

pub(super) struct MetronomeRuntimeHandle {
    command_tx: Sender<MetronomeWorkerCommand>,
    event_slot: MetronomeEventSlot,
    worker: Option<JoinHandle<()>>,
}

impl MetronomeRuntimeHandle {
    pub(super) fn spawn(config: MetronomeWorkerConfig) -> Result<Self, String> {
        let (command_tx, command_rx) = mpsc::channel();
        let event_slot = MetronomeEventSlot::new();
        let worker_event_slot = event_slot.clone();
        let worker = thread::Builder::new()
            .name("metronomes-generator-runtime".to_string())
            .spawn(move || worker_loop(command_rx, worker_event_slot, config))
            .map_err(|error| format!("failed to start Metronomes worker thread: {error}"))?;

        Ok(Self {
            command_tx,
            event_slot,
            worker: Some(worker),
        })
    }

    pub(super) fn configure(&self, config: MetronomeWorkerConfig) -> Result<(), String> {
        self.command_tx
            .send(MetronomeWorkerCommand::Configure(config))
            .map_err(|_| "Metronomes worker is no longer running".to_string())
    }

    pub(super) fn reset(&self, item_ids: Vec<NodeId>) -> Result<(), String> {
        self.command_tx
            .send(MetronomeWorkerCommand::Reset(item_ids))
            .map_err(|_| "Metronomes worker is no longer running".to_string())
    }

    pub(super) fn manual_tick(&self, item_id: NodeId) -> Result<(), String> {
        self.command_tx
            .send(MetronomeWorkerCommand::ManualTick(item_id))
            .map_err(|_| "Metronomes worker is no longer running".to_string())
    }

    pub(super) fn take_event(&self) -> Option<MetronomeWorkerEvent> {
        self.event_slot.take()
    }

    pub(super) fn has_pending(&self) -> bool {
        self.event_slot.has_pending()
    }

    pub(super) fn stop(&mut self) {
        let _ = self.command_tx.send(MetronomeWorkerCommand::Stop);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Drop for MetronomeRuntimeHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

fn worker_loop(
    command_rx: Receiver<MetronomeWorkerCommand>,
    event_slot: MetronomeEventSlot,
    mut config: MetronomeWorkerConfig,
) {
    let mut states = HashMap::<NodeId, MetronomeRuntimeState>::new();
    let mut last_update = Instant::now();

    loop {
        let interval = update_interval(config.update_rate_hz);
        match command_rx.recv_timeout(interval) {
            Ok(command) => {
                if handle_command(command, &mut config, &mut states, &event_slot) {
                    break;
                }
                last_update = Instant::now();
                continue;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }

        while let Ok(command) = command_rx.try_recv() {
            if handle_command(command, &mut config, &mut states, &event_slot) {
                return;
            }
            last_update = Instant::now();
        }

        let now = Instant::now();
        let delta_seconds = now.duration_since(last_update).as_secs_f64().max(0.0);
        last_update = now;

        let event = compute_metronome_update(&config, &mut states, delta_seconds);
        event_slot.publish(event);
    }
}

fn handle_command(
    command: MetronomeWorkerCommand,
    config: &mut MetronomeWorkerConfig,
    states: &mut HashMap<NodeId, MetronomeRuntimeState>,
    event_slot: &MetronomeEventSlot,
) -> bool {
    match command {
        MetronomeWorkerCommand::Configure(next_config) => {
            let active_ids = next_config
                .metronomes
                .iter()
                .map(|metronome| metronome.item_id)
                .collect::<HashSet<_>>();
            states.retain(|item_id, _| active_ids.contains(item_id));
            *config = next_config;
            false
        }
        MetronomeWorkerCommand::Reset(item_ids) => {
            for item_id in item_ids {
                states.insert(item_id, MetronomeRuntimeState::default());
            }
            false
        }
        MetronomeWorkerCommand::ManualTick(item_id) => {
            if let Some(event) = manual_tick(config, states, item_id) {
                event_slot.publish(event);
            }
            false
        }
        MetronomeWorkerCommand::Stop => true,
    }
}

pub(super) fn compute_metronome_update(
    config: &MetronomeWorkerConfig,
    states: &mut HashMap<NodeId, MetronomeRuntimeState>,
    delta_seconds: f64,
) -> MetronomeWorkerEvent {
    let mut event = MetronomeWorkerEvent::default();
    if delta_seconds == 0.0 {
        return event;
    }

    for metronome in &config.metronomes {
        if !metronome.enabled {
            continue;
        }
        let state = states.entry(metronome.item_id).or_default();
        reset_state_if_config_changed(state, metronome);
        state.elapsed_seconds += delta_seconds;

        let mut fired = 0u32;
        let mut last_gap_seconds = state.next_gap_seconds;
        while state.elapsed_seconds + f64::EPSILON >= state.next_gap_seconds
            && fired < MAX_TICKS_PER_UPDATE
        {
            state.elapsed_seconds -= state.next_gap_seconds;
            state.tick_count = state.tick_count.saturating_add(1);
            fired += 1;
            last_gap_seconds = state.next_gap_seconds;
            state.next_gap_seconds = metronome_gap_seconds(metronome, state.tick_count);
        }
        if fired == MAX_TICKS_PER_UPDATE
            && state.elapsed_seconds + f64::EPSILON >= state.next_gap_seconds
        {
            state.elapsed_seconds = state.elapsed_seconds.min(state.next_gap_seconds);
        }
        if fired == 0 {
            continue;
        }

        event.ticks.insert(
            metronome.item_id,
            MetronomeWorkerTick {
                item_id: metronome.item_id,
                label: metronome.label.clone(),
                fired,
                total_ticks: state.tick_count,
                interval_seconds: metronome.interval_seconds,
                last_gap_seconds,
            },
        );
    }
    event
}

fn manual_tick(
    config: &MetronomeWorkerConfig,
    states: &mut HashMap<NodeId, MetronomeRuntimeState>,
    item_id: NodeId,
) -> Option<MetronomeWorkerEvent> {
    let metronome = config
        .metronomes
        .iter()
        .find(|metronome| metronome.item_id == item_id)?;
    let state = states.entry(metronome.item_id).or_default();
    reset_state_if_config_changed(state, metronome);
    state.tick_count = state.tick_count.saturating_add(1);

    let mut event = MetronomeWorkerEvent::default();
    event.ticks.insert(
        metronome.item_id,
        MetronomeWorkerTick {
            item_id: metronome.item_id,
            label: metronome.label.clone(),
            fired: 1,
            total_ticks: state.tick_count,
            interval_seconds: metronome.interval_seconds,
            last_gap_seconds: state.next_gap_seconds,
        },
    );
    Some(event)
}

fn merge_metronome_event(existing: &mut MetronomeWorkerEvent, next: MetronomeWorkerEvent) {
    for (item_id, next_tick) in next.ticks {
        existing
            .ticks
            .entry(item_id)
            .and_modify(|tick| {
                tick.label = next_tick.label.clone();
                tick.fired = tick.fired.saturating_add(next_tick.fired);
                tick.total_ticks = next_tick.total_ticks;
                tick.interval_seconds = next_tick.interval_seconds;
                tick.last_gap_seconds = next_tick.last_gap_seconds;
            })
            .or_insert(next_tick);
    }
}

fn update_interval(update_rate_hz: u32) -> Duration {
    Duration::from_secs_f64(1.0 / update_rate_hz.max(1) as f64)
}
