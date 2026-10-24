use super::*;

pub(super) struct WsConnectionLifetime(pub UiClientLease);

impl Drop for WsConnectionLifetime {
    fn drop(&mut self) {
        self.0.close();
    }
}

pub(super) fn handle_ws_connection<T: ProjectLifecycle>(
    stream: &mut TcpStream,
    state: &ServerState<T>,
    request: &HttpRequest,
    grant: Arc<SessionGrant>,
) -> std::io::Result<()> {
    let runtime = Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| Error::other(format!("failed to build websocket runtime: {error}")))?;

    let ws_request = build_websocket_request(request)?;
    let mut response = match create_response(&ws_request) {
        Ok(response) => response,
        Err(tokio_tungstenite::tungstenite::Error::Http(response)) => {
            write_ws_handshake_response(&mut *stream, &response)
                .map_err(|error| Error::other(format!("failed to write websocket handshake rejection: {error}")))?;
            stream.flush()?;
            return Ok(());
        }
        Err(error) => {
            return Err(Error::new(
                ErrorKind::InvalidData,
                format!("invalid websocket upgrade request: {error}"),
            ));
        }
    };
    if header_contains_token(request, "sec-websocket-protocol", session::SESSION_PROTOCOL) {
        response.headers_mut().insert(
            "Sec-WebSocket-Protocol",
            session::SESSION_PROTOCOL.parse().expect("constant subprotocol"),
        );
    }
    write_ws_handshake_response(&mut *stream, &response)
        .map_err(|error| Error::other(format!("failed to write websocket handshake response: {error}")))?;
    stream.flush()?;
    stream.set_read_timeout(None)?;
    stream.set_write_timeout(None)?;
    stream.set_nonblocking(true)?;

    let tokio_stream = {
        let cloned_stream = stream.try_clone()?;
        let _guard = runtime.enter();
        tokio::net::TcpStream::from_std(cloned_stream)
            .map_err(|error| Error::other(format!("failed to convert websocket stream to tokio stream: {error}")))?
    };
    let mut websocket = runtime.block_on(WebSocketStream::from_raw_socket(
        tokio_stream,
        Role::Server,
        Some(ui_websocket_config()),
    ));

    let client_id = NEXT_WS_CLIENT_ID.fetch_add(1, Ordering::Relaxed);
    eprintln!("[ui-ws] upgraded connection to websocket (client_id={client_id})");
    let outbound = Arc::new(WsOutboundQueue::new(DEFAULT_OUTBOUND_CAPACITY));
    let connection = WsConnectionLifetime(UiClientLease::new(None));
    state
        .ws_hub
        .cmd_tx
        .send(WsHubCommand::RegisterClient {
            client_id,
            outbound: outbound.clone(),
            connection: connection.0.clone(),
        })
        .map_err(|_| Error::new(ErrorKind::BrokenPipe, "websocket hub unavailable"))?;

    let mut last_ping_at = Instant::now();
    let mut awaiting_pong_since = None::<Instant>;

    'connection: loop {
        if grant.check().is_err() {
            close_expired_session(&runtime, &mut websocket);
            break;
        }
        for _ in 0..outbound.len() {
            let Some(message) = outbound.pop() else {
                break;
            };
            match send_ws_outbound(&runtime, &mut websocket, message) {
                Ok(true) => break 'connection,
                Ok(false) => {}
                Err(err) => {
                    eprintln!("websocket write failed for client {client_id}: {err}");
                    break 'connection;
                }
            }
        }

        match poll_ws_message(&runtime, &mut websocket, WS_IO_POLL_INTERVAL) {
            Ok(UiWebSocketPoll::Idle) => {}
            Ok(UiWebSocketPoll::Closed) => break,
            Ok(UiWebSocketPoll::Message(message)) => match message {
                Message::Text(text) => {
                    awaiting_pong_since = None;
                    let message: WsClientMessage = match serde_json::from_str(text.as_ref()) {
                        Ok(message) => message,
                        Err(err) => {
                            let _ = outbound.push(WsOutbound::Message(WsServerMessage::Error {
                                code: None,
                                message: format!("invalid websocket message: {err}"),
                                request_id: None,
                            }));
                            continue;
                        }
                    };

                    if !handle_ws_client_message(message, client_id, &state.ws_hub, &outbound, &grant) {
                        if grant.check().is_err() {
                            close_expired_session(&runtime, &mut websocket);
                        }
                        break;
                    }
                }
                Message::Binary(_) => {
                    let _ = outbound.push(WsOutbound::Message(WsServerMessage::Error {
                        code: None,
                        message: "binary websocket messages are not supported".to_string(),
                        request_id: None,
                    }));
                }
                Message::Ping(_) => {
                    awaiting_pong_since = None;
                    if let Err(err) = flush_ws_stream(&runtime, &mut websocket) {
                        eprintln!("websocket flush failed for client {client_id}: {err}");
                        break;
                    }
                }
                Message::Pong(_) => {
                    awaiting_pong_since = None;
                }
                Message::Close(_) => break,
                Message::Frame(_) => {}
            },
            Err(err) => {
                eprintln!("websocket read failed for client {client_id}: {err}");
                break;
            }
        }

        let now = Instant::now();
        if let Some(since) = awaiting_pong_since
            && now.duration_since(since) > WS_PONG_TIMEOUT
        {
            eprintln!("[ui-ws] client {client_id} timed out waiting for pong");
            break;
        }
        if now.duration_since(last_ping_at) >= WS_PING_INTERVAL {
            match send_ws_outbound(&runtime, &mut websocket, WsOutbound::Ping(Vec::new())) {
                Ok(true) => break,
                Ok(false) => {}
                Err(err) => {
                    eprintln!("[ui-ws] failed to send ping for client {client_id}: {err}");
                    break;
                }
            }
            last_ping_at = now;
            if awaiting_pong_since.is_none() {
                awaiting_pong_since = Some(now);
            }
        }
    }

    // Fence actor work before waiting for hub admission; its queue may be full.
    drop(connection);
    let _ = state.ws_hub.cmd_tx.send(WsHubCommand::UnregisterClient { client_id });
    let _ = runtime.block_on(websocket.close(None));
    eprintln!("[ui-ws] websocket loop ended (client_id={client_id})");
    Ok(())
}

