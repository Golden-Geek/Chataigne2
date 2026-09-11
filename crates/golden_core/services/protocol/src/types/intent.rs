use super::*;

/// UI-originated edit intent.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum UiEditIntent {
    /// Begin a grouped edit session.
    BeginEdit {
        /// Client-generated id.
        client_edit_id: String,
        /// Optional label.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        label: Option<String>,
    },
    /// End a grouped edit session.
    EndEdit {
        /// Client-generated id.
        client_edit_id: String,
    },
    /// Set a parameter value.
    SetParam {
        /// Target node id.
        node: NodeId,
        /// New value.
        value: ParamValue,
        /// Requested coalescing behavior.
        behaviour: ParameterEventBehaviour,
    },
    /// Apply inspector text-entry semantics to a string parameter.
    SetTextParamSmart {
        /// Target parameter node id.
        node: NodeId,
        /// Text entered by the client.
        value: String,
        /// Requested coalescing behavior.
        #[serde(default, skip_serializing_if = "is_default_event_behaviour")]
        behaviour: ParameterEventBehaviour,
    },
    /// Set a parameter control state.
    SetParamControlState {
        /// Target parameter node id.
        node: NodeId,
        /// New control state payload.
        state: UiParameterControlStateDto,
    },
    /// Replace a parameter's live runtime constraints.
    SetParamConstraints {
        /// Target parameter node id.
        node: NodeId,
        /// New constraints payload.
        constraints: ParameterConstraints,
    },
    /// Move a node.
    MoveNode {
        /// Target node id.
        node: NodeId,
        /// New parent id.
        new_parent: NodeId,
        /// Optional previous sibling under the new parent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        new_prev_sibling: Option<NodeId>,
    },
    /// Remove a node.
    RemoveNode {
        /// Target node id.
        node: NodeId,
    },
    /// Remove multiple nodes in one intent transaction.
    RemoveNodes {
        /// Target node ids.
        nodes: Vec<NodeId>,
    },
    /// Creates a user item under `parent` from a node type id.
    CreateUserItem {
        /// Parent node id.
        parent: NodeId,
        /// Runtime node type identifier to instantiate.
        node_type: String,
        /// Optional explicit label for the new item.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        label: Option<String>,
        /// Optional direct parameter values applied before the intent completes.
        #[serde(default, skip_serializing_if = "is_empty_create_user_item_initial_params")]
        initial_params: Vec<UiCreateUserItemInitialParam>,
    },
    /// Creates a dashboard container widget from backend-owned defaults.
    CreateDashboardContainerWidget {
        /// Dashboard page or container receiving the widget.
        parent: NodeId,
        /// Optional explicit label.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        label: Option<String>,
        /// Optional placement hint.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        placement: Option<UiDashboardWidgetPlacement>,
        /// Optional child layout kind for the new container.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        layout_kind: Option<String>,
        /// Optional sibling after which insertion occurs.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        prev_sibling: Option<NodeId>,
    },
    /// Creates a dashboard node widget for one target node.
    CreateDashboardNodeWidget {
        /// Dashboard page or container receiving the widget.
        parent: NodeId,
        /// Target node rendered by the widget.
        target: NodeId,
        /// Optional placement hint.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        placement: Option<UiDashboardWidgetPlacement>,
        /// Optional sibling after which insertion occurs.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        prev_sibling: Option<NodeId>,
    },
    /// Creates a generic dashboard widget for one target parameter.
    CreateDashboardGenericWidget {
        /// Dashboard page or container receiving the widget.
        parent: NodeId,
        /// Target parameter bound by the widget.
        target: NodeId,
        /// Optional placement hint.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        placement: Option<UiDashboardWidgetPlacement>,
        /// Optional sibling after which insertion occurs.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        prev_sibling: Option<NodeId>,
    },
    /// Rebinds a dashboard node widget to one target node.
    BindDashboardNodeWidgetTarget {
        /// Existing dashboard node widget.
        widget: NodeId,
        /// Target node rendered by the widget.
        target: NodeId,
    },
    /// Rebinds a generic dashboard widget to one target parameter.
    BindDashboardGenericWidgetTarget {
        /// Existing generic dashboard widget.
        widget: NodeId,
        /// Target parameter bound by the widget.
        target: NodeId,
    },
    /// Wraps one dashboard widget in a newly-created container.
    WrapDashboardWidgetInContainer {
        /// Existing widget to wrap.
        widget: NodeId,
        /// Optional placement hint for the new container.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        placement: Option<UiDashboardWidgetPlacement>,
        /// Optional child layout kind for the new container.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        layout_kind: Option<String>,
    },
    /// Duplicates an existing node subtree under `new_parent`.
    DuplicateNode {
        /// Source node id to clone.
        source: NodeId,
        /// Parent receiving the duplicated subtree root.
        new_parent: NodeId,
        /// Optional sibling after which insertion occurs.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        new_prev_sibling: Option<NodeId>,
        /// Optional direct parameter values applied to the duplicated root before the intent completes.
        #[serde(default, skip_serializing_if = "is_empty_create_user_item_initial_params")]
        initial_params: Vec<UiCreateUserItemInitialParam>,
    },
    /// Materializes copied roots and dependent user items as one edit.
    DuplicateNodes {
        /// Existing subtree roots to clone.
        #[serde(default, skip_serializing_if = "is_empty_duplicate_node_specs")]
        nodes: Vec<UiDuplicateNodeSpec>,
        /// Fresh user items to create and expose to dependent references.
        #[serde(default, skip_serializing_if = "is_empty_duplicate_create_user_item_specs")]
        created_items: Vec<UiDuplicateCreateUserItemSpec>,
        /// Items whose initial parameters can reference roots created earlier in the batch.
        #[serde(default, skip_serializing_if = "is_empty_duplicate_dependent_user_items")]
        dependent_items: Vec<UiDuplicateDependentUserItem>,
    },
    /// Replaces one curve range with a sparse bezier fit of recorded samples.
    FitAnimationCurvePath {
        /// Target animation-curve node id.
        curve: NodeId,
        /// Recorded path samples.
        points: Vec<CurveFitPoint>,
        /// Fit controls.
        #[serde(default)]
        options: CurveBezierFitOptions,
    },
    /// Patch node metadata.
    PatchMeta {
        /// Target node id.
        node: NodeId,
        /// Metadata patch.
        patch: NodeMetaPatch,
    },
    /// Ensures one user-context scope exists on `owner`.
    EnsureUserContextScope {
        /// Scope owner node id.
        owner: NodeId,
    },
    /// Removes the user-context scope from `owner`.
    RemoveUserContextScope {
        /// Scope owner node id.
        owner: NodeId,
    },
    /// Adds or replaces one user-context entry.
    UpsertUserContextEntry {
        /// Scope owner node id.
        owner: NodeId,
        /// Symbol name.
        symbol: String,
        /// Parameter node backing this entry.
        param: NodeId,
    },
    /// Removes one user-context entry by symbol.
    RemoveUserContextEntry {
        /// Scope owner node id.
        owner: NodeId,
        /// Symbol to remove.
        symbol: String,
    },
    /// Sends an ephemeral typed-by-topic event directly to one runtime node.
    ///
    /// This is the public extension point for app-owned UI/runtime coordination. The event is
    /// delivered through the node inbox, but is not persisted, added to undo history, or echoed
    /// into the UI replay log.
    SendNodeEvent {
        /// Runtime node receiving the event.
        node: NodeId,
        /// App-owned event topic interpreted by the target node.
        topic: String,
        /// App-owned event payload.
        payload: serde_json::Value,
    },
    /// Request graph reevaluation.
    ReevaluateGraph,
    /// Clears retained logger records.
    ClearLogs,
    /// Sets logger retention capacity.
    SetLogMaxEntries {
        /// Requested maximum number of retained records.
        max_entries: usize,
    },
    /// Undo the last history transaction.
    Undo,
    /// Redo the last undone history transaction.
    Redo,
}

/// Ack status for a UI edit intent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum UiAckStatus {
    /// Accepted and applied now.
    Applied,
    /// Accepted but staged for later application.
    Staged,
    /// Rejected.
    Rejected,
}

/// Acknowledgement payload for UI edit intents.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct UiAck {
    /// Success flag.
    pub success: bool,
    /// Ack status.
    pub status: UiAckStatus,
    /// Optional error code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    /// Optional error message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    /// Optional earliest resulting event timestamp.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub earliest_event_time: Option<EngineTime>,
    /// Optional completion boundary covering the final event produced by the intent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_event_time: Option<EngineTime>,
    /// Current undo/redo state after applying the intent.
    pub history: UiHistoryState,
}
