use crate::app::module::common::generators::{self, AdmissionError, EventSink, EVENT_ROWS};
use golden_core::node::NodeId;
use golden_io::{BoundedWorker, WorkerContext, WorkerNotifier, WorkerReceive};
use std::{
    collections::{HashMap, HashSet},
    num::NonZeroUsize,
    time::{Duration, Instant},
};

use super::{sample_signal, synchronize_signal_state, SignalConfig, SignalRuntimeState};

#[derive(Clone, Debug)]
pub(super) struct SignalWorkerConfig {
    pub(super) update_rate_hz: u32,
    pub(super) signals: Vec<SignalConfig>,
}

impl SignalWorkerConfig {
    fn weight(&self) -> Result<usize, AdmissionError> {
        generators::check_items(self.signals.len())?;
        let mut weight = self
            .signals
            .capacity()
            .saturating_mul(std::mem::size_of::<SignalConfig>());
        let mut keys = 0usize;
        for item in &self.signals {
            generators::check_text(&item.label)?;
            generators::check_text(&item.value_decl_id)?;
            weight = weight
                .saturating_add(item.label.capacity())
                .saturating_add(item.value_decl_id.capacity());
            if let Some(curve) = &item.curve {
                keys = keys.saturating_add(curve.key_count());
                if keys > generators::MAX_CURVE_KEYS {
                    return Err(AdmissionError::Invalid("generator curve key limit is 16384"));
                }
                // Covers keys and the worker's fixed-size compiled segment cache.
                weight = weight.saturating_add(curve.key_count().saturating_mul(1024));
                for key in curve.keys() {
                    if let golden_core::node::CurveEasing::Script { source } = &key.easing {
                        weight = weight.saturating_add(source.capacity().saturating_mul(2));
                    }
                }
            }
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

#[derive(Debug)]
enum SignalWorkerCommand {
    Configure(SignalWorkerConfig),
    Reset(Vec<NodeId>),
}

pub(super) struct SignalRuntimeHandle {
    worker: BoundedWorker<SignalWorkerCommand, SignalWorkerEvent>,
}

impl SignalRuntimeHandle {
    pub(super) fn spawn(config: SignalWorkerConfig, notifier: WorkerNotifier) -> Result<Self, String> {
        config.weight().map_err(|error| error.to_string())?;
        let worker = BoundedWorker::spawn_notified(
            "signals-generator-runtime",
            generators::lifetimes(),
            generators::limits(),
            notifier,
            move |context| worker_loop(context, config),
        )
        .map_err(|error| error.to_string())?;
        Ok(Self { worker })
    }

    pub(super) fn configure(&self, config: SignalWorkerConfig) -> Result<(), AdmissionError> {
        let weight = config.weight()?;
        self.worker
            .send_weighted(SignalWorkerCommand::Configure(config), weight)
            .map_err(generators::admission_error)
    }

    pub(super) fn reset(&self, item_ids: Vec<NodeId>) -> Result<(), String> {
        generators::check_items(item_ids.len()).map_err(|error| error.to_string())?;
        let weight = item_ids.capacity().saturating_mul(std::mem::size_of::<NodeId>());
        self.worker
            .send_weighted(SignalWorkerCommand::Reset(item_ids), weight)
            .map_err(generators::admission_error)
            .map_err(|error| error.to_string())
    }

    pub(super) fn take_event(&self) -> Option<SignalWorkerEvent> {
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

    fn service(
        &mut self,
        now: Instant,
        command: Option<SignalWorkerCommand>,
        event_slot: &impl EventSink<SignalWorkerEvent>,
    ) -> bool {
        // Settle time under the old configuration before applying a command.
        // Ordinary commands cannot discard elapsed time or postpone the deadline.
        let delta_seconds = now.saturating_duration_since(self.last_update).as_secs_f64();
        if !event_slot.publish(compute_signal_update(&self.config, &mut self.states, delta_seconds)) {
            return true;
        }
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

fn worker_loop(context: WorkerContext<SignalWorkerCommand, SignalWorkerEvent>, config: SignalWorkerConfig) {
    let mut state = WorkerState::new(config, Instant::now());
    let publish = |event| publish_event(&context, event);
    while !context.cancellation().is_cancelled() {
        let command = match context.receive_until(state.next_update) {
            WorkerReceive::Command(command) => Some(command),
            WorkerReceive::Deadline => None,
            WorkerReceive::Disconnected => return,
        };
        if state.service(Instant::now(), command, &publish) {
            return;
        }
    }
}

// A slow owner backpressures the worker instead of growing a cross-generation
// event map. Each actor turn receives at most EVENT_ROWS callbacks. Cancellation
// interrupts capacity waits and preserves nonblocking owner destruction.
fn publish_event(context: &WorkerContext<SignalWorkerCommand, SignalWorkerEvent>, event: SignalWorkerEvent) -> bool {
    let mut rows = event.samples.into_iter();
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
            .publish_weighted(SignalWorkerEvent { samples: batch }, weight)
            .is_err()
        {
            return false;
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
            let active: HashSet<_> = config.signals.iter().map(|item| item.item_id).collect();
            for item_id in item_ids {
                if active.contains(&item_id) {
                    states.insert(item_id, SignalRuntimeState::default());
                }
            }
            false
        }
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

fn update_interval(update_rate_hz: u32) -> Duration {
    Duration::from_secs_f64(1.0 / update_rate_hz.max(1) as f64)
}

#[cfg(test)]
mod tests;
