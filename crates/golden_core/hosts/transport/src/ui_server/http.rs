use super::*;

mod request;
pub(super) use request::read_http_request;
#[cfg(test)]
pub(super) use request::validate_http_request_size;
#[cfg(test)]
mod tests;

pub(super) struct HttpResponse<'a> {
    pub(super) stream: &'a mut TcpStream,
    pub(super) cors_origin: Option<String>,
}

pub(super) fn handle_connection<T: ProjectLifecycle>(
    stream: &mut TcpStream,
    state: &ServerState<T>,
) -> std::io::Result<()> {
    stream.set_read_timeout(Some(HTTP_IO_TIMEOUT))?;
    stream.set_write_timeout(Some(HTTP_IO_TIMEOUT))?;
    let mut response = HttpResponse {
        stream,
        cors_origin: None,
    };
    let stream = &mut response;

    let request = match read_http_request(stream.stream) {
        Ok(request) => request,
        Err(err) => {
            if is_client_disconnect_error(&err) {
                return Ok(());
            }

            if let Err(write_err) = write_json_error(stream, "400 Bad Request", &format!("invalid request: {err}"))
                && !is_client_disconnect_error(&write_err)
            {
                return Err(write_err);
            }
            return Ok(());
        }
    };

    match state.request_access.admit(&request, stream.stream.local_addr()?) {
        Ok(origin) => stream.cors_origin = origin,
        Err(reason) => {
            write_json_error(stream, "403 Forbidden", reason)?;
            return Ok(());
        }
    }

    if request.method.eq_ignore_ascii_case("OPTIONS") {
        if let Err(reason) = validate_preflight(&request) {
            write_json_error(stream, "403 Forbidden", reason)?;
            return Ok(());
        }
        write_response(stream, "204 No Content", "text/plain", &[])?;
        return Ok(());
    }

    if request.method.eq_ignore_ascii_case("GET") && request.path == UI_WEBSOCKET_PATH {
        if !is_websocket_upgrade_request(&request) {
            write_json_error(stream, "426 Upgrade Required", "websocket upgrade required")?;
            return Ok(());
        }
        return handle_ws_connection(stream.stream, state, &request);
    }

    if request.method.eq_ignore_ascii_case("GET")
        && let Some(asset) = resolve_frontend_asset(state.frontend_assets, &request.path)
    {
        write_response(stream, "200 OK", asset.content_type, asset.bytes)?;
        return Ok(());
    }

    let route = request.path.as_str();
    match (request.method.as_str(), route) {
        ("GET", "/.well-known/chataigne") => {
            write_json(stream, "200 OK", &ui_discovery_document())?;
        }
        ("GET", "/api/ui/health") => {
            write_json(stream, "200 OK", &state.ws_hub.readiness.snapshot(&state.read_model))?;
        }
        ("GET", "/api/ui/user-contexts") => {
            write_json(stream, "200 OK", &state.read_model.current_user_contexts())?;
        }
        ("POST", "/api/ui/snapshot") => {
            let request_started = Instant::now();
            let payload: SnapshotRequest = if request.body.is_empty() {
                SnapshotRequest {
                    scope: UiSubscriptionScope::WholeGraph,
                    cancel_active_edit_session: false,
                }
            } else {
                serde_json::from_slice(&request.body)
                    .map_err(|err| Error::new(ErrorKind::InvalidData, err.to_string()))?
            };
            let request_parse_elapsed = request_started.elapsed();
            let scope = payload.scope;
            let cancel_active_edit_session = payload.cancel_active_edit_session;
            let client_instance_id = ui_client_instance_id_from_headers(&request.headers);

            let cancel_edit_started = Instant::now();
            let lock_wait_elapsed = if payload.cancel_active_edit_session {
                let lock_started = Instant::now();
                if let Some(client_instance_id) = client_instance_id.as_deref()
                    && let Err(error) = state.runtime.cancel_ui_edit_session(client_instance_id)
                {
                    write_application_error(stream, &error)?;
                    return Ok(());
                }
                lock_started.elapsed()
            } else {
                Duration::ZERO
            };
            let cancel_edit_elapsed = cancel_edit_started.elapsed();

            let capture_started = Instant::now();
            let capture = state.read_model.capture_snapshot(scope.clone());
            let capture_elapsed = capture_started.elapsed();
            let encoded = match state.snapshot_encoding.try_encode(capture) {
                Ok(receiver) => receive_encoded_snapshot(receiver)?,
                Err(error) => {
                    write_json_error(stream, "503 Service Unavailable", &error.to_string())?;
                    return Ok(());
                }
            };

            let write_response_started = Instant::now();
            let write_result = write_response(
                stream,
                "200 OK",
                "application/json; charset=utf-8",
                encoded.encoded.as_bytes(),
            );
            let write_response_elapsed = write_response_started.elapsed();

            eprintln!(
                "[ui-http] snapshot scope={scope:?} revision={:?} project_generation={} version={} nodes={} bytes={} request_parse_ms={} lock_wait_ms={} cancel_edit_ms={} capture_us={} materialize_ms={} encode_ms={} cache_hit={} write_response_ms={} cancel_active_edit_session={} total_ms={}",
                encoded.revision,
                encoded.project_generation,
                encoded.version,
                encoded.node_count,
                encoded.encoded.len(),
                request_parse_elapsed.as_millis(),
                lock_wait_elapsed.as_millis(),
                cancel_edit_elapsed.as_millis(),
                capture_elapsed.as_micros(),
                encoded.materialize_elapsed.as_millis(),
                encoded.encode_elapsed.as_millis(),
                encoded.cache_hit,
                write_response_elapsed.as_millis(),
                cancel_active_edit_session,
                request_started.elapsed().as_millis()
            );

            write_result?;
        }
        ("POST", "/api/ui/replay") => {
            let payload: ReplayRequest = serde_json::from_slice(&request.body)
                .map_err(|err| Error::new(ErrorKind::InvalidData, format!("invalid replay payload: {err}")))?;
            eprintln!("[ui-http] replay scope={:?} from={:?}", payload.scope, payload.from);

            let batch = state.read_model.replay(payload.from, payload.scope);

            write_json(stream, "200 OK", &batch)?;
        }
        ("POST", "/api/ui/reference-targets") => {
            let payload: ReferenceTargetsRequest = serde_json::from_slice(&request.body).map_err(|err| {
                Error::new(
                    ErrorKind::InvalidData,
                    format!("invalid reference-targets payload: {err}"),
                )
            })?;

            let targets = match state.runtime.reference_targets(payload.param) {
                Ok(value) => value,
                Err(error) => {
                    write_application_error(stream, &error)?;
                    return Ok(());
                }
            };

            write_json(stream, "200 OK", &targets)?;
        }
        ("POST", "/api/ui/context-candidates") => {
            let payload: ContextCandidatesRequest = serde_json::from_slice(&request.body).map_err(|err| {
                Error::new(
                    ErrorKind::InvalidData,
                    format!("invalid context-candidates payload: {err}"),
                )
            })?;

            let candidates = match state.runtime.context_candidates(payload.param) {
                Ok(value) => value,
                Err(error) => {
                    write_application_error(stream, &error)?;
                    return Ok(());
                }
            };

            write_json(stream, "200 OK", &candidates)?;
        }
        ("POST", "/api/ui/param-control-info") => {
            let payload: ParamControlInfoRequest = serde_json::from_slice(&request.body).map_err(|err| {
                Error::new(
                    ErrorKind::InvalidData,
                    format!("invalid param-control-info payload: {err}"),
                )
            })?;

            let info_result = state.runtime.param_control_info(payload.param);

            match info_result {
                Ok(info) => {
                    write_json(stream, "200 OK", &info)?;
                }
                Err(err) => {
                    write_application_error(stream, &err)?;
                }
            }
        }
        ("POST", "/api/ui/script-state") => {
            let payload: ScriptStateRequest = serde_json::from_slice(&request.body)
                .map_err(|err| Error::new(ErrorKind::InvalidData, format!("invalid script-state payload: {err}")))?;

            let state_result = state.runtime.script_state(payload.node);

            match state_result {
                Ok(script_state) => {
                    write_json(stream, "200 OK", &script_state)?;
                }
                Err(err) => {
                    write_application_error(stream, &err)?;
                }
            }
        }
        ("POST", "/api/ui/script-config") => {
            let payload: ScriptConfigRequest = serde_json::from_slice(&request.body)
                .map_err(|err| Error::new(ErrorKind::InvalidData, format!("invalid script-config payload: {err}")))?;

            let update_result = state
                .runtime
                .set_script_config(payload.node, payload.config, payload.force_reload);

            match update_result {
                Ok(_) => {
                    write_json(stream, "200 OK", &serde_json::json!({ "ok": true }))?;
                }
                Err(err) => {
                    write_application_error(stream, &err)?;
                }
            }
        }
        ("POST", "/api/ui/script-reload") => {
            let payload: ScriptReloadRequest = serde_json::from_slice(&request.body)
                .map_err(|err| Error::new(ErrorKind::InvalidData, format!("invalid script-reload payload: {err}")))?;

            let reload_result = state.runtime.reload_script(payload.node);

            match reload_result {
                Ok(_) => {
                    write_json(stream, "200 OK", &serde_json::json!({ "ok": true }))?;
                }
                Err(err) => {
                    write_application_error(stream, &err)?;
                }
            }
        }
        ("POST", "/api/ui/project-new") => {
            match project_host::create_new_project(&state.runtime, state.preferences.as_ref()) {
                Ok(_) => {
                    write_json(stream, "200 OK", &serde_json::json!({ "ok": true }))?;
                }
                Err(err) => {
                    write_application_error(stream, &err)?;
                }
            }
        }
        ("POST", "/api/ui/project-save") => {
            let payload: ProjectPathRequest = serde_json::from_slice(&request.body)
                .map_err(|err| Error::new(ErrorKind::InvalidData, format!("invalid project-save payload: {err}")))?;
            match project_host::save_project(&state.runtime, &payload.path, payload.ui_state) {
                Ok(_) => {
                    write_json(stream, "200 OK", &serde_json::json!({ "ok": true }))?;
                }
                Err(err) => {
                    write_application_error(stream, &err)?;
                }
            }
        }
        ("POST", "/api/ui/project-load") => {
            let payload: ProjectPathRequest = serde_json::from_slice(&request.body)
                .map_err(|err| Error::new(ErrorKind::InvalidData, format!("invalid project-load payload: {err}")))?;
            let load = if payload.recover {
                project_host::load_project_recovering(&state.runtime, &payload.path, state.preferences.as_ref())
            } else {
                project_host::load_project(&state.runtime, &payload.path, state.preferences.as_ref())
            };
            match load {
                Ok(load) => {
                    write_json(stream, "200 OK", &project_path_dto(load))?;
                }
                Err(err) => {
                    write_project_load_error(stream, "400 Bad Request", "project-load", &err)?;
                }
            }
        }
        ("POST", "/api/ui/project-upload-load") => {
            let payload: ProjectUploadRequest = serde_json::from_slice(&request.body).map_err(|err| {
                Error::new(
                    ErrorKind::InvalidData,
                    format!("invalid project-upload-load payload: {err}"),
                )
            })?;
            let load = if payload.recover {
                project_host::upload_project_and_load_recovering(
                    &state.runtime,
                    &payload.file_name,
                    &payload.contents,
                    state.preferences.as_ref(),
                )
            } else {
                project_host::upload_project_and_load(
                    &state.runtime,
                    &payload.file_name,
                    &payload.contents,
                    state.preferences.as_ref(),
                )
            };
            match load {
                Ok(load) => {
                    write_json(stream, "200 OK", &project_path_dto(load))?;
                }
                Err(err) => {
                    write_project_load_error(stream, "400 Bad Request", "project-upload-load", &err)?;
                }
            }
        }
        ("POST", "/api/ui/intent") => {
            let intent: UiEditIntent = serde_json::from_slice(&request.body)
                .map_err(|err| Error::new(ErrorKind::InvalidData, format!("invalid intent payload: {err}")))?;
            let client_instance_id = ui_client_instance_id_from_headers(&request.headers);

            let kind = ui_intent_kind(&intent);
            let result = match state
                .runtime
                .apply_ui_transaction(intent, client_instance_id.as_deref())
            {
                Ok(value) => value,
                Err(error) => {
                    write_application_error(stream, &error)?;
                    return Ok(());
                }
            };
            save_preferences_after_change(&state.runtime, state.preferences.as_ref(), result.preferences_changed);
            let timing = ui_intent_timing_from_application(kind, &result);
            log_ui_intent_timing("ui-http", &timing);

            write_json(stream, "200 OK", &result.acknowledgement)?;
        }
        ("POST", "/api/ui/intent/batch") => {
            let intents: Vec<UiEditIntent> = serde_json::from_slice(&request.body)
                .map_err(|err| Error::new(ErrorKind::InvalidData, format!("invalid intent batch payload: {err}")))?;
            if intents.len() > MAX_UI_INTENTS_PER_BATCH {
                write_json_error(
                    stream,
                    "413 Payload Too Large",
                    &format!("intent batch exceeds {MAX_UI_INTENTS_PER_BATCH} operations"),
                )?;
                return Ok(());
            }
            let client_instance_id = ui_client_instance_id_from_headers(&request.headers);

            let total_started = Instant::now();
            let batch = match state
                .runtime
                .apply_ui_transaction_batch(intents, client_instance_id.as_deref(), true)
            {
                Ok(value) => value,
                Err(error) => {
                    write_application_error(stream, &error)?;
                    return Ok(());
                }
            };
            save_preferences_after_change(&state.runtime, state.preferences.as_ref(), batch.preferences_changed);
            let mut total_events = 0usize;
            let mut acks = Vec::<UiAck>::with_capacity(batch.transactions.len());
            let mut timing_rows = Vec::<UiIntentTiming>::with_capacity(batch.transactions.len());
            for result in batch.transactions {
                total_events += result.events.events.len();
                timing_rows.push(ui_intent_timing_from_application("batch", &result));
                acks.push(result.acknowledgement);
            }
            let batch_total_ms = total_started.elapsed().as_millis();
            for timing in &timing_rows {
                log_ui_intent_timing("ui-http", timing);
            }
            if acks.len() > 1 || batch_total_ms >= 5 {
                eprintln!(
                    "[ui-http] intent_batch count={} events={} total_ms={}",
                    acks.len(),
                    total_events,
                    batch_total_ms
                );
            }

            write_json(stream, "200 OK", &acks)?;
        }
        _ => {
            write_json_error(stream, "404 Not Found", "unknown route")?;
        }
    }

    Ok(())
}

