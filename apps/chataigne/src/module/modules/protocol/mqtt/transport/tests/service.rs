use super::super::super::{
    tests::{create_mqtt_module, find_path},
    MqttModule,
};
use super::*;
use crate::app::{AppEngine, AppNode};
use golden_core::{
    edit::Edit,
    events::EventKind,
    node::{Node, NodeMetaPatch},
    parameter::{ParamValue, Parameter, ParameterChangeCheck, ParameterEventBehaviour},
    process_ctx::{ExecutionPhase, ProcessCtx},
    script::ScriptMethodCall,
};

mod recovery;

fn module(engine: &mut AppEngine, id: NodeId) -> &mut MqttModule {
    let AppNode::MqttModule(module) = engine.nodes.get_mut(id).unwrap() else {
        unreachable!()
    };
    module
}

fn attach(engine: &mut AppEngine, id: NodeId, control: Control, lifetimes: &RetirementPool, port: u16) {
    // Drain authored setup while disabled. Keep even an unexpected native start
    // confined to a listener owned by the fixture, never the default broker port.
    engine.edits.push(Edit::PatchMeta {
        node: id,
        patch: NodeMetaPatch {
            enabled: Some(false),
            ..Default::default()
        },
    });
    let remote_port = find_path(engine, id, "connection/remote_port").unwrap();
    engine.edits.push(Edit::SetParam {
        node: remote_port,
        value: ParamValue::Int(i32::from(port)),
        behaviour: ParameterEventBehaviour::Append,
    });
    engine.apply_edits().unwrap();
    quiet(engine);
    engine.edits.push(Edit::PatchMeta {
        node: id,
        patch: NodeMetaPatch {
            enabled: Some(true),
            ..Default::default()
        },
    });
    engine.apply_edits().unwrap();
    engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
    engine.apply_edits().unwrap();
    let snapshot = engine.process_tree_snapshot();
    module(engine, id).disable_transport_for_test(&snapshot);
    module(engine, id).last_transport_config = None;
    let notifier = module(engine, id).wake.as_ref().unwrap().notifier.clone();
    let handle = MqttTransportHandle::spawn_using(config(), id, notifier, lifetimes, pool(256), pool(256), move |_| {
        FakeBroker::new(control)
    })
    .unwrap();
    until(|| handle.is_connected());
    let node = module(engine, id);
    node.transport = Some(handle);
    node.incoming.activate();
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
        quiet = if stats.wake_callbacks_fired == 0 && stats.snapshot_rebuilds == 0 {
            quiet + 1
        } else {
            0
        };
        assert!(Instant::now() < deadline, "MQTT did not settle passively: {stats:?}");
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn publish(engine: &mut AppEngine, id: NodeId, text: &str) {
    engine.edits.push(Edit::CallNodeScriptMethod {
        node: id,
        call: ScriptMethodCall::prepare(
            "publishText",
            vec![
                ParamValue::Str("test/value".into()),
                ParamValue::Str(text.into()),
                ParamValue::Int(1),
            ],
        )
        .unwrap(),
    });
    engine.apply_edits().unwrap();
}

fn outgoing(events: &[golden_core::events::Event]) -> usize {
    events.iter().filter(|event| matches!(&event.kind, EventKind::Custom(custom) if custom.topic == crate::app::module::MODULE_OUTGOING_TRAFFIC_EVENT_TOPIC)).count()
}

#[test]
fn pending_receipts_are_idle_and_out_of_order_completions_rotate_in_bounded_snapshot_free_turns() {
    let (mut engine, id) = create_mqtt_module();
    let control = Control::default();
    let lifetimes = slots();
    let endpoint = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    attach(
        &mut engine,
        id,
        control.clone(),
        &lifetimes,
        endpoint.local_addr().unwrap().port(),
    );
    for _ in 0..40 {
        publish(&mut engine, id, "pending");
    }
    until(|| control.published.load(Ordering::Acquire) == 40);
    let receipts = module(&mut engine, id)
        .pending_publishes
        .iter()
        .map(|(receipt, _)| receipt.clone())
        .collect::<Vec<_>>();
    quiet(&mut engine);
    for _ in 0..100 {
        turn(&mut engine);
        let stats = engine.tick_stats();
        assert_eq!(stats.callbacks_fired, 0);
        assert_eq!(stats.wake_callbacks_fired, 0);
        assert_eq!(stats.snapshot_rebuilds, 0);
    }
    for pkid in 21..=40 {
        control.event(Event::Incoming(Packet::PubAck(rumqttc::PubAck { pkid })));
    }
    until(|| {
        receipts[20..]
            .iter()
            .all(|receipt| receipt.status() == PublishStatus::Acknowledged)
    });
    let mut traffic = 0;
    for _ in 0..5 {
        let time = engine.time;
        let node = module(&mut engine, id);
        let before = node.pending_publishes.len();
        assert!(!node.worker_wake_requires_tree_snapshot());
        let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, time);
        node.on_worker_wake(&mut ctx);
        assert!(before - node.pending_publishes.len() <= EVENT_TURN);
        assert!(ctx.tree_snapshot().is_none());
        engine.absorb_edits(&mut ctx).unwrap();
        engine.apply_edits().unwrap();
        traffic += outgoing(&engine.inbox.events);
        engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
    }
    assert_eq!(traffic, 20);
    assert_eq!(module(&mut engine, id).pending_publishes.len(), 20);
    quiet(&mut engine);
    module(&mut engine, id).stop_transport();
    assert!(lifetimes.wait_for_idle(Duration::from_secs(1)));
}

