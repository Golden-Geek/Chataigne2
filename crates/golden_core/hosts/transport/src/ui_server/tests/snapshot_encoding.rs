use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::time::Duration;

use golden_engine::engine::{Engine, EngineTime};
use golden_engine::node::Folder;
use golden_engine::ui_read_model::UiReadModel;
use golden_protocol::{UiProjectFileSpec, UiServerMessage, UiSubscriptionScope};

use super::super::snapshot_encoding::{
    SnapshotEncodingAdmissionError, SnapshotEncodingResult, SnapshotEncodingService, encode_websocket_message,
};
use super::super::{QueuePushResult, WsOutbound, WsOutboundQueue};

#[test]
fn whole_graph_encoding_reuses_one_bounded_completed_payload() {
    let engine = Engine::new(Folder::new("root"));
    let read_model = UiReadModel::from_engine(&engine, UiProjectFileSpec::default());
    let service = SnapshotEncodingService::spawn().expect("snapshot encoder should start");

    let first = service
        .try_encode(read_model.capture_snapshot(UiSubscriptionScope::WholeGraph))
        .expect("first snapshot should be admitted")
        .recv_timeout(Duration::from_secs(5))
        .expect("first snapshot worker should reply")
        .expect("first snapshot should encode");
    let second = service
        .try_encode(read_model.capture_snapshot(UiSubscriptionScope::WholeGraph))
        .expect("second snapshot should be admitted")
        .recv_timeout(Duration::from_secs(5))
        .expect("second snapshot worker should reply")
        .expect("second snapshot should encode");

    assert!(!first.cache_hit);
    assert!(second.cache_hit);
    assert!(Arc::ptr_eq(&first.encoded, &second.encoded));
    assert_eq!(first.revision, second.revision);
    assert_eq!(first.project_generation, second.project_generation);
    let metrics = service.metrics();
    assert_eq!(metrics.materialized, 1);
    assert_eq!(metrics.cache_hits, 1);
    assert_eq!(metrics.retained_cache_bytes, first.encoded.len());
}

#[test]
fn three_contended_clients_keep_captures_coherent_and_admission_bounded() {
    let first_job = Arc::new(AtomicBool::new(true));
    let (entered_tx, entered_rx) = mpsc::sync_channel(1);
    let release = Arc::new((Mutex::new(false), Condvar::new()));
    let hook = {
        let first_job = first_job.clone();
        let release = release.clone();
        Arc::new(move || {
            if !first_job.swap(false, Ordering::AcqRel) {
                return;
            }
            entered_tx.send(()).expect("test should observe blocked encoder");
            let (lock, wake) = &*release;
            let mut released = lock.lock().expect("release gate poisoned");
            while !*released {
                released = wake.wait(released).expect("release gate poisoned");
            }
        })
    };
    let service = SnapshotEncodingService::spawn_with_test_hook(2, hook).expect("snapshot encoder should start");
    let mut engine = Engine::new(Folder::new("root"));
    let read_model = UiReadModel::from_engine(&engine, UiProjectFileSpec::default());
    let old_revision = read_model.current_revision();

    let first = service
        .try_encode(read_model.capture_snapshot(UiSubscriptionScope::WholeGraph))
        .expect("first client should be admitted");
    entered_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("encoder should enter deterministic test gate");

    let previous_event_time = read_model.current_event_time();
    let root = engine.root;
    engine.add_node(Folder::new("new child"), Some(root));
    engine.apply_edits().expect("concurrent edit should apply");
    read_model.publish_engine_events_since(&engine, previous_event_time);
    let new_revision = read_model.current_revision();
    assert!(new_revision > old_revision);

    let second = service
        .try_encode(read_model.capture_snapshot(UiSubscriptionScope::WholeGraph))
        .expect("second client should be admitted");
    let third = service
        .try_encode(read_model.capture_snapshot(UiSubscriptionScope::WholeGraph))
        .expect("third client should be admitted");
    let fourth = service.try_encode(read_model.capture_snapshot(UiSubscriptionScope::WholeGraph));
    assert!(matches!(fourth, Err(SnapshotEncodingAdmissionError::Full)));

    let saturated = service.metrics();
    assert_eq!(saturated.active, 1);
    assert_eq!(saturated.queued, 2);
    assert_eq!(saturated.accepted, 3);
    assert_eq!(saturated.rejected, 1);

    let (lock, wake) = &*release;
    *lock.lock().expect("release gate poisoned") = true;
    wake.notify_all();

    let first = first
        .recv_timeout(Duration::from_secs(5))
        .expect("first client should receive a reply")
        .expect("first snapshot should encode");
    let second = second
        .recv_timeout(Duration::from_secs(5))
        .expect("second client should receive a reply")
        .expect("second snapshot should encode");
    let third = third
        .recv_timeout(Duration::from_secs(5))
        .expect("third client should receive a reply")
        .expect("third snapshot should encode");

    assert_eq!((first.node_count, first.revision), (1, old_revision));
    assert_eq!((second.node_count, second.revision), (2, new_revision));
    assert_eq!((third.node_count, third.revision), (2, new_revision));
    assert!(!first.cache_hit);
    assert!(!second.cache_hit);
    assert!(third.cache_hit);
    assert!(Arc::ptr_eq(&second.encoded, &third.encoded));

    let completed = service.metrics();
    assert_eq!(completed.peak_active, 1);
    assert_eq!(completed.materialized, 2);
    assert_eq!(completed.cache_hits, 1);
}

