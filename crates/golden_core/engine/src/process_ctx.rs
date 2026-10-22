use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use crate::edit::{Edit, EditOrigin, EditQueue, NodeTree};
use crate::engine::tree_revisions::RevisionSnapshot;
use crate::engine::{EngineTime, NodeWakeError, NodeWakeSource, NodeWaker};
pub use crate::engine::{SubtreeRevision, SubtreeStructureRevision};
use crate::events::{CustomEvent, EventFrame};
use crate::node::{
    DashboardWidgetTargetDescriptor, EventSubscription, Node, NodeId, NodeMetaPatch, NodeReference, NodeUuid,
    NodeWarning, PresentationHint,
};
use crate::parameter::{
    ParamValue, ParameterConstraints, ParameterControlState, ParameterEventBehaviour, ParameterSnapshot,
};
use serde::Serialize;

mod lease;
pub(crate) mod memory;
pub use lease::SnapshotLeaseError;

/// Conservative retained allocation weight for constraints, including shared enum
/// storage and spare capacities. Frozen enum storage uses its cached charge;
/// remaining owned metadata is traversed. Intended for native preparation
/// admission, not a realtime callback.
pub fn parameter_constraints_retained_bytes(constraints: &ParameterConstraints) -> usize {
    std::mem::size_of::<ParameterConstraints>().saturating_add(memory::constraints(constraints))
}

/// Parameter fields needed by a process-tree callback snapshot.
///
/// The full parameter DTO also carries editor defaults and UI hints; cloning those
/// for every runtime tree snapshot adds work without making them observable here.
#[derive(Clone, Debug, PartialEq)]
pub struct ProcessTreeParameterState {
    /// Current runtime value.
    pub value: ParamValue,
    /// Runtime value constraints.
    pub constraints: ParameterConstraints,
    /// Active parameter control state.
    pub control: ParameterControlState,
}

impl From<ParameterSnapshot> for ProcessTreeParameterState {
    fn from(snapshot: ParameterSnapshot) -> Self {
        Self {
            value: snapshot.value,
            constraints: snapshot.constraints,
            control: snapshot.control,
        }
    }
}

/// Read-only node record available during callback execution.
#[derive(Clone, Debug, PartialEq)]
pub struct ProcessTreeNodeSnapshot {
    /// Runtime node id.
    pub id: NodeId,
    /// Persistent node uuid.
    pub uuid: NodeUuid,
    /// Parent node id.
    pub parent: Option<NodeId>,
    /// First child in sibling chain.
    pub first_child: Option<NodeId>,
    /// Next sibling in parent chain.
    pub next_sibling: Option<NodeId>,
    /// Declared node type identifier.
    pub node_type: String,
    /// Node `decl_id` label used by path selectors.
    pub decl_id: String,
    /// Generated short name.
    pub short_name: String,
    /// User-visible label.
    pub label: String,
    /// Arbitrary classification tags from node metadata.
    pub tags: Vec<String>,
    /// Presentation hints from node metadata.
    pub presentation: PresentationHint,
    /// Effective enabled flag.
    ///
    /// This is `true` only when the node itself and every ancestor are enabled.
    pub enabled: bool,
    /// Whether the enabled flag may be toggled by UI/editor code.
    pub can_be_disabled: bool,
    /// Number of direct children.
    pub child_count: usize,
    /// Current parameter value when this node is a parameter.
    pub param_value: Option<ParamValue>,
    /// Current parameter constraints when this node is a parameter.
    pub param_constraints: Option<ParameterConstraints>,
    /// Current parameter control state when this node is a parameter.
    pub param_control: Option<ParameterControlState>,
    /// Dashboard widget capabilities exposed by this node.
    pub dashboard_widget_target: DashboardWidgetTargetDescriptor,
    /// Additional script-facing properties exposed by this node instance.
    pub script_properties: HashMap<String, ParamValue>,
    /// Additional script-facing methods exposed by this node instance.
    pub script_methods: Vec<String>,
}

impl ProcessTreeNodeSnapshot {
    /// Returns `true` when this snapshot represents a parameter node.
    pub fn is_parameter(&self) -> bool {
        self.param_value.is_some()
    }

    fn matches_child_key(&self, key: &str) -> bool {
        self.decl_id == key || self.short_name == key || self.label == key
    }

