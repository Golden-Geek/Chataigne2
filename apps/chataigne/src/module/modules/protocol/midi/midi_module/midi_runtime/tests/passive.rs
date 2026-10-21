use super::super::super::{MidiModule, MidiSystemMessage};
use super::*;
use golden_core::{
    edit::Edit,
    node::{Folder, Node, NodeId, NodeMetaPatch},
    parameter::{ParamValue, ParameterConstraints, ParameterEventBehaviour},
    process_ctx::ExecutionPhase,
};

fn module(engine: &crate::app::AppEngine, id: NodeId) -> &MidiModule {
    let crate::app::AppNode::MidiModule(node) = engine.nodes.get(id).unwrap() else {
        panic!("MIDI")
    };
    node
}
fn module_mut(engine: &mut crate::app::AppEngine, id: NodeId) -> &mut MidiModule {
    let crate::app::AppNode::MidiModule(node) = engine.nodes.get_mut(id).unwrap() else {
        panic!("MIDI")
    };
    node
}
fn turn(engine: &mut crate::app::AppEngine, delta: Duration) {
    engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
    engine.apply_edits().unwrap();
    engine.resolve().unwrap();
    engine.run_tick(delta).unwrap();
    engine.apply_edits().unwrap();
    engine.resolve().unwrap();
    assert_eq!(
        engine.tick_stats().callbacks_fired,
        0,
        "MIDI must never request periodic service"
    );
}
fn quiet(engine: &mut crate::app::AppEngine) {
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut count = 0;
    while count < 10 {
        turn(engine, Duration::from_millis(2));
        let stats = engine.tick_stats();
        count = if stats.wake_callbacks_fired == 0 && stats.snapshot_rebuilds == 0 {
            count + 1
        } else {
            0
        };
        assert!(Instant::now() < deadline, "MIDI did not become quiet");
        std::thread::sleep(Duration::from_millis(2));
    }
}
fn enable(engine: &mut crate::app::AppEngine, id: NodeId, enabled: bool) {
    engine.edits.push(Edit::PatchMeta {
        node: id,
        patch: NodeMetaPatch {
            enabled: Some(enabled),
            ..Default::default()
        },
    });
    engine.apply_edits().unwrap();
    engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
    engine.apply_edits().unwrap();
}
fn discovery(engine: &mut crate::app::AppEngine, id: NodeId, fail_first: bool) {
    let notifier = module(engine, id).service.wake.as_ref().unwrap().notifier.clone();
    let mut attempts = 0;
    let worker = MidiDiscovery::spawn_using(&pool(), notifier, "none", "none", move || {
        attempts += 1;
        if fail_first && attempts == 1 {
            return Err("fake catalog failure".into());
        }
        Ok(MidiPortOptions {
            inputs: vec![],
            outputs: vec![],
        })
    })
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    while !worker.has_pending() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    }
    module_mut(engine, id).discovery = Some(worker);
    module_mut(engine, id).service.discovery_retry = None;
}
fn fixture() -> (crate::app::AppEngine, NodeId) {
    let mut engine = crate::app::AppEngine::new(Folder::new("root").into());
    let mut node = MidiModule::create();
    node.node_data_mut().meta.enabled = false;
    engine.add_node(node.into(), None);
    for _ in 0..6 {
        engine.apply_edits().unwrap();
    }
    let id = engine.nodes.get(engine.root).unwrap().node_data().first_child.unwrap();
    discovery(&mut engine, id, false);
    enable(&mut engine, id, true);
    quiet(&mut engine);
    (engine, id)
}
fn assert_idle(engine: &mut crate::app::AppEngine) {
    for _ in 0..100 {
        turn(engine, Duration::from_millis(20));
        let stats = engine.tick_stats();
        assert_eq!(stats.wake_callbacks_fired, 0, "steady MIDI must not poll workers");
        assert_eq!(stats.snapshot_rebuilds, 0, "steady MIDI must not build snapshots");
    }
}

