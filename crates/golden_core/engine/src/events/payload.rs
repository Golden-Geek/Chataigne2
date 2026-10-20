use super::{CustomEvent, CustomEventRetention};
use crate::node::NodeId;
use golden_io::{PayloadPool, RetainedPayload, SharedPayload};
use serde_json::Value;
use std::sync::{Arc, Mutex, OnceLock, Weak};

#[derive(Clone, Debug)]
pub(super) struct PayloadStorage {
    value: Weak<Value>,
    bytes: usize,
}

impl CustomEvent {
    pub(crate) fn release_replay_payload(&mut self) {
        // Evicted lazy-log prefixes share one immutable empty value rather than
        // retaining JSON or allocating a replacement for each eviction.
        static EMPTY: OnceLock<Arc<Value>> = OnceLock::new();
        self.payload = EMPTY.get_or_init(|| Arc::new(Value::Null)).clone();
        self.payload_owner = None;
        self.payload_storage = None;
    }
    /// Route prepared JSON while preserving its transferable canonical owner.
    pub fn from_shared_payload(
        topic: impl Into<String>,
        origin: Option<NodeId>,
        payload: SharedPayload<Value>,
        retention: CustomEventRetention,
    ) -> Self {
        Self {
            topic: topic.into(),
            origin,
            payload: payload.arc().clone(),
            retention,
            local: None,
            payload_owner: Some(payload),
            payload_storage: None,
        }
    }

    /// Cache the full retained payload weight on its preparing worker. Include
    /// spare container/string capacity; this is the same producer contract as
    /// weighted IO admission. Mutation/replacement of the public Arc invalidates it.
    pub fn with_payload_storage_weight(mut self, bytes: usize) -> Self {
        self.payload_storage = Some(PayloadStorage {
            value: Arc::downgrade(&self.payload),
            bytes,
        });
        self
    }

    /// Replay accounting for custom JSON only. Prepared sources use their cached
    /// weight without traversing JSON. Unprepared sources receive a bounded walk;
    /// an unmeasurably complex value is treated as one oversized replay entry.
    pub fn replay_payload_bytes(&self) -> usize {
        if let Some(storage) = &self.payload_storage
            && storage
                .value
                .upgrade()
                .is_some_and(|value| Arc::ptr_eq(&value, &self.payload))
        {
            return storage.bytes;
        }
        json_storage(&self.payload, &mut 4096, 0)
            .map(|bytes| bytes.saturating_add(2 * std::mem::size_of::<usize>()))
            .unwrap_or(usize::MAX)
    }

    /// Lease JSON without copying or installing an independent final owner.
    /// The caller must validate structure and supply its full retained allocation
    /// weight before capture. Plain event JSON shares one fallback lifetime pool.
    pub fn try_lease_payload(&self, weight: usize) -> Result<RetainedPayload<Value>, String> {
        if let Some(owner) = &self.payload_owner
            && Arc::ptr_eq(owner.arc(), &self.payload)
        {
            return Ok(owner.lease());
        }
        static POOL: Mutex<Option<PayloadPool>> = Mutex::new(None);
        let mut pool = POOL.lock().unwrap_or_else(|p| p.into_inner());
        if pool.is_none() {
            *pool = Some(
                PayloadPool::new("engine-event-input", 16_384, 64 * 1024 * 1024)
                    .map_err(|error| format!("Event input disposer unavailable: {error}"))?,
            );
        }
        pool.as_ref()
            .expect("installed event input pool")
            .try_retain_arc(self.payload.clone(), weight)
            .map_err(|_| "Event input lifetime capacity exhausted".into())
    }
}

fn json_storage(value: &Value, remaining: &mut usize, depth: usize) -> Option<usize> {
    if *remaining == 0 || depth > 64 {
        return None;
    }
    *remaining -= 1;
    let mut bytes = std::mem::size_of::<Value>();
    match value {
        Value::String(value) => bytes = bytes.saturating_add(value.capacity()),
        Value::Array(values) => {
            bytes = bytes.saturating_add(values.capacity().saturating_mul(std::mem::size_of::<Value>()));
            for value in values {
                bytes = bytes.saturating_add(json_storage(value, remaining, depth + 1)?);
            }
        }
        Value::Object(values) => {
            // One complete B-tree node per entry plus a possible empty root.
            bytes = bytes.saturating_add((values.len() + 1).saturating_mul(1024));
            for (key, value) in values {
                bytes = bytes
                    .saturating_add(key.capacity())
                    .saturating_add(json_storage(value, remaining, depth + 1)?);
            }
        }
        _ => {}
    }
    Some(bytes)
}

// Lifetime metadata does not change an event's logical or serialized meaning.
impl PartialEq for CustomEvent {
    fn eq(&self, other: &Self) -> bool {
        self.topic == other.topic
            && self.origin == other.origin
            && self.payload == other.payload
            && self.retention == other.retention
            && self.local == other.local
    }
}
