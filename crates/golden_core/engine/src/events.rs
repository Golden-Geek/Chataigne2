use crate::engine::EngineTime;
use crate::node::{DeclId, NodeId, NodeMetaPatch};
use crate::parameter::{ParamValue, ParameterConstraints, ParameterControlState};
use std::ops::Index;
use std::sync::Arc;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Delivery and UI replay retention policy for one custom event.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum CustomEventRetention {
    /// Preserve every event in order.
    #[default]
    Replay,
    /// Keep only the latest event for the same topic and origin.
    Latest,
    /// Deliver only through the engine inbox, without UI replay or transport exposure.
    Transient,
}

/// Timestamped engine event emitted while applying edits.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Event {
    /// Engine time at which the event was produced.
    pub time: EngineTime,
    /// Event payload describing the semantic change.
    pub kind: EventKind,
}

/// User-defined event payload transported through the engine.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CustomEvent {
    /// Logical topic identifier used for event routing.
    pub topic: String,
    /// Node that originated the event, if known.
    pub origin: Option<NodeId>,
    /// Arbitrary JSON payload for custom data.
    #[serde(default)]
    pub payload: serde_json::Value,
    /// UI replay retention policy.
    #[serde(default)]
    pub retention: CustomEventRetention,
}

impl CustomEvent {
    /// Creates a custom event from raw JSON payload.
    pub fn new(topic: impl Into<String>, origin: Option<NodeId>, payload: serde_json::Value) -> Self {
        Self {
            topic: topic.into(),
            origin,
            payload,
            retention: CustomEventRetention::Replay,
        }
    }

    /// Creates a latest-wins custom event.
    pub fn latest(topic: impl Into<String>, origin: Option<NodeId>, payload: serde_json::Value) -> Self {
        Self {
            topic: topic.into(),
            origin,
            payload,
            retention: CustomEventRetention::Latest,
        }
    }

    /// Creates an engine-internal custom event that is excluded from UI replay.
    pub fn transient(topic: impl Into<String>, origin: Option<NodeId>, payload: serde_json::Value) -> Self {
        Self {
            topic: topic.into(),
            origin,
            payload,
            retention: CustomEventRetention::Transient,
        }
    }

    /// Creates a custom event by serializing a typed payload into JSON.
    pub fn from_payload<T: Serialize>(
        topic: impl Into<String>,
        origin: Option<NodeId>,
        payload: &T,
    ) -> serde_json::Result<Self> {
        Ok(Self::new(topic, origin, serde_json::to_value(payload)?))
    }

    /// Creates a latest-wins custom event by serializing a typed payload into JSON.
    pub fn from_latest_payload<T: Serialize>(
        topic: impl Into<String>,
        origin: Option<NodeId>,
        payload: &T,
    ) -> serde_json::Result<Self> {
        Ok(Self::latest(topic, origin, serde_json::to_value(payload)?))
    }

    /// Creates an engine-internal custom event by serializing a typed payload into JSON.
    pub fn from_transient_payload<T: Serialize>(
        topic: impl Into<String>,
        origin: Option<NodeId>,
        payload: &T,
    ) -> serde_json::Result<Self> {
        Ok(Self::transient(topic, origin, serde_json::to_value(payload)?))
    }

    /// Deserializes the JSON payload into a typed value.
    pub fn payload_as<T: DeserializeOwned>(&self) -> serde_json::Result<T> {
        T::deserialize(&self.payload)
    }
}

impl Event {
    /// Convenience constructor for a [`EventKind::Custom`] event.
    pub fn custom(time: EngineTime, event: CustomEvent) -> Self {
        Self {
            time,
            kind: EventKind::Custom(event),
        }
    }
}