#[test]
fn steady_discovery_has_no_periodic_callbacks_worker_polling_or_snapshots() {
    let (mut engine, id) = fixture();
    assert!(!module(&engine, id).needs_update());
    assert!(!module(&engine, id).worker_wake_requires_tree_snapshot());
    assert_idle(&mut engine);
}

#[test]
fn queued_input_requests_no_snapshot_while_structure_is_waiting_for_its_callback() {
    let (mut engine, id) = fixture();
    let snapshot = engine.process_tree_snapshot();
    let mut ctx = golden_core::process_ctx::ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    let node = module_mut(&mut engine, id);
    node.queue_cc_conversion(&mut ctx, 1, false);
    node.service_structure(&mut ctx, Some(&snapshot));
    node.enqueue_incoming_message_for_test(MidiMessage::NoteOn {
        channel: 1,
        note: 60,
        velocity: 1,
    });
    assert!(node.structure_work.busy());
    assert!(!node.structure_work.requires_snapshot());
    assert!(!node.worker_wake_requires_tree_snapshot());
    engine.absorb_edits(&mut ctx).unwrap();
    engine.apply_edits().unwrap();
    assert!(!module(&engine, id).structure_work.busy());
    assert!(module(&engine, id).worker_wake_requires_tree_snapshot());
    quiet(&mut engine);
}

#[test]
fn runtime_registration_publishes_capabilities_from_the_current_bound_configuration() {
    let (mut engine, id) = fixture();
    let port = module(&engine, id).input_port.id();
    let connection = module(&engine, id).base.connection_id().unwrap();
    let capability = engine
        .process_tree_snapshot()
        .child_ids_slice(connection)
        .iter()
        .copied()
        .find(|param| {
            engine
                .nodes
                .get(*param)
                .unwrap()
                .node_data()
                .meta
                .decl_id
                .0
                .rsplit('/')
                .next()
                == Some("can_receive")
        })
        .unwrap();
    engine.edits.push(Edit::SetParamConstraints {
        node: port,
        constraints: ParameterConstraints::default(),
    });
    engine.edits.push(Edit::SetParam {
        node: port,
        value: ParamValue::Enum("fake".into()),
        behaviour: ParameterEventBehaviour::Append,
    });
    engine.apply_edits().unwrap();
    // Prepared-item initialization can apply explicit values after init, before ready.
    // Rebind those values without relying on a later parameter callback or update.
    engine.inbox.clear();
    let snapshot = engine.process_tree_snapshot();
    module_mut(&mut engine, id).service = super::super::super::service::ServiceState::default();
    engine.edits.push(Edit::CallNodeMutation {
        node: id,
        needs_tree_snapshot: false,
        callback: Box::new(move |node, ctx| {
            node.engine_sync_bound_param_handles(&mut |id| snapshot.node(id).and_then(|node| node.param_value.clone()));
            let node = node.as_any_mut().downcast_mut::<MidiModule>().unwrap();
            node.register_service(ctx)
        }),
    });
    engine.apply_edits().unwrap();
    engine.apply_edits().unwrap();
    assert_eq!(
        engine
            .nodes
            .get(capability)
            .unwrap()
            .engine_param_snapshot()
            .unwrap()
            .value,
        ParamValue::Bool(true)
    );
}