pub(super) fn resolve_frontend_asset<'a>(assets: &'a [UiAsset], request_path: &str) -> Option<&'a UiAsset> {
    if assets.is_empty() || request_path.starts_with("/api/") || request_path.starts_with("/.well-known/") {
        return None;
    }

    let normalized_path = normalize_frontend_asset_path(request_path);
    if let Some(asset) = assets.iter().find(|asset| asset.path == normalized_path) {
        return Some(asset);
    }

    if looks_like_asset_path(normalized_path) {
        return None;
    }

    assets.iter().find(|asset| asset.path == "/index.html")
}

pub(super) fn normalize_frontend_asset_path(request_path: &str) -> &str {
    match request_path {
        "" | "/" => "/index.html",
        path => path,
    }
}

pub(super) fn looks_like_asset_path(request_path: &str) -> bool {
    request_path
        .rsplit('/')
        .next()
        .is_some_and(|segment| segment.contains('.'))
}

pub(super) fn is_websocket_upgrade_request(request: &HttpRequest) -> bool {
    request.method.eq_ignore_ascii_case("GET")
        && header_contains_token(request, "connection", "upgrade")
        && request
            .headers
            .get("upgrade")
            .is_some_and(|value| value.eq_ignore_ascii_case("websocket"))
        && request.headers.contains_key("sec-websocket-key")
}

