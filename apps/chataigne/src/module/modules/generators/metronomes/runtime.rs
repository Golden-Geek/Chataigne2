use crate::app::module::common::generators::{self, AdmissionError, EventSink, EVENT_ROWS};
use golden_core::node::NodeId;
use golden_io::{BoundedWorker, WorkerContext, WorkerNotifier, WorkerReceive};
use std::{
    collections::{HashMap, HashSet},
    num::NonZeroUsize,
    time::{Duration, Instant},
};

use super::{
    metronome_gap_seconds, synchronize_metronome_state, MetronomeConfig, MetronomeRuntimeState, MAX_TICKS_PER_UPDATE,
};

#[derive(Clone, Debug)]
pub(super) struct MetronomeWorkerConfig {
    pub(super) update_rate_hz: u32,
    pub(super) metronomes: Vec<MetronomeConfig>,
}

impl MetronomeWorkerConfig {
    fn weight(&self) -> Result<usize, AdmissionError> {
        generators::check_items(self.metronomes.len())?;
        let mut weight = self
            .metronomes
            .capacity()
            .saturating_mul(std::mem::size_of::<MetronomeConfig>());
        for item in &self.metronomes {
            generators::check_text(&item.label)?;
            generators::check_text(&item.value_decl_id)?;
            weight = weight
                .saturating_add(item.label.capacity())
                .saturating_add(item.value_decl_id.capacity());
        }
        if weight > generators::MAX_CONFIG_WEIGHT {
            return Err(AdmissionError::Invalid(
                "generator configuration exceeds 32 MiB retained-weight limit",
            ));
        }
        Ok(weight)
    }
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

#[derive(Debug)]
enum MetronomeWorkerCommand {
    Configure(MetronomeWorkerConfig),
    Reset(Vec<NodeId>),
    ManualTick(NodeId),
}

pub(super) struct MetronomeRuntimeHandle {
    worker: BoundedWorker<MetronomeWorkerCommand, MetronomeWorkerEvent>,
}

impl MetronomeRuntimeHandle {
    pub(super) fn spawn(config: MetronomeWorkerConfig, notifier: WorkerNotifier) -> Result<Self, String> {
        config.weight().map_err(|error| error.to_string())?;
        let worker = BoundedWorker::spawn_notified(
            "metronomes-generator-runtime",
            generators::lifetimes(),
            generators::limits(),
            notifier,
            move |context| worker_loop(context, WorkerState::new(config, Instant::now())),
        )
        .map_err(|error| error.to_string())?;
        Ok(Self { worker })
    }

    pub(super) fn configure(&self, config: MetronomeWorkerConfig) -> Result<(), AdmissionError> {
        let weight = config.weight()?;
        self.worker
            .send_weighted(MetronomeWorkerCommand::Configure(config), weight)
            .map_err(generators::admission_error)
    }

    pub(super) fn reset(&self, item_ids: Vec<NodeId>) -> Result<(), String> {
        generators::check_items(item_ids.len()).map_err(|error| error.to_string())?;
        let weight = item_ids.capacity().saturating_mul(std::mem::size_of::<NodeId>());
        self.worker
            .send_weighted(MetronomeWorkerCommand::Reset(item_ids), weight)
            .map_err(generators::admission_error)
            .map_err(|error| error.to_string())
    }

    pub(super) fn manual_tick(&self, item_id: NodeId) -> Result<(), String> {
        self.worker
            .send_weighted(
                MetronomeWorkerCommand::ManualTick(item_id),
                std::mem::size_of::<NodeId>(),
            )
            .map_err(generators::admission_error)
            .map_err(|error| error.to_string())
    }

    pub(super) fn take_event(&self) -> Option<MetronomeWorkerEvent> {
        let mut events = Vec::with_capacity(1);
        self.worker.drain_events(&mut events, NonZeroUsize::MIN);
        events.pop()
    }

    pub(super) fn has_pending(&self) -> bool {
        self.worker.has_pending()
    }

    pub(super) fn is_closed(&self) -> bool {
        !self.worker.is_accepting_commands()
    }

    pub(super) fn stop(&mut self) {
        self.worker.cancel();
    }
}

struct WorkerState {
    config: MetronomeWorkerConfig,
    states: HashMap<NodeId, MetronomeRuntimeState>,
    last_update: Instant,
    next_update: Instant,
    pending_command: Option<MetronomeWorkerCommand>,
}

impl WorkerState {
    fn new(config: MetronomeWorkerConfig, now: Instant) -> Self {
        let next_update = now + update_interval(config.update_rate_hz);
        Self {
            config,
            states: HashMap::new(),
            last_update: now,
            next_update,
            pending_command: None,
        }
    }

