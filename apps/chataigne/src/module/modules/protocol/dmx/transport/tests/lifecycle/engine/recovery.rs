use super::*;
use golden_core::node::NodeMetaPatch;

#[test]
fn a_confirmed_look_recovers_before_blocked_cleanup_without_replaying_a_failed_command() {
    for panic in [false, true] {
        let destination = receiver();
        let (mut engine, id) = fixture(&destination);
        let lifetimes = slots();
        let control = Control::default();
        let (entered, cleaning) = mpsc::sync_channel(1);
        let (release, cleanup) = mpsc::sync_channel(1);
        attach(
            &mut engine,
            id,
            &lifetimes,
            pool(8),
            control.clone(),
            None,
            Some((entered, cleanup)),
        );
        script(&mut engine, id, "[33]");
        pump(&mut engine, |engine| !base(engine, id).pending_sends.is_empty());
        let original = base(&mut engine, id).pending_sends.front().unwrap().clone();
        until(|| original.status() == SendStatus::Sent);
        settle(&mut engine, id);
        control.fail.store(!panic, Ordering::Release);
        control.panic.store(panic, Ordering::Release);
        script(&mut engine, id, "[77]");
        pump(&mut engine, |engine| !base(engine, id).pending_sends.is_empty());
        let failed = base(&mut engine, id).pending_sends.front().unwrap().clone();
        pump(&mut engine, |engine| base(engine, id).transport.is_none());
        cleaning.recv_timeout(Duration::from_secs(1)).unwrap();
        assert_eq!(failed.status(), SendStatus::FailedUnconfirmed);
        let started = Instant::now();
        let node = base(&mut engine, id);
        node.retry_at = Some(Instant::now());
        node.request_service();
        pump(&mut engine, |engine| {
            base(engine, id)
                .transport
                .as_ref()
                .is_some_and(DmxTransportHandle::is_ready)
        });
        assert!(started.elapsed() < Duration::from_secs(1));
        assert_eq!(receive(&destination)[0], 33, "recover confirmed state, not failed 77");
        assert_eq!(original.status(), SendStatus::Sent);
        assert_eq!(failed.status(), SendStatus::FailedUnconfirmed);
        assert_eq!(lifetimes.metrics().active, 1, "recovery cannot wait for old cleanup");
        release.send(()).unwrap();
        assert!(lifetimes.wait_for_idle(Duration::from_secs(1)));
        script(&mut engine, id, "[44]");
        settle(&mut engine, id);
        assert_eq!(receive(&destination)[0], 44);
        assert_eq!(failed.status(), SendStatus::FailedUnconfirmed);
        base(&mut engine, id).stop_transport();
    }
}

#[test]
fn reenable_initializes_latest_authored_look_and_idle_actor_remains_passive() {
    let destination = receiver();
    let (mut engine, id) = fixture(&destination);
    engine.edits.push(Edit::PatchMeta {
        node: id,
        patch: NodeMetaPatch {
            enabled: Some(false),
            ..Default::default()
        },
    });
    engine.apply_edits().unwrap();
    let output = base(&mut engine, id).output_frame.id();
    set(&mut engine, output, ParamValue::Bytes(vec![19, 20]));
    engine.edits.push(Edit::PatchMeta {
        node: id,
        patch: NodeMetaPatch {
            enabled: Some(true),
            ..Default::default()
        },
    });
    engine.apply_edits().unwrap();
    pump(&mut engine, |engine| {
        base(engine, id)
            .transport
            .as_ref()
            .is_some_and(DmxTransportHandle::is_ready)
    });
    settle(&mut engine, id);
    assert_eq!(receive(&destination), vec![19, 20]);
    for _ in 0..100 {
        turn(&mut engine);
        assert_eq!(engine.tick_stats().wake_callbacks_fired, 0);
        assert_eq!(engine.tick_stats().snapshot_rebuilds, 0);
    }
    base(&mut engine, id).stop_transport();
}

#[test]
fn reopening_a_saved_module_publishes_its_authored_look_without_an_old_invocation() {
    let destination = receiver();
    let (mut engine, id) = fixture(&destination);
    script(&mut engine, id, "[61,62]");
    settle(&mut engine, id);
    assert_eq!(receive(&destination), vec![61, 62]);
    let uuid = base(&mut engine, id).node_data().meta.uuid;
    base(&mut engine, id).stop_transport();
    let json = golden_core::app::to_sparse_project_json_pretty(&engine).unwrap();
    drop(engine);
    let mut loaded = golden_core::app::from_sparse_project_json::<crate::app::AppNode>(&json).unwrap();
    golden_core::app::prepare_engine_for_runtime(&mut loaded).unwrap();
    let id = loaded
        .nodes
        .iter()
        .find_map(|(id, node)| (node.node_data().meta.uuid == uuid).then_some(id))
        .unwrap();
    pump(&mut loaded, |engine| {
        base(engine, id)
            .transport
            .as_ref()
            .is_some_and(DmxTransportHandle::is_ready)
    });
    settle(&mut loaded, id);
    assert_eq!(receive(&destination), vec![61, 62]);
    assert!(base(&mut loaded, id).pending_sends.is_empty());
    assert!(!base(&mut loaded, id).has_pending_output());
    base(&mut loaded, id).stop_transport();
}