    /// Returns one script-facing property value by key.
    pub fn script_property(&self, key: &str) -> Option<Cow<'_, ParamValue>> {
        match key {
            "name" => Some(Cow::Owned(ParamValue::Str(self.label.clone()))),
            "enabled" => Some(Cow::Owned(ParamValue::Bool(self.enabled))),
            "type" => Some(Cow::Owned(ParamValue::Str(self.node_type.clone()))),
            "declId" => Some(Cow::Owned(ParamValue::Str(self.decl_id.clone()))),
            "value" => self.param_value.as_ref().map(Cow::Borrowed),
            _ => self.script_properties.get(key).map(Cow::Borrowed),
        }
    }

    /// Returns `true` when this node exposes `method` as a script-callable entrypoint.
    pub fn has_script_method(&self, method: &str) -> bool {
        self.script_methods.iter().any(|candidate| candidate == method) || CORE_SCRIPT_METHODS.contains(&method)
    }
}

const CORE_SCRIPT_METHODS: &[&str] = &[
    "setName",
    "setEnabled",
    "setDescription",
    "setReadOnly",
    "addNode",
    "removeNode",
    "addParameter",
    "removeParameter",
    "addFolder",
    "setParam",
    "listen",
    "unlisten",
    "getProperties",
    "getChildren",
    "getChild",
    "toString",
];

impl fmt::Display for ProcessTreeNodeSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(value) = &self.param_value {
            let constraints_text = self
                .param_constraints
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_else(|| "no constraints".to_string());
            return write!(
                f,
                "[{} <{}> {} ({})]",
                self.label, self.node_type, value, constraints_text
            );
        }

        if self.child_count == 0 {
            return write!(f, "[{} (<{}>)]", self.label, self.node_type);
        }

        let children_text = if self.child_count == 1 {
            "1 child".to_string()
        } else {
            format!("{} children", self.child_count)
        };
        write!(f, "[{} (<{}>), {}]", self.label, self.node_type, children_text)
    }
}

#[derive(Debug)]
struct ProcessTreeChildIndexes {
    ids_by_parent: HashMap<NodeId, Box<[NodeId]>>,
    ids_by_decl_id: HashMap<NodeId, HashMap<Box<str>, NodeId>>,
    ids_by_child_key: HashMap<NodeId, HashMap<Box<str>, NodeId>>,
}

/// Shared read-only tree snapshot for one processing pass.
#[derive(Debug)]
pub struct ProcessTreeSnapshot {
    weights: memory::AllocationWeights,
    storage_leases: std::sync::OnceLock<lease::StorageLeases>,
    revisions: RevisionSnapshot,
    root: NodeId,
    nodes: Arc<HashMap<NodeId, ProcessTreeNodeSnapshot>>,
    node_overrides: Arc<HashMap<NodeId, ProcessTreeNodeSnapshot>>,
    node_ids_by_uuid: Arc<HashMap<NodeUuid, NodeId>>,
    child_indexes: Arc<ProcessTreeChildIndexes>,
}

impl Clone for ProcessTreeSnapshot {
    fn clone(&self) -> Self {
        Self {
            weights: self.weights,
            storage_leases: std::sync::OnceLock::new(),
            revisions: self.revisions.clone(),
            root: self.root,
            nodes: self.nodes.clone(),
            node_overrides: self.node_overrides.clone(),
            node_ids_by_uuid: self.node_ids_by_uuid.clone(),
            child_indexes: self.child_indexes.clone(),
        }
    }
}

impl ProcessTreeSnapshot {
    /// Creates a new immutable tree snapshot.
    pub fn new(root: NodeId, nodes: HashMap<NodeId, ProcessTreeNodeSnapshot>) -> Self {
        let mut node_ids_by_uuid = HashMap::with_capacity(nodes.len());
        for (node_id, node) in &nodes {
            node_ids_by_uuid.entry(node.uuid).or_insert(*node_id);
        }
        let child_indexes = Self::build_child_indexes(&nodes);
        let mut snapshot = Self {
            weights: memory::AllocationWeights::default(),
            storage_leases: std::sync::OnceLock::new(),
            revisions: RevisionSnapshot::default(),
            root,
            nodes: Arc::new(nodes),
            node_overrides: Arc::new(HashMap::new()),
            node_ids_by_uuid: Arc::new(node_ids_by_uuid),
            child_indexes,
        };
        snapshot.weights = memory::weights(&snapshot);
        snapshot
    }

    pub(crate) fn from_indexed_nodes(
        root: NodeId,
        nodes: HashMap<NodeId, ProcessTreeNodeSnapshot>,
        node_ids_by_uuid: HashMap<NodeUuid, NodeId>,
        mut revisions: RevisionSnapshot,
    ) -> Self {
        let child_indexes = Self::build_child_indexes(&nodes);
        revisions.fold_subtrees(&nodes);
        let mut snapshot = Self {
            weights: memory::AllocationWeights::default(),
            storage_leases: std::sync::OnceLock::new(),
            revisions,
            root,
            nodes: Arc::new(nodes),
            node_overrides: Arc::new(HashMap::new()),
            node_ids_by_uuid: Arc::new(node_ids_by_uuid),
            child_indexes,
        };
        snapshot.weights = memory::weights(&snapshot);
        snapshot
    }