struct Input;
impl InputConnection for Input {
    fn is_present(&mut self) -> Result<bool, String> {
        Ok(true)
    }
}
#[test]
fn native_input_notifications_deliver_both_messages_through_passive_graph_turns() {
    let (mut engine, id) = fixture();
    let notifier = module(&engine, id).service.wake.as_ref().unwrap().notifier.clone();
    let (send, receive) = mpsc::channel();
    let input = MidiInputHandle::spawn_using(
        MidiInputConfig {
            port_variant: "fake".into(),
        },
        &pool(),
        notifier,
        move |_, sink| {
            send.send(sink).unwrap();
            Ok(Input)
        },
    )
    .unwrap();
    module_mut(&mut engine, id).input = Some(input);
    let sink = receive.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(sink.push(&[0x90, 60, 12]));
    assert!(sink.push(&[0x90, 61, 64]));
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        turn(&mut engine, Duration::from_millis(2));
        let snapshot = engine.process_tree_snapshot();
        let values = module(&engine, id).base.values_id().unwrap();
        let notes = snapshot
            .find_child_by_decl_id(values, "channel_1")
            .and_then(|channel| snapshot.find_child_by_decl_id(channel, "notes"));
        let values = notes.map(|notes| {
            [60, 61].map(|note| {
                snapshot
                    .find_child_by_decl_id(notes, &super::super::super::note_decl_id(note))
                    .and_then(|id| snapshot.node(id))
                    .and_then(|node| node.param_value.clone())
            })
        });
        if values == Some([Some(ParamValue::Int(12)), Some(ParamValue::Int(64))]) {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    }
    quiet(&mut engine);
    assert_idle(&mut engine);
}

