use std::{
    sync::mpsc::{self, Receiver, Sender, SyncSender, TrySendError},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

const HTTP_REQUEST_CHANNEL_CAPACITY: usize = 256;

use reqwest::{
    blocking::{multipart, Client, RequestBuilder},
    header::{HeaderName, HeaderValue, CONTENT_TYPE},
    StatusCode, Url,
};

use crate::app::module::common::http::{
    HttpAuthConfig, HttpAuthMode, HttpHeader, HttpReceivedValue, HttpRequestBody, HttpRequestPayload,
    response_json_received_values, HTTP_AUTH_BASIC, HTTP_AUTH_BEARER, HTTP_AUTH_HEADER,
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
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct HttpFailure {
    pub origin: HttpRequestOrigin,
    pub request: HttpRequestPayload,
    pub error: String,
    pub attempts: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum HttpWorkerEvent {
    Response(HttpResponse),
    Failure(HttpFailure),
}

pub(crate) struct HttpTransportHandle {
    command_tx: SyncSender<HttpWorkerCommand>,
    event_rx: Receiver<HttpWorkerEvent>,
    worker: Option<JoinHandle<()>>,
}

impl HttpTransportHandle {
    pub(crate) fn spawn(config: HttpTransportConfig) -> Result<Self, String> {
        let client = Client::builder()
            .timeout(config.timeout)
            .connect_timeout(config.timeout.min(Duration::from_secs(10)))
            .build()
            .map_err(|error| format!("failed to build HTTP client: {error}"))?;

        let (command_tx, command_rx) = mpsc::sync_channel(HTTP_REQUEST_CHANNEL_CAPACITY);
        let (event_tx, event_rx) = mpsc::channel();
        let thread_name = http_thread_name(config.base_address.as_str());

        let worker = thread::Builder::new()
            .name(thread_name)
            .spawn(move || worker_loop(client, config, command_rx, event_tx))
            .map_err(|error| format!("failed to start HTTP worker thread: {error}"))?;

        Ok(Self {
            command_tx,
            event_rx,
            worker: Some(worker),
        })
    }

    pub(crate) fn send(&self, request: HttpQueuedRequest) -> Result<(), String> {
        self.command_tx
            .try_send(HttpWorkerCommand::Send(Box::new(request)))
            .map_err(|error| match error {
                TrySendError::Full(_) => "HTTP request queue is full".to_string(),
                TrySendError::Disconnected(_) => "HTTP worker is no longer running".to_string(),
            })
    }

    pub(crate) fn try_recv(&self) -> Result<HttpWorkerEvent, mpsc::TryRecvError> {
        self.event_rx.try_recv()
    }

    pub(crate) fn stop(&mut self) {
        let _ = self.command_tx.send(HttpWorkerCommand::Stop);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Drop for HttpTransportHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

enum HttpWorkerCommand {
    Send(Box<HttpQueuedRequest>),
    Stop,
}

fn worker_loop(
    client: Client,
    config: HttpTransportConfig,
    command_rx: Receiver<HttpWorkerCommand>,
    event_tx: Sender<HttpWorkerEvent>,
) {
    while let Ok(command) = command_rx.recv() {
        match command {
            HttpWorkerCommand::Send(request) => {
                if send_worker_event(&event_tx, execute_with_retries(&client, &config, *request)) {
                    continue;
                }
                break;
            }
            HttpWorkerCommand::Stop => break,
        }
    }
}

fn send_worker_event(
    event_tx: &Sender<HttpWorkerEvent>,
    event: HttpWorkerEvent,
) -> bool {
    event_tx.send(event).is_ok()
}

fn execute_with_retries(
    client: &Client,
    config: &HttpTransportConfig,
    request: HttpQueuedRequest,
) -> HttpWorkerEvent {
    let max_attempts = config.retry_count.saturating_add(1).max(1);
    let started_at = Instant::now();
    let mut attempts = 0;
    let mut last_error = None;

    while attempts < max_attempts {
        attempts += 1;
        match execute_once(client, config, &request, started_at, attempts) {
            Ok(response) if should_retry_status(response.status) && attempts < max_attempts => {
                last_error = Some(format!(
                    "HTTP {} returned status {}",
                    response.request.method.label(),
                    response.status
                ));
                sleep_before_retry(config, attempts);
            }
            Ok(response) => return HttpWorkerEvent::Response(response),
            Err(error) if attempts < max_attempts => {
                last_error = Some(error);
                sleep_before_retry(config, attempts);
            }
            Err(error) => {
                return HttpWorkerEvent::Failure(HttpFailure {
                    origin: request.origin,
                    request: request.payload,
                    error,
                    attempts,
                });
            }
        }
    }

    HttpWorkerEvent::Failure(HttpFailure {
        origin: request.origin,
        request: request.payload,
        error: last_error.unwrap_or_else(|| "HTTP request failed without a response".to_string()),
        attempts,
    })
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
    let body = response
        .bytes()
        .map_err(|error| format!("failed to read HTTP response from {url_text}: {error}"))?
        .to_vec();

    let received_values = response_json_received_values(
        body.as_slice(),
        content_type.as_deref(),
        request.payload.path.as_str(),
        request.payload.value_path.as_str(),
    );

    Ok(HttpResponse {
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
    })
}

fn resolve_request_url(base_address: &str, path: &str, query: &str) -> Result<Url, String> {
    let base = Url::parse(base_address)
        .map_err(|error| format!("invalid HTTP base address '{base_address}': {error}"))?;
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

fn apply_headers(
    mut builder: RequestBuilder,
    headers: &[HttpHeader],
) -> Result<RequestBuilder, String> {
    for header in headers {
        let name = HeaderName::from_bytes(header.name.as_bytes())
            .map_err(|error| format!("invalid HTTP header name '{}': {error}", header.name))?;
        let value = HeaderValue::from_str(header.value.as_str())
            .map_err(|error| format!("invalid HTTP header '{}': {error}", header.name))?;
        builder = builder.header(name, value);
    }
    Ok(builder)
}

fn apply_auth(
    builder: RequestBuilder,
    auth: &HttpAuthConfig,
) -> Result<RequestBuilder, String> {
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

fn apply_body(
    builder: RequestBuilder,
    body: &HttpRequestBody,
) -> Result<RequestBuilder, String> {
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

    status == StatusCode::REQUEST_TIMEOUT
        || status == StatusCode::TOO_MANY_REQUESTS
        || status.is_server_error()
}

fn sleep_before_retry(config: &HttpTransportConfig, completed_attempts: u32) {
    if config.retry_delay.is_zero() {
        return;
    }
    let multiplier = completed_attempts.max(1);
    thread::sleep(config.retry_delay.saturating_mul(multiplier));
}

fn http_thread_name(base_address: &str) -> String {
    let suffix = base_address
        .chars()
        .map(|character| if character.is_ascii_alphanumeric() { character } else { '-' })
        .collect::<String>();
    format!("http-{suffix}")
}
