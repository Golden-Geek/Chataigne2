use crate::edit::Edit;
use crate::events::EventKind;
use crate::parameter::{ParamValue, ParameterChangeCheck, ParameterEventBehaviour};
use crate::process_ctx::ProcessCtx;

use super::{DeclId, Node, NodeId};

/// Typed conversion contract between Rust values and [`ParamValue`].
pub trait ParameterValueType: Clone {
    /// Converts a typed value into its runtime parameter representation.
    fn to_param_value(value: Self) -> ParamValue;

    /// Attempts to decode a runtime parameter value to this type.
    fn from_param_value(value: &ParamValue) -> Option<Self>;
}

impl ParameterValueType for i32 {
    fn to_param_value(value: Self) -> ParamValue {
        ParamValue::Int(value)
    }

    fn from_param_value(value: &ParamValue) -> Option<Self> {
        value.as_int()
    }
}

impl ParameterValueType for f64 {
    fn to_param_value(value: Self) -> ParamValue {
        ParamValue::Float(value)
    }

    fn from_param_value(value: &ParamValue) -> Option<Self> {
        value.as_float()
    }
}

impl ParameterValueType for String {
    fn to_param_value(value: Self) -> ParamValue {
        ParamValue::Str(value)
    }

    fn from_param_value(value: &ParamValue) -> Option<Self> {
        value.as_str()
    }
}

impl ParameterValueType for bool {
    fn to_param_value(value: Self) -> ParamValue {
        ParamValue::Bool(value)
    }

    fn from_param_value(value: &ParamValue) -> Option<Self> {
        value.as_bool()
    }
}

impl ParameterValueType for (f64, f64) {
    fn to_param_value(value: Self) -> ParamValue {
        ParamValue::Vec2(value.0, value.1)
    }

    fn from_param_value(value: &ParamValue) -> Option<Self> {
        value.as_vec2()
    }
}

impl ParameterValueType for (f64, f64, f64) {
    fn to_param_value(value: Self) -> ParamValue {
        ParamValue::Vec3(value.0, value.1, value.2)
    }

    fn from_param_value(value: &ParamValue) -> Option<Self> {
        value.as_vec3()
    }
}

impl ParameterValueType for (f64, f64, f64, f64) {
    fn to_param_value(value: Self) -> ParamValue {
        ParamValue::Color(value.0, value.1, value.2, value.3)
    }

    fn from_param_value(value: &ParamValue) -> Option<Self> {
        value.as_color()
    }
}

impl ParameterValueType for NodeId {
    fn to_param_value(value: Self) -> ParamValue {
        ParamValue::Reference(value)
    }

    fn from_param_value(value: &ParamValue) -> Option<Self> {
        if let ParamValue::Reference(node) = value {
            Some(*node)
        } else {
            None
        }
    }
}

impl ParameterValueType for ParamValue {
    fn to_param_value(value: Self) -> ParamValue {
        value
    }

    fn from_param_value(value: &ParamValue) -> Option<Self> {
        Some(value.clone())
    }
}

/// Typed handle to a parameter node id with local cached value.
#[derive(Clone, Debug, PartialEq)]
pub struct ParameterHandle<T: ParameterValueType + PartialEq> {
    node: NodeId,
    cached: T,
    change_check: ParameterChangeCheck,
    event_behaviour: ParameterEventBehaviour,
}

impl<T: ParameterValueType + PartialEq> ParameterHandle<T> {
    /// Creates a typed parameter handle with default policies.
    pub fn new(node: NodeId, initial: T) -> Self {
        Self {
            node,
            cached: initial,
            change_check: ParameterChangeCheck::ValueChange,
            event_behaviour: ParameterEventBehaviour::Coalesce,
        }
    }

    /// Creates a typed parameter handle with explicit policies.
    pub fn with_policies(
        node: NodeId,
        initial: T,
        change_check: ParameterChangeCheck,
        event_behaviour: ParameterEventBehaviour,
    ) -> Self {
        Self {
            node,
            cached: initial,
            change_check,
            event_behaviour,
        }
    }

    /// Returns the parameter node id.
    pub fn id(&self) -> NodeId {
        self.node
    }

    /// Returns the locally cached value.
    pub fn get_cached(&self) -> &T {
        &self.cached
    }

    /// Replaces the local cache without emitting an edit.
    pub fn set_cached(&mut self, value: T) {
        self.cached = value;
    }

    /// Returns the current change-check policy.
    pub fn change_check(&self) -> &ParameterChangeCheck {
        &self.change_check
    }

    /// Replaces the change-check policy.
    pub fn set_change_check(&mut self, change_check: ParameterChangeCheck) {
        self.change_check = change_check;
    }

    /// Returns the current event behavior.
    pub fn event_behaviour(&self) -> ParameterEventBehaviour {
        self.event_behaviour
    }