#[test]
fn an_old_inflight_receipt_cannot_suppress_new_authored_initialization() {
    let destination = receiver();
    let (mut engine, id) = fixture(&destination);
    let lifetimes = slots();
    let control = Control::default();
    let (entered, sending) = mpsc::sync_channel(1);
    let (release, blocked) = mpsc::sync_channel(1);
    attach(
        &mut engine,
        id,
        &lifetimes,
        pool(8),
        control,
        Some((entered, blocked)),
        None,
    );
    script(&mut engine, id, "[77]");
    pump(&mut engine, |engine| !base(engine, id).pending_sends.is_empty());
    let old = base(&mut engine, id).pending_sends.front().unwrap().clone();
    sending.recv_timeout(Duration::from_secs(1)).unwrap();
    engine.edits.push(Edit::PatchMeta {
        node: id,
        patch: NodeMetaPatch {
            enabled: Some(false),
            ..Default::default()
        },
    });
    engine.apply_edits().unwrap();
    let output = base(&mut engine, id).output_frame.id();
    set(&mut engine, output, ParamValue::Bytes(vec![9, 10]));
    engine.edits.push(Edit::PatchMeta {
        node: id,
        patch: NodeMetaPatch {
            enabled: Some(true),
            ..Default::default()
        },
    });
    engine.apply_edits().unwrap();
    pump(&mut engine, |engine| {
        base(engine, id)
            .transport
            .as_ref()
            .is_some_and(DmxTransportHandle::is_ready)
    });
    // The old provider is still blocked; its receipt must remain truthful and distinct.
    pump(&mut engine, |engine| base(engine, id).output_slots == vec![9, 10]);
    assert_eq!(receive(&destination), vec![9, 10]);
    assert_eq!(old.status(), SendStatus::Pending);
    release.send(()).unwrap();
    assert!(lifetimes.wait_for_idle(Duration::from_secs(1)));
    settle(&mut engine, id);
    assert_eq!(old.status(), SendStatus::Sent);
    assert_eq!(base(&mut engine, id).output_frame.get_ref().as_slice(), &[9, 10]);
    base(&mut engine, id).stop_transport();
}

fn blocked_loopback_input() -> (UdpSocket, AppEngine, NodeId) {
    let blocker = receiver();
    let (mut engine, id) = fixture(&blocker);
    let interface = base(&mut engine, id).network_interface.id();
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
    let port = base(&mut engine, id).listen_port.id();
    let receive = base(&mut engine, id).receive_enabled.id();
    set(
        &mut engine,
        port,
        ParamValue::Int(i32::from(blocker.local_addr().unwrap().port())),
    );
    set(&mut engine, receive, ParamValue::Bool(true));
    engine
        .dispatch_inbox(golden_core::process_ctx::ExecutionPhase::EngineTick)
        .unwrap();
    engine.apply_edits().unwrap();
    let config = base(&mut engine, id).transport_config().unwrap();
    assert_eq!(
        SocketAddr::new(config.bind_ip, config.listen_port),
        blocker.local_addr().unwrap()
    );
    assert!(config.receive_enabled);
    assert!(
        UdpSocket::bind(blocker.local_addr().unwrap()).is_err(),
        "fixture must hold the exact native bind tuple"
    );
    (blocker, engine, id)
}

#[test]
fn socket_open_failures_back_off_to_the_cap_and_reset_only_after_native_readiness() {
    let (blocker, mut engine, id) = blocked_loopback_input();
    for expected in [500, 1000, 2000, 4000, 8000, 8000, 8000] {
        pump(&mut engine, |engine| {
            let node = base(engine, id);
            node.transport.is_none() && node.retry_at.is_some()
        });
        assert_eq!(
            base(&mut engine, id).reconnect.next_delay(),
            Duration::from_millis(expected)
        );
        let node = base(&mut engine, id);
        node.retry_at = Some(Instant::now());
        node.request_service();
        // Consume the forced retry before the next failure predicate can see the old deadline.
        turn(&mut engine);
    }
    drop(blocker);
    pump(&mut engine, |engine| {
        let node = base(engine, id);
        if node.transport.is_none() {
            node.retry_at = Some(Instant::now());
            node.request_service();
        }
        node.transport.as_ref().is_some_and(DmxTransportHandle::is_ready)
    });
    settle(&mut engine, id);
    assert_eq!(base(&mut engine, id).reconnect.next_delay(), Duration::from_millis(250));
    assert!(base(&mut engine, id).retry_at.is_none());
    base(&mut engine, id).stop_transport();
}

#[test]
fn an_earlier_coalesced_wake_cannot_lose_the_actual_connection_retry_deadline() {
    let (blocker, mut engine, id) = blocked_loopback_input();
    pump(&mut engine, |engine| {
        let node = base(engine, id);
        node.transport.is_none() && node.retry_at.is_some()
    });
    let retry = base(&mut engine, id).retry_at.unwrap();
    // Simulate an obsolete output wake superseding the longer connection delay.
    base(&mut engine, id).wake.as_ref().unwrap().later(Duration::ZERO);
    drop(blocker);
    // Drive ordinary engine ticks; do not advance retry_at or publish another wake.
    pump(&mut engine, |engine| {
        base(engine, id)
            .transport
            .as_ref()
            .is_some_and(DmxTransportHandle::is_ready)
    });
    assert!(Instant::now() >= retry, "an early wake must not reconnect prematurely");
    settle(&mut engine, id);
    assert!(base(&mut engine, id).retry_at.is_none());
    assert_eq!(base(&mut engine, id).reconnect.next_delay(), Duration::from_millis(250));
    base(&mut engine, id).stop_transport();
}
