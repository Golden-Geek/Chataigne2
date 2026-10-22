use super::*;
use crate::app::module_command::{
    CapturedInvocation, ModuleCommandDeliveryPolicy, ModuleCommandExecuteEvent, ModuleCommandParamOverride,
    MODULE_COMMAND_EXECUTE_TOPIC,
};
use golden_core::events::CustomEvent;

fn command(engine: &mut AppEngine, id: NodeId) -> NodeId {
    let tester = path(engine, id, "command_tester");
    engine.add_user_item(AppControlLaunchProcessCommand::create().into(), Some(tester));
    for _ in 0..8 {
        engine.apply_edits().unwrap();
    }
    let id = engine.nodes.get(tester).unwrap().node_data().first_child.unwrap();
    let mode = path(engine, id, "mode");
    set(engine, mode, ParamValue::Enum("executable".into()));
    engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
    for _ in 0..8 {
        engine.apply_edits().unwrap();
    }
    id
}
fn execution(engine: &AppEngine, command: NodeId, program: &str) -> ModuleCommandExecuteEvent {
    ModuleCommandExecuteEvent {
        command_id: command,
        invocation_id: None,
        delivery_policy: ModuleCommandDeliveryPolicy::Standard,
        param_overrides: vec![
            ModuleCommandParamOverride {
                param_id: path(engine, command, "mode"),
                value: ParamValue::Enum("executable".into()),
            },
            ModuleCommandParamOverride {
                param_id: path(engine, command, "executable_path"),
                value: ParamValue::Str("discarded.exe".into()),
            },
            ModuleCommandParamOverride {
                param_id: path(engine, command, "executable_path"),
                value: ParamValue::Str(program.into()),
            },
        ],
    }
}
fn emit(engine: &mut AppEngine, event: ModuleCommandExecuteEvent) {
    engine.edits.push(Edit::EmitCustomEvent {
        event: CustomEvent::new(
            MODULE_COMMAND_EXECUTE_TOPIC,
            Some(event.command_id),
            serde_json::to_value(event).unwrap(),
        ),
    });
    engine.apply_edits().unwrap();
}
fn programs(control: &Control) -> Vec<String> {
    control
        .actions
        .lock()
        .unwrap()
        .iter()
        .map(|request| match request {
            Request::Launch(value, _) => value.executable_path.clone(),
            _ => panic!("unexpected request"),
        })
        .collect()
}

#[test]
fn ordinary_tick_preserves_command_before_later_script_and_last_override_without_overlay() {
    let (mut engine, id) = create();
    let slots = slots();
    let control = Control::default();
    attach(&mut engine, id, control.clone(), &slots);
    quiet(&mut engine, id);
    let cmd = command(&mut engine, id);
    quiet(&mut engine, id);
    let executable = path(&engine, cmd, "executable_path");
    set(&mut engine, executable, ParamValue::Str("x".repeat(1024 * 1024)));
    let event = execution(&engine, cmd, "earlier.exe");
    emit(&mut engine, event);
    invoke(&mut engine, id, "later.exe");
    for _ in 0..20 {
        engine.run_tick(Duration::from_millis(1)).unwrap();
        engine.apply_edits().unwrap();
        std::thread::sleep(Duration::from_millis(1));
    }
    quiet(&mut engine, id);
    assert_eq!(programs(&control), vec!["earlier.exe", "later.exe"]);
    let ParamValue::File(authored) = value(&engine, executable) else {
        panic!("expected the authored File value");
    };
    assert_eq!(authored.len(), 1024 * 1024);
    assert!(authored.bytes().all(|byte| byte == b'x'));
    finish(&mut engine, id, &slots);
}