    fn build_child_indexes(nodes: &HashMap<NodeId, ProcessTreeNodeSnapshot>) -> Arc<ProcessTreeChildIndexes> {
        // Most folders are small; index only wide parents to avoid one map and
        // cloned selector strings per folder on every full-tree snapshot.
        const DECL_INDEX_THRESHOLD: usize = 16;
        let parent_ids = nodes
            .iter()
            .filter_map(|(node_id, node)| node.first_child.map(|_| *node_id));
        let mut child_ids_by_parent = HashMap::new();
        let mut child_ids_by_decl_id = HashMap::new();
        let mut child_ids_by_key = HashMap::new();

        for parent in parent_ids {
            let mut child_ids = Vec::new();
            let mut child = nodes.get(&parent).and_then(|node| node.first_child);
            // Most declared folders have only a few children; reserve a hash set for
            // long sibling chains while still detecting malformed short cycles.
            let mut short_chain = [NodeId(0); 8];
            let mut short_len = 0;
            let mut visited = None::<HashSet<NodeId>>;
            while let Some(child_id) = child {
                if let Some(visited) = visited.as_mut() {
                    if !visited.insert(child_id) {
                        break;
                    }
                } else if short_chain[..short_len].contains(&child_id) {
                    break;
                } else if short_len < short_chain.len() {
                    short_chain[short_len] = child_id;
                    short_len += 1;
                } else {
                    let mut long_chain = HashSet::with_capacity(short_chain.len() * 2);
                    long_chain.extend(short_chain);
                    long_chain.insert(child_id);
                    visited = Some(long_chain);
                }
                child_ids.push(child_id);
                child = nodes.get(&child_id).and_then(|node| node.next_sibling);
            }

            if child_ids.len() > DECL_INDEX_THRESHOLD {
                let mut decl_ids = HashMap::with_capacity(child_ids.len());
                let mut keys = HashMap::with_capacity(child_ids.len());
                for child_id in &child_ids {
                    let Some(child) = nodes.get(child_id) else {
                        break;
                    };
                    // Match the ordered scan across all aliases: an earlier label also
                    // wins over a later declaration id. Do not allocate duplicate aliases.
                    for key in [&child.decl_id, &child.short_name, &child.label] {
                        if !keys.contains_key(key.as_str()) {
                            keys.insert(key.clone().into_boxed_str(), *child_id);
                        }
                    }
                    decl_ids
                        .entry(child.decl_id.clone().into_boxed_str())
                        .or_insert(*child_id);
                    if let Some(short_decl_id) = child.decl_id.rsplit('/').next()
                        && short_decl_id != child.decl_id
                    {
                        decl_ids.entry(short_decl_id.into()).or_insert(*child_id);
                    }
                }
                child_ids_by_decl_id.insert(parent, decl_ids);
                child_ids_by_key.insert(parent, keys);
            }
            child_ids_by_parent.insert(parent, child_ids.into_boxed_slice());
        }

        Arc::new(ProcessTreeChildIndexes {
            ids_by_parent: child_ids_by_parent,
            ids_by_decl_id: child_ids_by_decl_id,
            ids_by_child_key: child_ids_by_key,
        })
    }

    /// Returns the snapshot root node id.
    pub fn root(&self) -> NodeId {
        self.root
    }

