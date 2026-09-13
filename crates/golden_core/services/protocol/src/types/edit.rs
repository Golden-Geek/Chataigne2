use super::*;

/// Semantic hints used for tooling, UX, and interpretation.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
pub struct SemanticsHint {
    /// Optional high-level intent of the node.
    pub intent: Option<String>,
    /// Optional unit for value-oriented nodes.
    pub unit: Option<String>,
}

/// Complete metadata patch accepted by the public edit protocol.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
pub struct NodeMetaPatch {
    /// Optional short name replacement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub short_name: Option<String>,
    /// Optional enabled-state replacement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// Optional disablement capability replacement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub can_be_disabled: Option<bool>,
    /// Optional label replacement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Optional description replacement (`Some(None)` clears the description).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<Option<String>>,
    /// Optional tags replacement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    /// Optional user-edit permissions replacement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_permissions: Option<NodeUserPermissions>,
    /// Optional semantic hints replacement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantics: Option<SemanticsHint>,
    /// Optional presentation hints replacement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presentation: Option<PresentationHint>,
}

/// Direct parameter initializer applied immediately after one user-item is created.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct UiCreateUserItemInitialParam {
    /// Direct child decl id on the newly-created root node.
    pub decl_id: DeclId,
    /// Initial value to assign.
    pub value: ParamValue,
}

/// Optional size-enabled hints for dashboard widget creation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
pub struct UiDashboardWidgetSizeEnabled {
    /// Whether width should be enabled when the parent layout uses horizontal sizing.
    #[serde(default, skip_serializing_if = "is_false")]
    pub width: bool,
    /// Whether height should be enabled when the parent layout uses vertical sizing.
    #[serde(default, skip_serializing_if = "is_false")]
    pub height: bool,
}

/// UI-provided placement hint for dashboard widget creation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct UiDashboardWidgetPlacement {
    /// Anchor used by free-layout parents.
    pub anchor: String,
    /// Position used by free-layout parents.
    pub position: (f64, f64),
    /// Preferred widget width.
    pub width: CssValue,
    /// Preferred widget height.
    pub height: CssValue,
    /// Optional size enablement hints for non-free layouts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_enabled: Option<UiDashboardWidgetSizeEnabled>,
}

/// One existing subtree root to clone as part of a copy batch.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct UiDuplicateNodeSpec {
    /// Source node id to clone. Also acts as the key used by dependent references.
    pub source: NodeId,
    /// Parent receiving the duplicated subtree root.
    pub new_parent: NodeId,
    /// Optional sibling after which insertion occurs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_prev_sibling: Option<NodeId>,
    /// Optional direct parameter values applied to the duplicated root before the batch completes.
    #[serde(default, skip_serializing_if = "is_empty_create_user_item_initial_params")]
    pub initial_params: Vec<UiCreateUserItemInitialParam>,
}

/// One fresh user item to create as part of a copy batch.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct UiDuplicateCreateUserItemSpec {
    /// Source key used by dependent references to address this created item.
    pub source: NodeId,
    /// Parent receiving the created item.
    pub parent: NodeId,
    /// Runtime node type identifier to instantiate.
    pub node_type: String,
    /// Optional explicit label for the new item.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Optional direct parameter values applied to the created root before the batch completes.
    #[serde(default, skip_serializing_if = "is_empty_create_user_item_initial_params")]
    pub initial_params: Vec<UiCreateUserItemInitialParam>,
}

/// Initializer for an item that depends on roots materialized earlier in the same copy batch.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct UiDuplicateDependentUserItemInitialParam {
    /// Direct child decl id on the newly-created dependent item.
    pub decl_id: DeclId,
    /// Literal value or a reference resolved from the copy batch source map.
    pub value: UiDuplicateDependentInitialParamValue,
}

/// Value source for a dependent item initializer inside a copy batch.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum UiDuplicateDependentInitialParamValue {
    /// Use this parameter value as-is.
    Literal {
        /// Parameter value assigned directly to the dependent item.
        value: ParamValue,
    },
    /// Reference the copied root produced from `source`.
    DuplicatedNodeReference {
        /// Source key whose copied root becomes the reference target.
        source: NodeId,
    },
}

/// One dependent user item to create after copy-batch roots have been materialized.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct UiDuplicateDependentUserItem {
    /// Parent receiving the dependent item.
    pub parent: NodeId,
    /// Runtime node type identifier to instantiate.
    pub node_type: String,
    /// Optional explicit label for the dependent item.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Initial values applied after references to copied roots have been resolved.
    #[serde(default, skip_serializing_if = "is_empty_duplicate_dependent_initial_params")]
    pub initial_params: Vec<UiDuplicateDependentUserItemInitialParam>,
}

/// Post-edit direct child order for one parent node.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct UiChildrenOrderPatch {
    /// Parent whose child list changed.
    pub parent: NodeId,
    /// Complete direct child order after the operation.
    pub children: Vec<NodeId>,
}

