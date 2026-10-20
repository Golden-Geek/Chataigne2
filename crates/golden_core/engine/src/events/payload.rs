use super::{CustomEvent, CustomEventRetention};
use crate::node::NodeId;
use golden_io::{PayloadPool, RetainedPayload, SharedPayload};
use serde_json::Value;
use std::sync::{Arc, Mutex};

impl CustomEvent {
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
        }
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
