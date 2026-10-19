use super::super::{
    lifecycle::HttpTransports,
    transport::{HttpOutcome, HttpTransportHandle},
};
use super::transport::{config, request};
use super::*;
use golden_core::{edit::Edit, engine::NodeExecutionRule};
use golden_io::{BoundedWorker, RetirementPool, WorkerLimits};
use std::{num::NonZeroUsize, sync::mpsc, thread, time::Instant};

fn module(engine: &crate::app::AppEngine, id: NodeId) -> &HttpModule {
    let crate::app::AppNode::HttpModule(module) = engine.nodes.get(id).unwrap() else {
        panic!("HTTP")
    };
    module
}

fn response_callbacks(engine: &crate::app::AppEngine, id: NodeId) -> usize {
    engine.ui_event_log().iter().filter(|event| matches!(&event.kind, EventKind::Custom(event)
        if event.origin == Some(id) && event.payload.get("callback").and_then(serde_json::Value::as_str) == Some("responseReceived"))).count()
}

#[test]
fn idle_http_has_no_periodic_node_callbacks_or_compiled_kernel() {
    let (mut engine, id) = create_http_module();
    assert_eq!(module(&engine, id).execution_rule(), NodeExecutionRule::passive());
    assert!(!module(&engine, id).needs_update());
    for _ in 0..20 {
        run_http_runtime_ticks(&mut engine, 1);
        assert_eq!(engine.tick_stats().callbacks_fired, 0);
    }
}

#[test]
fn worker_publication_wakes_passive_graph_and_callback_delivery_exactly_once() {
    let (mut engine, id) = create_http_module();
    let pool = RetirementPool::new(NonZeroUsize::new(2).unwrap());
    let (thread_tx, thread_rx) = mpsc::channel();
    let crate::app::AppNode::HttpModule(node) = engine.nodes.get_mut(id).unwrap() else {
        panic!("HTTP")
    };
    node.transports = HttpTransports::with_lifetimes(pool);
    let notifier = node.wake.as_ref().unwrap().notifier.clone();
    node.transports
        .configure_using(
            config("http://127.0.0.1/".into()),
            notifier,
            |config, pool, previous, notifier| {
                HttpTransportHandle::spawn_using(config, pool, previous, notifier, move |_, _, request, _, _| {
                    thread_tx.send(thread::current().id()).unwrap();
                    Some(HttpOutcome::Response(test_response(
                        &request.payload.path,
                        Some("application/json"),
                        br#"{"level":7}"#.to_vec(),
                    )))
                })
            },
        )
        .unwrap();
    node.transport_dirty = false;
    let mut request = request();
    request.payload.path = "wake".into();
    node.send_for_test(request).unwrap();
    assert_ne!(
        thread_rx.recv_timeout(Duration::from_secs(2)).unwrap(),
        thread::current().id()
    );
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut wakes = 0;
    loop {
        run_http_runtime_ticks(&mut engine, 1);
        wakes += engine.tick_stats().wake_callbacks_fired;
        assert_eq!(engine.tick_stats().callbacks_fired, 0);
        if let Some(value) = find_path(&engine, id, "values/wake/level") {
            if param_value(&engine, value) == ParamValue::Int(7) && module(&engine, id).pending_events.is_empty() {
                break;
            }
        }
        assert!(
            Instant::now() < deadline,
            "worker wake must advance graph/callback receipts"
        );
        thread::sleep(Duration::from_millis(1));
    }
    assert!(wakes > 0);
    assert_eq!(response_callbacks(&engine, id), 1);
    for _ in 0..10 {
        run_http_runtime_ticks(&mut engine, 1);
    }
    assert_eq!(response_callbacks(&engine, id), 1);
    assert_eq!(engine.tick_stats().callbacks_fired, 0);
}

#[test]
fn retry_deadline_recovers_lifetime_capacity_without_another_edit_or_periodic_service() {
    let (mut engine, id) = create_http_module();
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    let (release_tx, release_rx) = mpsc::channel();
    let blocker = BoundedWorker::<(), ()>::spawn(
        "http-capacity-blocker",
        &pool,
        WorkerLimits {
            command_items: 1,
            command_weight: 1,
            event_items: 1,
            event_weight: 1,
        },
        move |_| {
            release_rx.recv_timeout(Duration::from_secs(3)).unwrap();
        },
    )
    .unwrap();
    let crate::app::AppNode::HttpModule(node) = engine.nodes.get_mut(id).unwrap() else {
        panic!("HTTP")
    };
    node.transports = HttpTransports::with_lifetimes(pool);
    node.transport_dirty = true;
    node.request_service();
    let deadline = Instant::now() + Duration::from_secs(3);
    while module(&engine, id).retry_at.is_none() {
        run_http_runtime_ticks(&mut engine, 1);
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(1));
    }
    release_tx.send(()).unwrap();
    drop(blocker);
    while module(&engine, id).transport_dirty {
        run_http_runtime_ticks(&mut engine, 1);
        assert_eq!(engine.tick_stats().callbacks_fired, 0);
        assert!(Instant::now() < deadline, "deadline must retry after capacity returns");
        thread::sleep(Duration::from_millis(2));
    }
    assert!(module(&engine, id).retry_at.is_none());
}

#[test]
fn destroy_undo_discards_runtime_input_and_fences_the_old_wake_source() {
    let (mut engine, id) = create_http_module();
    let crate::app::AppNode::HttpModule(node) = engine.nodes.get_mut(id).unwrap() else {
        panic!("HTTP")
    };
    node.enqueue_response_for_test(test_response(
        "stale",
        Some("application/json"),
        br#"{"value":9}"#.to_vec(),
    ));
    let old = node.wake.as_ref().unwrap().notifier.clone();
    engine.edits.push(Edit::RemoveNode { node: id });
    engine.apply_edits().unwrap();
    assert!(engine.undo().unwrap());
    assert!(module(&engine, id).pending_events.is_empty());
    for _ in 0..5 {
        run_http_runtime_ticks(&mut engine, 1);
    }
    old.notify();
    run_http_runtime_ticks(&mut engine, 1);
    assert_eq!(
        engine.tick_stats().wake_callbacks_fired,
        0,
        "removed source must not wake the restored owner"
    );
    assert!(find_path(&engine, id, "values/stale").is_none());
    assert_eq!(response_callbacks(&engine, id), 0);
    assert_eq!(engine.tick_stats().callbacks_fired, 0);
}
