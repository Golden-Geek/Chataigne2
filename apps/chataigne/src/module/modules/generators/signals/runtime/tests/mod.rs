mod admission;

use super::super::SignalShape;
use super::*;

fn config() -> SignalWorkerConfig {
    SignalWorkerConfig {
        update_rate_hz: 1,
        signals: vec![SignalConfig {
            item_id: NodeId(1),
            value_decl_id: "signal".into(),
            label: "Signal".into(),
            enabled: true,
            shape: SignalShape::Saw,
            frequency_hz: 1.0,
            phase: 0.0,
            range_min: 0.0,
            range_max: 1.0,
            curve: None,
            seed: 0,
        }],
    }
}

#[test]
fn continuous_configuration_commands_preserve_elapsed_time_cycles_and_deadlines() {
    let origin = Instant::now();
    let mut worker = WorkerState::new(config(), origin);
    let slot = SignalEventSlot::new();
    for step in 1..=25 {
        let now = origin + Duration::from_millis(step * 100);
        worker.service(now, Some(SignalWorkerCommand::Configure(config())), &slot);
        assert_eq!(worker.next_update, origin + Duration::from_secs(step / 10 + 1));
    }
    let event = slot.take().unwrap();
    let sample = &event.samples[&NodeId(1)];
    assert!((worker.states[&NodeId(1)].elapsed_seconds - 2.5).abs() < 1e-12);
    assert!((sample.value - 0.5).abs() < 1e-12);
    assert_eq!((sample.cycle, sample.cycles), (2, 2));
}

#[test]
fn frequency_and_shape_edits_preserve_accumulated_phase() {
    let origin = Instant::now();
    let mut worker = WorkerState::new(config(), origin);
    let slot = SignalEventSlot::new();
    let mut next = config();
    next.signals[0].frequency_hz = 2.0;
    worker.service(
        origin + Duration::from_millis(250),
        Some(SignalWorkerCommand::Configure(next.clone())),
        &slot,
    );
    worker.service(origin + Duration::from_millis(500), None, &slot);
    assert!((slot.take().unwrap().samples[&NodeId(1)].value - 0.75).abs() < 1e-12);
    next.signals[0].shape = SignalShape::ReverseSaw;
    worker.service(
        origin + Duration::from_millis(500),
        Some(SignalWorkerCommand::Configure(next)),
        &slot,
    );
    worker.service(origin + Duration::from_millis(750), None, &slot);
    let sample = slot.take().unwrap().samples.remove(&NodeId(1)).unwrap();
    assert!((sample.value - 0.75).abs() < 1e-12);
    assert_eq!((sample.cycle, sample.cycles), (1, 1));
}

#[test]
fn targeted_reset_preserves_other_signals_and_accounts_time_before_reset() {
    let origin = Instant::now();
    let mut config = config();
    let mut other = config.signals[0].clone();
    other.item_id = NodeId(2);
    config.signals.push(other);
    let mut worker = WorkerState::new(config, origin);
    let slot = SignalEventSlot::new();
    worker.service(
        origin + Duration::from_millis(750),
        Some(SignalWorkerCommand::Reset(vec![NodeId(1)])),
        &slot,
    );
    worker.service(origin + Duration::from_millis(1250), None, &slot);
    let event = slot.take().unwrap();
    assert!((event.samples[&NodeId(1)].value - 0.5).abs() < 1e-12);
    assert!((event.samples[&NodeId(2)].value - 0.25).abs() < 1e-12);
    assert_eq!(event.samples[&NodeId(2)].cycles, 1);
}

#[test]
fn delayed_first_service_retains_all_crossed_cycles() {
    let origin = Instant::now();
    let mut worker = WorkerState::new(config(), origin);
    let slot = SignalEventSlot::new();
    worker.service(origin + Duration::from_millis(2500), None, &slot);
    let sample = slot.take().unwrap().samples.remove(&NodeId(1)).unwrap();
    assert_eq!((sample.cycle, sample.cycles), (2, 2));
}

use std::sync::Mutex;

#[derive(Default)]
struct SignalEventSlot(Mutex<Option<SignalWorkerEvent>>);

impl SignalEventSlot {
    fn new() -> Self {
        Self::default()
    }
    fn take(&self) -> Option<SignalWorkerEvent> {
        self.0.lock().unwrap().take()
    }
}

impl EventSink<SignalWorkerEvent> for SignalEventSlot {
    fn publish(&self, event: SignalWorkerEvent) -> bool {
        if event.samples.is_empty() {
            return true;
        }
        let mut slot = self.0.lock().unwrap();
        if let Some(existing) = slot.as_mut() {
            merge_signal_event(existing, event);
        } else {
            *slot = Some(event);
        }
        true
    }
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
