use serde::{Deserialize, Serialize};

use super::NodeId;

/// Controls whether an event reaching a node should notify, pass through, or stop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventPropagation {
    /// Notify this node when it is interested and continue bubbling.
    Notify,
    /// Do not notify this node, but still allow bubbling to continue.
    PassOn,
    /// Notify this node when it is interested and stop bubbling.
    Stop,
}

/// Runtime listener subscription targeting a node and optional subtree depth.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EventSubscription {
    /// Subscription root node id.
    pub node: NodeId,
    /// Maximum descendant depth from `node` that should match.
    ///
    /// `0` matches only `node`, `1` includes direct children, and so on.
    pub max_depth: u32,
}

impl EventSubscription {
    /// Creates a subscription matching events originating exactly from `node`.
    pub fn node(node: NodeId) -> Self {
        Self { node, max_depth: 0 }
    }

    /// Creates a subscription matching `node` and descendants up to `max_depth`.
    pub fn subtree(node: NodeId, max_depth: u32) -> Self {
        Self { node, max_depth }
    }
}
