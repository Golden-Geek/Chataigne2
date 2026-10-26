use super::*;
use golden_engine::{application::ProjectReplacement, define_node_enum};

define_node_enum!(
    enum ReplacementNode {}
);
impl ProjectLifecycle for ReplacementNode {}

fn runtime() -> ProductionRuntime<ReplacementNode> {
    ProductionRuntime::new(Engine::new(Folder::new("Before").into()), UiProjectFileSpec::default()).unwrap()
}

fn replace(runtime: &ProductionRuntime<ReplacementNode>) {
    runtime
        .replace_project(ProjectReplacement {
            engine: Engine::new(Folder::new("After").into()),
            project_file: UiProjectFileSpec::default(),
            reason: "replacement race".into(),
            recover: false,
        })
        .unwrap();
}

fn hub(runtime: ProductionRuntime<ReplacementNode>) -> WsHubContext<ReplacementNode> {
    WsHubContext {
        read_model: runtime.read_model(),
        runtime,
        preferences: None,
        session_id: "test".into(),
        readiness: Arc::new(UiSessionReadiness::default()),
        snapshot_encoding: SnapshotEncodingService::spawn().unwrap(),
    }
}

fn assert_stale_subscription(context: &WsHubContext<ReplacementNode>, from: UiReplayCursor) {
    let client = client_with_subscription_count(0);
    let outbound = client.outbound.clone();
    let mut clients = HashMap::from([(1, client)]);
    handle_ws_hub_command(
        context,
        &mut clients,
        &mut HashMap::new(),
        &mut PendingWsSnapshots::default(),
        WsHubCommand::Subscribe {
            client_id: 1,
            subscription_id: "stale".into(),
            interest: UiInterest::workbench("view", UiSubscriptionScope::WholeGraph),
            from: Some(from),
        },
    );
    assert!(clients[&1].subscriptions["stale"].awaiting_resync);
    assert!(
        matches!(outbound.pop(),Some(WsOutbound::Message(WsServerMessage::ResyncRequired{reason,..})) if reason=="project_generation_changed")
    );
}

#[test]
fn old_snapshot_cursor_is_rejected_after_unsubscribe_and_replacement() {
    let runtime = runtime();
    let model = runtime.read_model();
    let old = model.current_snapshot();
    replace(&runtime);
    let cached = model.current_snapshot();
    let captured = model.capture_snapshot(UiSubscriptionScope::WholeGraph).materialize();
    assert_ne!(old.project_generation, cached.project_generation);
    assert_eq!(cached.project_generation, captured.project_generation);
    assert_eq!(cached.project_generation, model.current_project_generation().get());
    assert_stale_subscription(
        &hub(runtime),
        UiReplayCursor {
            project_generation: old.project_generation,
            at: cached.at,
        },
    );
}

#[test]
fn old_replay_cursor_returns_only_resync_without_leaking_new_project_events() {
    let runtime = runtime();
    let model = runtime.read_model();
    let old = model.current_cursor();
    replace(&runtime);
    let replay = model.replay_from_cursor(Some(old), UiSubscriptionScope::WholeGraph);
    assert_eq!(replay.project_generation, model.current_project_generation().get());
    assert_eq!(replay.batch.events.len(), 1);
    assert!(
        matches!(&replay.batch.events[0].kind,UiEventKind::Custom{topic,payload,..}
        if topic=="__transport.resync_required"&&payload["reason"]=="project_generation_changed")
    );
}

#[test]
fn delayed_encoding_preserves_snapshot_generation_and_cannot_resume_a_new_project() {
    use std::sync::{Condvar, Mutex};
    struct Release(Arc<(Mutex<bool>, Condvar)>);
    impl Drop for Release {
        fn drop(&mut self) {
            let (lock, wake) = &*self.0;
            *lock.lock().unwrap() = true;
            wake.notify_all();
        }
    }
    let runtime = runtime();
    let model = runtime.read_model();
    let old = model.current_cursor();
    let gate = Arc::new((Mutex::new(false), Condvar::new()));
    let release = Release(gate.clone());
    let (entered_tx, entered_rx) = mpsc::sync_channel(1);
    let encoder = SnapshotEncodingService::spawn_with_test_hook(
        1,
        Arc::new(move || {
            entered_tx.send(()).unwrap();
            let (lock, wake) = &*gate;
            let mut ready = lock.lock().unwrap();
            while !*ready {
                ready = wake.wait(ready).unwrap();
            }
        }),
    )
    .unwrap();
    let pending = encoder
        .try_encode(model.capture_snapshot(UiSubscriptionScope::WholeGraph))
        .unwrap();
    entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    replace(&runtime);
    drop(release);
    let result = pending.recv_timeout(Duration::from_secs(5)).unwrap().unwrap();
    let snapshot: golden_protocol::UiSnapshot = serde_json::from_str(&result.encoded).unwrap();
    assert_eq!(result.project_generation, old.project_generation);
    assert_eq!(snapshot.project_generation, old.project_generation);
    assert_ne!(snapshot.project_generation, model.current_snapshot().project_generation);
    assert_stale_subscription(
        &hub(runtime),
        UiReplayCursor {
            project_generation: snapshot.project_generation,
            at: snapshot.at,
        },
    );
}