pub(super) fn header_contains_token(request: &HttpRequest, header_name: &str, expected_token: &str) -> bool {
    request.headers.get(header_name).is_some_and(|value| {
        value
            .split(',')
            .any(|part| part.trim().eq_ignore_ascii_case(expected_token))
    })
}

pub(super) fn write_json<T: serde::Serialize>(
    stream: &mut HttpResponse<'_>,
    status: &str,
    payload: &T,
) -> std::io::Result<()> {
    let body = serde_json::to_vec(payload)
        .map_err(|err| Error::new(ErrorKind::InvalidData, format!("failed to serialize json: {err}")))?;
    write_response(stream, status, "application/json; charset=utf-8", &body)
}

pub(super) fn write_json_error(stream: &mut HttpResponse<'_>, status: &str, message: &str) -> std::io::Result<()> {
    write_json(
        stream,
        status,
        &golden_protocol::UiErrorResponse {
            error: message.into(),
            code: None,
        },
    )
}

pub(super) fn project_load_recovery_dto(
    recovery: &golden_engine::engine::ProjectLoadRecoveryReport,
) -> Option<ProjectLoadRecoveryDto> {
    if recovery.is_empty() {
        None
    } else {
        Some(ProjectLoadRecoveryDto::from(recovery))
    }
}