fn close_expired_session(runtime: &Runtime, websocket: &mut UiWebSocketStream) {
    use tokio_tungstenite::tungstenite::protocol::{CloseFrame, frame::coding::CloseCode};
    let _ = runtime.block_on(websocket.close(Some(CloseFrame {
        code: CloseCode::Policy,
        reason: "session expired or revoked".into(),
    })));
}

pub(super) fn send_ws_outbound(
    runtime: &Runtime,
    websocket: &mut UiWebSocketStream,
    outbound: WsOutbound,
) -> std::io::Result<bool> {
    let close_requested = matches!(outbound, WsOutbound::Close);
    let message = match outbound {
        WsOutbound::Message(message) => {
            let text = serde_json::to_string(&message).map_err(|err| {
                Error::new(
                    ErrorKind::InvalidData,
                    format!("failed to serialize websocket message: {err}"),
                )
            })?;
            Message::text(text)
        }
        WsOutbound::Snapshot { request_id, snapshot } => {
            Message::text(encode_websocket_message(&request_id, &snapshot)?)
        }
        WsOutbound::Ping(payload) => Message::Ping(payload.into()),
        WsOutbound::Close => Message::Close(None),
    };

    runtime
        .block_on(async { websocket.send(message).await })
        .map_err(|error| Error::other(format!("failed to write websocket frame: {error}")))?;
    Ok(close_requested)
}

pub(super) fn poll_ws_message(
    runtime: &Runtime,
    websocket: &mut UiWebSocketStream,
    poll_interval: Duration,
) -> std::io::Result<UiWebSocketPoll> {
    match runtime.block_on(async { timeout(poll_interval, websocket.next()).await }) {
        Err(_) => Ok(UiWebSocketPoll::Idle),
        Ok(None) => Ok(UiWebSocketPoll::Closed),
        Ok(Some(Ok(message))) => Ok(UiWebSocketPoll::Message(message)),
        Ok(Some(Err(error))) => Err(Error::other(format!("websocket read failed: {error}"))),
    }
}