#[test]
fn sixty_four_local_sources_hold_admission_and_completed_results_drain_sixteen_per_turn() {
    let (mut engine, id) = create();
    let slots = slots();
    let control = Control::default();
    let (entered, entry) = mpsc::sync_channel(1);
    let (release, blocked) = mpsc::sync_channel(1);
    let mut provider = fake(control.clone());
    provider.execute_gate = Some((entered, blocked));
    install(&mut engine, id, provider, &slots, payloads(32), payloads(256));
    quiet(&mut engine, id);
    for _ in 0..64 {
        invoke(&mut engine, id, "fixture.exe");
    }
    assert_eq!(module(&mut engine, id).service.admission.metrics().active, 64);
    engine.edits.push(Edit::CallNodeScriptMethod {
        node: id,
        call: ScriptMethodCall::prepare("launchApp", vec![ParamValue::Str("refused.exe".into())]).unwrap(),
    });
    assert!(engine.apply_edits().is_err());
    assert!(control.actions.lock().unwrap().is_empty());
    engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
    engine.apply_edits().unwrap();
    assert_eq!(module(&mut engine, id).service.outgoing.len(), 64);
    for _ in 0..8 {
        let mut ctx = context(&engine);
        module(&mut engine, id).service(&mut ctx);
        flush(&mut engine, &mut ctx);
    }
    entry.recv_timeout(Duration::from_secs(1)).unwrap();
    assert_eq!(module(&mut engine, id).service.pending.len(), 64);
    release.send(()).unwrap();
    until(|| control.actions.lock().unwrap().len() == 64);
    until(|| module(&mut engine, id).service.pending.iter().all(Receipt::complete));
    let mut missing = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    module(&mut engine, id).service(&mut missing);
    assert!(missing.edits.pending.is_empty());
    assert_eq!(module(&mut engine, id).service.pending.len(), 64);
    let mut ctx = context(&engine);
    module(&mut engine, id).service(&mut ctx);
    assert_eq!(module(&mut engine, id).service.pending.len(), 48);
    flush(&mut engine, &mut ctx);
    assert_eq!(callbacks(&engine.inbox.events, "appControlCommandRequested"), 16);
    quiet(&mut engine, id);
    until(|| module(&mut engine, id).service.admission.metrics().active == 0);
    assert_eq!(control.actions.lock().unwrap().len(), 64);
    finish(&mut engine, id, &slots);
}

#[test]
fn removing_a_queued_command_prevents_its_effect_while_unrelated_edits_continue() {
    let (mut engine, id) = create();
    let slots = slots();
    let control = Control::default();
    let (entered, entry) = mpsc::sync_channel(1);
    let (release, blocked) = mpsc::sync_channel(1);
    let mut provider = fake(control.clone());
    provider.execute_gate = Some((entered, blocked));
    install(&mut engine, id, provider, &slots, payloads(32), payloads(256));
    quiet(&mut engine, id);
    let cmd = command(&mut engine, id);
    quiet(&mut engine, id);
    invoke(&mut engine, id, "started.exe");
    turn(&mut engine);
    entry.recv_timeout(Duration::from_secs(1)).unwrap();
    let event = execution(&engine, cmd, "removed.exe");
    emit(&mut engine, event);
    turn(&mut engine);
    engine.edits.push(Edit::RemoveNode { node: cmd });
    engine.apply_edits().unwrap();
    let other = Parameter::new("Unrelated", ParamValue::Int(0), ParameterChangeCheck::ValueChange);
    engine.add_node(other.into(), None);
    engine.apply_edits().unwrap();
    let other = engine.nodes.get(engine.root).unwrap().node_data().last_child.unwrap();
    let before = Instant::now();
    for n in 1..=30 {
        set(&mut engine, other, ParamValue::Int(n));
        turn(&mut engine);
    }
    assert!(before.elapsed() < Duration::from_millis(500));
    assert_eq!(value(&engine, other), ParamValue::Int(30));
    assert!(control.actions.lock().unwrap().is_empty());
    release.send(()).unwrap();
    quiet(&mut engine, id);
    assert_eq!(programs(&control), vec!["started.exe"]);
    assert!(module(&mut engine, id).service.runtime.as_ref().unwrap().running());
    finish(&mut engine, id, &slots);
}

