use super::*;

pub(super) fn prepare_response_callback(response: &HttpResponse) -> Vec<serde_json::Value> {
    let body_arg = response_script_body_arg(response.body.as_slice());
    let is_json =
        content_type_is_json(response.content_type.as_deref()) || body_looks_like_json(response.body.as_slice());
    let json_arg = if is_json {
        response_script_json_arg(response.body.as_slice())
    } else {
        serde_json::Value::Null
    };
    let bytes_arg = if matches!(body_arg, serde_json::Value::String(_)) {
        serde_json::Value::Null
    } else {
        crate::app::module::script_api::bytes_arg(response.body.as_slice())
    };
    let response_arg = serde_json::json!({
        "method": response.request.method.label(),
        "path": response.request.path.as_str(),
        "query": response.request.query.as_str(),
        "valuePath": response.request.value_path.as_str(),
        "url": response.url.as_str(),
        "status": response.status,
        "statusText": response.status_text.as_str(),
        "ok": (200..300).contains(&response.status),
        "headers": headers_script_arg(response.headers.as_slice()),
        "contentType": response.content_type.as_deref(),
        "isJson": is_json && !json_arg.is_null(),
        "body": body_arg.clone(),
        "bodyBytes": response.body.len(),
        "bytes": bytes_arg,
        "json": json_arg,
        "elapsedMs": response.elapsed_ms,
        "attempts": response.attempts,
        "source": origin_script_arg(&response.origin),
    });

    vec![serde_json::json!(response.status), body_arg, response_arg]
}

impl HttpModule {
    pub(super) fn response_received_callback(&self, response: HttpResponse) -> CustomEvent {
        crate::app::module::script_api::script_callback_event(
            self.id(),
            HTTP_RESPONSE_RECEIVED_CALLBACK,
            response.callback_args,
        )
    }

    pub(super) fn request_failed_callback(&self, failure: HttpFailure) -> CustomEvent {
        crate::app::module::script_api::script_callback_event(
            self.id(),
            HTTP_REQUEST_FAILED_CALLBACK,
            failure.callback_args,
        )
    }
}

pub(super) fn prepare_failure_callback(failure: &HttpFailure) -> Vec<serde_json::Value> {
    let failure_arg = serde_json::json!({
        "method": failure.request.method.label(),
        "path": failure.request.path.as_str(),
        "query": failure.request.query.as_str(),
        "valuePath": failure.request.value_path.as_str(),
        "error": failure.error.as_str(),
        "attempts": failure.attempts,
        "source": origin_script_arg(&failure.origin),
    });

    vec![serde_json::json!(failure.error.as_str()), failure_arg]
}

pub(super) fn response_application_failure(response: HttpResponse) -> HttpFailure {
    let mut failure = HttpFailure {
        origin: response.origin,
        request: response.request,
        attempts: response.attempts,
        error: "HTTP response received, but queued value edits did not complete; an applied prefix may remain. The request was sent and is not retried.".into(),
        callback_args: Vec::new(),
    };
    failure.callback_args = prepare_failure_callback(&failure);
    failure
}

fn headers_script_arg(headers: &[HttpHeader]) -> serde_json::Value {
    serde_json::Value::Array(
        headers
            .iter()
            .map(|header| {
                serde_json::json!({
                    "name": header.name.as_str(),
                    "value": header.value.as_str(),
                })
            })
            .collect(),
    )
}

fn origin_script_arg(origin: &HttpRequestOrigin) -> serde_json::Value {
    match origin {
        HttpRequestOrigin::Command {
            command_id,
            command_type,
        } => serde_json::json!({
            "kind": "command",
            "commandId": command_id.0,
            "commandType": command_type.as_str(),
        }),
        HttpRequestOrigin::Script { method } => serde_json::json!({
            "kind": "script",
            "method": method.as_str(),
        }),
    }
}