/// Event variants emitted by the engine when edits are applied.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum EventKind {
    /// A parameter value was changed by `SetParam`.
    ParamChanged {
        /// Parameter node that changed.
        param: NodeId,
        /// Previous value before the change.
        old_value: ParamValue,
        /// Value after the change.
        new_value: ParamValue,
    },
    /// A parameter control state was changed.
    ParamControlChanged {
        /// Parameter node that changed.
        param: NodeId,
        /// Previous control state before the change.
        old_state: ParameterControlState,
        /// Control state after the change.
        new_state: ParameterControlState,
    },
    /// A parameter's live constraints were changed.
    ParamConstraintsChanged {
        /// Parameter node that changed.
        param: NodeId,
        /// Previous constraint state.
        old_constraints: ParameterConstraints,
        /// New constraint state.
        new_constraints: Box<ParameterConstraints>,
    },

    /// A child node was attached under a parent.
    ChildAdded {
        /// Parent receiving the child.
        parent: NodeId,
        /// Child node that was added.
        child: NodeId,
        /// Declared slot id of the added child.
        decl_id: DeclId,
    },
    /// A child node was detached from a parent.
    ChildRemoved {
        /// Parent that lost the child.
        parent: NodeId,
        /// Child node that was removed.
        child: NodeId,
    },
    /// A child node under a parent was replaced by a new node id.
    ChildReplaced {
        /// Parent where replacement happened.
        parent: NodeId,
        /// Previous child id.
        old: NodeId,
        /// New child id.
        new: NodeId,
        /// Declared slot id of the replacement child.
        decl_id: DeclId,
    },
    /// A child node changed parent.
    ChildMoved {
        /// Child that moved.
        child: NodeId,
        /// Previous parent id.
        old_parent: NodeId,
        /// New parent id.
        new_parent: NodeId,
    },
    /// A child was reordered within the same parent.
    ChildReordered {
        /// Parent within which ordering changed.
        parent: NodeId,
        /// Child whose position changed.
        child: NodeId,
    },

    /// A node id was created in the store.
    NodeCreated {
        /// Node id that was created.
        node: NodeId,
    },
    /// A node id was deleted from the store.
    NodeDeleted {
        /// Node id that was deleted.
        node: NodeId,
    },

    /// Node metadata was patched.
    MetaChanged {
        /// Node whose metadata changed.
        node: NodeId,
        /// Metadata patch applied to the node.
        patch: NodeMetaPatch,
    },

    /// An atomic graph transaction containing multiple operations.
    GraphTransaction {
        /// The transaction payload.
        transaction: crate::ui_sync::UiGraphTransaction,
    },

    /// User-defined event emitted through the edit pipeline.
    Custom(CustomEvent),
}

impl EventKind {
    /// Returns the node used as the bubbling origin for this event when available.
    pub fn propagation_origin(&self) -> Option<NodeId> {
        match self {
            Self::ParamChanged { param, .. } => Some(*param),
            Self::ParamControlChanged { param, .. } => Some(*param),
            Self::ParamConstraintsChanged { param, .. } => Some(*param),
            Self::ChildAdded { child, .. } => Some(*child),
            Self::ChildRemoved { parent, .. } => Some(*parent),
            Self::ChildReplaced { new, .. } => Some(*new),
            Self::ChildMoved { child, .. } => Some(*child),
            Self::ChildReordered { child, .. } => Some(*child),
            Self::NodeCreated { node } => Some(*node),
            Self::NodeDeleted { .. } => None,
            Self::MetaChanged { node, .. } => Some(*node),
            Self::GraphTransaction { .. } => None,
            Self::Custom(event) => event.origin,
        }
    }
}

/// Engine-owned event buffer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Inbox {
    /// Stored events in emission order.
    pub events: Vec<Event>,
}

impl Default for Inbox {
    fn default() -> Self {
        Self::new()
    }
}

/// Shared event bundle visible to one callback frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EventFrame {
    events: Vec<Arc<Event>>,
}

impl EventFrame {
    /// Creates an empty event frame.
    #[must_use]
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    /// Creates a frame from shared event handles.
    #[must_use]
    pub fn from_shared(events: Vec<Arc<Event>>) -> Self {
        Self { events }
    }

    /// Appends a shared event handle.
    pub fn push_shared(&mut self, event: Arc<Event>) {
        self.events.push(event);
    }

    /// Returns true when the frame contains no events.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Returns the number of visible events.
    #[must_use]
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Iterates visible events as shared handles.
    pub fn iter(&self) -> std::slice::Iter<'_, Arc<Event>> {
        self.events.iter()
    }
}

impl<'a> IntoIterator for &'a EventFrame {
    type Item = &'a Arc<Event>;
    type IntoIter = std::slice::Iter<'a, Arc<Event>>;

    fn into_iter(self) -> Self::IntoIter {
        self.events.iter()
    }
}

impl IntoIterator for EventFrame {
    type Item = Arc<Event>;
    type IntoIter = std::vec::IntoIter<Arc<Event>>;

    fn into_iter(self) -> Self::IntoIter {
        self.events.into_iter()
    }
}

impl Index<usize> for EventFrame {
    type Output = Arc<Event>;

    fn index(&self, index: usize) -> &Self::Output {
        &self.events[index]
    }
}

impl Inbox {
    /// Creates an empty inbox.
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    /// Removes all currently buffered events.
    pub fn clear(&mut self) {
        self.events.clear();
    }

    /// Appends an event to the end of the inbox.
    pub fn push(&mut self, event: Event) {
        self.events.push(event);
    }
}
