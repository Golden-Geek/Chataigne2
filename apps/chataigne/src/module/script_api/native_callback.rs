use std::{
    mem::size_of,
    sync::{Arc, Mutex, OnceLock},
};

use golden_core::{
    events::{CustomEvent, CustomEventRetention},
    node::NodeId,
};
use golden_io::PayloadPool;
use serde_json::Value;

const MAX_CALLBACK_BYTES: usize = 32 * 1024 * 1024;
const MAX_CALLBACK_VALUES: usize = 262_144;
const MAX_CALLBACK_DEPTH: usize = 64;

/// Prepared on a host worker. Its canonical reservation follows ordinary event Arc
/// copies, including routing and replay owners, after the preparing envelope is gone.
#[derive(Clone, Debug)]
pub(crate) struct NativeCallbackPayload {
    payload: Arc<Value>,
    retained_bytes: usize,
}

impl NativeCallbackPayload {
    pub(crate) fn prepare(callback: &str, args: Vec<Value>) -> Result<Self, String> {
        Self::prepare_in(&callback_pool()?, callback, args)
    }

    fn prepare_in(pool: &PayloadPool, callback: &str, args: Vec<Value>) -> Result<Self, String> {
        let payload = Arc::new(super::script_callback_payload(callback, args));
        let retained_bytes = json_weight(&payload)? + 2 * size_of::<usize>();
        pool.try_retain_arc(payload.clone(), retained_bytes)
            .map_err(|_| "native callback lifetime capacity exhausted; callback was not admitted".to_string())?;
        Ok(Self {
            payload,
            retained_bytes,
        })
    }

    pub(crate) fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }

    #[cfg(test)]
    pub(crate) fn payload(&self) -> &Value {
        &self.payload
    }

    /// Binding a prepared payload performs no JSON traversal, copy, or serialization.
    pub(crate) fn event(&self, module: NodeId) -> CustomEvent {
        CustomEvent {
            topic: super::MODULE_SCRIPT_CALLBACK_TOPIC.into(),
            origin: Some(module),
            payload: self.payload.clone(),
            retention: CustomEventRetention::Replay,
        }
    }
}

fn callback_pool() -> Result<PayloadPool, String> {
    static POOL: OnceLock<Mutex<Option<PayloadPool>>> = OnceLock::new();
    let mut pool = POOL
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    if pool.is_none() {
        *pool = Some(
            PayloadPool::new("chataigne-native-callbacks", 16_384, 256 * 1024 * 1024)
                .map_err(|error| format!("failed to start native callback disposer: {error}"))?,
        );
    }
    Ok(pool.as_ref().expect("installed callback disposer").clone())
}

fn json_weight(value: &Value) -> Result<usize, String> {
    fn visit(value: &Value, nodes: &mut usize, depth: usize) -> Result<usize, String> {
        *nodes += 1;
        if *nodes > MAX_CALLBACK_VALUES || depth > MAX_CALLBACK_DEPTH {
            return Err("native callback exceeds 262144 values / 64 levels; callback was not admitted".into());
        }
        let mut weight = size_of::<Value>();
        match value {
            Value::String(value) => weight += value.capacity(),
            Value::Array(values) => {
                weight = weight.saturating_add(values.capacity().saturating_mul(size_of::<Value>()));
                for value in values {
                    weight = weight.saturating_add(visit(value, nodes, depth + 1)?);
                    check_bytes(weight)?;
                }
            }
            Value::Object(values) => {
                // Cover map nodes, spare key/value slots, keys, and child allocations.
                weight += 1024 + values.len() * (size_of::<String>() + size_of::<Value>() + 4 * size_of::<usize>());
                for (key, value) in values {
                    weight = weight
                        .saturating_add(key.capacity())
                        .saturating_add(visit(value, nodes, depth + 1)?);
                    check_bytes(weight)?;
                }
            }
            _ => {}
        }
        check_bytes(weight)?;
        Ok(weight)
    }
    visit(value, &mut 0, 0)
}

fn check_bytes(weight: usize) -> Result<(), String> {
    if weight > MAX_CALLBACK_BYTES - 2 * size_of::<usize>() {
        Err("native callback exceeds 32 MiB; callback was not admitted".into())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests;
