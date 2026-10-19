use std::{
    io::Read,
    num::NonZeroUsize,
    sync::OnceLock,
    time::{Duration, Instant},
};

const HTTP_REQUEST_CHANNEL_CAPACITY: usize = 256;
const HTTP_REQUEST_BYTES: usize = 8 * 1024 * 1024;
pub(super) const HTTP_RESPONSE_CAPACITY: usize = 4;
pub(super) const HTTP_EVENT_BYTES: usize = 32 * 1024 * 1024;
pub(super) const HTTP_RESPONSE_BODY_BYTES: usize = 1024 * 1024;

use golden_io::{
    BoundedWorker, PendingDrain, PendingSendError, RetirementPool, WorkerCancellation, WorkerContext, WorkerLimits,
};

mod generation;
mod weights;
pub(super) use generation::GenerationFence;

pub(super) fn worker_lifetimes() -> &'static RetirementPool {
    static POOL: OnceLock<RetirementPool> = OnceLock::new();
    POOL.get_or_init(|| RetirementPool::new(NonZeroUsize::new(16).unwrap()))
}

use reqwest::{
    blocking::{multipart, Client, RequestBuilder},
    header::{HeaderName, HeaderValue, CONTENT_TYPE},
    StatusCode, Url,
};

use crate::app::module::common::http::{
    response_json_received_values, HttpAuthConfig, HttpAuthMode, HttpHeader, HttpReceivedValue, HttpRequestBody,
    HttpRequestPayload, HTTP_AUTH_BASIC, HTTP_AUTH_BEARER, HTTP_AUTH_HEADER,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct HttpTransportConfig {
    pub base_address: String,
    pub default_headers: Vec<HttpHeader>,
    pub auth: HttpAuthConfig,
    pub timeout: Duration,
    pub retry_count: u32,
    pub retry_delay: Duration,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct HttpQueuedRequest {
    pub origin: HttpRequestOrigin,
    pub payload: HttpRequestPayload,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum HttpRequestOrigin {
    Command {
        command_id: golden_core::node::NodeId,
        command_type: String,
    },
    Script {
        method: String,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct HttpResponse {
    pub origin: HttpRequestOrigin,
    pub request: HttpRequestPayload,
    pub url: String,
    pub status: u16,
    pub status_text: String,
    pub headers: Vec<HttpHeader>,
    pub content_type: Option<String>,
    pub body: Vec<u8>,
    pub received_values: Result<Option<Vec<HttpReceivedValue>>, String>,
    pub elapsed_ms: u128,
    pub attempts: u32,
    pub callback_args: Vec<serde_json::Value>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct HttpFailure {
    pub origin: HttpRequestOrigin,
    pub request: HttpRequestPayload,
    pub error: String,
    pub attempts: u32,
    pub callback_args: Vec<serde_json::Value>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum HttpWorkerEvent {
    Response(HttpResponse),
    Failure(HttpFailure),
}

pub(crate) struct HttpTransportHandle {
    worker: BoundedWorker<HttpQueuedRequest, HttpWorkerEvent>,
    completion: GenerationFence,
    predecessor: Option<GenerationFence>,
}

type HttpExecutor = dyn FnMut(
        &Client,
        &HttpTransportConfig,
        &HttpQueuedRequest,
        &WorkerCancellation,
        &mut AttemptProgress,
    ) -> Option<HttpWorkerEvent>
    + Send;

impl HttpTransportHandle {
    #[cfg(test)]
    pub(super) fn spawn(config: HttpTransportConfig) -> Result<Self, String> {
        Self::spawn_with_lifetimes(config, &RetirementPool::new(NonZeroUsize::MIN))
    }

    #[cfg(test)]
    pub(super) fn spawn_with_lifetimes(
        config: HttpTransportConfig,
        lifetimes: &RetirementPool,
    ) -> Result<Self, String> {
        Self::spawn_generation(config, lifetimes, None)
    }

    pub(super) fn spawn_generation(
        config: HttpTransportConfig,
        lifetimes: &RetirementPool,
        predecessor: Option<GenerationFence>,
    ) -> Result<Self, String> {
        Self::spawn_using(config, lifetimes, predecessor, execute_with_retries)
    }

    pub(super) fn spawn_using<F>(
        config: HttpTransportConfig,
        lifetimes: &RetirementPool,
        predecessor: Option<GenerationFence>,
        execute: F,
    ) -> Result<Self, String>
    where
        F: FnMut(
                &Client,
                &HttpTransportConfig,
                &HttpQueuedRequest,
                &WorkerCancellation,
                &mut AttemptProgress,
            ) -> Option<HttpWorkerEvent>
            + Send
            + 'static,
    {
        Self::spawn_worker(config, lifetimes, predecessor, Box::new(execute), None)
    }

    #[cfg(test)]
    pub(super) fn spawn_with_retirement_hook<F, R>(
        config: HttpTransportConfig,
        lifetimes: &RetirementPool,
        predecessor: Option<GenerationFence>,
        execute: F,
        retire: R,
    ) -> Result<Self, String>
    where
        F: FnMut(
                &Client,
                &HttpTransportConfig,
                &HttpQueuedRequest,
                &WorkerCancellation,
                &mut AttemptProgress,
            ) -> Option<HttpWorkerEvent>
            + Send
            + 'static,
        R: FnOnce() + Send + 'static,
    {
        Self::spawn_worker(
            config,
            lifetimes,
            predecessor,
            Box::new(execute),
            Some(Box::new(retire)),
        )
    }

    fn spawn_worker(
        config: HttpTransportConfig,
        lifetimes: &RetirementPool,
        predecessor: Option<GenerationFence>,
        execute: Box<HttpExecutor>,
        retirement_hook: Option<Box<dyn FnOnce() + Send>>,
    ) -> Result<Self, String> {
        let completion = GenerationFence::default();
        let worker_completion = completion.clone();
        let worker_predecessor = predecessor.clone();
        let worker = BoundedWorker::spawn(
            "chataigne-http",
            lifetimes,
            WorkerLimits {
                command_items: HTTP_REQUEST_CHANNEL_CAPACITY,
                command_weight: HTTP_REQUEST_BYTES,
                event_items: HTTP_RESPONSE_CAPACITY,
                event_weight: HTTP_EVENT_BYTES,
            },
            move |context| {
                let mut complete = worker_completion.guard();
                let ready = worker_predecessor
                    .as_ref()
                    .is_none_or(|previous| previous.wait(context.cancellation()));
                worker_loop(config, context, ready, &mut complete, execute, retirement_hook);
            },
        )
        .map_err(|error| format!("failed to start HTTP worker: {error}"))?;
        Ok(Self {
            worker,
            completion,
            predecessor,
        })
    }

    pub(crate) fn send(&self, request: HttpQueuedRequest) -> Result<(), String> {
        let weight = weights::request_weight(&request)?;
        self.worker.send_weighted(request, weight).map_err(|error| match error {
            PendingSendError::Full(_) => "HTTP request queue item/byte capacity is full".to_string(),
            PendingSendError::Disconnected(_) => "HTTP worker is no longer running".to_string(),
        })
    }

    pub(crate) fn drain_events(&self, events: &mut Vec<HttpWorkerEvent>, budget: NonZeroUsize) -> PendingDrain {
        self.worker.drain_events(events, budget)
    }

    pub(crate) fn has_pending(&self) -> bool {
        self.worker.has_pending() || self.completion.is_drained()
    }

    pub(crate) fn stop(&mut self) {
        self.worker.cancel();
        if let Some(previous) = &self.predecessor {
            previous.wake();
        }
    }

    pub(super) fn finish_admission(&mut self) {
        self.worker.finish_admission();
    }

    pub(super) fn completion_fence(&self) -> GenerationFence {
        self.completion.clone()
    }

    pub(super) fn requests_drained(&self) -> bool {
        self.completion.is_drained()
    }
}

impl Drop for HttpTransportHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

#[derive(Default)]
pub(super) struct AttemptProgress {
    pub attempts: u32,
}

fn worker_loop(
    config: HttpTransportConfig,
    context: WorkerContext<HttpQueuedRequest, HttpWorkerEvent>,
    predecessor_ready: bool,
    complete: &mut generation::CompletionGuard,
    mut execute: Box<HttpExecutor>,
    retirement_hook: Option<Box<dyn FnOnce() + Send>>,
) {
    if context.cancellation().is_cancelled() {
        return;
    }
    // Client construction, blocking request I/O, and client/runtime destruction stay here.
    let mut unavailable =
        (!predecessor_ready).then(|| "previous HTTP generation ended before resolving its requests".to_string());
    let client = if unavailable.is_none() {
        match Client::builder()
            .timeout(config.timeout)
            .connect_timeout(config.timeout.min(Duration::from_secs(10)))
            .build()
        {
            Ok(client) => Some(client),
            Err(error) => {
                unavailable = Some(format!("failed to build HTTP client: {error}"));
                None
            }
        }
    } else {
        None
    };
    if unavailable.is_some() {
        context.close_command_admission();
    }
    while let Some(request) = context.receive() {
        let event = if let Some(error) = &unavailable {
            Some(failed_request(request, error.clone(), 0))
        } else {
            let mut progress = AttemptProgress::default();
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                execute(
                    client.as_ref().expect("available HTTP client"),
                    &config,
                    &request,
                    context.cancellation(),
                    &mut progress,
                )
            })) {
                Ok(result) => {
                    drop(request);
                    result
                }
                Err(_) => {
                    let error =
                        "HTTP request worker failed; earlier attempts may have reached the endpoint".to_string();
                    unavailable = Some(
                        "HTTP worker unavailable after an earlier request failed; this request was not sent".into(),
                    );
                    context.close_command_admission();
                    Some(failed_request(request, error, progress.attempts))
                }
            }
        };
        let Some(mut event) = event else { return };
        if let HttpWorkerEvent::Failure(failure) = &mut event {
            // A diagnostic may quote a large admitted URL. Keep the failure reply smaller
            // than the request budget so reporting an error cannot strand later commands.
            let mut end = failure.error.len().min(4096);
            while !failure.error.is_char_boundary(end) {
                end -= 1;
            }
            failure.error.truncate(end);
            failure.error.shrink_to_fit();
            failure.callback_args = super::callbacks::prepare_failure_callback(failure);
        }
        let weight = match weights::event_weight(&event) {
            Ok(weight) if weight <= HTTP_EVENT_BYTES => weight,
            _ => {
                let (origin, request, attempts) = match event {
                    HttpWorkerEvent::Response(response) => (response.origin, response.request, response.attempts),
                    HttpWorkerEvent::Failure(failure) => (failure.origin, failure.request, failure.attempts),
                };
                event = HttpWorkerEvent::Failure(HttpFailure {
                    origin,
                    request,
                    attempts,
                    error: "HTTP response exceeds retained payload or JSON complexity limits".into(),
                    callback_args: Vec::new(),
                });
                if let HttpWorkerEvent::Failure(failure) = &mut event {
                    failure.callback_args = super::callbacks::prepare_failure_callback(failure);
                }
                weights::event_weight(&event).expect("admitted request fits in a failure reply")
            }
        };
        if context.publish_weighted(event, weight).is_err() {
            return;
        }
    }
    if !context.cancellation().is_cancelled() {
        complete.drained();
        if let Some(retire) = retirement_hook {
            retire();
        }
    }
}

fn failed_request(request: HttpQueuedRequest, error: String, attempts: u32) -> HttpWorkerEvent {
    HttpWorkerEvent::Failure(HttpFailure {
        origin: request.origin,
        request: request.payload,
        error,
        attempts,
        callback_args: Vec::new(),
    })
}

fn execute_with_retries(
    client: &Client,
    config: &HttpTransportConfig,
    request: &HttpQueuedRequest,
    cancellation: &WorkerCancellation,
    progress: &mut AttemptProgress,
) -> Option<HttpWorkerEvent> {
    let max_attempts = config.retry_count.saturating_add(1).max(1);
    let started_at = Instant::now();
    let mut attempts = 0;
    let mut last_error = None;

    while attempts < max_attempts {
        if cancellation.is_cancelled() {
            return None;
        }
        attempts += 1;
        progress.attempts = attempts;
        match execute_once(client, config, request, started_at, attempts) {
            Ok(response) if should_retry_status(response.status) && attempts < max_attempts => {
                last_error = Some(format!(
                    "HTTP {} returned status {}",
                    response.request.method.label(),
                    response.status
                ));
                if wait_before_retry(config, attempts, cancellation) {
                    return None;
                }
            }
            Ok(response) => return Some(HttpWorkerEvent::Response(response)),
            Err(error) if attempts < max_attempts => {
                last_error = Some(error);
                if wait_before_retry(config, attempts, cancellation) {
                    return None;
                }
            }
            Err(error) => {
                return Some(HttpWorkerEvent::Failure(HttpFailure {
                    origin: request.origin.clone(),
                    request: request.payload.clone(),
                    error,
                    attempts,
                    callback_args: Vec::new(),
                }));
            }
        }
    }

    Some(HttpWorkerEvent::Failure(HttpFailure {
        origin: request.origin.clone(),
        request: request.payload.clone(),
        error: last_error.unwrap_or_else(|| "HTTP request failed without a response".to_string()),
        attempts,
        callback_args: Vec::new(),
    }))
}

fn execute_once(
    client: &Client,
    config: &HttpTransportConfig,
    request: &HttpQueuedRequest,
    started_at: Instant,
    attempts: u32,
) -> Result<HttpResponse, String> {
    let url = resolve_request_url(
        config.base_address.as_str(),
        request.payload.path.as_str(),
        request.payload.query.as_str(),
    )?;
    let url_text = url.as_str().to_string();
    let builder = client.request(request.payload.method.to_reqwest(), url);
    let builder = apply_headers(builder, config.default_headers.as_slice())?;
    let builder = apply_headers(builder, request.payload.headers.as_slice())?;
    let builder = apply_auth(builder, &config.auth)?;
    let builder = apply_body(builder, &request.payload.body)?;

    let response = builder
        .send()
        .map_err(|error| format!("HTTP request to {url_text} failed: {error}"))?;
    let status = response.status();
    let status_text = status
        .canonical_reason()
        .map(str::to_string)
        .unwrap_or_else(|| status.to_string());
    let headers = response_headers(response.headers())?;
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    if response
        .content_length()
        .is_some_and(|length| length > HTTP_RESPONSE_BODY_BYTES as u64)
    {
        return Err(format!(
            "HTTP response from {url_text} exceeds the {}-byte body limit",
            HTTP_RESPONSE_BODY_BYTES
        ));
    }
    let mut body = Vec::new();
    response
        .take(HTTP_RESPONSE_BODY_BYTES as u64 + 1)
        .read_to_end(&mut body)
        .map_err(|error| format!("failed to read HTTP response from {url_text}: {error}"))?;
    if body.len() > HTTP_RESPONSE_BODY_BYTES {
        return Err(format!(
            "HTTP response from {url_text} exceeds the {}-byte body limit",
            HTTP_RESPONSE_BODY_BYTES
        ));
    }

    let received_values = response_json_received_values(
        body.as_slice(),
        content_type.as_deref(),
        request.payload.path.as_str(),
        request.payload.value_path.as_str(),
    );

    let mut response = HttpResponse {
        origin: request.origin.clone(),
        request: request.payload.clone(),
        url: url_text,
        status: status.as_u16(),
        status_text,
        headers,
        content_type,
        body,
        received_values,
        elapsed_ms: started_at.elapsed().as_millis(),
        attempts,
        callback_args: Vec::new(),
    };
    response.callback_args = super::callbacks::prepare_response_callback(&response);
    Ok(response)
}

fn resolve_request_url(base_address: &str, path: &str, query: &str) -> Result<Url, String> {
    let base =
        Url::parse(base_address).map_err(|error| format!("invalid HTTP base address '{base_address}': {error}"))?;
    let path = path.trim();
    if path.contains("://") || path.starts_with("//") {
        return Err("HTTP request paths must be relative to the module Base address".to_string());
    }

    let relative = path.trim_start_matches('/');
    let mut url = base
        .join(relative)
        .map_err(|error| format!("failed to resolve HTTP request path '{path}': {error}"))?;
    append_query(&mut url, query);
    Ok(url)
}

fn append_query(url: &mut Url, query: &str) {
    let query = query.trim().trim_start_matches('?');
    if query.is_empty() {
        return;
    }

    let merged = match url.query() {
        Some(existing) if !existing.trim().is_empty() => format!("{existing}&{query}"),
        _ => query.to_string(),
    };
    url.set_query(Some(merged.as_str()));
}

fn apply_headers(mut builder: RequestBuilder, headers: &[HttpHeader]) -> Result<RequestBuilder, String> {
    for header in headers {
        let name = HeaderName::from_bytes(header.name.as_bytes())
            .map_err(|error| format!("invalid HTTP header name '{}': {error}", header.name))?;
        let value = HeaderValue::from_str(header.value.as_str())
            .map_err(|error| format!("invalid HTTP header '{}': {error}", header.name))?;
        builder = builder.header(name, value);
    }
    Ok(builder)
}

fn apply_auth(builder: RequestBuilder, auth: &HttpAuthConfig) -> Result<RequestBuilder, String> {
    match auth.mode {
        HttpAuthMode::None => Ok(builder),
        HttpAuthMode::Basic => {
            if auth.username.trim().is_empty() {
                return Err(format!("{HTTP_AUTH_BASIC} auth requires a username"));
            }
            Ok(builder.basic_auth(auth.username.clone(), Some(auth.password.clone())))
        }
        HttpAuthMode::Bearer => {
            let token = auth.token.trim();
            if token.is_empty() {
                return Err(format!("{HTTP_AUTH_BEARER} auth requires a token"));
            }
            Ok(builder.bearer_auth(token.to_string()))
        }
        HttpAuthMode::Header => {
            if auth.header_name.trim().is_empty() {
                return Err(format!("{HTTP_AUTH_HEADER} auth requires a header name"));
            }
            apply_headers(
                builder,
                &[HttpHeader {
                    name: auth.header_name.clone(),
                    value: auth.header_value.clone(),
                }],
            )
        }
    }
}

fn apply_body(builder: RequestBuilder, body: &HttpRequestBody) -> Result<RequestBuilder, String> {
    match body {
        HttpRequestBody::Empty => Ok(builder),
        HttpRequestBody::Text { text, content_type } => {
            let builder = if content_type.trim().is_empty() {
                builder
            } else {
                builder.header(CONTENT_TYPE, content_type.trim())
            };
            Ok(builder.body(text.clone()))
        }
        HttpRequestBody::Json { value } => Ok(builder.json(value)),
        HttpRequestBody::Form { fields } => {
            let pairs = fields
                .iter()
                .map(|field| (field.name.as_str(), field.value.as_str()))
                .collect::<Vec<_>>();
            Ok(builder.form(&pairs))
        }
        HttpRequestBody::Multipart { fields, files } => {
            let mut form = multipart::Form::new();
            for field in fields {
                form = form.text(field.name.clone(), field.value.clone());
            }
            for file in files {
                let mut part = multipart::Part::file(file.path.as_str())
                    .map_err(|error| format!("failed to open upload file '{}': {error}", file.path))?;
                if let Some(file_name) = file.file_name.as_ref().filter(|value| !value.trim().is_empty()) {
                    part = part.file_name(file_name.clone());
                }
                if let Some(content_type) = file.content_type.as_ref().filter(|value| !value.trim().is_empty()) {
                    part = part
                        .mime_str(content_type)
                        .map_err(|error| format!("invalid upload content type '{content_type}': {error}"))?;
                }
                form = form.part(file.field_name.clone(), part);
            }
            Ok(builder.multipart(form))
        }
    }
}

fn response_headers(headers: &reqwest::header::HeaderMap) -> Result<Vec<HttpHeader>, String> {
    headers
        .iter()
        .map(|(name, value)| {
            Ok(HttpHeader {
                name: name.as_str().to_string(),
                value: value
                    .to_str()
                    .map_err(|error| format!("HTTP response header '{}' is not valid UTF-8: {error}", name.as_str()))?
                    .to_string(),
            })
        })
        .collect()
}

fn should_retry_status(status: u16) -> bool {
    let Ok(status) = StatusCode::from_u16(status) else {
        return false;
    };

    status == StatusCode::REQUEST_TIMEOUT || status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error()
}

fn wait_before_retry(config: &HttpTransportConfig, completed_attempts: u32, cancellation: &WorkerCancellation) -> bool {
    let multiplier = completed_attempts.max(1);
    cancellation.wait(config.retry_delay.saturating_mul(multiplier))
}
