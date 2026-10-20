mod admission;

use super::*;
use golden_core::node::NodeUuid;

fn config(interval_seconds: f64) -> MetronomeWorkerConfig {
    MetronomeWorkerConfig {
        update_rate_hz: 1,
        metronomes: vec![MetronomeConfig {
            item_id: NodeId(1),
            item_uuid: NodeUuid(uuid::Uuid::from_u128(1)),
            value_decl_id: "beat".into(),
            label: "Beat".into(),
            enabled: true,
            interval_seconds,
            randomize_gap: false,
            random_min: 1.0,
            random_max: 1.0,
            seed: 0,
        }],
    }
}

#[test]
fn configuration_and_manual_tick_commands_preserve_natural_ticks_time_and_deadlines() {
    let origin = Instant::now();
    let mut worker = WorkerState::new(config(0.5), origin);
    let slot = MetronomeEventSlot::new();
    for step in 1..=20 {
        let now = origin + Duration::from_millis(step * 100);
        let command = if step % 2 == 0 {
            MetronomeWorkerCommand::ManualTick(NodeId(1))
        } else {
            MetronomeWorkerCommand::Configure(config(0.5))
        };
        worker.service(now, Some(command), &slot);
        assert_eq!(worker.next_update, origin + Duration::from_secs(step / 10 + 1));
    }
    let event = slot.take().unwrap();
    let tick = &event.ticks[&NodeId(1)];
    assert_eq!((tick.fired, tick.total_ticks), (14, 14));
    assert!(worker.states[&NodeId(1)].elapsed_seconds.abs() < 1e-12);
}

#[test]
fn interval_edits_preserve_elapsed_time_since_the_previous_tick() {
    let origin = Instant::now();
    let mut worker = WorkerState::new(config(1.0), origin);
    let slot = MetronomeEventSlot::new();
    worker.service(
        origin + Duration::from_millis(750),
        Some(MetronomeWorkerCommand::Configure(config(2.0))),
        &slot,
    );
    worker.service(origin + Duration::from_millis(1250), None, &slot);
    assert!((worker.states[&NodeId(1)].elapsed_seconds - 1.25).abs() < 1e-12);
    assert!(slot.take().is_none());
    worker.service(origin + Duration::from_secs(2), None, &slot);
    assert_eq!(slot.take().unwrap().ticks[&NodeId(1)].total_ticks, 1);
}

#[test]
fn targeted_reset_preserves_other_metronomes() {
    let origin = Instant::now();
    let mut config = config(1.0);
    let mut other = config.metronomes[0].clone();
    other.item_id = NodeId(2);
    config.metronomes.push(other);
    let mut worker = WorkerState::new(config, origin);
    let slot = MetronomeEventSlot::new();
    worker.service(
        origin + Duration::from_millis(750),
        Some(MetronomeWorkerCommand::Reset(vec![NodeId(1)])),
        &slot,
    );
    worker.service(origin + Duration::from_millis(1250), None, &slot);
    let event = slot.take().unwrap();
    assert!(!event.ticks.contains_key(&NodeId(1)));
    assert_eq!(event.ticks[&NodeId(2)].total_ticks, 1);
    assert!((worker.states[&NodeId(1)].elapsed_seconds - 0.5).abs() < 1e-12);
}

#[test]
fn catch_up_keeps_overdue_time_across_bounded_service_turns() {
    let origin = Instant::now();
    let now = origin + Duration::from_millis(6005);
    let mut worker = WorkerState::new(config(0.01), origin);
    let slot = MetronomeEventSlot::new();
    let mut fired = 0;
    for _ in 0..5 {
        worker.service(now, None, &slot);
        let event = slot.take().unwrap();
        let tick = &event.ticks[&NodeId(1)];
        assert!(tick.fired <= MAX_TICKS_PER_UPDATE);
        fired += tick.fired;
    }
    assert_eq!(fired, 600);
    assert_eq!(worker.states[&NodeId(1)].tick_count, 600);
    assert!((worker.states[&NodeId(1)].elapsed_seconds - 0.005).abs() < 1e-10);
}

#[test]
fn tempo_command_waits_for_the_old_tempos_entire_due_prefix() {
    let origin = Instant::now();
    let now = origin + Duration::from_millis(6005);
    let mut worker = WorkerState::new(config(0.01), origin);
    let slot = MetronomeEventSlot::new();
    worker.service(now, Some(MetronomeWorkerCommand::Configure(config(2.0))), &slot);
    assert!(worker.pending_command.is_some());
    assert_eq!(worker.config.metronomes[0].interval_seconds, 0.01);
    for _ in 0..4 {
        worker.service(now, None, &slot);
    }
    assert!(worker.pending_command.is_none());
    assert_eq!(worker.config.metronomes[0].interval_seconds, 2.0);
    assert_eq!(slot.take().unwrap().ticks[&NodeId(1)].total_ticks, 600);
    worker.service(now + Duration::from_secs(1), None, &slot);
    assert!((worker.states[&NodeId(1)].elapsed_seconds - 1.005).abs() < 1e-10);
    assert!(slot.take().is_none());
}

#[test]
fn stop_cancels_an_enormous_elapsed_backlog_between_worker_turns() {
    let pool = golden_io::RetirementPool::new(NonZeroUsize::MIN);
    let origin = Instant::now() - Duration::from_secs(1_000_000);
    let state = WorkerState::new(config(0.001), origin);
    let worker = BoundedWorker::spawn("metronome-catch-up", &pool, generators::limits(), move |context| {
        worker_loop(context, state)
    })
    .unwrap();
    worker
        .send_weighted(MetronomeWorkerCommand::Configure(config(2.0)), 1024)
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while worker.event_metrics().depth == 0 {
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    worker.cancel();
    assert!(
        pool.wait_for_idle(Duration::from_secs(3)),
        "cancellation must not wait for one billion due ticks or a full event mailbox"
    );
    let mut events = Vec::new();
    worker.drain_events(&mut events, NonZeroUsize::new(8).unwrap());
    assert!(!events.is_empty());
    assert!(events.last().unwrap().ticks[&NodeId(1)].total_ticks < 1_000_000_000);
}

use std::sync::Mutex;

#[derive(Default)]
struct MetronomeEventSlot(Mutex<Option<MetronomeWorkerEvent>>);

impl MetronomeEventSlot {
    fn new() -> Self {
        Self::default()
    }
    fn take(&self) -> Option<MetronomeWorkerEvent> {
        self.0.lock().unwrap().take()
    }
}

impl EventSink<MetronomeWorkerEvent> for MetronomeEventSlot {
    fn publish(&self, event: MetronomeWorkerEvent) -> bool {
        if event.ticks.is_empty() {
            return true;
        }
        let mut slot = self.0.lock().unwrap();
        if let Some(existing) = slot.as_mut() {
            merge_metronome_event(existing, event);
        } else {
            *slot = Some(event);
        }
        true
    }
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