    fn service(
        &mut self,
        now: Instant,
        command: Option<MetronomeWorkerCommand>,
        event_slot: &impl EventSink<MetronomeWorkerEvent>,
    ) -> bool {
        // Settle time under the old configuration before applying a command.
        // Ordinary commands cannot discard elapsed time or postpone the deadline.
        let delta_seconds = if self.pending_command.is_some() {
            0.0
        } else {
            let delta = now.saturating_duration_since(self.last_update).as_secs_f64();
            self.last_update = now;
            delta
        };
        if !event_slot.publish(compute_metronome_update(&self.config, &mut self.states, delta_seconds)) {
            return true;
        }
        let command = self.pending_command.take().or(command);
        if command.is_some()
            && self.config.metronomes.iter().filter(|item| item.enabled).any(|item| {
                self.states
                    .get(&item.item_id)
                    .is_some_and(|state| state.elapsed_seconds + f64::EPSILON >= state.next_gap_seconds)
            })
        {
            // Preserve the old tempo's due prefix before applying a new intent.
            // The next turn continues with zero additional elapsed time.
            self.pending_command = command;
            return false;
        }
        let old_rate = self.config.update_rate_hz;
        if command.is_some_and(|command| handle_command(command, &mut self.config, &mut self.states, event_slot)) {
            return true;
        }
        if now >= self.next_update || old_rate != self.config.update_rate_hz {
            self.next_update = now + update_interval(self.config.update_rate_hz);
        }
        false
    }
}

fn worker_loop(context: WorkerContext<MetronomeWorkerCommand, MetronomeWorkerEvent>, mut state: WorkerState) {
    let publish = |event| publish_event(&context, event);
    while !context.cancellation().is_cancelled() {
        let command = if state.pending_command.is_some() {
            None
        } else {
            match context.receive_until(state.next_update) {
                WorkerReceive::Command(command) => Some(command),
                WorkerReceive::Deadline => None,
                WorkerReceive::Disconnected => return,
            }
        };
        if state.service(Instant::now(), command, &publish) {
            return;
        }
    }
}

// A slow owner backpressures the worker instead of growing a cross-generation
// event map. Each actor turn receives at most EVENT_ROWS callbacks. Cancellation
// interrupts capacity waits and preserves nonblocking owner destruction.
fn publish_event(
    context: &WorkerContext<MetronomeWorkerCommand, MetronomeWorkerEvent>,
    event: MetronomeWorkerEvent,
) -> bool {
    let mut rows = event.ticks.into_iter();
    loop {
        let batch: HashMap<_, _> = rows.by_ref().take(EVENT_ROWS).collect();
        if batch.is_empty() {
            return true;
        }
        let weight = batch
            .values()
            .map(|row| std::mem::size_of_val(row) + row.label.capacity() + 64)
            .sum();
        if context
            .publish_weighted(MetronomeWorkerEvent { ticks: batch }, weight)
            .is_err()
        {
            return false;
        }
    }
}

fn handle_command(
    command: MetronomeWorkerCommand,
    config: &mut MetronomeWorkerConfig,
    states: &mut HashMap<NodeId, MetronomeRuntimeState>,
    event_slot: &impl EventSink<MetronomeWorkerEvent>,
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
            let active: HashSet<_> = config.metronomes.iter().map(|item| item.item_id).collect();
            for item_id in item_ids {
                if active.contains(&item_id) {
                    states.insert(item_id, MetronomeRuntimeState::default());
                }
            }
            false
        }
        MetronomeWorkerCommand::ManualTick(item_id) => {
            if let Some(event) = manual_tick(config, states, item_id) {
                return !event_slot.publish(event);
            }
            false
        }
    }
}

pub(super) fn compute_metronome_update(
    config: &MetronomeWorkerConfig,
    states: &mut HashMap<NodeId, MetronomeRuntimeState>,
    delta_seconds: f64,
) -> MetronomeWorkerEvent {
    let mut event = MetronomeWorkerEvent::default();

    for metronome in &config.metronomes {
        if !metronome.enabled {
            continue;
        }
        let state = states.entry(metronome.item_id).or_default();
        synchronize_metronome_state(state, metronome);
        state.elapsed_seconds += delta_seconds;

        let mut fired = 0u32;
        let mut last_gap_seconds = state.next_gap_seconds;
        while state.elapsed_seconds + f64::EPSILON >= state.next_gap_seconds && fired < MAX_TICKS_PER_UPDATE {
            state.elapsed_seconds -= state.next_gap_seconds;
            state.tick_count = state.tick_count.saturating_add(1);
            fired += 1;
            last_gap_seconds = state.next_gap_seconds;
            state.next_gap_seconds = metronome_gap_seconds(metronome, state.tick_count);
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
    synchronize_metronome_state(state, metronome);
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

fn update_interval(update_rate_hz: u32) -> Duration {
    Duration::from_secs_f64(1.0 / update_rate_hz.max(1) as f64)
}

#[cfg(test)]
mod tests;