#[test]
fn prior_project_origin_does_not_suppress_an_equal_time_event_in_the_new_project() {
    let runtime = runtime();
    let model = runtime.read_model();
    let old = model.current_project_generation();
    replace(&runtime);
    let current = model.current_project_generation();
    let before = model.current_revision();
    let root = model
        .current_snapshot()
        .nodes
        .iter()
        .find(|node| node.meta.label == "After")
        .unwrap()
        .node_id;
    let result = runtime
        .apply_ui_transaction(
            UiEditIntent::PatchMeta {
                node: root,
                patch: golden_protocol::NodeMetaPatch {
                    label: Some("New event".into()),
                    ..Default::default()
                },
            },
            None,
        )
        .unwrap();
    assert!(result.acknowledgement.success);
    assert_eq!(result.project_generation, current);
    let mut origins = result
        .events
        .events
        .iter()
        .map(|event| {
            (
                event.time,
                WsEventOrigin {
                    project_generation: old,
                    client_id: 1,
                    include_self_events: false,
                },
            )
        })
        .collect();
    let mut client = client_with_subscription_count(1);
    let subscription = client.subscriptions.get_mut("subscription-0").unwrap();
    subscription.project_generation = current;
    subscription.cursor = Some(before);
    let outbound = client.outbound.clone();
    let mut clients = HashMap::from([(1, client)]);
    dispatch_ws_batches(&model, &mut clients, &mut origins, true);
    let Some(WsOutbound::Message(WsServerMessage::Delta {
        project_generation,
        deltas,
        ..
    })) = outbound.pop()
    else {
        panic!("New project event was suppressed")
    };
    assert_eq!(project_generation, current.get());
    assert!(serde_json::to_string(&deltas).unwrap().contains("New event"));
}

#[test]
fn delayed_transaction_receipt_retains_the_generation_captured_before_replacement() {
    let runtime = runtime();
    let model = runtime.read_model();
    let old = model.current_project_generation();
    let pending = runtime
        .submit_ui_transaction_batch(vec![UiEditIntent::ReevaluateGraph], None, true)
        .unwrap();
    replace(&runtime);
    assert_ne!(old, model.current_project_generation());
    let result = pending.wait().unwrap();
    assert!(result.transactions[0].acknowledgement.success);
    assert_eq!(result.transactions[0].project_generation, old);
}

#[test]
fn latest_wins_queue_never_merges_project_generations() {
    let queue = WsOutboundQueue::new(4);
    let old = observation_message(1);
    let mut next = observation_message(1);
    if let WsOutbound::Message(WsServerMessage::Delta { project_generation, .. }) = &mut next {
        *project_generation = 2;
    }
    assert_eq!(queue.push(old), QueuePushResult::Queued);
    assert_eq!(queue.push(next), QueuePushResult::Queued);
    assert_eq!(queue.len(), 2);
}

#[test]
fn project_generation_resyncs_an_idle_subscription_even_when_event_times_match() {
    let engine = Engine::new(ReplacementNode::from(Folder::new("Before")));
    let runtime = ProductionRuntime::new(engine, UiProjectFileSpec::default()).unwrap();
    let read_model = runtime.read_model();
    let previous = read_model.current_project_generation();
    let client = client_with_subscription_count(1);
    let outbound = client.outbound.clone();
    let mut clients = HashMap::from([(1, client)]);
    runtime
        .replace_project(ProjectReplacement {
            engine: Engine::new(Folder::new("After").into()),
            project_file: UiProjectFileSpec::default(),
            reason: "same event clock".into(),
            recover: false,
        })
        .unwrap();
    assert_ne!(read_model.current_project_generation(), previous);
    // The cached client boundary can equal the new project's boundary exactly.
    // EventTime is meaningful only within its project generation.
    clients
        .get_mut(&1)
        .unwrap()
        .subscriptions
        .get_mut("subscription-0")
        .unwrap()
        .cursor = Some(read_model.current_revision());
    let mut origins = HashMap::new();
    dispatch_ws_batches(&read_model, &mut clients, &mut origins, false);
    assert!(
        matches!(outbound.pop(), Some(WsOutbound::Message(WsServerMessage::ResyncRequired { reason, .. })) if reason == "project_generation_changed")
    );
    assert!(clients[&1].subscriptions["subscription-0"].awaiting_resync);
    dispatch_ws_batches(&read_model, &mut clients, &mut origins, false);
    assert!(
        outbound.pop().is_none(),
        "An idle replacement must request exactly one resync"
    );
}
