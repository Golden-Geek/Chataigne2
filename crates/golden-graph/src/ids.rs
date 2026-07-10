use golden_model::EntityId;
use serde::{Deserialize, Serialize};

macro_rules! graph_id {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(EntityId);

        impl $name {
            pub fn new() -> Self {
                Self(EntityId::new())
            }

            pub const fn from_entity(value: EntityId) -> Self {
                Self(value)
            }

            pub const fn as_entity(self) -> EntityId {
                self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }
    };
}

graph_id!(GraphId);
graph_id!(GraphNodeId);
graph_id!(GraphPortId);
graph_id!(GraphEdgeId);
