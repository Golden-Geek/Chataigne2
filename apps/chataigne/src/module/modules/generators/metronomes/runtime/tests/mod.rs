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
    let origin = Instant::now() - Duration::from_secs(1_000_000);
    let state = WorkerState::new(config(0.001), origin);
    let slot = MetronomeEventSlot::new();
    let worker_slot = slot.clone();
    let stopped = Arc::new(AtomicBool::new(false));
    let worker_stopped = stopped.clone();
    let (commands, receiver) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    commands.send(MetronomeWorkerCommand::Configure(config(2.0))).unwrap();
    let worker = thread::spawn(move || {
        worker_loop(receiver, worker_slot, state, worker_stopped);
        done_tx.send(()).unwrap();
    });
    let deadline = Instant::now() + Duration::from_secs(3);
    while !slot.has_pending() {
        assert!(Instant::now() < deadline);
        thread::yield_now();
    }
    stopped.store(true, Ordering::Release);
    let _ = commands.send(MetronomeWorkerCommand::Stop);
    done_rx
        .recv_timeout(Duration::from_secs(3))
        .expect("cancellation must not wait for one billion due ticks");
    worker.join().unwrap();
    assert!(slot.take().unwrap().ticks[&NodeId(1)].total_ticks < 1_000_000_000);
}
