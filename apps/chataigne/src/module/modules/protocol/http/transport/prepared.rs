use super::{weights, HttpFailure, HttpOutcome, HttpRequestOrigin, HttpResponse, HTTP_EVENT_BYTES};
use crate::app::module::script_api::NativeCallbackPayload;
use golden_core::{events::CustomEvent, node::NodeId};
use golden_io::{PayloadPool, RetainedPayload};
use std::{
    mem::size_of,
    ops::Deref,
    sync::{Mutex, OnceLock},
};

#[derive(Clone, Debug)]
pub(crate) struct PreparedHttpResponse {
    data: RetainedPayload<HttpResponse>,
    callback: NativeCallbackPayload,
    application_failure: NativeCallbackPayload,
    weight: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct PreparedHttpFailure {
    data: RetainedPayload<HttpFailure>,
    callback: NativeCallbackPayload,
    weight: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct HttpResultRejection {
    pub description: String,
    pub command_id: Option<NodeId>,
    pub attempts: u32,
    pub error: String,
}

#[derive(Clone, Debug)]
pub(crate) enum HttpWorkerEvent {
    Response(PreparedHttpResponse),
    Failure(PreparedHttpFailure),
    Rejected(HttpResultRejection),
}

impl HttpWorkerEvent {
    pub(super) fn retained_weight(&self) -> usize {
        match self {
            Self::Response(response) => response.weight,
            Self::Failure(failure) => failure.weight,
            Self::Rejected(rejection) => {
                size_of::<Self>() + rejection.description.capacity() + rejection.error.capacity()
            }
        }
    }
}

impl Deref for PreparedHttpResponse {
    type Target = HttpResponse;
    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl Deref for PreparedHttpFailure {
    type Target = HttpFailure;
    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl PreparedHttpResponse {
    pub(crate) fn response_callback(&self, module: NodeId) -> CustomEvent {
        self.callback.event(module)
    }
    pub(crate) fn application_failure_callback(&self, module: NodeId) -> CustomEvent {
        self.application_failure.event(module)
    }
}

impl PreparedHttpFailure {
    pub(crate) fn callback(&self, module: NodeId) -> CustomEvent {
        self.callback.event(module)
    }
}

impl HttpResultRejection {
    fn from_outcome(outcome: &HttpOutcome) -> Self {
        let (origin, request, attempts) = match outcome {
            HttpOutcome::Response(response) => (&response.origin, &response.request, response.attempts),
            HttpOutcome::Failure(failure) => (&failure.origin, &failure.request, failure.attempts),
        };
        Self {
            description: format!(
                "{} {}",
                request.method.label(),
                super::super::diagnostic_url(&request.path)
            ),
            command_id: match origin {
                HttpRequestOrigin::Command { command_id, .. } => Some(*command_id),
                _ => None,
            },
            attempts,
            error: String::new(),
        }
    }

    fn rejected(mut self, error: &str) -> HttpWorkerEvent {
        self.error = bounded(error);
        HttpWorkerEvent::Rejected(self)
    }
}

/// Runs on the HTTP worker before publication. Never return large unretained payloads
/// to the owner, including when storage admission or callback preparation fails.
pub(crate) fn prepare(outcome: HttpOutcome) -> HttpWorkerEvent {
    let pool = match result_pool() {
        Ok(pool) => pool,
        Err(error) => return HttpResultRejection::from_outcome(&outcome).rejected(&error),
    };
    prepare_in(outcome, &pool, NativeCallbackPayload::prepare)
}

fn prepare_in(
    mut outcome: HttpOutcome,
    pool: &PayloadPool,
    mut prepare_callback: impl FnMut(&str, Vec<serde_json::Value>) -> Result<NativeCallbackPayload, String>,
) -> HttpWorkerEvent {
    if let HttpOutcome::Failure(failure) = &mut outcome {
        failure.error = bounded(&failure.error);
    }
    let mut args = callback_args(&outcome);
    let validation = weights::event_weight(&outcome)
        .and_then(|data| args_weight(&args).map(|callbacks| data.saturating_add(callbacks)));
    if validation.is_err() || validation.is_ok_and(|weight| weight > HTTP_EVENT_BYTES) {
        outcome = limited_failure(outcome);
        args = callback_args(&outcome);
    }
    let rejection = HttpResultRejection::from_outcome(&outcome);
    let result: Result<HttpWorkerEvent, String> = (|| {
        let data_weight = weights::event_weight(&outcome)? + 2 * size_of::<usize>();
        args_weight(&args)?;
        match outcome {
            HttpOutcome::Response(response) => {
                let callback = prepare_callback(super::super::HTTP_RESPONSE_RECEIVED_CALLBACK, args)?;
                let failure_args = super::super::callbacks::prepare_application_failure_callback(&response);
                args_weight(&failure_args)?;
                let application_failure = prepare_callback(super::super::HTTP_REQUEST_FAILED_CALLBACK, failure_args)?;
                let weight = data_weight
                    + callback.retained_bytes()
                    + application_failure.retained_bytes()
                    + size_of::<PreparedHttpResponse>()
                    + size_of::<HttpWorkerEvent>();
                check_weight(weight)?;
                let data = pool
                    .try_retain(response, data_weight)
                    .map_err(|_| "HTTP result lifetime capacity exhausted".to_string())?;
                Ok(HttpWorkerEvent::Response(PreparedHttpResponse {
                    data,
                    callback,
                    application_failure,
                    weight,
                }))
            }
            HttpOutcome::Failure(failure) => {
                let callback = prepare_callback(super::super::HTTP_REQUEST_FAILED_CALLBACK, args)?;
                let weight = data_weight
                    + callback.retained_bytes()
                    + size_of::<PreparedHttpFailure>()
                    + size_of::<HttpWorkerEvent>();
                check_weight(weight)?;
                let data = pool
                    .try_retain(failure, data_weight)
                    .map_err(|_| "HTTP result lifetime capacity exhausted".to_string())?;
                Ok(HttpWorkerEvent::Failure(PreparedHttpFailure { data, callback, weight }))
            }
        }
    })();
    match result {
        Ok(event) => event,
        Err(error) => rejection.rejected(&error),
    }
}

fn callback_args(outcome: &HttpOutcome) -> Vec<serde_json::Value> {
    match outcome {
        HttpOutcome::Response(response) => super::super::callbacks::prepare_response_callback(response),
        HttpOutcome::Failure(failure) => super::super::callbacks::prepare_failure_callback(failure),
    }
}

fn args_weight(args: &[serde_json::Value]) -> Result<usize, String> {
    args.iter().try_fold(0usize, |weight, arg| {
        Ok(weight.saturating_add(weights::json_weight(arg)?))
    })
}

fn check_weight(weight: usize) -> Result<(), String> {
    if weight > HTTP_EVENT_BYTES {
        Err("HTTP prepared result exceeds 32 MiB".into())
    } else {
        Ok(())
    }
}

fn limited_failure(outcome: HttpOutcome) -> HttpOutcome {
    let (origin, request, attempts) = match outcome {
        HttpOutcome::Response(response) => (response.origin, response.request, response.attempts),
        HttpOutcome::Failure(failure) => (failure.origin, failure.request, failure.attempts),
    };
    HttpOutcome::Failure(HttpFailure {
        origin,
        request,
        attempts,
        error: "HTTP response exceeds retained payload or JSON complexity limits".into(),
    })
}

fn bounded(text: &str) -> String {
    let mut end = text.len().min(4096);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_string()
}

fn result_pool() -> Result<PayloadPool, String> {
    static POOL: OnceLock<Mutex<Option<PayloadPool>>> = OnceLock::new();
    let mut pool = POOL
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    if pool.is_none() {
        *pool = Some(
            PayloadPool::new("chataigne-http-results", 256, 256 * 1024 * 1024)
                .map_err(|error| format!("failed to start HTTP result disposer: {error}"))?,
        );
    }
    Ok(pool.as_ref().expect("installed HTTP result disposer").clone())
}

#[cfg(test)]
mod tests;