#[test]
fn mtc_and_clock_expire_from_the_earliest_deadline_without_periodic_ticks() {
    let (mut engine, id) = fixture();
    module_mut(&mut engine, id).enqueue_incoming_message_for_test(MidiMessage::System(MidiSystemMessage::TimingClock));
    module_mut(&mut engine, id).enqueue_incoming_message_for_test(MidiMessage::System(
        MidiSystemMessage::TimeCodeQuarterFrame { value: 0 },
    ));
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut started = false;
    let mut mtc_stopped_first = false;
    loop {
        turn(&mut engine, Duration::from_millis(10));
        let node = module(&engine, id);
        started |= node.mtc_stream_active && node.midi_clock_state.running;
        mtc_stopped_first |= started && !node.mtc_stream_active && node.midi_clock_state.running;
        if started && !node.mtc_stream_active && !node.midi_clock_state.running {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(mtc_stopped_first);
    assert!(module(&engine, id).activity_deadline().is_none());
    quiet(&mut engine);
    assert_idle(&mut engine);
}

#[test]
fn a_single_retry_deadline_wakes_without_requesting_a_tree_snapshot() {
    let (mut engine, id) = fixture();
    engine.edits.push(Edit::CallNodeMutation {
        node: id,
        needs_tree_snapshot: false,
        callback: Box::new(move |node, ctx| {
            let node = node.as_any_mut().downcast_mut::<MidiModule>().unwrap();
            node.input_dirty = true;
            node.service.input_retry = Some(Instant::now() + Duration::from_millis(40));
            node.request_service(ctx);
            Ok(())
        }),
    });
    engine.apply_edits().unwrap();
    turn(&mut engine, Duration::ZERO);
    assert!(module(&engine, id).input_dirty);
    assert!(!module(&engine, id).service_requires_snapshot());
    let deadline = Instant::now() + Duration::from_secs(2);
    while module(&engine, id).input_dirty {
        std::thread::sleep(Duration::from_millis(5));
        turn(&mut engine, Duration::from_millis(5));
        assert_eq!(engine.tick_stats().snapshot_rebuilds, 0);
        assert!(Instant::now() < deadline);
    }
    assert!(module(&engine, id).service.input_retry.is_none());
    quiet(&mut engine);
    assert_idle(&mut engine);
}

#[test]
fn a_failed_native_worker_retains_one_retry_instead_of_busy_polling() {
    let (mut engine, id) = fixture();
    let notifier = module(&engine, id).service.wake.as_ref().unwrap().notifier.clone();
    let input = MidiInputHandle::spawn_using(
        MidiInputConfig {
            port_variant: "fake".into(),
        },
        &pool(),
        notifier,
        |_, _| Err::<Input, _>("fake input failure".into()),
    )
    .unwrap();
    module_mut(&mut engine, id).input = Some(input);
    let deadline = Instant::now() + Duration::from_secs(2);
    while module(&engine, id).input.is_some() {
        turn(&mut engine, Duration::from_millis(2));
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(module(&engine, id).input_dirty);
    assert!(module(&engine, id)
        .service
        .input_retry
        .is_some_and(|at| at > Instant::now()));
    assert_idle(&mut engine);
    enable(&mut engine, id, false);
    assert!(module(&engine, id).service.input_retry.is_none());
}

struct BlockedOutput {
    entered: mpsc::Sender<()>,
    gate: Gate,
}
impl OutputConnection for BlockedOutput {
    fn send(&mut self, _: &[u8]) -> Result<(), String> {
        Ok(())
    }
    fn is_present(&mut self) -> Result<bool, String> {
        Ok(true)
    }
}
impl Drop for BlockedOutput {
    fn drop(&mut self) {
        let _ = self.entered.send(());
        self.gate.wait();
    }
}
#[test]
fn blocked_retiring_cleanup_never_polls_and_its_exit_rearms_reconfiguration() {
    let (mut engine, id) = fixture();
    let notifier = module(&engine, id).service.wake.as_ref().unwrap().notifier.clone();
    let pool = pool();
    let gate = Gate::default();
    let _release = Release(gate.clone());
    let (entered, observed) = mpsc::channel();
    let blocked = gate.clone();
    let mut output = MidiOutputHandle::spawn_using(
        MidiOutputConfig {
            port_variant: "fake".into(),
        },
        &pool,
        notifier,
        move |_| Ok(BlockedOutput { entered, gate: blocked }),
    )
    .unwrap();
    output.finish_admission();
    module_mut(&mut engine, id).retiring_output = Some(output);
    module_mut(&mut engine, id).output_dirty = true;
    observed.recv_timeout(Duration::from_secs(2)).unwrap();
    quiet(&mut engine);
    assert!(module(&engine, id).retiring_output.is_some());
    assert!(pool.try_reserve().is_err());
    assert_idle(&mut engine);
    gate.release();
    let deadline = Instant::now() + Duration::from_secs(2);
    while module(&engine, id).retiring_output.is_some() {
        turn(&mut engine, Duration::from_millis(2));
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(!module(&engine, id).output_dirty);
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn disable_cancels_activity_and_enable_restarts_passive_discovery() {
    let (mut engine, id) = fixture();
    module_mut(&mut engine, id).enqueue_incoming_message_for_test(MidiMessage::System(MidiSystemMessage::TimingClock));
    for _ in 0..8 {
        turn(&mut engine, Duration::from_millis(2));
    }
    assert!(module(&engine, id).midi_clock_state.running);
    enable(&mut engine, id, false);
    assert!(module(&engine, id).activity_deadline().is_none());
    std::thread::sleep(Duration::from_millis(550));
    quiet(&mut engine);
    assert_idle(&mut engine);
    discovery(&mut engine, id, false);
    enable(&mut engine, id, true);
    quiet(&mut engine);
    assert!(!module(&engine, id).midi_clock_state.running);
    assert_idle(&mut engine);
}

#[test]
fn discovery_failure_recovers_from_a_worker_result_without_actor_polling() {
    let (mut engine, id) = fixture();
    discovery(&mut engine, id, true);
    module(&engine, id).service.wake.as_ref().unwrap().notifier.notify();
    let deadline = Instant::now() + Duration::from_secs(3);
    let port = module(&engine, id).input_port.id();
    let mut failed = false;
    loop {
        turn(&mut engine, Duration::from_millis(5));
        let warnings = &engine.nodes.get(port).unwrap().node_data().meta.presentation.warnings;
        let has_warning = warnings
            .iter()
            .any(|w| w.id == super::super::super::MIDI_PORT_OPTIONS_WARNING_ID);
        failed |= has_warning;
        if failed && !has_warning {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    quiet(&mut engine);
    assert_idle(&mut engine);
}