#[test]
fn continuous_input_preserves_every_callback_and_allows_unrelated_edits_between_bounded_turns() {
    const COUNT: usize = 256;
    let (mut engine, id) = create_mqtt_module();
    engine.add_node(
        Parameter::new("progress", ParamValue::Int(0), ParameterChangeCheck::ValueChange).into(),
        None,
    );
    engine.apply_edits().unwrap();
    let marker = *engine
        .process_tree_snapshot()
        .child_ids_slice(engine.root)
        .last()
        .unwrap();
    let control = Control::default();
    let lifetimes = slots();
    let endpoint = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    attach(
        &mut engine,
        id,
        control.clone(),
        &lifetimes,
        endpoint.local_addr().unwrap().port(),
    );
    quiet(&mut engine);
    control.incoming.store(COUNT, Ordering::Release);
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut received = Vec::new();
    let mut timings = Vec::new();
    let mut after = Some(engine.time);
    while received.len() < COUNT {
        let index = timings.len() as i32 + 1;
        engine.edits.push(Edit::SetParam {
            node: marker,
            value: ParamValue::Int(index),
            behaviour: ParameterEventBehaviour::Append,
        });
        let started = Instant::now();
        turn(&mut engine);
        timings.push(started.elapsed().as_micros());
        assert_eq!(
            engine.nodes.get(marker).unwrap().engine_param_snapshot().unwrap().value,
            ParamValue::Int(index)
        );
        let node = module(&mut engine, id);
        assert!(node.incoming.len() <= 64);
        let metrics = node.transport.as_ref().unwrap().worker.event_metrics();
        assert!(metrics.depth <= EVENT_ITEMS && metrics.retained_weight <= 4 * 1024 * 1024);
        assert_eq!(engine.tick_stats().callbacks_fired, 0);
        assert!(engine.tick_stats().edits_applied <= 512);
        let events = engine.ui_events_since(after);
        after = events.last().map(|event| event.time).or(after);
        for event in &events {
            if let EventKind::Custom(custom) = &event.kind {
                if custom.topic == crate::app::module::script_api::MODULE_SCRIPT_CALLBACK_TOPIC
                    && custom.payload["callback"] == "messageReceived"
                {
                    received.push(custom.payload["args"][1].as_str().unwrap().parse::<usize>().unwrap());
                }
            }
        }
        assert!(
            Instant::now() < deadline,
            "received {}/{} callbacks",
            received.len(),
            COUNT
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(received, (0..COUNT).rev().collect::<Vec<_>>());
    let value = find_path(&engine, id, "values/stream").unwrap();
    assert_eq!(
        engine.nodes.get(value).unwrap().engine_param_snapshot().unwrap().value,
        ParamValue::Int(0)
    );
    timings.sort_unstable();
    println!(
        "mqtt_actor_load messages={COUNT} turns={} p95_us={} max_us={}",
        timings.len(),
        timings[(timings.len() - 1) * 95 / 100],
        timings.last().unwrap()
    );
    quiet(&mut engine);
    module(&mut engine, id).stop_transport();
    assert!(lifetimes.wait_for_idle(Duration::from_secs(1)));
}
