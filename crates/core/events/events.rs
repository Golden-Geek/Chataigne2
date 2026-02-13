use crate::engine::EngineTime;
use crate::node::{NodeId, NodeMetaPatch};
use crate::parameter::ParamValue;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Event {
    pub time: EngineTime,
    pub kind: EventKind,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CustomEvent {
    pub topic: String,
    #[serde(default)]
    pub payload: serde_json::Value,
}

impl CustomEvent {
    pub fn new(topic: impl Into<String>, payload: serde_json::Value) -> Self {
        Self {
            topic: topic.into(),
            payload,
        }
    }

    pub fn from_payload<T: Serialize>(topic: impl Into<String>, payload: &T) -> serde_json::Result<Self> {
        Ok(Self::new(topic, serde_json::to_value(payload)?))
    }

    pub fn payload_as<T: DeserializeOwned>(&self) -> serde_json::Result<T> {
        serde_json::from_value(self.payload.clone())
    }
}

impl Event {
    pub fn custom(time: EngineTime, event: CustomEvent) -> Self {
        Self {
            time,
            kind: EventKind::Custom(event),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum EventKind {
    ParamChanged { param: NodeId, old_value: ParamValue },

    // Structure-level
    ChildAdded { parent: NodeId, child: NodeId },
    ChildRemoved { parent: NodeId, child: NodeId },
    ChildReplaced { parent: NodeId, old: NodeId, new: NodeId },
    ChildMoved { child: NodeId, old_parent: NodeId, new_parent: NodeId },
    ChildReordered { parent: NodeId, child: NodeId },

    // Identity/lifecycle-level
    NodeCreated { node: NodeId },
    NodeDeleted { node: NodeId },

    // Metadata-level
    MetaChanged { node: NodeId, patch: NodeMetaPatch },

    Custom(CustomEvent),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Inbox {
    pub events: Vec<Event>,
}

impl Inbox {
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    pub fn clear(&mut self) {
        self.events.clear();
    }

    pub fn push(&mut self, event: Event) {
        self.events.push(event);
    }
}