#[test]
fn websocket_snapshot_envelope_preserves_the_generated_protocol_shape() {
    let engine = Engine::new(Folder::new("root"));
    let read_model = UiReadModel::from_engine(&engine, UiProjectFileSpec::default());
    let service = SnapshotEncodingService::spawn().expect("snapshot encoder should start");
    let snapshot = service
        .try_encode(read_model.capture_snapshot(UiSubscriptionScope::WholeGraph))
        .expect("snapshot should be admitted")
        .recv_timeout(Duration::from_secs(5))
        .expect("snapshot worker should reply")
        .expect("snapshot should encode");

    let encoded = encode_websocket_message("request \\\"one\\\"", &snapshot).expect("envelope should encode");
    let decoded: UiServerMessage = serde_json::from_str(&encoded).expect("envelope should match generated protocol");
    let UiServerMessage::Snapshot {
        request_id,
        snapshot: decoded_snapshot,
    } = decoded
    else {
        panic!("expected snapshot response");
    };
    assert_eq!(request_id, "request \\\"one\\\"");
    assert_eq!(decoded_snapshot.at, snapshot.revision);
    assert_eq!(decoded_snapshot.nodes.len(), snapshot.node_count);
}

#[test]
fn outbound_queue_allows_one_bounded_snapshot_beyond_the_regular_message_budget() {
    let queue = WsOutboundQueue::with_limits(2, 32);
    let snapshot = SnapshotEncodingResult {
        encoded: Arc::from("x".repeat(128)),
        node_count: 1,
        version: 1,
        revision: EngineTime {
            tick: 0,
            micro: 0,
            seq: 0,
        },
        project_generation: 1,
        materialize_elapsed: Duration::ZERO,
        encode_elapsed: Duration::ZERO,
        cache_hit: false,
    };

    assert_eq!(
        queue.push(WsOutbound::Snapshot {
            request_id: "first".to_string(),
            snapshot: snapshot.clone(),
        }),
        QueuePushResult::Queued
    );
    assert_eq!(
        queue.push(WsOutbound::Snapshot {
            request_id: "second".to_string(),
            snapshot,
        }),
        QueuePushResult::Full
    );
    assert!(matches!(queue.pop(), Some(WsOutbound::Snapshot { .. })));
    assert_eq!(queue.retained_bytes(), 0);
}