    /// Number of nodes retained by this immutable tree view.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Cached conservative allocation reservation, including shared indexes and spare
    /// container capacity. Shared storage is charged in full; this is not RSS telemetry.
    /// Overlay accounting revisits only overridden records, never the shared node graph.
    pub fn retained_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(2 * std::mem::size_of::<usize>())
            .saturating_add(self.weights.total())
    }

    /// Constant-time authored subtree identity for prepared read models. Includes descendant
    /// parameter, metadata, and topology writes before callbacks run. Ancestor metadata and
    /// inherited enabled state are outside this subtree; check its root's `enabled` separately.
    /// Custom events and writes in disjoint subtrees do not invalidate this revision.
    pub fn subtree_revision(&self, node: NodeId) -> Option<SubtreeRevision> {
        self.node(node).map(|_| self.revisions.revision(node))
    }

    /// Constant-time identity for named-node bindings: topology/order, UUID/type,
    /// label/short-name aliases and enablement. Ignores parameter value/control/constraint
    /// and presentation-only changes. Opaque writes invalidate conservatively. Ancestor
    /// enablement is outside this subtree; inspect the root's `enabled` separately.
    /// Tokens include their root and engine lifetime; synthetic overlays start a new lifetime.
    pub fn subtree_structure_revision(&self, node: NodeId) -> Option<SubtreeStructureRevision> {
        self.node(node).map(|_| self.revisions.structure_revision(node))
    }

    /// Returns one node snapshot by id.
    pub fn node(&self, node: NodeId) -> Option<&ProcessTreeNodeSnapshot> {
        self.node_overrides.get(&node).or_else(|| self.nodes.get(&node))
    }

    /// Returns a cloned snapshot with selected parameter values replaced.
    pub fn with_param_values(&self, values: impl IntoIterator<Item = (NodeId, ParamValue)>) -> Self {
        let mut snapshot = self.clone();
        let mut changed = false;
        for (node_id, value) in values {
            if let Some(node) = snapshot.node(node_id)
                && node.param_value.is_some()
            {
                let mut replacement = node.clone();
                replacement.param_value = Some(value);
                Arc::make_mut(&mut snapshot.node_overrides).insert(node_id, replacement);
                changed = true;
            }
        }
        if changed {
            // Overlays have no engine mutation clock; invalidate their read models
            // conservatively while keeping clones of an unchanged snapshot comparable.
            snapshot.revisions = RevisionSnapshot::default();
            snapshot.weights.overlays = memory::overlays(&snapshot.node_overrides);
            snapshot.weights.revisions = snapshot.revisions.retained_heap_bytes();
        }
        snapshot
    }

    /// Returns the first child that matches `key` under `parent`.
    ///
    /// Child keys are matched against `decl_id`, `short_name`, then `label`.
    pub fn find_child(&self, parent: NodeId, key: &str) -> Option<NodeId> {
        if let Some(children) = self.child_indexes.ids_by_child_key.get(&parent) {
            return children.get(key).copied();
        }
        for child_id in self.child_ids_slice(parent).iter().copied() {
            let child_snapshot = self.node(child_id)?;
            if child_snapshot.matches_child_key(key) {
                return Some(child_id);
            }
        }
        None
    }

    /// Returns the first direct child whose declaration id matches `decl_id`.
    ///
    /// Declared child ids can be stored as full paths such as `parameters/receiver`;
    /// callers may pass either the full declaration id or its final path segment.
    pub fn find_child_by_decl_id(&self, parent: NodeId, decl_id: &str) -> Option<NodeId> {
        if let Some(children) = self.child_indexes.ids_by_decl_id.get(&parent) {
            return children.get(decl_id).copied();
        }
        for child_id in self.child_ids_slice(parent) {
            let child = self.node(*child_id)?;
            if child.decl_id == decl_id || child.decl_id.rsplit('/').next() == Some(decl_id) {
                return Some(*child_id);
            }
        }
        None
    }

    /// Borrows all direct child node ids for `parent` in sibling order.
    ///
    /// The returned slice is owned by this immutable snapshot and performs no
    /// allocation. Prefer it in traversal-heavy code that does not need to
    /// mutate the list.
    pub fn child_ids_slice(&self, parent: NodeId) -> &[NodeId] {
        self.child_indexes.ids_by_parent.get(&parent).map_or(&[], Box::as_ref)
    }

    /// Returns all direct child node ids for `parent` in sibling order.
    pub fn child_ids(&self, parent: NodeId) -> Vec<NodeId> {
        self.child_ids_slice(parent).to_vec()
    }

    /// Returns one direct child by zero-based sibling index.
    pub fn child_at(&self, parent: NodeId, index: usize) -> Option<NodeId> {
        self.child_ids_slice(parent).get(index).copied()
    }

    /// Returns the previous sibling of `node` under `parent`, when present.
    pub fn previous_sibling(&self, parent: NodeId, node: NodeId) -> Option<NodeId> {
        let position = self.child_ids_slice(parent).iter().position(|child| *child == node)?;
        position
            .checked_sub(1)
            .and_then(|previous| self.child_at(parent, previous))
    }

    /// Returns the current runtime node id for `uuid`, when present in this snapshot.
    pub fn node_id_by_uuid(&self, uuid: NodeUuid) -> Option<NodeId> {
        self.node_ids_by_uuid.get(&uuid).copied()
    }

    /// Resolves a slash-separated child path from `start`.
    ///
    /// Supported segments:
    /// - `.` keeps current node.
    /// - empty segments are ignored.
    ///
    /// Parent traversal (`..`) is intentionally not supported.
    pub fn resolve_path_from(&self, start: NodeId, path: &str) -> Option<NodeId> {
        if path.trim().is_empty() || path.trim() == "." {
            return Some(start);
        }

        let mut current = start;
        for segment in path.split('/') {
            let segment = segment.trim();
            if segment.is_empty() || segment == "." {
                continue;
            }
            if segment == ".." {
                return None;
            }
            current = self.find_child(current, segment)?;
        }

        Some(current)
    }
}

