use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Barrier, Condvar, Mutex, mpsc};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use golden_engine::app::{ProjectLifecycle, prepare_engine_for_runtime};
use golden_engine::application::{ProductionRuntime, ProjectSaveRequest};
use golden_engine::define_node_enum;
use golden_engine::edit::{Edit, NodeTree};
use golden_engine::engine::{Engine, EngineTime};
use golden_engine::node::{Folder, NodeMetaPatch};
use golden_engine::ui_read_model::UiReadModel;
use golden_protocol::{UiEditIntent, UiProjectFileSpec, UiServerMessage, UiSnapshot, UiSubscriptionScope};

use super::super::snapshot_encoding::{
    SnapshotEncodingAdmissionError, SnapshotEncodingResult, SnapshotEncodingService, encode_websocket_message,
};
use super::super::{QueuePushResult, WsOutbound, WsOutboundQueue};

define_node_enum!(
    enum SnapshotQualificationNode {}
);

impl ProjectLifecycle for SnapshotQualificationNode {}

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

#[test]
fn save_resync_and_engine_ticks_progress_during_three_client_snapshot_contention() {
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
    let root: SnapshotQualificationNode = Folder::new("Root").into();
    let mut engine = Engine::new(root);
    prepare_engine_for_runtime(&mut engine).expect("qualification engine should prepare");
    let root = engine.root;
    let runtime = ProductionRuntime::new(
        engine,
        UiProjectFileSpec::from_project_file_spec(SnapshotQualificationNode::project_file_spec(), None),
    );
    let read_model = runtime.read_model();
    let old_revision = read_model.current_revision();
    let first = service
        .try_encode(read_model.capture_snapshot(UiSubscriptionScope::WholeGraph))
        .expect("first resync should be admitted");
    entered_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("encoder should enter deterministic test gate");

    let edit_started = Instant::now();
    let edit = runtime.apply_ui_transaction(
        UiEditIntent::PatchMeta {
            node: root,
            patch: NodeMetaPatch {
                label: Some("Root after edit".to_string()),
                ..Default::default()
            },
        },
        None,
    );
    assert!(edit.acknowledgement.success);
    let edit_elapsed = edit_started.elapsed();
    assert!(edit_elapsed < Duration::from_secs(1));
    let new_revision = read_model.current_revision();
    assert!(new_revision > old_revision);

    let second = service
        .try_encode(read_model.capture_snapshot(UiSubscriptionScope::WholeGraph))
        .expect("second resync should be admitted");
    let third = service
        .try_encode(read_model.capture_snapshot(UiSubscriptionScope::WholeGraph))
        .expect("third resync should be admitted");
    let saturated = service.metrics();
    assert_eq!((saturated.active, saturated.queued), (1, 2));

    let keep_ticking = Arc::new(AtomicBool::new(true));
    let tick_count = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let max_tick_micros = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let tick_worker = {
        let runtime = runtime.clone();
        let keep_ticking = keep_ticking.clone();
        let tick_count = tick_count.clone();
        let max_tick_micros = max_tick_micros.clone();
        std::thread::spawn(move || {
            while keep_ticking.load(Ordering::Acquire) {
                let started = Instant::now();
                runtime
                    .run_tick(Duration::from_millis(5))
                    .expect("runtime tick should progress during snapshot encoding");
                max_tick_micros.fetch_max(started.elapsed().as_micros() as u64, Ordering::AcqRel);
                tick_count.fetch_add(1, Ordering::Relaxed);
                std::thread::yield_now();
            }
        })
    };

    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    let save_barrier = Arc::new(Barrier::new(4));
    let saves_started = Instant::now();
    let save_workers: Vec<_> = (0..3)
        .map(|index| {
            let runtime = runtime.clone();
            let save_barrier = save_barrier.clone();
            let path = std::env::temp_dir().join(format!(
                "golden-t12-contention-{}-{unique}-{index}.json",
                std::process::id()
            ));
            std::thread::spawn(move || {
                save_barrier.wait();
                let result = runtime.save_project(ProjectSaveRequest {
                    path: path.to_string_lossy().into_owned(),
                    ui_state: None,
                });
                (path, result)
            })
        })
        .collect();
    save_barrier.wait();
    for worker in save_workers {
        let (path, result) = worker.join().expect("save worker should not panic");
        let result = result.expect("save should complete while snapshot encoder is blocked");
        assert!(result.encoded_bytes > 0);
        let saved = std::fs::read_to_string(&path).expect("saved project should be readable");
        assert!(saved.contains("Root after edit"));
        std::fs::remove_file(&path).expect("temporary project should be removable");
    }
    let saves_elapsed = saves_started.elapsed();
    keep_ticking.store(false, Ordering::Release);
    tick_worker.join().expect("tick worker should stop");
    let completed_ticks = tick_count.load(Ordering::Acquire);
    let observed_max_tick_micros = max_tick_micros.load(Ordering::Acquire);
    assert!(completed_ticks > 0);
    assert!(observed_max_tick_micros < 500_000);
    eprintln!(
        "transport_contention clients=3 saves=3 edit_us={} saves_ms={} ticks={} max_tick_us={} encoder_active={} encoder_queued={}",
        edit_elapsed.as_micros(),
        saves_elapsed.as_millis(),
        completed_ticks,
        observed_max_tick_micros,
        saturated.active,
        saturated.queued,
    );

    let (lock, wake) = &*release;
    *lock.lock().expect("release gate poisoned") = true;
    wake.notify_all();
    let snapshots: Vec<_> = [first, second, third]
        .into_iter()
        .map(|receiver| {
            receiver
                .recv_timeout(Duration::from_secs(5))
                .expect("resync should complete")
                .expect("resync snapshot should encode")
        })
        .collect();
    let first_snapshot: UiSnapshot = serde_json::from_str(&snapshots[0].encoded).expect("old snapshot should decode");
    let second_snapshot: UiSnapshot = serde_json::from_str(&snapshots[1].encoded).expect("new snapshot should decode");
    assert_eq!(first_snapshot.at, old_revision);
    assert_eq!(first_snapshot.nodes[0].meta.label, "Root");
    assert_eq!(second_snapshot.at, new_revision);
    assert_eq!(second_snapshot.nodes[0].meta.label, "Root after edit");
    assert!(Arc::ptr_eq(&snapshots[1].encoded, &snapshots[2].encoded));
    let metrics = service.metrics();
    assert_eq!(metrics.peak_active, 1);
    assert_eq!(metrics.materialized, 2);
    assert_eq!(metrics.cache_hits, 1);
}