#[test]
fn disabling_during_a_started_action_preserves_its_result_and_cancels_the_accepted_suffix() {
    let (mut engine, id) = create();
    let slots = slots();
    let control = Control::default();
    let (entered, entry) = mpsc::sync_channel(1);
    let (release, blocked) = mpsc::sync_channel(1);
    let mut provider = fake(control.clone());
    provider.execute_gate = Some((entered, blocked));
    install(&mut engine, id, provider, &slots, payloads(32), payloads(256));
    quiet(&mut engine, id);
    invoke(&mut engine, id, "started.exe");
    turn(&mut engine);
    entry.recv_timeout(Duration::from_secs(1)).unwrap();
    let started_receipt = module(&mut engine, id).service.pending.pop_front().unwrap();
    for _ in 0..63 {
        invoke(&mut engine, id, "cancelled.exe");
    }
    for _ in 0..8 {
        turn(&mut engine);
    }
    module(&mut engine, id).service.pending.push_front(started_receipt);
    assert_eq!(module(&mut engine, id).service.pending.len(), 64);
    engine.edits.push(Edit::PatchMeta {
        node: id,
        patch: NodeMetaPatch {
            enabled: Some(false),
            ..Default::default()
        },
    });
    engine.apply_edits().unwrap();
    assert!(module(&mut engine, id).service.runtime.is_none());
    release.send(()).unwrap();
    assert!(slots.wait_for_idle(Duration::from_secs(1)));
    let mut ctx = context(&engine);
    module(&mut engine, id).service(&mut ctx);
    flush(&mut engine, &mut ctx);
    assert_eq!(callbacks(&engine.inbox.events, "appControlCommandRequested"), 1);
    assert_eq!(module(&mut engine, id).service.pending.len(), 48);
    until(|| {
        turn(&mut engine);
        module(&mut engine, id).service.pending.is_empty()
    });
    assert_eq!(programs(&control), vec!["started.exe"]);
}

#[test]
fn native_mailbox_bytes_refuse_captured_overrides_before_its_item_limit() {
    let (mut engine, id) = create();
    let cmd = command(&mut engine, id);
    let snapshot = engine.process_tree_snapshot();
    let mut event = execution(&engine, cmd, "ignored.exe");
    event.param_overrides.push(ModuleCommandParamOverride {
        param_id: NodeId(u64::MAX),
        value: ParamValue::Str("x".repeat(100_000)),
    });
    let invocation = CapturedInvocation::capture(
        &snapshot,
        cmd,
        Some(CustomEvent::new(
            MODULE_COMMAND_EXECUTE_TOPIC,
            Some(cmd),
            serde_json::to_value(event).unwrap(),
        )),
        &["app_control_module"],
    )
    .unwrap();
    let slots = slots();
    let control = Control::default();
    let (entered, entry) = mpsc::sync_channel(1);
    let (release, blocked) = mpsc::sync_channel(1);
    let mut provider = fake(control.clone());
    provider.execute_gate = Some((entered, blocked));
    install(&mut engine, id, provider, &slots, payloads(32), payloads(256));
    quiet(&mut engine, id);
    invoke(&mut engine, id, "started.exe");
    turn(&mut engine);
    entry.recv_timeout(Duration::from_secs(1)).unwrap();
    let mut accepted = Vec::new();
    loop {
        let producer = engine.node_waker(cmd).unwrap();
        let time = engine.time;
        let runtime = module(&mut engine, id).service.runtime.as_ref().unwrap();
        let source = runtime
            .capture(
                app_control_runtime::Input::Command(invocation.clone()),
                producer,
                time,
                RetirementPool::new(NonZeroUsize::MIN).try_reserve().unwrap(),
            )
            .unwrap();
        match runtime.submit(source) {
            Ok(receipt) => accepted.push(receipt),
            Err(_) => break,
        }
        assert!(accepted.len() < 64);
    }
    assert!(accepted.len() > 1);
    module(&mut engine, id).service.runtime = None;
    release.send(()).unwrap();
    assert!(slots.wait_for_idle(Duration::from_secs(1)));
    assert!(accepted.iter().all(Receipt::complete));
    assert!(accepted.iter().all(|receipt| receipt.take().unwrap().is_err()));
    assert_eq!(programs(&control), vec!["started.exe"]);
}