/// Runtime phase of the current processing pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecutionPhase {
    /// Main engine tick.
    EngineTick,
    /// Additional stabilization pass after the main tick.
    EndOfTickStabilization,
}

/// Mutable context passed to node callbacks during processing.
///
/// # Examples
/// ```rust
/// use golden_engine::engine::EngineTime;
/// use golden_engine::process_ctx::{ExecutionPhase, ProcessCtx};
///
/// let time = EngineTime { tick: 1, micro: 0, seq: 0 };
/// let ctx = ProcessCtx::new(ExecutionPhase::EngineTick, time);
/// assert!(ctx.events.is_empty());
/// ```
pub struct ProcessCtx {
    /// Phase currently being processed.
    pub phase: ExecutionPhase,
    /// Events visible to nodes for the current callback.
    pub events: EventFrame,
    /// Edits requested by nodes during callbacks.
    pub edits: EditQueue,
    /// Current engine timestamp for emitted operations.
    pub time: EngineTime,
    /// Time elapsed since this node's previous update callback.
    ///
    /// This value is meaningful in `Node::update`.
    pub delta_time: Duration,
    /// Total runtime elapsed since engine start.
    pub runtime_elapsed: Duration,
    /// Optional read-only tree snapshot shared for this callback pass.
    tree_snapshot: Option<Arc<ProcessTreeSnapshot>>,
    /// Live parameter targets requested by a callback that does not need the whole tree.
    referenced_params: HashMap<NodeUuid, (NodeId, ParamValue)>,
    /// Scoped registration source supplied to engine-owned ready callbacks.
    node_wake_source: Option<(NodeWakeSource, NodeId)>,
    script_args: Option<crate::script::ScriptArguments>,
    current_event: Option<usize>,
}

impl ProcessCtx {
    /// Creates a processing context for a given phase and time.
    pub fn new(phase: ExecutionPhase, time: EngineTime) -> Self {
        Self {
            phase,
            events: EventFrame::new(),
            edits: EditQueue::new(),
            time,
            delta_time: Duration::ZERO,
            runtime_elapsed: Duration::ZERO,
            tree_snapshot: None,
            referenced_params: HashMap::new(),
            node_wake_source: None,
            script_args: None,
            current_event: None,
        }
    }

    /// Attaches a wake reservation source scoped to the current ready callback's node.
    pub(crate) fn set_node_wake_source(&mut self, source: NodeWakeSource, node: NodeId) {
        self.node_wake_source = Some((source, node));
    }

    pub(crate) fn set_script_args(&mut self, args: Option<crate::script::ScriptArguments>) {
        self.script_args = args;
    }

