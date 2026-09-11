use super::*;

/// UI-facing event kind.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum UiEventKind {
    /// Graph transaction containing multiple atomic updates.
    GraphTransaction {
        /// Atomic graph transaction payload.
        #[serde(flatten)]
        transaction: UiGraphTransaction,
    },
    /// Parameter changed.
    ParamChanged {
        /// Parameter node id.
        param: NodeId,
        /// Previous value.
        old_value: ParamValue,
        /// New value.
        new_value: ParamValue,
    },
    /// Parameter control state changed.
    ParamControlChanged {
        /// Parameter node id.
        param: NodeId,
        /// Previous control state.
        old_state: UiParameterControlStateDto,
        /// New control state.
        new_state: UiParameterControlStateDto,
    },
    /// Parameter constraints changed.
    ParamConstraintsChanged {
        /// Parameter node id.
        param: NodeId,
        /// Previous constraints.
        old_constraints: ParameterConstraints,
        /// New constraints.
        new_constraints: Box<ParameterConstraints>,
    },
    /// Child added.
    ChildAdded {
        /// Parent id.
        parent: NodeId,
        /// Child id.
        child: NodeId,
        /// Declared slot id.
        decl_id: DeclId,
        /// Current direct child order for the parent when available.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        parent_children: Option<Vec<NodeId>>,
    },
    /// Child removed.
    ChildRemoved {
        /// Parent id.
        parent: NodeId,
        /// Child id.
        child: NodeId,
    },
    /// Child replaced.
    ChildReplaced {
        /// Parent id.
        parent: NodeId,
        /// Old child id.
        old: NodeId,
        /// New child id.
        new: NodeId,
        /// Declared slot id.
        decl_id: DeclId,
    },
    /// Child moved.
    ChildMoved {
        /// Child id.
        child: NodeId,
        /// Previous parent id.
        old_parent: NodeId,
        /// New parent id.
        new_parent: NodeId,
        /// Current direct child order for the previous parent when available.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        old_parent_children: Option<Vec<NodeId>>,
        /// Current direct child order for the new parent when available.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        new_parent_children: Option<Vec<NodeId>>,
    },
    /// Child reordered.
    ChildReordered {
        /// Parent id.
        parent: NodeId,
        /// Child id.
        child: NodeId,
        /// Current direct child order for the parent when available.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        parent_children: Option<Vec<NodeId>>,
    },
    /// Node created.
    NodeCreated {
        /// Node id.
        node: NodeId,
        /// Node snapshot for incremental UI insertion when the node is still live.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        snapshot: Option<Box<UiNodeDto>>,
    },
    /// Node deleted.
    NodeDeleted {
        /// Node id.
        node: NodeId,
    },
    /// Metadata changed.
    MetaChanged {
        /// Node id.
        node: NodeId,
        /// Applied patch.
        patch: NodeMetaPatch,
    },
    /// Custom event payload.
    Custom {
        /// Topic.
        topic: String,
        /// Origin node when known.
        origin: Option<NodeId>,
        /// Raw JSON payload.
        payload: serde_json::Value,
        /// Replay and transport retention policy.
        retention: CustomEventRetention,
    },
}

/// UI-facing event payload.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct UiEventDto {
    /// Event time.
    pub time: EngineTime,
    /// Event payload.
    #[serde(flatten)]
    pub kind: UiEventKind,
}

/// Event replay batch.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct UiEventBatch {
    /// Replay cursor used by the request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<EngineTime>,
    /// Last event timestamp included in this batch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<EngineTime>,
    /// Latest runtime timing metrics sampled by the host loop.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime: Option<UiRuntimeStatsDto>,
    /// Delivered events.
    pub events: Vec<UiEventDto>,
}

/// Runtime timing metrics exposed to the UI.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct UiRuntimeStatsDto {
    /// Engine ticks completed per second over the latest sampling window.
    pub engine_hz: f64,
    /// Currently published immutable runtime generation.
    pub generation_id: u64,
    /// Current actor control queue depth.
    pub control_queue_depth: u64,
    /// Peak actor control queue depth since startup.
    pub control_queue_peak: u64,
    /// Control operations admitted since startup.
    pub control_received: u64,
    /// Control operations applied since startup.
    pub control_applied: u64,
    /// Control operations rejected since startup.
    pub control_rejected: u64,
    /// Cumulative actor queue wait time in nanoseconds.
    pub control_wait_ns: u64,
    /// Cumulative actor application time in nanoseconds.
    pub control_apply_ns: u64,
    /// Generations compiled successfully since startup.
    pub compilation_applied: u64,
    /// Generation compile failures since startup.
    pub compilation_rejected: u64,
    /// Sparse semantic batches completed.
    pub sparse_batches: u64,
    /// Dense semantic batches completed.
    pub dense_batches: u64,
    /// Compile-assigned work units completed.
    pub work_units: u64,
    /// Authoritative external effects committed.
    pub effects_committed: u64,
    /// Non-authoritative external effects suppressed by the routing policy.
    pub effects_suppressed: u64,
}
