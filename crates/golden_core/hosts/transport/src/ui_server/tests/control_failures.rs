use super::*;
use golden_engine::application::ControlErrorKind;
use golden_protocol::UiControlErrorCode;
use std::sync::Mutex;

type Gate = (mpsc::Sender<()>, mpsc::Receiver<()>);
static GATE: Mutex<Option<Gate>> = Mutex::new(None);
static TEST_LOCK: Mutex<()> = Mutex::new(());
golden_engine::define_node_enum!(
    enum ControlTestNode {}
);

impl ProjectLifecycle for ControlTestNode {
    fn settle_pending_operations(_engine: &mut Engine<Self>) -> Result<(), String> {
        let gate = GATE.lock().unwrap().take();
        if let Some((entered, release)) = gate {
            entered.send(()).unwrap();
            let _ = release.recv();
        }
        Ok(())
    }
}

struct ReleaseOnDrop(Option<mpsc::Sender<()>>);
impl Drop for ReleaseOnDrop {
    fn drop(&mut self) {
        if let Some(sender) = self.0.take() {
            let _ = sender.send(());
        }
    }
}

fn http_intent(state: &ServerState<ControlTestNode>) -> (String, serde_json::Value) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let mut client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    client.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    let body = serde_json::to_vec(&UiEditIntent::ReevaluateGraph).unwrap();
    let admission = super::super::access::RequestAdmission {
        origin: None,
        authority: listener.local_addr().unwrap().to_string(),
    };
    let session = state.sessions.bootstrap(&admission, Default::default(), None).unwrap();
    write!(
        client,
        "POST /api/ui/intent HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer {}\r\nContent-Length: {}\r\n\r\n",
        listener.local_addr().unwrap(),
        session.token,
        body.len()
    )
    .unwrap();
    client.write_all(&body).unwrap();
    let (mut server, _) = listener.accept().unwrap();
    handle_connection(&mut server, state).unwrap();
    drop(server);
    let mut response = String::new();
    client.read_to_string(&mut response).unwrap();
    let (headers, body) = response.split_once("\r\n\r\n").unwrap();
    (
        headers.lines().next().unwrap().to_owned(),
        serde_json::from_str(body).unwrap(),
    )
}

#[test]
fn saturated_actor_rejects_http_and_websocket_without_false_acceptance_then_recovers() {
    let _test = TEST_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let runtime = ProductionRuntime::<ControlTestNode>::new(
        Engine::new(Folder::new("root").into()),
        UiProjectFileSpec::default(),
    )
    .unwrap();
    let read_model = runtime.read_model();
    let readiness = Arc::new(UiSessionReadiness::default());
    let encoding = SnapshotEncodingService::spawn().unwrap();
    let (cmd_tx, _cmd_rx) = mpsc::sync_channel(1);
    let state = ServerState {
        frontend_security: Arc::new(FrontendSecurity::default()),
        sessions: Arc::new(SessionRegistry::new(golden_protocol::UiSessionCapabilities::ALL)),
        request_access: Arc::new(RequestAccessPolicy::new(&[], &[]).unwrap()),
        runtime: runtime.clone(),
        read_model: read_model.clone(),
        ws_hub: WsHubHandle {
            cmd_tx,
            readiness: readiness.clone(),
        },
        snapshot_encoding: encoding.clone(),
        frontend_assets: &[],
        preferences: None,
    };
    let context = WsHubContext {
        runtime: runtime.clone(),
        read_model,
        preferences: None,
        session_id: "test".into(),
        readiness,
        snapshot_encoding: encoding,
    };
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let release = ReleaseOnDrop(Some(release_tx));
    *GATE.lock().unwrap() = Some((entered_tx, release_rx));
    let active = runtime
        .submit_ui_transaction_batch(vec![UiEditIntent::ReevaluateGraph], None, true)
        .unwrap();
    entered_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    let mut queued = Vec::new();
    let admission_error = loop {
        match runtime.submit_ui_transaction_batch(Vec::new(), None, true) {
            Ok(pending) => {
                queued.push(pending);
                assert!(queued.len() <= 4096, "bounded test admission");
            }
            Err(error) => break error,
        }
    };
    assert_eq!(
        admission_error.control_error().unwrap().kind(),
        ControlErrorKind::Overloaded
    );
    let (status, response) = http_intent(&state);
    assert!(status.contains("503"));
    assert_eq!(response["code"], "overloaded");
    assert!(
        response.get("success").is_none(),
        "no authoritative acknowledgement exists"
    );
    let mut clients = HashMap::from([(1, client_with_subscription_count(0))]);
    let outbound = clients[&1].outbound.clone();
    handle_ws_hub_command(
        &context,
        &mut clients,
        &mut HashMap::new(),
        &mut Default::default(),
        WsHubCommand::Intent {
            client_id: 1,
            request_id: "overloaded".into(),
            intent: Box::new(UiEditIntent::Undo),
            include_self_events: true,
        },
    );
    assert!(
        matches!(outbound.pop(), Some(WsOutbound::Message(WsServerMessage::Error {
        code: Some(UiControlErrorCode::Overloaded), request_id: Some(id), ..
    })) if id == "overloaded")
    );
    assert!(
        outbound.pop().is_none(),
        "rejected admission must never emit Accepted or Applied"
    );
    drop(release);
    assert!(active.wait().unwrap().transactions[0].acknowledgement.success);
    for pending in queued {
        pending.wait().unwrap();
    }
    let (status, response) = http_intent(&state);
    assert!(status.contains("200"));
    assert_eq!(response["success"], true);
}