#[test]
fn every_script_method_and_authored_running_intent_share_the_native_boundary_without_feedback() {
    let (mut engine, id) = create();
    watch_apps(&mut engine, id, 1);
    let slots = slots();
    let control = Control::default();
    attach(&mut engine, id, control.clone(), &slots);
    quiet(&mut engine, id);
    for (method, args) in [
        ("launchWatchedApp", vec![ParamValue::Str("Watch0".into())]),
        ("launchApp", vec![ParamValue::Str("fixture.exe".into())]),
        ("launchCommandLine", vec![ParamValue::Str("fixture command".into())]),
        ("killProcess", vec![ParamValue::Str("fixture".into())]),
        (
            "controlWindow",
            vec![ParamValue::Str("fixture".into()), ParamValue::Str("minimize".into())],
        ),
    ] {
        call(&mut engine, id, method, args);
    }
    let running = path(&engine, id, "values/watched_apps_values/Watch0/running");
    set(&mut engine, running, ParamValue::Bool(true));
    quiet(&mut engine, id);
    assert_eq!(control.actions.lock().unwrap().len(), 6);
    let samples = control.samples.load(Ordering::Acquire);
    until(|| control.samples.load(Ordering::Acquire) > samples);
    quiet(&mut engine, id);
    assert_eq!(value(&engine, running), ParamValue::Bool(false));
    assert_eq!(control.actions.lock().unwrap().len(), 6);
    assert!(matches!(&control.actions.lock().unwrap()[5],Request::Running(path,true) if path=="fixture/0.exe"));
    finish(&mut engine, id, &slots);
}

#[test]
fn full_native_mailbox_keeps_one_source_behind_an_absolute_retry_deadline_without_busy_wakes() {
    let (mut engine, id) = create();
    let slots = slots();
    let control = Control::default();
    let (entered, entry) = mpsc::sync_channel(1);
    let (release, blocked) = mpsc::sync_channel(1);
    let mut provider = fake(control.clone());
    provider.execute_gate = Some((entered, blocked));
    install(&mut engine, id, provider, &slots, payloads(32), payloads(256));
    quiet(&mut engine, id);
    let args = ScriptMethodCall::prepare("launchApp", vec![ParamValue::Str("external.exe".into())]).unwrap();
    let snapshot = engine.process_tree_snapshot();
    let producer = engine.node_waker(id).unwrap();
    let time = engine.time;
    let admission = RetirementPool::new(NonZeroUsize::new(128).unwrap());
    let capture = |runtime: &AppControlRuntime| {
        runtime
            .capture(
                app_control_runtime::Input::Script {
                    module: id,
                    method: "launchApp",
                    args: args.args().clone(),
                    snapshot: snapshot.try_lease().unwrap(),
                },
                producer.clone(),
                time,
                admission.try_reserve().unwrap(),
            )
            .unwrap()
    };
    let runtime = module(&mut engine, id).service.runtime.as_ref().unwrap();
    let first = runtime.submit(capture(runtime)).ok().unwrap();
    entry.recv_timeout(Duration::from_secs(1)).unwrap();
    let receipts = (0..64)
        .map(|_| runtime.submit(capture(runtime)).ok().unwrap())
        .collect::<Vec<_>>();
    invoke(&mut engine, id, "after-pressure.exe");
    turn(&mut engine);
    let deadline = module(&mut engine, id).service.output_retry_at.unwrap();
    assert_eq!(module(&mut engine, id).service.outgoing.len(), 1);
    for _ in 0..10 {
        turn(&mut engine);
        assert_eq!(module(&mut engine, id).service.output_retry_at, Some(deadline));
        assert_eq!(engine.tick_stats().wake_callbacks_fired, 0);
    }
    release.send(()).unwrap();
    until(|| control.actions.lock().unwrap().len() == 65);
    for _ in 0..10 {
        turn(&mut engine);
        assert_eq!(module(&mut engine, id).service.output_retry_at, Some(deadline));
        assert_eq!(control.actions.lock().unwrap().len(), 65);
    }
    until(|| {
        turn(&mut engine);
        control.actions.lock().unwrap().len() == 66
    });
    quiet(&mut engine, id);
    assert!(module(&mut engine, id).service.outgoing.is_empty());
    assert_eq!(programs(&control).last().unwrap(), "after-pressure.exe");
    assert!(first.complete());
    assert!(receipts.iter().all(Receipt::complete));
    finish(&mut engine, id, &slots);
}

