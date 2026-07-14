//! Reusable hierarchical statechart primitives for Golden applications.

mod domain;
mod ids;
mod model;

pub use domain::{
    StatechartEdgeData, StatechartGraphAdapter, StatechartGraphAdapterError, StatechartGraphData,
    StatechartGraphDocument, StatechartGraphDomain, StatechartGraphEnvelope, StatechartNodeData, StatechartPortData,
};
pub use ids::{RegionId, StateId, StatechartId, TransitionId};
pub use model::{
    ActiveConfiguration, EnterPolicy, HistoryPolicy, LifecycleEvent, Region, StateHistory, StateKind, StateNode,
    StatePath, StateUiLayout, Statechart, StatechartError, Transition, TransitionOutcome,
};

/// Current authored statechart schema version.
pub const STATECHART_SCHEMA_VERSION: u32 = 1;

#[cfg(test)]
mod domain_tests;
#[cfg(test)]
mod tests;