#[test]
fn disconnected_client_session_is_released_after_saturation() {
    let _test = TEST_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let runtime = ProductionRuntime::<ControlTestNode>::new(
        Engine::new(Folder::new("root").into()),
        UiProjectFileSpec::default(),
    )
    .unwrap();
    let context = WsHubContext {
        runtime: runtime.clone(),
        read_model: runtime.read_model(),
        preferences: None,
        session_id: "test".into(),
        readiness: Arc::new(UiSessionReadiness::default()),
        snapshot_encoding: SnapshotEncodingService::spawn().unwrap(),
    };
    let physical = super::super::websocket::WsConnectionLifetime(UiClientLease::new(None));
    let mut clients = HashMap::new();
    let mut origins = HashMap::new();
    let mut snapshots = Default::default();
    handle_ws_hub_command(
        &context,
        &mut clients,
        &mut origins,
        &mut snapshots,
        WsHubCommand::RegisterClient {
            client_id: 1,
            outbound: Arc::new(WsOutboundQueue::new(DEFAULT_OUTBOUND_CAPACITY)),
            connection: physical.0.clone(),
        },
    );
    handle_ws_hub_command(
        &context,
        &mut clients,
        &mut origins,
        &mut snapshots,
        WsHubCommand::BindClientInstance {
            client_id: 1,
            client_instance_id: "browser".into(),
        },
    );
    handle_ws_hub_command(
        &context,
        &mut clients,
        &mut origins,
        &mut snapshots,
        WsHubCommand::Intent {
            client_id: 1,
            request_id: "begin".into(),
            intent: Box::new(UiEditIntent::BeginEdit {
                client_edit_id: "old edit".into(),
                label: None,
            }),
            include_self_events: true,
        },
    );
    assert!(runtime.history_state().unwrap().active_edit_session);
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let release = ReleaseOnDrop(Some(release_tx));
    *GATE.lock().unwrap() = Some((entered_tx, release_rx));
    let active = runtime
        .submit_ui_transaction_batch(vec![UiEditIntent::ReevaluateGraph], Some("browser"), true)
        .unwrap();
    entered_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    let mut queued = Vec::new();
    while let Ok(operation) = runtime.submit_ui_transaction_batch(Vec::new(), None, true) {
        queued.push(operation);
    }
    // The socket ends while both queues are full. No hub cleanup can run yet.
    let (hub_tx, _hub_rx) = mpsc::sync_channel(1);
    hub_tx.send(WsHubCommand::UnregisterClient { client_id: 99 }).unwrap();
    assert!(matches!(
        hub_tx.try_send(WsHubCommand::UnregisterClient { client_id: 1 }),
        Err(mpsc::TrySendError::Full(_))
    ));
    drop(physical);
    drop(release);
    active.wait().unwrap();
    for operation in queued {
        operation.wait().unwrap();
    }
    runtime.run_tick(Duration::from_millis(1)).unwrap();
    assert!(
        !runtime.history_state().unwrap().active_edit_session,
        "closed connections must not strand an active edit"
    );
    assert!(clients.contains_key(&1), "release must precede hub cleanup");
    clients.insert(2, client_with_subscription_count(0));
    handle_ws_hub_command(
        &context,
        &mut clients,
        &mut origins,
        &mut snapshots,
        WsHubCommand::BindClientInstance {
            client_id: 2,
            client_instance_id: "browser".into(),
        },
    );
    handle_ws_hub_command(
        &context,
        &mut clients,
        &mut origins,
        &mut snapshots,
        WsHubCommand::Intent {
            client_id: 2,
            request_id: "replacement".into(),
            intent: Box::new(UiEditIntent::BeginEdit {
                client_edit_id: "old edit".into(),
                label: None,
            }),
            include_self_events: true,
        },
    );
    assert!(runtime.history_state().unwrap().active_edit_session);
    handle_ws_hub_command(
        &context,
        &mut clients,
        &mut origins,
        &mut snapshots,
        WsHubCommand::UnregisterClient { client_id: 1 },
    );
    let received = runtime.runtime_metrics().control_received;
    handle_ws_hub_command(
        &context,
        &mut clients,
        &mut origins,
        &mut snapshots,
        WsHubCommand::Intent {
            client_id: 1,
            request_id: "stale".into(),
            intent: Box::new(UiEditIntent::EndEdit {
                client_edit_id: "old edit".into(),
            }),
            include_self_events: true,
        },
    );
    assert_eq!(
        runtime.runtime_metrics().control_received,
        received,
        "unregistered connections cannot submit edits"
    );
    assert!(
        runtime.history_state().unwrap().active_edit_session,
        "late old cleanup must not end the replacement session"
    );
    handle_ws_hub_command(
        &context,
        &mut clients,
        &mut origins,
        &mut snapshots,
        WsHubCommand::UnregisterClient { client_id: 2 },
    );
    assert!(!runtime.history_state().unwrap().active_edit_session);
}
