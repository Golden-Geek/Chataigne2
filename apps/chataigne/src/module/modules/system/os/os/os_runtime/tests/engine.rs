use super::*;
use crate::app::{AppEngine, AppNode, OsModule, OsWakeOnLanCommand};
use golden_core::{
    edit::Edit,
    engine::EngineEditError,
    node::{NodeId, NodeMetaPatch},
    parameter::{ParamValue, ParameterEventBehaviour},
    process_ctx::{ExecutionPhase, ProcessCtx},
};

fn module(engine: &mut AppEngine, id: NodeId) -> &mut OsModule {
    let AppNode::OsModule(module) = engine.nodes.get_mut(id).unwrap() else {
        panic!()
    };
    module
}
fn create() -> (AppEngine, NodeId) {
    let mut engine = AppEngine::new(Folder::new("root").into());
    engine.add_node(OsModule::create().into(), None);
    for _ in 0..8 {
        engine.apply_edits().unwrap();
    }
    engine.resolve().unwrap();
    let id = engine.nodes.get(engine.root).unwrap().node_data().first_child.unwrap();
    (engine, id)
}
fn attach(
    engine: &mut AppEngine,
    id: NodeId,
    control: Control,
    slots: &RetirementPool,
    input: PayloadPool,
    cleanup: Option<Gate>,
) {
    let wake = module(engine, id).wake.as_ref().unwrap().notifier.clone();
    module(engine, id).runtime = Some(spawn(control, slots, input, payloads(256), None, cleanup, wake));
}
fn turn(engine: &mut AppEngine) {
    engine.apply_edits().unwrap();
    engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
    engine.apply_edits().unwrap();
    engine.run_tick(Duration::from_millis(1)).unwrap();
    engine.apply_edits().unwrap();
}
fn quiet(engine: &mut AppEngine) {
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut quiet = 0;
    while quiet < 10 {
        turn(engine);
        let stats = engine.tick_stats();
        let pending = engine.nodes.iter().any(|(_, node)| matches!(node, AppNode::OsModule(module) if !module.pending.is_empty() || !module.outgoing.is_empty()));
        quiet = if stats.wake_callbacks_fired == 0 && stats.snapshot_rebuilds == 0 && !pending {
            quiet + 1
        } else {
            0
        };
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn call(engine: &mut AppEngine, id: NodeId, method: &str, args: Vec<ParamValue>) {
    engine.edits.push(Edit::CallNodeScriptMethod {
        node: id,
        call: ScriptMethodCall::prepare(method, args).unwrap(),
    });
    engine.apply_edits().unwrap();
}
fn callback_names(events: &[golden_core::events::Event]) -> Vec<String> {
    events
        .iter()
        .filter_map(|event| {
            let golden_core::events::EventKind::Custom(event) = &event.kind else {
                return None;
            };
            event
                .payload
                .get("callback")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        })
        .collect()
}

#[test]
fn construction_is_inert_and_connected_service_settles_without_scheduled_polling() {
    let node = OsModule::create();
    assert!(node.runtime.is_none());
    assert!(!node.needs_update());
    let (mut engine, id) = create();
    assert!(module(&mut engine, id).runtime.is_none());
    let slots = slots();
    let control = Control::default();
    attach(&mut engine, id, control.clone(), &slots, payloads(16), None);
    quiet(&mut engine);
    assert_eq!(control.samples.load(Ordering::Acquire), 1);
    let architecture = module(&mut engine, id).architecture.id();
    assert_eq!(
        engine
            .nodes
            .get(architecture)
            .unwrap()
            .engine_param_snapshot()
            .unwrap()
            .value,
        ParamValue::Str("fake-arch".into())
    );
    for _ in 0..100 {
        turn(&mut engine);
        let stats = engine.tick_stats();
        assert_eq!(stats.callbacks_fired, 0);
        assert_eq!(stats.wake_callbacks_fired, 0);
        assert_eq!(stats.snapshot_rebuilds, 0);
    }
    module(&mut engine, id).runtime = None;
    assert!(slots.wait_for_idle(Duration::from_secs(1)));
}

#[test]
fn ready_outcome_survives_a_snapshot_free_actor_wake() {
    let (mut engine, id) = create();
    let slots = slots();
    let control = Control::default();
    attach(&mut engine, id, control.clone(), &slots, payloads(16), None);
    quiet(&mut engine);
    call(&mut engine, id, "logout", vec![]);
    let deadline = Instant::now() + Duration::from_secs(3);
    while control.actions.lock().unwrap().is_empty() {
        turn(&mut engine);
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    until(|| module(&mut engine, id).pending.front().unwrap().complete());
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    module(&mut engine, id).service(&mut ctx);
    assert_eq!(
        module(&mut engine, id).pending.len(),
        1,
        "result must remain owned until a snapshot is available"
    );
    assert!(ctx.edits.pending.is_empty());
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    ctx.set_tree_snapshot(engine.process_tree_snapshot());
    module(&mut engine, id).service(&mut ctx);
    engine.absorb_edits(&mut ctx).unwrap();
    engine.apply_edits().unwrap();
    assert_eq!(
        callback_names(&engine.inbox.events)
            .iter()
            .filter(|name| name.as_str() == "systemCommandRequested")
            .count(),
        1
    );
    quiet(&mut engine);
    assert!(module(&mut engine, id).pending.is_empty());
    module(&mut engine, id).runtime = None;
    assert!(slots.wait_for_idle(Duration::from_secs(1)));
}

#[test]
fn queued_metrics_hold_original_quota_and_retirement_rejects_all_value_writes() {
    let (mut engine, id) = create();
    let slots = slots();
    let input = payloads(1);
    attach(&mut engine, id, Control::default(), &slots, input.clone(), None);
    let frame = module(&mut engine, id)
        .runtime
        .as_ref()
        .unwrap()
        .take_metrics()
        .unwrap();
    let target = module(&mut engine, id).global_ratio.id();
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    ctx.set_tree_snapshot(engine.process_tree_snapshot());
    module(&mut engine, id).project_metrics(&mut ctx, frame);
    engine.absorb_edits(&mut ctx).unwrap();
    module(&mut engine, id).runtime = None;
    assert!(slots.wait_for_idle(Duration::from_secs(1)));
    assert!(input.try_reserve(1).is_err());
    assert!(matches!(
        engine.apply_edits(),
        Err(EngineEditError::EditPreconditionFailed { .. })
    ));
    assert_eq!(
        engine.nodes.get(target).unwrap().engine_param_snapshot().unwrap().value,
        ParamValue::Float(0.0)
    );
    assert!(input.wait_for_idle(Duration::from_secs(1)));
}

fn recovery_before_cleanup(panic: bool) {
    let (mut engine, id) = create();
    let slots = slots();
    let control = Control::default();
    control.fail_sample.store(!panic, Ordering::Release);
    control.panic_sample.store(panic, Ordering::Release);
    let (entered, entry) = mpsc::sync_channel(1);
    let (release, blocked) = mpsc::sync_channel(1);
    attach(&mut engine, id, control, &slots, payloads(16), Some((entered, blocked)));
    entry.recv_timeout(Duration::from_secs(1)).unwrap();
    turn(&mut engine);
    assert!(module(&mut engine, id).runtime.is_none());
    assert!(module(&mut engine, id).retry_at.is_some());
    assert_eq!(slots.metrics().active, 1);
    // No host-control operation is issued. Release the fake cleanup and install
    // the next controlled provider while preserving the actor's recovery state.
    release.send(()).unwrap();
    assert!(slots.wait_for_idle(Duration::from_secs(1)));
    let recovered = Control::default();
    attach(&mut engine, id, recovered.clone(), &slots, payloads(16), None);
    quiet(&mut engine);
    assert!(module(&mut engine, id).runtime.as_ref().unwrap().is_running());
    assert_eq!(recovered.samples.load(Ordering::Acquire), 1);
    module(&mut engine, id).runtime = None;
    assert!(slots.wait_for_idle(Duration::from_secs(1)));
}

#[test]
fn scripts_and_command_overrides_use_the_same_fake_boundary_and_captured_fields() {
    use crate::app::module_command::{
        ModuleCommandDeliveryPolicy, ModuleCommandExecuteEvent, ModuleCommandParamOverride,
    };
    use golden_core::events::CustomEvent;
    let (mut engine, id) = create();
    let slots = slots();
    let control = Control::default();
    attach(&mut engine, id, control.clone(), &slots, payloads(16), None);
    quiet(&mut engine);
    let tester = engine
        .process_tree_snapshot()
        .find_child_by_decl_id(id, "command_tester")
        .unwrap();
    engine.add_user_item(OsWakeOnLanCommand::create().into(), Some(tester));
    for _ in 0..5 {
        turn(&mut engine);
    }
    let command = engine.nodes.get(tester).unwrap().node_data().first_child.unwrap();
    let mac = engine
        .process_tree_snapshot()
        .find_child_by_decl_id(command, "mac_address")
        .unwrap();
    engine.edits.push(Edit::SetParam {
        node: mac,
        value: ParamValue::Str("x".repeat(1024 * 1024)),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().unwrap();
    let execution = ModuleCommandExecuteEvent {
        command_id: command,
        invocation_id: None,
        delivery_policy: ModuleCommandDeliveryPolicy::Standard,
        param_overrides: vec![
            ModuleCommandParamOverride {
                param_id: mac,
                value: ParamValue::Str("bad".into()),
            },
            ModuleCommandParamOverride {
                param_id: mac,
                value: ParamValue::Str("AA:BB:CC:DD:EE:FF".into()),
            },
        ],
    };
    engine.edits.push(Edit::EmitCustomEvent {
        event: CustomEvent::new(
            crate::app::module_command::MODULE_COMMAND_EXECUTE_TOPIC,
            Some(command),
            serde_json::to_value(execution).unwrap(),
        ),
    });
    engine.apply_edits().unwrap();
    quiet(&mut engine);
    assert_eq!(control.actions.lock().unwrap().len(), 1);
    let Request::WakeOnLan(request) = control.actions.lock().unwrap()[0].clone() else {
        panic!()
    };
    assert_eq!(request.mac_address, "AA:BB:CC:DD:EE:FF");
    call(&mut engine, id, "shutdown", vec![]);
    quiet(&mut engine);
    assert_eq!(control.actions.lock().unwrap().len(), 2);
    engine.edits.push(Edit::PatchMeta {
        node: id,
        patch: NodeMetaPatch {
            enabled: Some(false),
            ..Default::default()
        },
    });
    engine.apply_edits().unwrap();
    assert!(module(&mut engine, id).runtime.is_none());
    assert!(slots.wait_for_idle(Duration::from_secs(1)));
}

#[test]
fn worker_failure_schedules_recovery_before_old_cleanup_releases_its_slot() {
    recovery_before_cleanup(false);
}
#[test]
fn worker_panic_schedules_recovery_before_old_cleanup_releases_its_slot() {
    recovery_before_cleanup(true);
}

#[test]
fn ordinary_tick_preserves_earlier_command_before_later_script_without_manual_dispatch() {
    use crate::app::{
        module_command::{ModuleCommandDeliveryPolicy, ModuleCommandExecuteEvent},
        OsShutdownCommand,
    };
    use golden_core::events::CustomEvent;
    let (mut engine, id) = create();
    let slots = slots();
    let control = Control::default();
    attach(&mut engine, id, control.clone(), &slots, payloads(16), None);
    quiet(&mut engine);
    let tester = engine
        .process_tree_snapshot()
        .find_child_by_decl_id(id, "command_tester")
        .unwrap();
    engine.add_user_item(OsShutdownCommand::create().into(), Some(tester));
    for _ in 0..5 {
        turn(&mut engine);
    }
    let command = engine.nodes.get(tester).unwrap().node_data().first_child.unwrap();
    let execute = ModuleCommandExecuteEvent {
        command_id: command,
        param_overrides: vec![],
        invocation_id: None,
        delivery_policy: ModuleCommandDeliveryPolicy::Standard,
    };
    engine.edits.push(Edit::EmitCustomEvent {
        event: CustomEvent::new(
            crate::app::module_command::MODULE_COMMAND_EXECUTE_TOPIC,
            Some(command),
            serde_json::to_value(execute).unwrap(),
        ),
    });
    call(&mut engine, id, "reboot", vec![]);
    for _ in 0..20 {
        engine.run_tick(Duration::from_millis(1)).unwrap();
        engine.apply_edits().unwrap();
        std::thread::sleep(Duration::from_millis(1));
    }
    quiet(&mut engine);
    let actions = control.actions.lock().unwrap();
    assert!(matches!(
        &actions[..],
        [
            Request::Control(HostControlAction::Shutdown),
            Request::Control(HostControlAction::Reboot)
        ]
    ));
    drop(actions);
    module(&mut engine, id).runtime = None;
    assert!(slots.wait_for_idle(Duration::from_secs(1)));
}

#[test]
fn local_capture_admits_64_sources_and_completed_outcomes_drain_in_sixteen_record_turns() {
    let (mut engine, id) = create();
    let slots = slots();
    let control = Control::default();
    attach(&mut engine, id, control.clone(), &slots, payloads(16), None);
    quiet(&mut engine);
    for _ in 0..64 {
        call(&mut engine, id, "logout", vec![]);
    }
    assert_eq!(module(&mut engine, id).admission.metrics().active, 64);
    engine.edits.push(Edit::CallNodeScriptMethod {
        node: id,
        call: ScriptMethodCall::prepare("logout", vec![]).unwrap(),
    });
    assert!(engine.apply_edits().is_err());
    assert_eq!(module(&mut engine, id).admission.metrics().active, 64);
    assert!(control.actions.lock().unwrap().is_empty());
    engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
    engine.apply_edits().unwrap();
    assert_eq!(module(&mut engine, id).outgoing.len(), 64);
    for _ in 0..8 {
        let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
        module(&mut engine, id).service(&mut ctx);
        engine.absorb_edits(&mut ctx).unwrap();
        engine.apply_edits().unwrap();
    }
    until(|| control.actions.lock().unwrap().len() == 64);
    until(|| module(&mut engine, id).pending.iter().all(Receipt::complete));
    assert_eq!(module(&mut engine, id).pending.len(), 64);
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    ctx.set_tree_snapshot(engine.process_tree_snapshot());
    module(&mut engine, id).service(&mut ctx);
    assert_eq!(module(&mut engine, id).pending.len(), 48);
    engine.absorb_edits(&mut ctx).unwrap();
    engine.apply_edits().unwrap();
    assert_eq!(
        callback_names(&engine.inbox.events)
            .iter()
            .filter(|name| name.as_str() == "systemCommandRequested")
            .count(),
        16
    );
    quiet(&mut engine);
    until(|| module(&mut engine, id).admission.metrics().active == 0);
    assert_eq!(control.actions.lock().unwrap().len(), 64);
    module(&mut engine, id).runtime = None;
    assert!(slots.wait_for_idle(Duration::from_secs(1)));
}

#[test]
fn native_mailbox_byte_limit_precedes_its_item_limit_for_captured_overrides() {
    use crate::app::module_command::{
        CapturedInvocation, ModuleCommandDeliveryPolicy, ModuleCommandExecuteEvent, ModuleCommandParamOverride,
    };
    use golden_core::events::CustomEvent;
    let (mut engine, id) = create();
    let tester = engine
        .process_tree_snapshot()
        .find_child_by_decl_id(id, "command_tester")
        .unwrap();
    engine.add_user_item(OsWakeOnLanCommand::create().into(), Some(tester));
    for _ in 0..5 {
        engine.apply_edits().unwrap();
    }
    let command = engine.nodes.get(tester).unwrap().node_data().first_child.unwrap();
    let snapshot = engine.process_tree_snapshot();
    let execute = ModuleCommandExecuteEvent {
        command_id: command,
        invocation_id: None,
        delivery_policy: ModuleCommandDeliveryPolicy::Standard,
        param_overrides: vec![ModuleCommandParamOverride {
            param_id: NodeId(u64::MAX),
            value: ParamValue::Str("x".repeat(100_000)),
        }],
    };
    let invocation = CapturedInvocation::capture(
        &snapshot,
        command,
        Some(CustomEvent::new(
            crate::app::module_command::MODULE_COMMAND_EXECUTE_TOPIC,
            Some(command),
            serde_json::to_value(execute).unwrap(),
        )),
        &["os_module"],
    )
    .unwrap();
    let slots = slots();
    let control = Control::default();
    let (entered, entry) = mpsc::sync_channel(1);
    let (release, blocked) = mpsc::sync_channel(1);
    let mut runtime = spawn(
        control.clone(),
        &slots,
        payloads(8),
        payloads(256),
        Some((entered, blocked)),
        None,
        WorkerNotifier::new(|| {}),
    );
    let first = super::script(&runtime, &engine, "shutdown", vec![]).unwrap();
    entry.recv_timeout(Duration::from_secs(1)).unwrap();
    let producer = engine.node_waker(command).unwrap();
    let mut receipts = Vec::new();
    loop {
        match runtime.send(Input::Command(invocation.clone()), producer.clone(), Method::WakeOnLan) {
            Ok(receipt) => receipts.push(receipt),
            Err(_) => break,
        }
        assert!(receipts.len() < 64, "byte admission must stop before item admission");
    }
    assert!(receipts.len() > 1 && receipts.len() < 64);
    runtime.stop();
    release.send(()).unwrap();
    drop(runtime);
    assert!(slots.wait_for_idle(Duration::from_secs(1)));
    assert!(first.complete());
    for receipt in receipts {
        assert!(receipt.take().unwrap().err().unwrap().contains("not sent"));
    }
    assert_eq!(control.actions.lock().unwrap().len(), 1);
}

#[test]
fn backoff_escalates_until_a_healthy_metrics_sample_is_published() {
    let (mut engine, id) = create();
    let slots = slots();
    for delay in [250, 500, 1000] {
        let control = Control::default();
        control.fail_sample.store(true, Ordering::Release);
        attach(&mut engine, id, control, &slots, payloads(16), None);
        let before = Instant::now();
        turn(&mut engine);
        assert!(module(&mut engine, id).runtime.is_none());
        assert!(
            module(&mut engine, id)
                .retry_at
                .unwrap()
                .saturating_duration_since(before)
                >= Duration::from_millis(delay)
        );
        assert!(slots.wait_for_idle(Duration::from_secs(1)));
    }
    attach(&mut engine, id, Control::default(), &slots, payloads(16), None);
    quiet(&mut engine);
    module(&mut engine, id).runtime = None;
    assert!(slots.wait_for_idle(Duration::from_secs(1)));
    let control = Control::default();
    control.fail_sample.store(true, Ordering::Release);
    attach(&mut engine, id, control, &slots, payloads(16), None);
    let before = Instant::now();
    turn(&mut engine);
    let delay = module(&mut engine, id)
        .retry_at
        .unwrap()
        .saturating_duration_since(before);
    assert!(delay >= Duration::from_millis(250) && delay < Duration::from_millis(500));
    assert!(slots.wait_for_idle(Duration::from_secs(1)));
}
