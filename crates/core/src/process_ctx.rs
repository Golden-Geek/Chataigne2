use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use crate::edit::{Edit, EditOrigin, EditQueue};
use crate::engine::EngineTime;
use crate::events::{CustomEvent, Event};
use crate::node::{
    DashboardWidgetTargetDescriptor, EventSubscription, Node, NodeId, NodeMetaPatch, NodeUuid, NodeWarning,
};
use crate::parameter::{ParamValue, ParameterConstraints, ParameterEventBehaviour};
use serde::Serialize;

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
    /// Node enabled flag.
    pub enabled: bool,
    /// Whether the enabled flag may be toggled by UI/editor code.
    pub can_be_disabled: bool,
    /// Number of direct children.
    pub child_count: usize,
    /// Current parameter value when this node is a parameter.
    pub param_value: Option<ParamValue>,
    /// Current parameter constraints when this node is a parameter.
    pub param_constraints: Option<ParameterConstraints>,
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

    fn matches_decl_id(&self, decl_id: &str) -> bool {
        self.decl_id == decl_id || self.decl_id.rsplit('/').next() == Some(decl_id)
    }

    fn matches_child_key(&self, key: &str) -> bool {
        self.decl_id == key || self.short_name == key || self.label == key
    }

    /// Returns one script-facing property value by key.
    pub fn script_property(&self, key: &str) -> Option<&ParamValue> {
        self.script_properties.get(key)
    }

    /// Returns `true` when this node exposes `method` as a script-callable entrypoint.
    pub fn has_script_method(&self, method: &str) -> bool {
        self.script_methods.iter().any(|candidate| candidate == method)
    }
}

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

/// Shared read-only tree snapshot for one processing pass.
#[derive(Clone, Debug)]
pub struct ProcessTreeSnapshot {
    root: NodeId,
    nodes: HashMap<NodeId, ProcessTreeNodeSnapshot>,
}

impl ProcessTreeSnapshot {
    /// Creates a new immutable tree snapshot.
    pub fn new(root: NodeId, nodes: HashMap<NodeId, ProcessTreeNodeSnapshot>) -> Self {
        Self { root, nodes }
    }

    /// Returns the snapshot root node id.
    pub fn root(&self) -> NodeId {
        self.root
    }

    /// Returns one node snapshot by id.
    pub fn node(&self, node: NodeId) -> Option<&ProcessTreeNodeSnapshot> {
        self.nodes.get(&node)
    }

    /// Returns the first child that matches `key` under `parent`.
    ///
    /// Child keys are matched against `decl_id`, `short_name`, then `label`.
    pub fn find_child(&self, parent: NodeId, key: &str) -> Option<NodeId> {
        let mut child = self.node(parent)?.first_child;
        while let Some(child_id) = child {
            let child_snapshot = self.node(child_id)?;
            if child_snapshot.matches_child_key(key) {
                return Some(child_id);
            }
            child = child_snapshot.next_sibling;
        }
        None
    }

    /// Returns the first direct child whose declaration id matches `decl_id`.
    ///
    /// Declared child ids can be stored as full paths such as `parameters/receiver`;
    /// callers may pass either the full declaration id or its final path segment.
    pub fn find_child_by_decl_id(&self, parent: NodeId, decl_id: &str) -> Option<NodeId> {
        let mut child = self.node(parent)?.first_child;
        while let Some(child_id) = child {
            let child_snapshot = self.node(child_id)?;
            if child_snapshot.matches_decl_id(decl_id) {
                return Some(child_id);
            }
            child = child_snapshot.next_sibling;
        }
        None
    }

    /// Returns all direct child node ids for `parent` in sibling order.
    pub fn child_ids(&self, parent: NodeId) -> Vec<NodeId> {
        let mut children = Vec::new();
        let mut child = self.node(parent).and_then(|node| node.first_child);
        while let Some(child_id) = child {
            children.push(child_id);
            child = self.node(child_id).and_then(|snapshot| snapshot.next_sibling);
        }
        children
    }

    /// Returns one direct child by zero-based sibling index.
    pub fn child_at(&self, parent: NodeId, index: usize) -> Option<NodeId> {
        let mut current = 0usize;
        let mut child = self.node(parent)?.first_child;
        while let Some(child_id) = child {
            if current == index {
                return Some(child_id);
            }
            current = current.saturating_add(1);
            child = self.node(child_id).and_then(|snapshot| snapshot.next_sibling);
        }
        None
    }

    /// Returns the previous sibling of `node` under `parent`, when present.
    pub fn previous_sibling(&self, parent: NodeId, node: NodeId) -> Option<NodeId> {
        let mut previous = None;
        let mut child = self.node(parent)?.first_child;
        while let Some(child_id) = child {
            if child_id == node {
                return previous;
            }
            previous = Some(child_id);
            child = self.node(child_id).and_then(|snapshot| snapshot.next_sibling);
        }

        None
    }

    /// Returns the current runtime node id for `uuid`, when present in this snapshot.
    pub fn node_id_by_uuid(&self, uuid: NodeUuid) -> Option<NodeId> {
        self.nodes
            .iter()
            .find_map(|(node_id, node)| (node.uuid == uuid).then_some(*node_id))
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
    pub events: Vec<Event>,
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
}

impl ProcessCtx {
    /// Creates a processing context for a given phase and time.
    pub fn new(phase: ExecutionPhase, time: EngineTime) -> Self {
        Self {
            phase,
            events: Vec::new(),
            edits: EditQueue::new(),
            time,
            delta_time: Duration::ZERO,
            runtime_elapsed: Duration::ZERO,
            tree_snapshot: None,
        }
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

    /// Queues one script-property update on `node`.
    pub fn set_node_script_property(&mut self, node: NodeId, property: impl Into<String>, value: ParamValue) {
        self.edits.push(Edit::SetNodeScriptProperty {
            node,
            property: property.into(),
            value,
        });
    }

    /// Queues one script-method invocation on `node`.
    pub fn call_node_script_method(&mut self, node: NodeId, method: impl Into<String>, args: Vec<ParamValue>) {
        self.edits.push(Edit::CallNodeScriptMethod {
            node,
            method: method.into(),
            args,
        });
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

#[cfg(test)]
mod process_ctx_tests;