#[test]
fn watched_launch_kill_and_window_commands_accept_only_their_active_fields() {
    let (mut engine, id) = create();
    watch_apps(&mut engine, id, 1);
    let slots = slots();
    let control = Control::default();
    attach(&mut engine, id, control.clone(), &slots);
    quiet(&mut engine, id);
    let tester = path(&engine, id, "command_tester");
    let nodes: Vec<AppNode> = vec![
        AppControlLaunchProcessCommand::create().into(),
        crate::app::AppControlKillProcessCommand::create().into(),
        crate::app::AppControlWindowControlCommand::create().into(),
    ];
    for node in nodes {
        engine.add_user_item(node, Some(tester));
        for _ in 0..8 {
            engine.apply_edits().unwrap();
        }
    }
    quiet(&mut engine, id);
    engine.add_user_item(AppControlLaunchProcessCommand::create().into(), Some(tester));
    for _ in 0..8 {
        engine.apply_edits().unwrap();
    }
    let shell = engine.nodes.get(tester).unwrap().node_data().last_child.unwrap();
    let mode = path(&engine, shell, "mode");
    set(&mut engine, mode, ParamValue::Enum("command_line".into()));
    engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
    for _ in 0..8 {
        engine.apply_edits().unwrap();
    }
    let line = path(&engine, shell, "command_line");
    set(&mut engine, line, ParamValue::Str("fixture shell".into()));
    let commands = engine.process_tree_snapshot().child_ids(tester);
    assert_eq!(commands.len(), 4);
    for command in commands {
        emit(
            &mut engine,
            ModuleCommandExecuteEvent {
                command_id: command,
                invocation_id: None,
                delivery_policy: ModuleCommandDeliveryPolicy::Standard,
                param_overrides: vec![],
            },
        );
    }
    quiet(&mut engine, id);
    let actions = control.actions.lock().unwrap();
    assert_eq!(actions.len(), 4);
    assert!(actions
        .iter()
        .any(|value| matches!(value,Request::Launch(_,Some(path)) if path=="fixture/0.exe")));
    assert!(actions.iter().any(|value| matches!(value,
        Request::Kill(request,Some(path)) if path=="fixture/0.exe"
            &&request.match_mode==crate::app::module::common::app_control::ProcessMatchMode::Exact
    )));
    assert!(actions.iter().any(|value| matches!(value,
        Request::Window(request,Some(path)) if path=="fixture/0.exe"
            &&request.match_mode==crate::app::module::common::app_control::ProcessMatchMode::Exact
    )));
    assert!(actions.iter().any(|value| matches!(value,
        Request::Launch(request,None) if request.mode==crate::app::module::common::app_control::LaunchMode::CommandLine
            &&request.command_line=="fixture shell"
    )));
    drop(actions);
    finish(&mut engine, id, &slots);
}
