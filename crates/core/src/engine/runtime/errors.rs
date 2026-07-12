use super::*;

#[derive(Debug)]
/// Errors that can occur during engine runtime execution.
pub enum EngineRuntimeError {
    /// Wrapper for edit-application failures.
    Edit(EngineEditError),
    /// A node declared a dependency on a missing node id.
    MissingDependency {
        /// The node that declared the dependency.
        node: NodeId,
        /// The missing dependency node id.
        dependency: NodeId,
    },
    /// Dependency graph contains at least one cycle.
    DependencyCycle {
        /// The nodes involved in the dependency cycle.
        nodes: Vec<NodeId>,
    },
    /// Node requested an invalid zero-hertz update rate.
    InvalidUpdateRate {
        /// The node that requested the invalid rate.
        node: NodeId,
        /// The invalid update rate.
        rate_hz: NodeUpdateRate,
    },
    /// Runtime exceeded stabilization pass limit and likely entered an event/edit loop.
    InfiniteEventEditCycle {
        /// The tick number where the cycle was detected.
        tick: u64,
        /// The number of stabilization passes executed.
        passes: usize,
    },
    /// Runtime exceeded update callback budget for a single tick.
    UpdateBudgetExceeded {
        /// The tick number where the budget was exceeded.
        tick: u64,
        /// The number of update callbacks executed.
        callbacks: usize,
        /// The maximum number of update callbacks allowed.
        limit: usize,
    },
}

impl fmt::Display for EngineRuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Edit(err) => write!(f, "{err}"),
            Self::MissingDependency { node, dependency } => {
                write!(f, "node {:?} depends on missing node {:?}", node, dependency)
            }
            Self::DependencyCycle { nodes } => {
                write!(f, "dependency cycle detected among nodes {:?}", nodes)
            }
            Self::InvalidUpdateRate { node, rate_hz } => {
                write!(f, "node {:?} declared invalid update rate {}hz", node, rate_hz)
            }
            Self::InfiniteEventEditCycle { tick, passes } => {
                write!(
                    f,
                    "runtime aborted at tick {tick} after {passes} stabilization passes (possible event/edit cycle)"
                )
            }
            Self::UpdateBudgetExceeded { tick, callbacks, limit } => {
                write!(
                    f,
                    "runtime aborted at tick {tick}: update callbacks {callbacks} exceeded limit {limit}"
                )
            }
        }
    }
}

impl Error for EngineRuntimeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Edit(err) => Some(err),
            _ => None,
        }
    }
}

impl From<EngineEditError> for EngineRuntimeError {
    fn from(value: EngineEditError) -> Self {
        Self::Edit(value)
    }
}