pub(super) fn project_path_dto(load: project_host::ProjectLoadResult) -> ProjectPathDto {
    let recovery = project_load_recovery_dto(&load.recovery);
    ProjectPathDto {
        path: load.path,
        ui_state: load.ui_state,
        recovery,
    }
}

pub(super) fn write_project_load_error(
    stream: &mut HttpResponse<'_>,
    status: &str,
    operation: &str,
    error: &project_host::ProjectLoadError,
) -> std::io::Result<()> {
    let recovery = error.recovery.as_ref().and_then(project_load_recovery_dto);
    let code = error
        .control
        .as_ref()
        .map(|error| application_error_code(&ApplicationError::Control(error.clone())));
    let status = if code.is_some() {
        "503 Service Unavailable"
    } else {
        status
    };
    write_json(
        stream,
        status,
        &serde_json::json!({
            "error": format!("{operation} failed: {error}"),
            "code": code.flatten(),
            "recoverable": recovery.is_some(),
            "recovery": recovery,
        }),
    )
}

pub(super) fn write_response(
    stream: &mut HttpResponse<'_>,
    status: &str,
    content_type: &str,
    body: &[u8],
) -> std::io::Result<()> {
    let cors = stream.cors_origin.as_ref().map_or_else(String::new, |origin| {
        format!(
            "Access-Control-Allow-Origin: {origin}\r\n\
         Access-Control-Allow-Methods: GET, POST, OPTIONS\r\n\
         Access-Control-Allow-Headers: Content-Type, X-GC-UI-Client-Instance, Authorization\r\n"
        )
    });
    let headers = format!(
        "HTTP/1.1 {status}\r\n\
         Content-Type: {content_type}\r\n\
         Content-Length: {}\r\n\
         Vary: Origin\r\n\
         {cors}\
         Connection: close\r\n\r\n",
        body.len()
    );

    stream.stream.write_all(headers.as_bytes())?;
    if !body.is_empty() {
        stream.stream.write_all(body)?;
    }
    stream.stream.flush()?;
    Ok(())
}

pub(super) fn is_client_disconnect_error(err: &Error) -> bool {
    matches!(
        err.kind(),
        ErrorKind::UnexpectedEof
            | ErrorKind::ConnectionReset
            | ErrorKind::ConnectionAborted
            | ErrorKind::BrokenPipe
            | ErrorKind::NotConnected
    )
}
