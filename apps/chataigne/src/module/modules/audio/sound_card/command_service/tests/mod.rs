use super::*;
use golden_core::{engine::EngineTime, process_ctx::ExecutionPhase};
use std::time::{Duration, Instant};

fn fixture() -> (crate::app::AppEngine, NodeId, NodeId) {
    let (mut engine, module) = super::super::tests::create_sound_card();
    engine.add_node(crate::app::SoundCardStopAllFilesCommand::create().into(), None);
    engine.add_node(crate::app::SoundCardSetMasterVolumeCommand::create().into(), None);
    for _ in 0..8 {
        engine.apply_edits().unwrap();
    }
    let command = engine
        .process_tree_snapshot()
        .child_ids_slice(engine.root)
        .iter()
        .copied()
        .find(|id| {
            engine
                .nodes
                .get(*id)
                .is_some_and(|node| node.get_type() == "sound_card_stop_all_files_command")
        })
        .unwrap();
    (engine, module, command)
}

fn job(
    engine: &mut crate::app::AppEngine,
    module: NodeId,
    command: NodeId,
    lifetime: Arc<AtomicBool>,
    pool: &PayloadPool,
) -> Job {
    Job {
        source: Source::Command(CapturedInvocation {
            snapshot: engine.process_tree_snapshot().try_lease().unwrap(),
            execution: None,
            module,
            command,
            override_weight: 0,
        }),
        producer: engine.node_waker(command).unwrap(),
        lifetime,
        module,
        reservation: pool.try_reserve(REQUEST_BYTES).unwrap(),
    }
}

