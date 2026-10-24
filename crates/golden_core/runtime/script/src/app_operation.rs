//! Bounded, transferable app requests. Domain execution remains with the application host.

use crate::{ScriptAppOperationReply, input};
use golden_io::{PayloadPool, PayloadReservation, RetainedPayload, SharedPayload};
use serde_json::{Value, json};
use std::sync::Mutex;

/// Maximum retained request or response storage, including its envelope.
pub const SCRIPT_APP_OPERATION_BYTES: usize = 32 * 1024 * 1024;
/// Maximum JSON values admitted before materialization.
pub const SCRIPT_APP_OPERATION_VALUES: usize = 2_000_000;
/// Maximum JSON nesting admitted before materialization.
pub const SCRIPT_APP_OPERATION_DEPTH: usize = 128;
const ENVELOPE_BYTES: usize = 8192;

struct RequestData {
    request_id: String,
    namespace: String,
    operation: String,
    payload: Value,
}

/// An admitted request, including capacity for its eventual completion.
pub struct ScriptAppOperationRequest {
    data: RetainedPayload<RequestData>,
    reply: PayloadReservation,
}

/// Immutable result admitted before the application commits its mutation.
pub struct ScriptAppOperationResult {
    value: Value,
    weight: usize,
}

/// Input and response capacity reserved before JSON parsing begins.
pub struct ScriptAppOperationReservation {
    request_id: String,
    namespace: String,
    operation: String,
    input: PayloadReservation,
    reply: PayloadReservation,
}

fn reserve() -> Result<PayloadReservation, String> {
    static POOL: Mutex<Option<PayloadPool>> = Mutex::new(None);
    let mut pool = POOL.lock().unwrap_or_else(|poison| poison.into_inner());
    if pool.is_none() {
        *pool = Some(
            PayloadPool::new("script-app-operations", 64, 512 * 1024 * 1024)
                .map_err(|error| format!("Script app-operation disposer unavailable: {error}"))?,
        );
    }
    pool.as_ref()
        .expect("installed app-operation pool")
        .try_reserve(SCRIPT_APP_OPERATION_BYTES)
        .map_err(|_| "Script app-operation lifetime capacity exhausted".into())
}

impl ScriptAppOperationRequest {
    /// Admit both request and response before constructing JSON. Names use the public app limit.
    pub fn reserve(
        request_id: &str,
        namespace: &str,
        operation: &str,
    ) -> Result<ScriptAppOperationReservation, String> {
        if [request_id, namespace, operation]
            .into_iter()
            .any(|name| name.trim().is_empty() || name.len() > 128)
        {
            return Err("Script request id, namespace and operation require 1..128 bytes".into());
        }
        Ok(ScriptAppOperationReservation {
            request_id: request_id.into(),
            namespace: namespace.into(),
            operation: operation.into(),
            input: reserve()?,
            reply: reserve()?,
        })
    }

    /// Caller correlation key, scoped to the requesting script runtime.
    pub fn request_id(&self) -> &str {
        &self.data.request_id
    }
    /// Public app namespace.
    pub fn namespace(&self) -> &str {
        &self.data.namespace
    }
    /// Public app operation name.
    pub fn operation(&self) -> &str {
        &self.data.operation
    }
    /// Protected immutable input. The application owns decoding its domain DTO.
    pub fn payload(&self) -> &Value {
        &self.data.payload
    }

    /// Check response admission inside the application's rollback boundary, before commit.
    pub fn admit_result(mut value: Value) -> Result<ScriptAppOperationResult, String> {
        let weight = input::json_weight_with_limits(
            &value,
            SCRIPT_APP_OPERATION_BYTES - ENVELOPE_BYTES,
            SCRIPT_APP_OPERATION_VALUES - 8,
            SCRIPT_APP_OPERATION_DEPTH - 4,
        )
        .map_err(str::to_owned)?;
        input::canonicalize_objects(&mut value);
        Ok(ScriptAppOperationResult { value, weight })
    }

    /// Seal the exact result under the pre-admitted response lifetime.
    /// Payload moves into the callback envelope without cloning the authored result.
    pub fn into_callback(self, result: Result<ScriptAppOperationResult, String>) -> SharedPayload<Value> {
        let request_id = self.data.request_id.clone();
        let mut weight = ENVELOPE_BYTES;
        let reply = match result {
            Ok(result) => {
                weight = weight.saturating_add(result.weight);
                ScriptAppOperationReply::Applied {
                    request_id,
                    value: result.value,
                }
            }
            Err(mut message) => {
                if message.capacity() > 4096 {
                    let mut end = message.len().min(4096);
                    while !message.is_char_boundary(end) {
                        end -= 1;
                    }
                    message = message[..end].to_owned();
                }
                weight = weight.saturating_add(message.capacity());
                ScriptAppOperationReply::Rejected { request_id, message }
            }
        };
        // Match the Rust/TS DTO while moving its potentially large JSON value. Ordinary
        // serde_json::to_value would copy it through Serialize; tests lock this shape.
        let reply = match reply {
            ScriptAppOperationReply::Applied { request_id, value } => {
                let mut fields = serde_json::Map::new();
                fields.insert("status".into(), "applied".into());
                fields.insert("request_id".into(), request_id.into());
                fields.insert("value".into(), value);
                Value::Object(fields)
            }
            ScriptAppOperationReply::Rejected { request_id, message } => {
                json!({ "status": "rejected", "request_id": request_id, "message": message })
            }
        };
        let mut fields = serde_json::Map::new();
        fields.insert("callback".into(), "onAppOperationResult".into());
        fields.insert("args".into(), Value::Array(vec![reply]));
        let callback = Value::Object(fields);
        self.reply
            .share_retained(callback, weight)
            .expect("reply fits its reserved lifetime")
    }
}

impl ScriptAppOperationReservation {
    /// Seal parsed JSON under the original reservation, retaining rejected data on its producer.
    pub fn finish(self, mut payload: Value) -> Result<ScriptAppOperationRequest, crate::ScriptInputError<Value>> {
        let weight = match input::json_weight_with_limits(
            &payload,
            SCRIPT_APP_OPERATION_BYTES - ENVELOPE_BYTES,
            SCRIPT_APP_OPERATION_VALUES,
            SCRIPT_APP_OPERATION_DEPTH,
        ) {
            Ok(weight) => weight.saturating_add(1024),
            Err(message) => return Err(crate::ScriptInputError::new(payload, message)),
        };
        input::canonicalize_objects(&mut payload);
        let data = RequestData {
            request_id: self.request_id,
            namespace: self.namespace,
            operation: self.operation,
            payload,
        };
        Ok(ScriptAppOperationRequest {
            data: self.input.retain(data, weight).expect("request fits its reservation"),
            reply: self.reply,
        })
    }
}

#[cfg(test)]
mod tests;