    /// Replaces the event behavior.
    pub fn set_event_behaviour(&mut self, event_behaviour: ParameterEventBehaviour) {
        self.event_behaviour = event_behaviour;
    }

    /// Updates this handle cache from a runtime [`ParamValue`].
    ///
    /// Returns `true` when decoding succeeds.
    pub fn apply_runtime_value(&mut self, value: &ParamValue) -> bool {
        if let Some(decoded) = T::from_param_value(value) {
            self.cached = decoded;
            true
        } else {
            false
        }
    }

    /// Queues a `SetParam` edit using this handle policies and updates local cache.
    pub fn set(&mut self, ctx: &mut ProcessCtx, value: T) {
        let value_changed = self.cached != value;
        let should_emit = self.change_check == ParameterChangeCheck::None || value_changed;

        if should_emit {
            ctx.set_param_with_behaviour(self.node, T::to_param_value(value.clone()), self.event_behaviour);
        }

        self.cached = value;
    }
}

/// Handle to an existing node id with structural edit helpers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NodeHandle {
    node: NodeId,
}

impl NodeHandle {
    /// Creates a node handle from an existing node id.
    pub fn new(node: NodeId) -> Self {
        Self { node }
    }

    /// Returns the wrapped node id.
    pub fn id(&self) -> NodeId {
        self.node
    }

    /// Queues removal of this node.
    pub fn remove(&self, ctx: &mut ProcessCtx) {
        ctx.edits.push(Edit::RemoveNode { node: self.node });
    }

    /// Queues movement of this node under a new parent.
    pub fn move_to(&self, ctx: &mut ProcessCtx, new_parent: NodeId, after: Option<NodeId>) {
        ctx.edits.push(Edit::MoveNode {
            node: self.node,
            new_parent,
            new_prev_sibling: after,
        });
    }

    /// Queues replacement of this node id by a typed node value.
    pub fn replace_with<N: Node + 'static>(&self, ctx: &mut ProcessCtx, new_node: N) {
        ctx.replace_node(self.node, new_node);
    }

    /// Queues replacement of this node id by a boxed node value.
    pub fn replace_with_boxed(&self, ctx: &mut ProcessCtx, new_node: Box<dyn Node>) {
        ctx.replace_node_boxed(self.node, new_node);
    }
}

/// Declared optional child slot identified by parent + `decl_id`.
///
/// The slot may currently map to a runtime node id (`current`) or be absent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PotentialNodeHandle {
    parent: NodeId,
    decl_id: DeclId,
    current: Option<NodeId>,
    pending_create: bool,
}

impl PotentialNodeHandle {
    /// Creates an empty potential slot under `parent`.
    pub fn new(parent: NodeId, decl_id: impl Into<String>) -> Self {
        Self {
            parent,
            decl_id: DeclId(decl_id.into()),
            current: None,
            pending_create: false,
        }
    }

    /// Creates a potential slot with a known current runtime node id.
    pub fn with_current(parent: NodeId, decl_id: impl Into<String>, current: NodeId) -> Self {
        Self {
            parent,
            decl_id: DeclId(decl_id.into()),
            current: Some(current),
            pending_create: false,
        }
    }

    /// Returns the parent node id for this slot.
    pub fn parent(&self) -> NodeId {
        self.parent
    }

    /// Returns the declared slot id.
    pub fn decl_id(&self) -> &DeclId {
        &self.decl_id
    }

    /// Returns `true` when a runtime node is currently bound to this slot.
    pub fn is_present(&self) -> bool {
        self.current.is_some()
    }

    /// Returns `true` when this slot has queued creation and is waiting for `ChildAdded`.
    pub fn is_pending_create(&self) -> bool {
        self.pending_create
    }

    /// Returns the currently bound node id, when present.
    pub fn current_id(&self) -> Option<NodeId> {
        self.current
    }

    /// Returns a concrete node handle when this slot is present.
    pub fn get(&self) -> Option<NodeHandle> {
        self.current.map(NodeHandle::new)
    }

    /// Binds an existing node id as the current materialized slot value.
    pub fn bind_existing(&mut self, node: NodeId) {
        self.current = Some(node);
        self.pending_create = false;
    }

    /// Detaches local knowledge of the current node id without queuing edits.
    pub fn detach_current(&mut self) -> Option<NodeId> {
        self.pending_create = false;
        self.current.take()
    }

    /// Moves an existing node under this slot parent and marks it as current.
    pub fn attach_existing(&mut self, ctx: &mut ProcessCtx, node: NodeId, after: Option<NodeId>) {
        ctx.edits.push(Edit::MoveNode {
            node,
            new_parent: self.parent,
            new_prev_sibling: after,
        });
        self.current = Some(node);
        self.pending_create = false;
    }