fn receive(service: &CommandService) -> RetainedPayload<Prepared> {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let mut events = Vec::new();
        service
            .worker
            .as_ref()
            .unwrap()
            .drain_events(&mut events, NonZeroUsize::new(1).unwrap());
        if let Some(prepared) = events.pop() {
            return prepared;
        }
        assert!(
            Instant::now() < deadline,
            "preparation must publish without engine polling"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn context() -> ProcessCtx {
    ProcessCtx::new(
        ExecutionPhase::EngineTick,
        EngineTime {
            tick: 1,
            micro: 0,
            seq: 0,
        },
    )
}

#[test]
fn error_reports_bound_utf8_without_splitting_a_character() {
    let text = bounded_error("音".repeat(4096));
    assert!(text.len() <= 4096);
    assert_eq!(text, "音".repeat(1365));
}

#[test]
fn prepared_retention_admission_recovers_after_background_disposal() {
    let pool = PayloadPool::new("sound-card-command-test", 1, REQUEST_BYTES).unwrap();
    let reservation = pool.try_reserve(REQUEST_BYTES).unwrap();
    assert!(pool.try_reserve(1).is_err());
    let prepared = reservation.retain(vec![0u8; 4096], REQUEST_BYTES).unwrap();
    assert!(pool.try_reserve(1).is_err());
    drop(prepared);
    assert!(pool.wait_for_idle(std::time::Duration::from_secs(2)));
    assert!(pool.try_reserve(REQUEST_BYTES).is_ok());
}

#[test]
fn preparation_is_passive_and_retains_input_until_result_disposal() {
    let (mut engine, module, command) = fixture();
    let pool = PayloadPool::new("sound-card-preparation-test", 1, REQUEST_BYTES).unwrap();
    let wake = RuntimeWakeSender::new(engine.node_waker(module).unwrap()).unwrap();
    let mut service = CommandService::default();
    service
        .enqueue(
            job(&mut engine, module, command, Arc::new(AtomicBool::new(true)), &pool),
            &wake,
        )
        .unwrap();
    let prepared = receive(&service);
    assert!(matches!(
        *prepared.request.lock().unwrap(),
        Some(Ok(SoundCardCommandRequest::StopAllFiles))
    ));
    assert!(pool.try_reserve(1).is_err());
    drop(prepared);
    drop(service);
    assert!(pool.wait_for_idle(Duration::from_secs(3)));
}

#[test]
fn retired_source_is_discarded_before_preparation_and_recovers_capacity() {
    let (mut engine, module, command) = fixture();
    let pool = PayloadPool::new("sound-card-retired-source-test", 1, REQUEST_BYTES).unwrap();
    let request = job(&mut engine, module, command, Arc::new(AtomicBool::new(true)), &pool);
    engine.edits.push(Edit::RemoveNode { node: command });
    engine.apply_edits().unwrap();
    assert!(request.producer.is_closed());
    let wake = RuntimeWakeSender::new(engine.node_waker(module).unwrap()).unwrap();
    let mut service = CommandService::default();
    service.enqueue(request, &wake).unwrap();
    assert!(pool.wait_for_idle(Duration::from_secs(3)));
    let mut events = Vec::new();
    service
        .worker
        .as_ref()
        .unwrap()
        .drain_events(&mut events, NonZeroUsize::new(1).unwrap());
    assert!(events.is_empty());
}

#[test]
fn replacement_runtime_cannot_receive_a_prepared_request_for_an_older_lifetime() {
    let (mut engine, module, command) = fixture();
    let pool = PayloadPool::new("sound-card-stale-command-test", 1, REQUEST_BYTES).unwrap();
    let lifetime = Arc::new(AtomicBool::new(true));
    let wake = RuntimeWakeSender::new(engine.node_waker(module).unwrap()).unwrap();
    let mut service = CommandService::default();
    service
        .enqueue(job(&mut engine, module, command, lifetime.clone(), &pool), &wake)
        .unwrap();
    let prepared = receive(&service);
    let runtime = super::super::runtime::SoundCardRuntime::start_selected(
        golden_audio::SampleRate::new(48000).unwrap(),
        Some(golden_audio::NullBackend::backend_id()),
    )
    .unwrap();
    assert!(
        prepared.live(),
        "even an older live handle cannot redirect to its replacement"
    );
    let mut delivery = Delivery {
        prepared,
        outcome: None,
        admitted_sequence: None,
        gain_done: false,
        gain: EditReceipt::default(),
        warning: EditReceipt::default(),
        callback: None,
        report_ready: false,
    };
    let node = engine
        .nodes
        .get_mut(module)
        .unwrap()
        .as_any_mut()
        .downcast_mut::<SoundCardModule>()
        .unwrap();
    node.runtime = Some(runtime);
    assert!(!delivery.service(&mut context(), node));
    assert!(delivery.outcome.as_ref().unwrap().is_err());
    assert!(delivery.prepared.request.lock().unwrap().is_some());
}

#[test]
fn native_admission_is_not_replayed_when_warning_publication_retries() {
    let (mut engine, module, command) = fixture();
    let pool = PayloadPool::new("sound-card-native-once-test", 1, REQUEST_BYTES).unwrap();
    let runtime = super::super::runtime::SoundCardRuntime::start_selected(
        golden_audio::SampleRate::new(48000).unwrap(),
        Some(golden_audio::NullBackend::backend_id()),
    )
    .unwrap();
    let lifetime = runtime.command_lifetime();
    let wake = RuntimeWakeSender::new(engine.node_waker(module).unwrap()).unwrap();
    let mut service = CommandService::default();
    service
        .enqueue(job(&mut engine, module, command, lifetime, &pool), &wake)
        .unwrap();
    let prepared = receive(&service);
    let node = engine
        .nodes
        .get_mut(module)
        .unwrap()
        .as_any_mut()
        .downcast_mut::<SoundCardModule>()
        .unwrap();
    node.runtime = Some(runtime);
    let mut delivery = Delivery {
        prepared,
        outcome: None,
        admitted_sequence: None,
        gain_done: false,
        gain: EditReceipt::default(),
        warning: EditReceipt::default(),
        callback: None,
        report_ready: false,
    };
    let mut first = context();
    assert!(!delivery.service(&mut first, node));
    let sequence = delivery.outcome.as_ref().unwrap().as_ref().unwrap().get();
    assert!(delivery.prepared.request.lock().unwrap().is_none());
    engine.edits.push(Edit::SetParam {
        node: NodeId(u64::MAX),
        value: ParamValue::Bool(true),
        behaviour: golden_core::parameter::ParameterEventBehaviour::Coalesce,
    });
    engine.edits.pending.extend(first.edits.drain());
    assert!(
        engine.apply_edits().is_err(),
        "fail the suffix before its warning publication"
    );
    let node = engine
        .nodes
        .get(module)
        .unwrap()
        .as_any()
        .downcast_ref::<SoundCardModule>()
        .unwrap();
    let mut retry = context();
    assert!(!delivery.service(&mut retry, node));
    assert!(!retry.edits.pending.is_empty(), "retry the unpublished warning");
    assert_eq!(delivery.outcome.as_ref().unwrap().as_ref().unwrap().get(), sequence);
    let next = node
        .runtime
        .as_ref()
        .unwrap()
        .admit(golden_audio::AudioCommand::StopAllFiles)
        .unwrap();
    assert_eq!(next.get(), sequence + 1);
}

#[test]
fn prepared_gain_rejects_a_removed_authored_target_before_native_admission() {
    let (mut engine, module, _) = fixture();
    let snapshot = engine.process_tree_snapshot();
    let command = snapshot
        .child_ids_slice(engine.root)
        .iter()
        .copied()
        .find(|id| snapshot.node(*id).unwrap().node_type == "sound_card_set_master_volume_command")
        .unwrap();
    let runtime = super::super::runtime::SoundCardRuntime::start_selected(
        golden_audio::SampleRate::new(48000).unwrap(),
        Some(golden_audio::NullBackend::backend_id()),
    )
    .unwrap();
    let lifetime = runtime.command_lifetime();
    let before = runtime.admit(golden_audio::AudioCommand::StopAllFiles).unwrap().get();
    let pool = PayloadPool::new("sound-card-stale-gain-test", 1, REQUEST_BYTES).unwrap();
    let wake = RuntimeWakeSender::new(engine.node_waker(module).unwrap()).unwrap();
    let mut service = CommandService::default();
    service
        .enqueue(job(&mut engine, module, command, lifetime, &pool), &wake)
        .unwrap();
    let prepared = receive(&service);
    let gain = prepared.gain.as_ref().unwrap().node;
    engine.edits.push(Edit::RemoveNode { node: gain });
    engine.apply_edits().unwrap();
    let snapshot = engine.process_tree_snapshot();
    let node = engine
        .nodes
        .get_mut(module)
        .unwrap()
        .as_any_mut()
        .downcast_mut::<SoundCardModule>()
        .unwrap();
    node.runtime = Some(runtime);
    let mut delivery = Delivery {
        prepared,
        outcome: None,
        admitted_sequence: None,
        gain_done: false,
        gain: EditReceipt::default(),
        warning: EditReceipt::default(),
        callback: None,
        report_ready: false,
    };
    let mut ctx = context();
    ctx.set_tree_snapshot(snapshot);
    assert!(!delivery.service(&mut ctx, node));
    assert!(delivery.outcome.as_ref().unwrap().is_err());
    assert!(delivery.admitted_sequence.is_none());
    assert!(delivery.prepared.request.lock().unwrap().is_some());
    assert_eq!(
        node.runtime
            .as_ref()
            .unwrap()
            .admit(golden_audio::AudioCommand::StopAllFiles)
            .unwrap()
            .get(),
        before + 1
    );
}

#[test]
fn cancelling_a_full_result_mailbox_does_not_join_and_releases_all_input() {
    let (mut engine, module, command) = fixture();
    let pool = PayloadPool::new("sound-card-cancel-command-test", 130, 130 * REQUEST_BYTES).unwrap();
    let wake = RuntimeWakeSender::new(engine.node_waker(module).unwrap()).unwrap();
    let mut service = CommandService::default();
    let mut accepted = 0;
    let Job {
        source,
        producer,
        lifetime,
        reservation,
        ..
    } = job(&mut engine, module, command, Arc::new(AtomicBool::new(true)), &pool);
    drop(reservation);
    let Source::Command(source) = source else {
        unreachable!()
    };
    for _ in 0..300 {
        if let Err(error) = service.enqueue(
            Job {
                source: Source::Command(source.clone()),
                producer: producer.clone(),
                lifetime: lifetime.clone(),
                module,
                reservation: pool.try_reserve(REQUEST_BYTES).unwrap(),
            },
            &wake,
        ) {
            assert!(error.contains("mailbox is full"), "{error}");
            break;
        }
        accepted += 1;
    }
    assert!(accepted > 0 && accepted <= 2 * REQUESTS + 1);
    let started = Instant::now();
    drop(service);
    assert!(started.elapsed() < Duration::from_millis(100));
    assert!(pool.wait_for_idle(Duration::from_secs(3)));
}

#[test]
fn deferred_dispatch_cannot_mutate_a_replacement_module() {
    let (mut engine, module, command) = fixture();
    let invocation = CapturedInvocation {
        snapshot: engine.process_tree_snapshot().try_lease().unwrap(),
        execution: None,
        module,
        command,
        override_weight: 0,
    };
    let queued = Arc::new(Mutex::new(Vec::new()));
    let capture = queued.clone();
    engine.edits.push(Edit::CallNodeMutation {
        node: command,
        needs_tree_snapshot: true,
        callback: Box::new(move |_, ctx| {
            SoundCardModule::dispatch_command(ctx, invocation)?;
            capture.lock().unwrap().extend(ctx.edits.drain());
            Ok(())
        }),
    });
    engine.apply_edits().unwrap();
    assert_eq!(queued.lock().unwrap().len(), 1);
    engine.edits.push(Edit::ReplaceNode {
        node: module,
        new_node: Box::new(golden_core::node::Folder::new("Replacement")),
    });
    engine.apply_edits().unwrap();
    engine.edits.pending.extend(queued.lock().unwrap().drain(..));
    engine
        .apply_edits()
        .expect("retired dispatch must be a no-op, not an edit failure against the replacement");
    assert_eq!(engine.nodes.get(module).unwrap().get_type(), "folder");
}

#[test]
fn retired_runtime_retains_a_failure_result_and_worker_failure_can_restart() {
    let (mut engine, module, command) = fixture();
    let pool = PayloadPool::new("sound-card-worker-recovery-test", 1, REQUEST_BYTES).unwrap();
    let wake = RuntimeWakeSender::new(engine.node_waker(module).unwrap()).unwrap();
    let mut service = CommandService::default();
    service
        .enqueue(
            job(&mut engine, module, command, Arc::new(AtomicBool::new(false)), &pool),
            &wake,
        )
        .unwrap();
    let rejected = receive(&service);
    assert!(!rejected.live());
    assert!(matches!(*rejected.request.lock().unwrap(), Some(Err(_))));
    drop(rejected);
    assert!(pool.wait_for_idle(Duration::from_secs(3)));
    service.worker.as_ref().unwrap().cancel();
    let stopped = service
        .enqueue(
            job(&mut engine, module, command, Arc::new(AtomicBool::new(true)), &pool),
            &wake,
        )
        .unwrap_err();
    assert!(stopped.contains("worker stopped"), "{stopped}");
    let deadline = Instant::now() + Duration::from_secs(3);
    while service.worker.is_some() {
        let node = engine
            .nodes
            .get(module)
            .unwrap()
            .as_any()
            .downcast_ref::<SoundCardModule>()
            .unwrap();
        service.service(&mut context(), node);
        assert!(Instant::now() < deadline, "failed worker must retire");
        std::thread::sleep(Duration::from_millis(1));
    }
    service
        .enqueue(
            job(&mut engine, module, command, Arc::new(AtomicBool::new(true)), &pool),
            &wake,
        )
        .unwrap();
    assert!(receive(&service).live());
}

#[test]
fn command_dispatch_and_all_five_script_methods_use_passive_preparation() {
    use golden_core::{events::EventKind, node::NodeReference, script::ScriptMethodCall};
    let (mut engine, module, command) = fixture();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        engine.run_tick(Duration::from_millis(20)).unwrap();
        let node = engine
            .nodes
            .get(module)
            .unwrap()
            .as_any()
            .downcast_ref::<SoundCardModule>()
            .unwrap();
        if node.runtime.is_some() && node.runtime_request.is_none() {
            break;
        }
        assert!(Instant::now() < deadline, "Null runtime must become available");
        std::thread::sleep(Duration::from_millis(1));
    }
    let snapshot = engine.process_tree_snapshot();
    let module_uuid = snapshot.node(module).unwrap().uuid;
    let target = snapshot.find_child_by_decl_id(command, "target_module").unwrap();
    let master = super::super::find_path(&snapshot, module, "parameters/output/master_gain_db").unwrap();
    let outputs = super::super::find_path(&snapshot, module, super::super::OUTPUT_CHANNELS_PATH).unwrap();
    let channel = snapshot.child_ids_slice(outputs)[0];
    let channel_uuid = snapshot.node(channel).unwrap().uuid;
    engine.edits.push(Edit::SetParam {
        node: target,
        value: ParamValue::Reference(NodeReference::new(module_uuid)),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.edits.push(Edit::EmitCustomEvent {
        event: CustomEvent::new(
            crate::app::module_command::MODULE_COMMAND_EXECUTE_TOPIC,
            Some(command),
            serde_json::json!({"command_id":command}),
        ),
    });
    for (method, args) in [
        (
            "playFile",
            vec![
                ParamValue::Str("missing-c01-fixture.wav".into()),
                ParamValue::Str("c01-test".into()),
            ],
        ),
        ("stopFile", vec![ParamValue::Str("c01-test".into())]),
        ("stopAllFiles", vec![]),
        ("setMasterVolume", vec![ParamValue::Float(-3.0)]),
        ("setMasterVolume", vec![ParamValue::Float(-6.0)]),
        (
            "setChannelVolume",
            vec![
                ParamValue::Reference(NodeReference::new(channel_uuid)),
                ParamValue::Float(-12.0),
            ],
        ),
    ] {
        engine.edits.push(Edit::CallNodeScriptMethod {
            node: module,
            call: ScriptMethodCall::prepare(method, args).unwrap(),
        });
    }
    engine.apply_edits().unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        engine.run_tick(Duration::from_millis(20)).unwrap();
        let snapshot = engine.process_tree_snapshot();
        let result = engine
            .ui_events_since(None)
            .into_iter()
            .find_map(|event| match event.kind {
                EventKind::Custom(event) if event.topic == SOUND_CARD_COMMAND_RESULT_TOPIC => event
                    .payload_as::<SoundCardCommandResultEvent>()
                    .ok()
                    .filter(|result| result.command_id == command),
                _ => None,
            });
        if snapshot.node(master).unwrap().param_value == Some(ParamValue::Float(-6.0))
            && snapshot.node(channel).unwrap().param_value == Some(ParamValue::Float(-12.0))
            && result
                .as_ref()
                .is_some_and(|result| result.admitted_sequence.is_some() && result.error.is_none())
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "passive dispatch: master={:?}, channel={:?}, result={result:?}",
            snapshot.node(master).unwrap().param_value,
            snapshot.node(channel).unwrap().param_value
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    let results = engine
        .ui_events_since(None)
        .into_iter()
        .filter(|event| {
            matches!(&event.kind,
        EventKind::Custom(event) if event.topic == SOUND_CARD_COMMAND_RESULT_TOPIC)
        })
        .count();
    assert_eq!(results, 1, "the command result must publish once across receipt turns");
}