/// Incremental metadata patch for one UI node.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct UiNodeMetaPatch {
    /// Replacement display label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Replacement short script/reference name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub short_name: Option<String>,
    /// Replacement enabled state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// Replacement disablement capability.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub can_be_disabled: Option<bool>,
    /// Replacement optional description, where `Some(None)` clears it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<Option<String>>,
    /// Replacement user-edit permissions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_permissions: Option<NodeUserPermissions>,
    /// Replacement tag list.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    /// Replacement presentation hints.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presentation: Option<PresentationHint>,
}

impl UiNodeMetaPatch {
    /// Returns whether the patch leaves every metadata field unchanged.
    pub fn is_empty(&self) -> bool {
        self.label.is_none()
            && self.short_name.is_none()
            && self.enabled.is_none()
            && self.can_be_disabled.is_none()
            && self.description.is_none()
            && self.user_permissions.is_none()
            && self.tags.is_none()
            && self.presentation.is_none()
    }
}

/// Incremental parameter patch for one UI parameter node.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct UiParamPatch {
    /// Replacement parameter value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<ParamValue>,
    /// Replacement control state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub control: Option<UiParameterControlStateDto>,
    /// Replacement runtime constraints.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub constraints: Option<ParameterConstraints>,
}

/// One deterministic graph patch operation inside an atomic UI transaction.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum UiGraphOp {
    /// Inserts a node that did not exist in the client's graph.
    NodeCreated {
        /// Full UI snapshot for the created node.
        snapshot: Box<UiNodeDto>,
        /// Parent receiving the node, if attached.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        parent: Option<NodeId>,
        /// Direct child index under `parent`, if known.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        index: Option<usize>,
    },
    /// Inserts a complete subtree that did not exist in the client's graph.
    ///
    /// Used for bulk insertions (N > 8 nodes) to avoid an O(NÂ²) `ui_child_index` scan
    /// and to reduce the op list to a single entry.
    SubtreeInserted {
        /// Root of the inserted subtree.
        root: NodeId,
        /// Parent node where `root` was attached.
        parent: NodeId,
        /// Full snapshots for all inserted nodes (root and descendants, depth-first).
        nodes: Vec<UiNodeDto>,
        /// Final direct child order for `parent` after insertion. A multi-root transaction
        /// may defer this to a later op for the same parent, avoiding repeated large lists.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        parent_children_after: Option<Vec<NodeId>>,
    },
    /// Inserts newly materialized direct children into one unchanged sibling order.
    ///
    /// Emitted after the subtree snapshots in an insertion-only transaction. The count lets
    /// clients reject a stale baseline instead of silently applying a splice to the wrong graph.
    ChildrenInserted {
        /// Parent whose direct children receive the new roots.
        parent: NodeId,
        /// Number of children in the client's graph before this insertion.
        expected_before_count: usize,
        /// Insertion position in that previous direct child order.
        index: usize,
        /// New direct child ids in final sibling order.
        children: Vec<NodeId>,
    },
    /// Removes a subtree from the client's graph.
    SubtreeRemoved {
        /// Root of the removed subtree.
        root: NodeId,
        /// Root and descendant ids removed by this operation.
        removed_ids: Vec<NodeId>,
        /// Post-removal child order for the former parent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        parent_after: Option<UiChildrenOrderPatch>,
    },
    /// Moves one existing node between parents or positions.
    NodeMoved {
        /// Node that moved.
        node: NodeId,
        /// Previous parent before the move.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        old_parent: Option<NodeId>,
        /// New parent after the move.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        new_parent: Option<NodeId>,
        /// Post-move child order for the previous parent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        old_parent_after: Option<UiChildrenOrderPatch>,
        /// Post-move child order for the new parent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        new_parent_after: Option<UiChildrenOrderPatch>,
    },
    /// Replaces the direct child order for one parent.
    ChildrenReordered {
        /// Parent whose children were reordered.
        parent: NodeId,
        /// Complete direct child order after the reorder.
        children: Vec<NodeId>,
    },
    /// Applies an incremental metadata patch to one node.
    NodeMetaPatched {
        /// Node whose metadata changed.
        node: NodeId,
        /// Metadata fields that changed.
        patch: UiNodeMetaPatch,
    },
    /// Applies an incremental parameter patch.
    ParamPatched {
        /// Node owning the parameter in the UI graph.
        node: NodeId,
        /// Parameter node that changed.
        param: NodeId,
        /// Parameter fields that changed.
        patch: UiParamPatch,
    },
    /// Replaces the UI undo/redo history state.
    HistoryPatched {
        /// Current history state after the transaction.
        history: UiHistoryState,
    },
    /// Adds or drops UI logger records.
    LoggerPatched {
        /// New logger records appended by the transaction.
        records_added: Vec<LogRecord>,
        /// Earliest retained record id after dropping old records.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        dropped_before: Option<u64>,
    },
}

/// Atomic UI graph transaction applied in order against a known graph version.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct UiGraphTransaction {
    /// Monotonic transaction id within the current project epoch.
    pub tx_id: u64,
    /// Project epoch this transaction belongs to.
    pub epoch: u64,
    /// Graph version expected before applying `ops`.
    pub base_graph_version: u64,
    /// Graph version after applying `ops`.
    pub next_graph_version: u64,
    /// Ordered patch operations applied atomically by the UI.
    pub ops: Vec<UiGraphOp>,
}
