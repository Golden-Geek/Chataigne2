//! Stable identities shared by Golden model, runtime, protocol, and UI layers.

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

/// Process-local identifier for a materialized model entity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
pub struct NodeId(pub u64);

/// Persistent UUID assigned to a model entity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
pub struct NodeUuid(pub Uuid);

impl NodeUuid {
    /// Returns the nil UUID value.
    #[must_use]
    pub fn nil() -> Self {
        Self(Uuid::nil())
    }

    /// Returns `true` when this UUID is nil.
    #[must_use]
    pub fn is_nil(&self) -> bool {
        self.0.is_nil()
    }
}

impl Default for NodeUuid {
    fn default() -> Self {
        Self::nil()
    }
}

/// Declaration identifier used to refer to model definitions.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
pub struct DeclId(pub String);

#[cfg(test)]
mod tests;
