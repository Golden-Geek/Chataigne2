use crate::{
    ScriptInputError,
    input::{self, MAX_INPUT_BYTES, MAX_LABEL_BYTES},
};
use golden_io::{PayloadReservation, SharedPayload};
use serde_json::Value;
use std::sync::Arc;

/// An admitted custom event. Every raw payload Arc shares one background final owner,
/// including after the event crosses the script bridge into engine routing/replay.
#[derive(Clone, Debug)]
pub struct ScriptCustomEvent {
    topic: String,
    payload: SharedPayload<Value>,
}

/// Capacity admitted before custom-event JSON materialization.
pub struct ScriptEventReservation {
    topic: String,
    permit: PayloadReservation,
}

impl ScriptCustomEvent {
    /// Prepare on the producer boundary. Rejection returns original allocations.
    pub fn prepare(topic: impl Into<String>, mut payload: Value) -> Result<Self, ScriptInputError<(String, Value)>> {
        let topic = topic.into();
        let admission = || {
            if topic.capacity() > MAX_LABEL_BYTES {
                return Err("script event topic exceeds 1024 retained bytes");
            }
            let weight = input::json_weight(&payload)?;
            Ok((input::reserve(weight)?, weight))
        };
        match admission() {
            Ok((permit, weight)) => {
                input::canonicalize_objects(&mut payload);
                Ok(Self {
                    topic,
                    payload: permit.share_retained(payload, weight).expect("event fits admission"),
                })
            }
            Err(message) => Err(ScriptInputError::new((topic, payload), message)),
        }
    }

    /// Reserve before bounded JSON parsing, including partial conversion failure.
    pub fn reserve(topic: &str) -> Result<ScriptEventReservation, String> {
        if topic.len() > MAX_LABEL_BYTES {
            return Err("script event topic exceeds 1024 bytes".into());
        }
        Ok(ScriptEventReservation {
            topic: topic.to_owned(),
            permit: input::reserve(MAX_INPUT_BYTES)?,
        })
    }

    /// Bounded event topic.
    pub fn topic(&self) -> &str {
        &self.topic
    }
    /// Shares the protected original allocation. Do not register it in another pool.
    pub fn payload(&self) -> &Arc<Value> {
        self.payload.arc()
    }
    /// Transfer topic and protected payload into a shared-data consumer.
    pub fn into_parts(self) -> (String, Arc<Value>) {
        (self.topic, self.payload.arc().clone())
    }

    /// Forward the original owner into an event adapter that supports lifetime leases.
    pub fn into_shared_parts(self) -> (String, SharedPayload<Value>) {
        (self.topic, self.payload)
    }
}

impl ScriptEventReservation {
    /// Publish an immutable payload under the original lifetime reservation.
    pub fn finish(self, mut payload: Value) -> Result<ScriptCustomEvent, ScriptInputError<(String, Value)>> {
        match input::json_weight(&payload) {
            Ok(weight) => {
                input::canonicalize_objects(&mut payload);
                Ok(ScriptCustomEvent {
                    topic: self.topic,
                    payload: self
                        .permit
                        .share_retained(payload, weight)
                        .expect("event fits admission"),
                })
            }
            Err(message) => Err(ScriptInputError::new((self.topic, payload), message)),
        }
    }
}

#[cfg(test)]
mod tests;