#[test]
#[ignore = "manual immutable transport snapshot capture/materialization/encoding measurement"]
fn measure_transport_snapshot_encoding_at_scale() {
    for node_count in [1_000, 10_000, 100_000] {
        let mut engine = Engine::new(Folder::new("root"));
        let mut tree = NodeTree::new(Folder::new("scale root"));
        for index in 0..node_count {
            tree.push_child(NodeTree::new(Folder::new(format!("node {index}"))));
        }
        engine.edits.push(Edit::AddNodeTree {
            tree,
            parent: engine.root,
            prev_sibling: None,
        });
        engine.apply_edits().expect("scale tree should attach");
        let read_model = UiReadModel::from_engine(&engine, UiProjectFileSpec::default());
        let service = SnapshotEncodingService::spawn().expect("snapshot encoder should start");

        let capture_started = Instant::now();
        let captures: Vec<_> = (0..3)
            .map(|_| read_model.capture_snapshot(UiSubscriptionScope::WholeGraph))
            .collect();
        let capture_elapsed = capture_started.elapsed();
        let transport_started = Instant::now();
        let receivers: Vec<_> = captures
            .into_iter()
            .map(|capture| service.try_encode(capture).expect("scale snapshot should be admitted"))
            .collect();
        let results: Vec<_> = receivers
            .into_iter()
            .map(|receiver| {
                receiver
                    .recv_timeout(Duration::from_secs(30))
                    .expect("scale snapshot should complete")
                    .expect("scale snapshot should encode")
            })
            .collect();
        let transport_elapsed = transport_started.elapsed();

        assert_eq!(results[0].node_count, node_count + 2);
        assert!(!results[0].cache_hit);
        assert!(results[1].cache_hit && results[2].cache_hit);
        assert!(Arc::ptr_eq(&results[0].encoded, &results[1].encoded));
        assert!(Arc::ptr_eq(&results[0].encoded, &results[2].encoded));
        eprintln!(
            "transport_snapshot nodes={} clients=3 capture_total_us={} capture_each_us={} materialize_ms={} encode_ms={} total_ms={} bytes={} cache_hits={}",
            results[0].node_count,
            capture_elapsed.as_micros(),
            capture_elapsed.as_micros() / 3,
            results[0].materialize_elapsed.as_millis(),
            results[0].encode_elapsed.as_millis(),
            transport_elapsed.as_millis(),
            results[0].encoded.len(),
            service.metrics().cache_hits,
        );
    }
}