    /// Captures the current script invocation without cloning its values. The caller
    /// may retain the admitted lease for asynchronous use without copying argument data.
    /// Forwarding a different argument slice cannot capture the original invocation.
    pub fn capture_script_args(&self, args: &[ParamValue]) -> Result<crate::script::ScriptArguments, &'static str> {
        self.script_args
            .as_ref()
            .filter(|shared| shared.len() == args.len() && shared.as_ptr() == args.as_ptr())
            .cloned()
            .ok_or("argument capture requires the current engine-owned script invocation")
    }

    /// Reserves a coalesced wake handle before starting this node's workers.
    ///
    /// Available in engine-owned ready, inbox, script/mutation, scheduled, and worker callbacks.
    /// Standalone, init, attached, and destroy contexts cannot register one.
    pub fn node_waker(&self) -> Result<NodeWaker, NodeWakeError> {
        let (source, node) = self.node_wake_source.as_ref().ok_or(NodeWakeError::Unavailable)?;
        source.register(*node)
    }
    /// Reserves another live node's lifetime for work admitted by this callback.
    /// The snapshot validates the target before a bounded wake slot is reserved.
    pub fn node_waker_for(&self, node: NodeId) -> Result<NodeWaker, NodeWakeError> {
        let (source, _) = self.node_wake_source.as_ref().ok_or(NodeWakeError::Unavailable)?;
        if self
            .tree_snapshot
            .as_ref()
            .is_none_or(|snapshot| snapshot.node(node).is_none())
        {
            return Err(NodeWakeError::Closed);
        }
        source.register(node)
    }

    /// Attaches a read-only tree snapshot to this context.
    pub fn set_tree_snapshot(&mut self, snapshot: Arc<ProcessTreeSnapshot>) {
        self.tree_snapshot = Some(snapshot);
    }

    /// Clears any attached tree snapshot.
    pub fn clear_tree_snapshot(&mut self) {
        self.tree_snapshot = None;
    }

    /// Returns the attached tree snapshot when available.
    pub fn tree_snapshot(&self) -> Option<&ProcessTreeSnapshot> {
        self.tree_snapshot.as_deref()
    }

    /// Returns a cloned shared tree snapshot handle when available.
    pub fn tree_snapshot_arc(&self) -> Option<Arc<ProcessTreeSnapshot>> {
        self.tree_snapshot.clone()
    }

    /// Returns the live parameter target resolved for an inbox reference request.
    /// The UUID, rather than the optional cached node id, determines identity.
    pub fn referenced_param(&self, reference: &NodeReference) -> Option<(NodeId, &ParamValue)> {
        self.referenced_params
            .get(&reference.uuid())
            .map(|(node, value)| (*node, value))
    }

    pub(crate) fn cache_referenced_param(&mut self, uuid: NodeUuid, node: NodeId, value: ParamValue) {
        self.referenced_params.insert(uuid, (node, value));
    }

    /// Queues a parameter update edit.
    pub fn set_param(&mut self, node: NodeId, value: ParamValue) {
        self.set_param_with_behaviour(node, value, ParameterEventBehaviour::Coalesce);
    }

    /// Begins an edit session for grouping subsequent intents into one undo boundary.
    pub fn begin_edit_session(&mut self, origin: EditOrigin, client_edit_id: impl Into<String>, label: Option<String>) {
        self.edits.push(Edit::BeginEditSession {
            origin,
            label,
            client_edit_id: client_edit_id.into(),
            ui_client_instance_id: None,
        });
    }

    /// Ends an edit session previously opened with [`Self::begin_edit_session`].
    pub fn end_edit_session(&mut self, client_edit_id: impl Into<String>) {
        self.edits.push(Edit::EndEditSession {
            client_edit_id: client_edit_id.into(),
        });
    }

    /// Queues a parameter update edit with an explicit coalescing strategy.
    pub fn set_param_with_behaviour(&mut self, node: NodeId, value: ParamValue, behaviour: ParameterEventBehaviour) {
        self.edits.push(Edit::SetParam { node, value, behaviour });
    }
    /// Queue a direct value change attributed to its runtime producer.
    pub fn set_param_from(
        &mut self,
        source: NodeId,
        node: NodeId,
        value: ParamValue,
        behaviour: ParameterEventBehaviour,
    ) {
        self.edits.push_from(source, Edit::SetParam { node, value, behaviour });
    }

    /// The exact event being dispatched by the standard node inbox callbacks.
    /// Its value may differ from the latest tree snapshot after appended edits.
    pub fn current_event(&self) -> Option<&crate::events::Event> {
        self.current_event
            .filter(|index| *index < self.events.len())
            .map(|index| self.events[index].as_ref())
    }
    pub(crate) fn swap_current_event(&mut self, index: Option<usize>) -> Option<usize> {
        std::mem::replace(&mut self.current_event, index)
    }

    /// Queues one script-property update on `node`.
    pub fn set_node_script_property(&mut self, node: NodeId, write: crate::script::ScriptPropertyWrite) {
        self.edits.push(Edit::SetNodeScriptProperty { node, write });
    }

    /// Queues one script-method invocation on `node`.
    pub fn call_node_script_method(&mut self, node: NodeId, call: crate::script::ScriptMethodCall) {
        self.edits.push(Edit::CallNodeScriptMethod { node, call });
    }

    /// Queues one runtime Rust callback invocation on `node`.
    ///
    /// The callback is executed by the engine while applying queued edits.
    pub fn call_node_mutation<F>(&mut self, node: NodeId, callback: F)
    where
        F: FnOnce(&mut dyn Node, &mut ProcessCtx) -> Result<(), String> + Send + 'static,
    {
        self.edits.push(Edit::CallNodeMutation {
            node,
            callback: Box::new(callback),
            needs_tree_snapshot: true,
        });
    }

    /// Queues one runtime Rust callback invocation on `node` whose callback never
    /// reads `ctx.tree_snapshot()`.
    ///
    /// The engine skips the whole-tree snapshot build for these, which keeps bulk
    /// paths (project load, subtree paste) from going `O(n^2)` on mutation batches.
    pub fn call_node_mutation_without_snapshot<F>(&mut self, node: NodeId, callback: F)
    where
        F: FnOnce(&mut dyn Node, &mut ProcessCtx) -> Result<(), String> + Send + 'static,
    {
        self.edits.push(Edit::CallNodeMutation {
            node,
            callback: Box::new(callback),
            needs_tree_snapshot: false,
        });
    }

    /// Sets or replaces the default warning on `node`.
    pub fn set_node_warning(&mut self, node: NodeId, message: impl Into<String>) {
        self.set_node_warning_with(node, None, message, None);
    }

    /// Sets or replaces one warning by id on `node`.
    ///
    /// `warning_id = None` uses the default empty warning id.
    pub fn set_node_warning_with(
        &mut self,
        node: NodeId,
        warning_id: Option<&str>,
        message: impl Into<String>,
        detail: Option<&str>,
    ) {
        self.edits.push(Edit::SetNodeWarning {
            node,
            warning: NodeWarning {
                id: warning_id.unwrap_or_default().to_string(),
                message: message.into(),
                detail: detail.map(str::to_string),
            },
        });
    }

    /// Clears one warning by id on `node`.
    ///
    /// `warning_id = None` clears all warnings on `node`.
    pub fn clear_node_warning(&mut self, node: NodeId, warning_id: Option<&str>) {
        self.edits.push(Edit::ClearNodeWarning {
            node,
            warning_id: warning_id.map(str::to_string),
        });
    }

    /// Clears all warnings on `node`.
    pub fn clear_all_node_warnings(&mut self, node: NodeId) {
        self.clear_node_warning(node, None);
    }

    /// Sets warning surfacing depth for descendant warnings on `node`.
    pub fn set_node_child_warning_depth(&mut self, node: NodeId, max_depth: u32) {
        self.edits.push(Edit::SetNodeChildWarningDepth { node, max_depth });
    }

    /// Queues a metadata patch for `node`.
    pub fn patch_node_meta(&mut self, node: NodeId, patch: NodeMetaPatch) {
        self.edits.push(Edit::PatchMeta { node, patch });
    }

    /// Queues insertion of a child node.
    pub fn add_child<N: Node + 'static>(&mut self, parent: NodeId, child: N, after: Option<NodeId>) {
        self.add_child_boxed(parent, Box::new(child), after);
    }

    /// Queues insertion of a boxed child node.
    pub fn add_child_boxed(&mut self, parent: NodeId, child: Box<dyn Node>, after: Option<NodeId>) {
        self.edits.push(Edit::AddNode {
            parent,
            prev_sibling: after,
            node: child,
        });
    }

    /// Queues insertion of a detached child subtree.
    pub fn add_child_tree(&mut self, parent: NodeId, tree: NodeTree, after: Option<NodeId>) {
        self.edits.push(Edit::AddNodeTree {
            parent,
            prev_sibling: after,
            tree,
        });
    }

    /// Queues insertion of a typed user-curated item node.
    pub fn add_user_item<N: Node + 'static>(&mut self, parent: NodeId, child: N, after: Option<NodeId>) {
        self.add_user_item_boxed(parent, Box::new(child), after);
    }

    /// Queues insertion of a boxed user-curated item node.
    pub fn add_user_item_boxed(&mut self, parent: NodeId, child: Box<dyn Node>, after: Option<NodeId>) {
        self.edits.push(Edit::AddUserItem {
            parent,
            prev_sibling: after,
            node: child,
        });
    }

    /// Queues insertion of a detached user-curated item subtree.
    pub fn add_user_item_tree(&mut self, parent: NodeId, tree: NodeTree, after: Option<NodeId>) {
        self.edits.push(Edit::AddUserItemTree {
            parent,
            prev_sibling: after,
            tree,
        });
    }

    /// Queues insertion of a blueprint-backed user-curated item.
    pub fn add_blueprint_item(
        &mut self,
        parent: NodeId,
        blueprint_id: impl Into<String>,
        label: Option<String>,
        after: Option<NodeId>,
    ) {
        self.edits.push(Edit::CreateBlueprintInstance {
            blueprint_id: blueprint_id.into(),
            parent,
            prev_sibling: after,
            label,
        });
    }

    /// Queues replacement of a node by a typed node value.
    pub fn replace_node<N: Node + 'static>(&mut self, node: NodeId, new_node: N) {
        self.replace_node_boxed(node, Box::new(new_node));
    }

    /// Queues replacement of a node by a boxed node value.
    pub fn replace_node_boxed(&mut self, node: NodeId, new_node: Box<dyn Node>) {
        self.edits.push(Edit::ReplaceNode { node, new_node });
    }

    /// Queues a custom event to be emitted by the engine.
    pub fn emit_custom_event(&mut self, event: CustomEvent) {
        self.edits.push(Edit::EmitCustomEvent { event });
    }

    /// Requests a full graph execution-rule reevaluation.
    pub fn reevaluate_graph(&mut self) {
        self.edits.push(Edit::ReevaluateGraph);
    }

    /// Queues a direct listener subscription from `subscriber` to `target`.
    pub fn add_event_listener(&mut self, subscriber: NodeId, target: NodeId) {
        self.add_event_listener_subtree(subscriber, target, 0);
    }

    /// Queues a listener subscription from `subscriber` to `target` subtree.
    pub fn add_event_listener_subtree(&mut self, subscriber: NodeId, target: NodeId, max_depth: u32) {
        self.edits.push(Edit::AddEventListener {
            subscriber,
            subscription: EventSubscription::subtree(target, max_depth),
        });
    }

    /// Queues removal of a direct listener subscription from `subscriber` to `target`.
    pub fn remove_event_listener(&mut self, subscriber: NodeId, target: NodeId) {
        self.remove_event_listener_subtree(subscriber, target, 0);
    }

    /// Queues removal of a subtree listener subscription from `subscriber` to `target`.
    pub fn remove_event_listener_subtree(&mut self, subscriber: NodeId, target: NodeId, max_depth: u32) {
        self.edits.push(Edit::RemoveEventListener {
            subscriber,
            subscription: EventSubscription::subtree(target, max_depth),
        });
    }

    /// Serializes and queues a custom event payload.
    ///
    /// # Examples
    /// ```rust
    /// use golden_engine::engine::EngineTime;
    /// use golden_engine::process_ctx::{ExecutionPhase, ProcessCtx};
    ///
    /// let mut ctx = ProcessCtx::new(
    ///     ExecutionPhase::EngineTick,
    ///     EngineTime { tick: 0, micro: 0, seq: 0 },
    /// );
    ///
    /// ctx.emit_custom_payload("transport.play", None, &true)
    ///     .expect("payload should serialize");
    ///
    /// assert_eq!(ctx.edits.pending.len(), 1);
    /// ```
    pub fn emit_custom_payload<U: Serialize>(
        &mut self,
        topic: impl Into<String>,
        origin: Option<NodeId>,
        payload: &U,
    ) -> serde_json::Result<()> {
        let event = CustomEvent::from_payload(topic, origin, payload)?;
        self.emit_custom_event(event);
        Ok(())
    }

    /// Serializes and queues a latest-wins custom event payload.
    pub fn emit_latest_custom_payload<U: Serialize>(
        &mut self,
        topic: impl Into<String>,
        origin: Option<NodeId>,
        payload: &U,
    ) -> serde_json::Result<()> {
        let event = CustomEvent::from_latest_payload(topic, origin, payload)?;
        self.emit_custom_event(event);
        Ok(())
    }

    /// Serializes and queues an engine-internal custom event payload.
    ///
    /// Transient events are delivered to node inboxes but are excluded from UI
    /// replay and transport publication.
    pub fn emit_transient_custom_payload<U: Serialize>(
        &mut self,
        topic: impl Into<String>,
        origin: Option<NodeId>,
        payload: &U,
    ) -> serde_json::Result<()> {
        let event = CustomEvent::from_transient_payload(topic, origin, payload)?;
        self.emit_custom_event(event);
        Ok(())
    }

    /// Returns `true` when `param` changed in this callback frame.
    pub fn has_param_changed(&self, param: NodeId) -> bool {
        self.events.iter().any(|event| matches!(event.kind, crate::events::EventKind::ParamChanged { param: changed, .. } if changed == param))
    }

    /// Returns `true` when at least one structural event is present in this callback frame.
    pub fn has_structure_changed(&self) -> bool {
        self.events.iter().any(|event| {
            matches!(
                event.kind,
                crate::events::EventKind::ChildAdded { .. }
                    | crate::events::EventKind::ChildRemoved { .. }
                    | crate::events::EventKind::ChildReplaced { .. }
                    | crate::events::EventKind::ChildMoved { .. }
                    | crate::events::EventKind::ChildReordered { .. }
                    | crate::events::EventKind::NodeCreated { .. }
                    | crate::events::EventKind::NodeDeleted { .. }
            )
        })
    }
}
