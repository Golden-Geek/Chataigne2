use super::*;
use crate::app::{
    AppEngine, DmxSendFrameCommand,
    module_command::{ModuleCommandDeliveryPolicy, ModuleCommandExecuteEvent},
    module_modules_protocol_dmx::tests::service::{base, fixture, receive, receiver, set, settle, turn},
};
use golden_core::{
    edit::Edit,
    events::CustomEvent,
    node::{Node, NodeId},
    parameter::ParamValue,
    script::ScriptMethodCall,
};

#[test]
fn continuous_full_frames_use_bounded_actor_turns_and_settle_without_polling() {
    let destination = receiver();
    let (mut engine, id) = fixture(&destination);
    let (interface, port, receive_enabled) = {
        let node = base(&mut engine, id);
        (
            node.network_interface.id(),
            node.listen_port.id(),
            node.receive_enabled.id(),
        )
    };
    let interface_uuid = engine.nodes.get(interface).unwrap().node_data().meta.uuid;
    let options = ["any", "127.0.0.1"].map(|value| golden_core::parameter::ParameterEnumOption {
        variant_id: value.into(),
        value: ParamValue::Enum(value.into()),
        label: value.into(),
        tags: vec![],
        ordering: None,
    });
    engine.edits.push(Edit::SetParamEnumOptions {
        node: interface,
        expected_uuid: interface_uuid,
        expected_value: ParamValue::Enum("any".into()),
        selected_value: None,
        options: options.to_vec().into(),
    });
    engine.apply_edits().unwrap();
    set(&mut engine, interface, ParamValue::Enum("127.0.0.1".into()));
    set(
        &mut engine,
        port,
        ParamValue::Int(i32::from(destination.local_addr().unwrap().port())),
    );
    set(&mut engine, receive_enabled, ParamValue::Bool(true));
    // Apply callbacks without a worker turn; the fake takes over before any open.
    engine
        .dispatch_inbox(golden_core::process_ctx::ExecutionPhase::EngineTick)
        .unwrap();
    engine.apply_edits().unwrap();
    let lifetimes = slots();
    let control = Control::default();
    control.continuous.store(true, Ordering::Release);
    attach(&mut engine, id, &lifetimes, pool(8), control.clone(), None, None);
    let mut samples = Vec::with_capacity(256);
    let mut published = 0;
    for _ in 0..256 {
        let started = Instant::now();
        turn(&mut engine);
        samples.push(started.elapsed().as_micros());
        assert!(engine.tick_stats().wake_callbacks_fired <= 1);
        published += engine.tick_stats().wake_callbacks_fired;
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(published > 10, "continuous input must exercise publication");
    assert!(control.reads.load(Ordering::Acquire) > 10);
    samples.sort_unstable();
    eprintln!(
        "DMX controlled continuous input: 256 full engine turns, {published} wake turns, p95={}us max={}us, CPU={} logical processors",
        samples[243],
        samples[255],
        std::thread::available_parallelism().unwrap()
    );
    assert!(samples[255] < 100_000, "small controlled fixture exceeded 100 ms");
    control.continuous.store(false, Ordering::Release);
    settle(&mut engine, id);
    for _ in 0..100 {
        turn(&mut engine);
        assert_eq!(engine.tick_stats().wake_callbacks_fired, 0);
        assert_eq!(engine.tick_stats().snapshot_rebuilds, 0);
    }
    base(&mut engine, id).stop_transport();
    assert!(lifetimes.wait_for_idle(Duration::from_secs(1)));
}

fn attach(
    engine: &mut AppEngine,
    id: NodeId,
    lifetimes: &RetirementPool,
    output: PayloadPool,
    control: Control,
    send_gate: Option<Gate>,
    cleanup: Option<Gate>,
) {
    let node = base(engine, id);
    let config = node.transport_config().unwrap();
    node.stop_transport();
    let wake = node.wake.as_ref().unwrap().notifier.clone();
    let handle = DmxTransportHandle::spawn_using(config.clone(), wake, lifetimes, output, pool(8), move |_| {
        Ok(Fake {
            control,
            send_gate,
            cleanup,
        })
    })
    .unwrap();
    until(|| handle.is_ready());
    while handle.take_status().is_some() {}
    let node = base(engine, id);
    node.transport = Some(handle);
    node.last_transport_config = Some(config);
    node.transport_dirty = false;
}
fn script(engine: &mut AppEngine, id: NodeId, text: &str) {
    engine.edits.push(Edit::CallNodeScriptMethod {
        node: id,
        call: ScriptMethodCall::prepare("sendFrame", vec![ParamValue::Str(text.into())]).unwrap(),
    });
    engine.apply_edits().unwrap();
}
fn pump(engine: &mut AppEngine, mut done: impl FnMut(&mut AppEngine) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !done(engine) {
        turn(engine);
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn failure_recovers_while_cleanup_is_blocked(panic: bool) {
    let destination = receiver();
    let (mut engine, id) = fixture(&destination);
    let lifetimes = slots();
    let control = Control::default();
    control.panic.store(panic, Ordering::Release);
    control.fail.store(!panic, Ordering::Release);
    let (entered, entered_rx) = mpsc::sync_channel(1);
    let (release, cleanup) = mpsc::sync_channel(1);
    attach(
        &mut engine,
        id,
        &lifetimes,
        pool(8),
        control,
        None,
        Some((entered, cleanup)),
    );
    script(&mut engine, id, "[7]");
    pump(&mut engine, |engine| base(engine, id).transport.is_none());
    entered_rx.recv_timeout(Duration::from_secs(1)).unwrap();
    assert_eq!(lifetimes.metrics().active, 1);
    let node = base(&mut engine, id);
    assert!(node.retry_at.is_some());
    assert!(node.pending_sends.is_empty());
    assert!(!node.has_pending_output());
    // Advance the existing retry deadline, keeping recovery on its production path.
    node.retry_at = Some(Instant::now());
    node.request_service();
    pump(&mut engine, |engine| {
        base(engine, id)
            .transport
            .as_ref()
            .is_some_and(DmxTransportHandle::is_ready)
    });
    assert_eq!(lifetimes.metrics().active, 1, "old cleanup still owns its slot");
    script(&mut engine, id, "[44]");
    settle(&mut engine, id);
    assert_eq!(
        receive(&destination)[0],
        44,
        "failed prefix must not replay on recovery"
    );
    release.send(()).unwrap();
    assert!(lifetimes.wait_for_idle(Duration::from_secs(1)));
    base(&mut engine, id).stop_transport();
}
#[test]
fn native_failure_recovers_on_the_actor_before_blocked_cleanup_finishes() {
    failure_recovers_while_cleanup_is_blocked(false);
}
#[test]
fn stopped_native_worker_recovers_on_the_actor_before_blocked_cleanup_finishes() {
    failure_recovers_while_cleanup_is_blocked(true);
}

#[test]
fn full_native_mailbox_retains_the_prepared_request_without_actor_polling_or_replay() {
    let destination = receiver();
    let (mut engine, id) = fixture(&destination);
    let lifetimes = slots();
    let control = Control::default();
    let (entered, entered_rx) = mpsc::sync_channel(1);
    let (release, blocked) = mpsc::sync_channel(1);
    attach(
        &mut engine,
        id,
        &lifetimes,
        pool(128),
        control.clone(),
        Some((entered, blocked)),
        None,
    );
    let handle = base(&mut engine, id).transport.as_ref().unwrap();
    let first = handle.send(DmxFrame::new(1, vec![1]).unwrap()).unwrap();
    entered_rx.recv_timeout(Duration::from_secs(1)).unwrap();
    let queued = (0..64)
        .map(|_| handle.send(DmxFrame::new(1, vec![2]).unwrap()).unwrap())
        .collect::<Vec<_>>();
    script(&mut engine, id, "[42]");
    // Allow preparation to complete, then let the full native queue reject it once.
    for _ in 0..20 {
        turn(&mut engine);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(base(&mut engine, id).has_pending_output());
    assert!(base(&mut engine, id).pending_sends.is_empty());
    assert!(base(&mut engine, id).output_slots.is_empty());
    for _ in 0..100 {
        turn(&mut engine);
        assert_eq!(engine.tick_stats().wake_callbacks_fired, 0);
        assert_eq!(engine.tick_stats().snapshot_rebuilds, 0);
    }
    assert_eq!(control.sent.load(Ordering::Acquire), 0);
    release.send(()).unwrap();
    pump(&mut engine, |engine| {
        !base(engine, id).has_pending_output() && base(engine, id).pending_sends.is_empty()
    });
    assert_eq!(first.status(), SendStatus::Sent);
    assert!(queued.iter().all(|receipt| receipt.status() == SendStatus::Sent));
    assert_eq!(control.sent.load(Ordering::Acquire), 66);
    assert_eq!(base(&mut engine, id).output_slots, vec![42]);
    base(&mut engine, id).stop_transport();
    assert!(lifetimes.wait_for_idle(Duration::from_secs(1)));
}

#[test]
fn output_payload_pressure_keeps_endpoint_and_prepared_source_until_capacity_returns() {
    let destination = receiver();
    let (mut engine, id) = fixture(&destination);
    let lifetimes = slots();
    let output = pool(1);
    let control = Control::default();
    let (entered, entered_rx) = mpsc::sync_channel(1);
    let (release, blocked) = mpsc::sync_channel(1);
    attach(
        &mut engine,
        id,
        &lifetimes,
        output.clone(),
        control.clone(),
        Some((entered, blocked)),
        None,
    );
    base(&mut engine, id)
        .transport
        .as_ref()
        .unwrap()
        .send(DmxFrame::new(1, vec![1]).unwrap())
        .unwrap();
    entered_rx.recv_timeout(Duration::from_secs(1)).unwrap();
    script(&mut engine, id, "[42]");
    for _ in 0..20 {
        turn(&mut engine);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(base(&mut engine, id).transport.as_ref().unwrap().is_ready());
    assert!(base(&mut engine, id).has_pending_output());
    assert!(base(&mut engine, id).retry_at.is_none(), "overload must not reconnect");
    release.send(()).unwrap();
    pump(&mut engine, |engine| {
        !base(engine, id).has_pending_output() && base(engine, id).pending_sends.is_empty()
    });
    assert_eq!(control.sent.load(Ordering::Acquire), 2);
    base(&mut engine, id).stop_transport();
    assert!(lifetimes.wait_for_idle(Duration::from_secs(1)));
    assert!(output.wait_for_idle(Duration::from_secs(1)));
}

#[test]
fn deleting_a_command_after_native_admission_rejects_its_queued_frame_without_killing_endpoint() {
    let destination = receiver();
    let (mut engine, id) = fixture(&destination);
    let tester = engine
        .process_tree_snapshot()
        .find_child_by_decl_id(id, "command_tester")
        .unwrap();
    engine.add_user_item(DmxSendFrameCommand::create().into(), Some(tester));
    for _ in 0..5 {
        turn(&mut engine);
    }
    let command = engine.nodes.get(tester).unwrap().node_data().first_child.unwrap();
    let slots_id = engine
        .process_tree_snapshot()
        .find_child_by_decl_id(command, "slots")
        .unwrap();
    set(&mut engine, slots_id, ParamValue::Str("[7]".into()));
    let lifetimes = slots();
    let control = Control::default();
    let (entered, entered_rx) = mpsc::sync_channel(1);
    let (release, blocked) = mpsc::sync_channel(1);
    attach(
        &mut engine,
        id,
        &lifetimes,
        pool(8),
        control.clone(),
        Some((entered, blocked)),
        None,
    );
    base(&mut engine, id)
        .transport
        .as_ref()
        .unwrap()
        .send(DmxFrame::new(1, vec![1]).unwrap())
        .unwrap();
    entered_rx.recv_timeout(Duration::from_secs(1)).unwrap();
    let execution = ModuleCommandExecuteEvent {
        command_id: command,
        param_overrides: vec![],
        invocation_id: None,
        delivery_policy: ModuleCommandDeliveryPolicy::Standard,
    };
    engine.edits.push(Edit::EmitCustomEvent {
        event: CustomEvent::new(
            crate::app::module_command::MODULE_COMMAND_EXECUTE_TOPIC,
            Some(command),
            serde_json::to_value(execution).unwrap(),
        ),
    });
    engine.apply_edits().unwrap();
    pump(&mut engine, |engine| !base(engine, id).pending_sends.is_empty());
    let receipt = base(&mut engine, id).pending_sends.front().unwrap().clone();
    engine.edits.push(Edit::RemoveNode { node: command });
    engine.apply_edits().unwrap();
    release.send(()).unwrap();
    until(|| receipt.status() != SendStatus::Pending);
    assert_eq!(receipt.status(), SendStatus::NotSent);
    assert_eq!(control.sent.load(Ordering::Acquire), 1);
    assert!(base(&mut engine, id).transport.as_ref().unwrap().is_ready());
    base(&mut engine, id).stop_transport();
    assert!(lifetimes.wait_for_idle(Duration::from_secs(1)));
}