pub(super) fn flush_ws_stream(runtime: &Runtime, websocket: &mut UiWebSocketStream) -> std::io::Result<()> {
    runtime
        .block_on(async { websocket.flush().await })
        .map_err(|error| Error::other(format!("failed to flush websocket stream: {error}")))
}

pub(super) fn build_websocket_request(request: &HttpRequest) -> std::io::Result<WsRequest> {
    let mut builder = WsRequest::builder()
        .method(Method::GET)
        .uri(request.path.as_str())
        .version(Version::HTTP_11);

    for (name, value) in &request.headers {
        builder = builder.header(name.as_str(), value.as_str());
    }

    builder
        .body(())
        .map_err(|error| Error::new(ErrorKind::InvalidData, format!("invalid websocket request: {error}")))
}

pub(super) fn ui_websocket_config() -> WebSocketConfig {
    WebSocketConfig::default()
        .max_message_size(Some(WS_MAX_PAYLOAD_BYTES))
        .max_frame_size(Some(WS_MAX_PAYLOAD_BYTES))
}

pub(super) fn handle_ws_client_message(
    message: WsClientMessage,
    client_id: u64,
    hub: &WsHubHandle,
    outbound: &Arc<WsOutboundQueue>,
    grant: &SessionGrant,
) -> bool {
    let capability = match &message {
        WsClientMessage::Intent { .. }
        | WsClientMessage::IntentBatch { .. }
        | WsClientMessage::Hello {
            client_instance_id: Some(_),
            ..
        } => Capability::Author,
        WsClientMessage::Hello {
            client_instance_id: None,
            ..
        }
        | WsClientMessage::Subscribe { .. }
        | WsClientMessage::Unsubscribe { .. }
        | WsClientMessage::Snapshot { .. }
        | WsClientMessage::Replay { .. } => Capability::Observe,
    };
    if let Err(error) = grant.require(capability) {
        let request_id = match &message {
            WsClientMessage::Intent { request_id, .. }
            | WsClientMessage::IntentBatch { request_id, .. }
            | WsClientMessage::Snapshot { request_id, .. }
            | WsClientMessage::Replay { request_id, .. } => Some(request_id.clone()),
            _ => None,
        };
        let _ = outbound.push(WsOutbound::Message(WsServerMessage::Error {
            code: None,
            message: error.message.into(),
            request_id,
        }));
        return grant.check().is_ok();
    }
    match message {
        WsClientMessage::Hello {
            protocol_version,
            client_instance_id,
        } => {
            if protocol_version != UI_PROTOCOL_VERSION {
                let _ = outbound.push(WsOutbound::Message(WsServerMessage::Error {
                    code: None,
                    message: format!("protocol mismatch: client={protocol_version}, server={UI_PROTOCOL_VERSION}"),
                    request_id: None,
                }));
            }

            if let Some(client_instance_id) = client_instance_id {
                let Some(client_instance_id) = normalize_ui_client_instance_id(&client_instance_id) else {
                    let _ = outbound.push(WsOutbound::Message(WsServerMessage::Error {
                        code: None,
                        message: "client_instance_id is invalid".to_string(),
                        request_id: None,
                    }));
                    return true;
                };

                if !submit_ws_hub_command(
                    hub,
                    outbound,
                    WsHubCommand::BindClientInstance {
                        client_id,
                        client_instance_id,
                    },
                    None,
                    None,
                ) {
                    return false;
                }
            }
            true
        }
        WsClientMessage::Subscribe {
            subscription_id,
            interest,
            from,
        } => {
            if subscription_id.trim().is_empty() {
                let _ = outbound.push(WsOutbound::Message(WsServerMessage::Error {
                    code: None,
                    message: "subscription_id cannot be empty".to_string(),
                    request_id: None,
                }));
                return true;
            }

            submit_ws_hub_command(
                hub,
                outbound,
                WsHubCommand::Subscribe {
                    client_id,
                    subscription_id,
                    interest,
                    from,
                },
                None,
                None,
            )
        }
        WsClientMessage::Unsubscribe { subscription_id } => submit_ws_hub_command(
            hub,
            outbound,
            WsHubCommand::Unsubscribe {
                client_id,
                subscription_id,
            },
            None,
            None,
        ),
        WsClientMessage::Snapshot { request_id, scope } => {
            if request_id.trim().is_empty() {
                let _ = outbound.push(WsOutbound::Message(WsServerMessage::Error {
                    code: None,
                    message: "request_id cannot be empty".to_string(),
                    request_id: None,
                }));
                return true;
            }
            submit_ws_hub_command(
                hub,
                outbound,
                WsHubCommand::Snapshot {
                    client_id,
                    request_id: request_id.clone(),
                    scope,
                },
                Some(request_id),
                None,
            )
        }
        WsClientMessage::Replay {
            request_id,
            scope,
            from,
        } => {
            if request_id.trim().is_empty() {
                let _ = outbound.push(WsOutbound::Message(WsServerMessage::Error {
                    code: None,
                    message: "request_id cannot be empty".to_string(),
                    request_id: None,
                }));
                return true;
            }
            submit_ws_hub_command(
                hub,
                outbound,
                WsHubCommand::Replay {
                    client_id,
                    request_id: request_id.clone(),
                    scope,
                    from,
                },
                Some(request_id),
                None,
            )
        }
        WsClientMessage::Intent {
            request_id,
            intent,
            include_self_events,
        } => {
            if request_id.trim().is_empty() {
                let _ = outbound.push(WsOutbound::Message(WsServerMessage::Error {
                    code: None,
                    message: "request_id cannot be empty".to_string(),
                    request_id: None,
                }));
                return true;
            }

            submit_ws_hub_command(
                hub,
                outbound,
                WsHubCommand::Intent {
                    client_id,
                    request_id: request_id.clone(),
                    intent,
                    include_self_events,
                },
                Some(request_id.clone()),
                Some(request_id),
            )
        }
        WsClientMessage::IntentBatch {
            request_id,
            intents,
            include_self_events,
        } => {
            if request_id.trim().is_empty() {
                let _ = outbound.push(WsOutbound::Message(WsServerMessage::Error {
                    code: None,
                    message: "request_id cannot be empty".to_string(),
                    request_id: None,
                }));
                return true;
            }
            if intents.is_empty() {
                let _ = outbound.push(WsOutbound::Message(WsServerMessage::Error {
                    code: None,
                    message: "intent batch cannot be empty".to_string(),
                    request_id: Some(request_id),
                }));
                return true;
            }
            if intents.len() > MAX_UI_INTENTS_PER_BATCH {
                let _ = outbound.push(WsOutbound::Message(WsServerMessage::Error {
                    code: None,
                    message: format!("intent batch exceeds {MAX_UI_INTENTS_PER_BATCH} operations"),
                    request_id: Some(request_id),
                }));
                return true;
            }

            submit_ws_hub_command(
                hub,
                outbound,
                WsHubCommand::IntentBatch {
                    client_id,
                    request_id: request_id.clone(),
                    intents,
                    include_self_events,
                },
                Some(request_id.clone()),
                Some(request_id),
            )
        }
    }
}

pub(super) fn submit_ws_hub_command(
    hub: &WsHubHandle,
    outbound: &WsOutboundQueue,
    command: WsHubCommand,
    error_request_id: Option<String>,
    received_request_id: Option<String>,
) -> bool {
    match hub.cmd_tx.try_send(command) {
        Ok(()) => {
            let Some(request_id) = received_request_id else {
                return true;
            };
            outbound.push(WsOutbound::Message(WsServerMessage::Control {
                update: UiControlUpdate::pending(request_id, UiControlPhase::Received),
            })) != QueuePushResult::Full
        }
        Err(TrySendError::Full(_)) => {
            outbound.push(WsOutbound::Message(WsServerMessage::Error {
                code: None,
                message: "websocket command capacity is exhausted; retry later".to_string(),
                request_id: error_request_id,
            })) != QueuePushResult::Full
        }
        Err(TrySendError::Disconnected(_)) => false,
    }
}