    /// Removes the current slot node if present and clears local binding.
    pub fn clear(&mut self, ctx: &mut ProcessCtx) -> Option<NodeId> {
        let removed = self.current.take();
        if let Some(node) = removed {
            ctx.edits.push(Edit::RemoveNode { node });
        }
        self.pending_create = false;
        removed
    }

    /// Queues replacement (or creation) of this slot content.
    ///
    /// If a current node is known, `ReplaceNode` is queued. If absent, `AddNode` is
    /// queued under the slot parent.
    ///
    /// Replacement preserves the node id, so `current` stays bound when present.
    /// Creation still requires runtime id allocation, so `current` remains `None`.
    pub fn replace_with<N: Node + 'static>(&mut self, ctx: &mut ProcessCtx, new_node: N) {
        self.replace_with_boxed(ctx, Box::new(new_node));
    }

    /// Boxed variant of [`Self::replace_with`].
    pub fn replace_with_boxed(&mut self, ctx: &mut ProcessCtx, new_node: Box<dyn Node>) {
        if let Some(current) = self.current {
            ctx.replace_node_boxed(current, new_node);
            self.pending_create = false;
        } else {
            let mut new_node = new_node;
            new_node.node_data_mut().meta.decl_id = self.decl_id.clone();
            ctx.add_child_boxed(self.parent, new_node, None);
            self.current = None;
            self.pending_create = true;
        }
    }

    /// Reconciles a `ChildAdded` event and binds this slot when `decl_id` matches.
    pub fn reconcile_child_added(&mut self, parent: NodeId, child: NodeId, decl_id: &DeclId) -> bool {
        if parent == self.parent && decl_id == &self.decl_id {
            self.current = Some(child);
            self.pending_create = false;
            return true;
        }
        false
    }

    /// Reconciles a `ChildReplaced` event and binds this slot when `decl_id` matches.
    pub fn reconcile_child_replaced(&mut self, parent: NodeId, _old: NodeId, new: NodeId, decl_id: &DeclId) -> bool {
        if parent == self.parent && decl_id == &self.decl_id {
            self.current = Some(new);
            self.pending_create = false;
            return true;
        }
        false
    }

    /// Reconciles a `ChildRemoved` event and clears this slot when its current node is removed.
    pub fn reconcile_child_removed(&mut self, parent: NodeId, child: NodeId) -> bool {
        if parent == self.parent && self.current == Some(child) {
            self.current = None;
            self.pending_create = false;
            return true;
        }
        false
    }

    /// Reconciles this slot from a generic engine event.
    pub fn reconcile_event(&mut self, event: &EventKind) -> bool {
        match event {
            EventKind::ChildAdded {
                parent,
                child,
                decl_id,
            } => self.reconcile_child_added(*parent, *child, decl_id),
            EventKind::ChildReplaced {
                parent,
                old,
                new,
                decl_id,
            } => self.reconcile_child_replaced(*parent, *old, *new, decl_id),
            EventKind::ChildRemoved { parent, child } => self.reconcile_child_removed(*parent, *child),
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::EngineTime;
    use crate::process_ctx::ExecutionPhase;

    #[test]
    fn parameter_handle_set_queues_coalesced_set_param_and_updates_cache() {
        let mut handle = ParameterHandle::new(NodeId(10), 0.5f64);
        let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, EngineTime { tick: 0, micro: 0, seq: 0 });

        handle.set(&mut ctx, 0.75);

        assert_eq!(*handle.get_cached(), 0.75);
        assert_eq!(ctx.edits.pending.len(), 1);
        match &ctx.edits.pending[0].edit {
            Edit::SetParam {
                node,
                value,
                behaviour,
            } => {
                assert_eq!(*node, NodeId(10));
                assert_eq!(value, &ParamValue::Float(0.75));
                assert_eq!(*behaviour, ParameterEventBehaviour::Coalesce);
            }
            other => panic!("expected SetParam edit, got {:?}", std::mem::discriminant(other)),
        }
    }

    #[test]
    fn parameter_handle_value_change_policy_skips_unchanged_values() {
        let mut handle = ParameterHandle::with_policies(
            NodeId(11),
            1.0f64,
            ParameterChangeCheck::ValueChange,
            ParameterEventBehaviour::Coalesce,
        );
        let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, EngineTime { tick: 0, micro: 0, seq: 0 });

        handle.set(&mut ctx, 1.0);
        assert!(ctx.edits.pending.is_empty());

        handle.set_change_check(ParameterChangeCheck::None);
        handle.set(&mut ctx, 1.0);
        assert_eq!(ctx.edits.pending.len(), 1);
    }

    #[test]
    fn parameter_handle_can_update_cache_from_runtime_value() {
        let mut handle = ParameterHandle::new(NodeId(12), 0.0f64);
        let ok = handle.apply_runtime_value(&ParamValue::Int(3));
        assert!(ok);
        assert_eq!(*handle.get_cached(), 3.0);
    }

    #[test]
    fn node_handle_helpers_queue_structural_edits() {
        let handle = NodeHandle::new(NodeId(20));
        let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, EngineTime { tick: 0, micro: 0, seq: 0 });

        handle.move_to(&mut ctx, NodeId(30), Some(NodeId(31)));
        handle.remove(&mut ctx);

        assert_eq!(ctx.edits.pending.len(), 2);
        match &ctx.edits.pending[0].edit {
            Edit::MoveNode {
                node,
                new_parent,
                new_prev_sibling,
            } => {
                assert_eq!(*node, NodeId(20));
                assert_eq!(*new_parent, NodeId(30));
                assert_eq!(*new_prev_sibling, Some(NodeId(31)));
            }
            _ => panic!("expected MoveNode edit"),
        }

        match &ctx.edits.pending[1].edit {
            Edit::RemoveNode { node } => assert_eq!(*node, NodeId(20)),
            _ => panic!("expected RemoveNode edit"),
        }
    }

    #[test]
    fn potential_handle_clear_removes_bound_node() {
        let mut handle = PotentialNodeHandle::with_current(NodeId(1), "value", NodeId(2));
        let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, EngineTime { tick: 0, micro: 0, seq: 0 });

        let removed = handle.clear(&mut ctx);
        assert_eq!(removed, Some(NodeId(2)));
        assert!(!handle.is_present());
        assert_eq!(ctx.edits.pending.len(), 1);
        match &ctx.edits.pending[0].edit {
            Edit::RemoveNode { node } => assert_eq!(*node, NodeId(2)),
            _ => panic!("expected RemoveNode edit"),
        }
    }

    #[test]
    fn potential_handle_replace_without_current_adds_node_under_parent() {
        let mut handle = PotentialNodeHandle::new(NodeId(5), "value");
        let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, EngineTime { tick: 0, micro: 0, seq: 0 });

        handle.replace_with(&mut ctx, crate::node::Folder::new("slot-value"));
        assert!(!handle.is_present());
        assert!(handle.is_pending_create());
        assert_eq!(ctx.edits.pending.len(), 1);
        match &ctx.edits.pending[0].edit {
            Edit::AddNode { parent, node, .. } => {
                assert_eq!(*parent, NodeId(5));
                assert_eq!(node.node_data().meta.decl_id, DeclId("value".to_string()));
            }
            _ => panic!("expected AddNode edit"),
        }
    }

    #[test]
    fn potential_handle_replace_with_current_queues_replace() {
        let mut handle = PotentialNodeHandle::with_current(NodeId(5), "value", NodeId(6));
        let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, EngineTime { tick: 0, micro: 0, seq: 0 });

        handle.replace_with(&mut ctx, crate::node::Folder::new("slot-value"));
        assert!(handle.is_present());
        assert_eq!(handle.current_id(), Some(NodeId(6)));
        assert_eq!(ctx.edits.pending.len(), 1);
        match &ctx.edits.pending[0].edit {
            Edit::ReplaceNode { node, .. } => assert_eq!(*node, NodeId(6)),
            _ => panic!("expected ReplaceNode edit"),
        }
    }

    #[test]
    fn potential_handle_attach_existing_moves_and_binds() {
        let mut handle = PotentialNodeHandle::new(NodeId(8), "target");
        let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, EngineTime { tick: 0, micro: 0, seq: 0 });

        handle.attach_existing(&mut ctx, NodeId(42), None);
        assert_eq!(handle.current_id(), Some(NodeId(42)));
        assert_eq!(ctx.edits.pending.len(), 1);
        match &ctx.edits.pending[0].edit {
            Edit::MoveNode {
                node,
                new_parent,
                new_prev_sibling,
            } => {
                assert_eq!(*node, NodeId(42));
                assert_eq!(*new_parent, NodeId(8));
                assert_eq!(*new_prev_sibling, None);
            }
            _ => panic!("expected MoveNode edit"),
        }
    }

    #[test]
    fn potential_handle_reconcile_child_added_binds_pending_slot() {
        let mut handle = PotentialNodeHandle::new(NodeId(9), "value");
        let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, EngineTime { tick: 0, micro: 0, seq: 0 });

        handle.replace_with(&mut ctx, crate::node::Folder::new("slot-value"));
        assert!(handle.is_pending_create());
        assert_eq!(handle.current_id(), None);

        let changed = handle.reconcile_event(&EventKind::ChildAdded {
            parent: NodeId(9),
            child: NodeId(99),
            decl_id: DeclId("value".to_string()),
        });
        assert!(changed);
        assert!(!handle.is_pending_create());
        assert_eq!(handle.current_id(), Some(NodeId(99)));
    }
}
