use crate::color::Color;
use crate::edit::Edit;
use crate::events::EventKind;
use crate::parameter::{
    CssUnit, CssValue, Enum, File, ParamValue, ParameterChangeCheck, ParameterEventBehaviour, Vec2, Vec3,
};
use crate::process_ctx::ProcessCtx;
use std::marker::PhantomData;
use std::path::PathBuf;

use super::{DeclId, Node, NodeId, NodeMetaPatch, NodeReference, NodeUuid};

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

impl ParameterValueType for File {
    fn to_param_value(value: Self) -> ParamValue {
        ParamValue::File(value.into_inner())
    }

    fn from_param_value(value: &ParamValue) -> Option<Self> {
        match value {
            ParamValue::File(path) | ParamValue::Str(path) => Some(File::new(path.clone())),
            _ => None,
        }
    }
}

impl ParameterValueType for PathBuf {
    fn to_param_value(value: Self) -> ParamValue {
        ParamValue::File(value.to_string_lossy().to_string())
    }

    fn from_param_value(value: &ParamValue) -> Option<Self> {
        match value {
            ParamValue::File(path) => Some(PathBuf::from(path)),
            ParamValue::Str(path) => Some(PathBuf::from(path)),
            _ => None,
        }
    }
}

impl ParameterValueType for Enum {
    fn to_param_value(value: Self) -> ParamValue {
        ParamValue::Enum(value.into_inner())
    }

    fn from_param_value(value: &ParamValue) -> Option<Self> {
        value.as_enum().map(Enum::new)
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

impl ParameterValueType for (f64, CssUnit) {
    fn to_param_value(value: Self) -> ParamValue {
        ParamValue::CssValue(CssValue::from(value))
    }

    fn from_param_value(value: &ParamValue) -> Option<Self> {
        value.as_css_value().map(Into::into)
    }
}

impl ParameterValueType for CssValue {
    fn to_param_value(value: Self) -> ParamValue {
        ParamValue::CssValue(value)
    }

    fn from_param_value(value: &ParamValue) -> Option<Self> {
        value.as_css_value()
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

impl ParameterValueType for Vec2 {
    fn to_param_value(value: Self) -> ParamValue {
        ParamValue::Vec2(value.x, value.y)
    }

    fn from_param_value(value: &ParamValue) -> Option<Self> {
        value.as_vec2().map(|(x, y)| Vec2::new(x, y))
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

impl ParameterValueType for Vec3 {
    fn to_param_value(value: Self) -> ParamValue {
        ParamValue::Vec3(value.x, value.y, value.z)
    }

    fn from_param_value(value: &ParamValue) -> Option<Self> {
        value.as_vec3().map(|(x, y, z)| Vec3::new(x, y, z))
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

impl ParameterValueType for Color {
    fn to_param_value(value: Self) -> ParamValue {
        ParamValue::Color(value.r(), value.g(), value.b(), value.a())
    }

    fn from_param_value(value: &ParamValue) -> Option<Self> {
        value.as_color().map(|(r, g, b, a)| Color::new(r, g, b, a))
    }
}

impl ParameterValueType for NodeReference {
    fn to_param_value(value: Self) -> ParamValue {
        ParamValue::Reference(value)
    }

    fn from_param_value(value: &ParamValue) -> Option<Self> {
        if let ParamValue::Reference(reference) = value {
            Some(reference.clone())
        } else {
            None
        }
    }
}

impl ParameterValueType for NodeUuid {
    fn to_param_value(value: Self) -> ParamValue {
        ParamValue::Reference(NodeReference::new(value))
    }

    fn from_param_value(value: &ParamValue) -> Option<Self> {
        if let ParamValue::Reference(reference) = value {
            Some(reference.uuid())
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
    /// Creates an unbound typed parameter handle with default policies.
    ///
    /// The runtime node id is assigned later by the engine.
    pub fn new(initial: T) -> Self {
        Self {
            node: NodeId(0),
            cached: initial,
            change_check: ParameterChangeCheck::ValueChange,
            event_behaviour: ParameterEventBehaviour::Coalesce,
        }
    }

    /// Creates an unbound typed parameter handle with explicit policies.
    ///
    /// The runtime node id is assigned later by the engine.
    pub fn with_policies(
        initial: T,
        change_check: ParameterChangeCheck,
        event_behaviour: ParameterEventBehaviour,
    ) -> Self {
        Self {
            node: NodeId(0),
            cached: initial,
            change_check,
            event_behaviour,
        }
    }

    /// Creates an unbound typed parameter handle with a `Default` cached value.
    ///
    /// This is useful when the runtime value is declared through `#[children(...)]` and the
    /// handle will be synchronized by engine preprocessing.
    pub fn unbound() -> Self
    where
        T: Default,
    {
        Self::new(T::default())
    }

    /// Returns the parameter node id.
    pub fn id(&self) -> NodeId {
        self.node
    }

    /// Returns `true` when this handle is bound to a runtime node id.
    pub fn is_bound(&self) -> bool {
        self.node.0 != 0
    }

    /// Rebinds this handle to a new runtime node id.
    pub fn set_node_id(&mut self, node: NodeId) {
        self.node = node;
    }

    /// Clears runtime node binding for this handle.
    pub fn clear_node_id(&mut self) {
        self.node = NodeId(0);
    }

    /// Returns a shared reference to the locally cached value.
    pub fn get_ref(&self) -> &T {
        &self.cached
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
        let queued_value = T::to_param_value(value.clone());
        let is_trigger = matches!(&queued_value, ParamValue::Trigger());
        let value_changed = self.cached != value;
        let should_emit = is_trigger || self.change_check == ParameterChangeCheck::None || value_changed;

        if should_emit && self.is_bound() {
            ctx.set_param_with_behaviour(self.node, queued_value, self.event_behaviour);
        }

        self.cached = value;
    }

    /// Sets or replaces the default warning on this parameter node.
    ///
    /// No-op when the handle is not bound yet.
    pub fn set_warning(&self, ctx: &mut ProcessCtx, message: impl Into<String>) {
        if !self.is_bound() {
            return;
        }

        ctx.set_node_warning(self.node, message);
    }

    /// Sets or replaces one warning on this parameter node.
    ///
    /// `warning_id = None` uses the default empty warning id.
    /// No-op when the handle is not bound yet.
    pub fn set_warning_with(
        &self,
        ctx: &mut ProcessCtx,
        warning_id: Option<&str>,
        message: impl Into<String>,
        detail: Option<&str>,
    ) {
        if !self.is_bound() {
            return;
        }

        ctx.set_node_warning_with(self.node, warning_id, message, detail);
    }

    /// Clears one warning by id on this parameter node.
    ///
    /// `warning_id = None` clears all warnings.
    /// No-op when the handle is not bound yet.
    pub fn clear_warning(&self, ctx: &mut ProcessCtx, warning_id: Option<&str>) {
        if !self.is_bound() {
            return;
        }

        ctx.clear_node_warning(self.node, warning_id);
    }

    /// Clears all warnings on this parameter node.
    ///
    /// No-op when the handle is not bound yet.
    pub fn clear_warnings(&self, ctx: &mut ProcessCtx) {
        if !self.is_bound() {
            return;
        }

        ctx.clear_all_node_warnings(self.node);
    }

    /// Sets descendant warning surfacing depth on this parameter node.
    ///
    /// No-op when the handle is not bound yet.
    pub fn set_child_warning_depth(&self, ctx: &mut ProcessCtx, max_depth: u32) {
        if !self.is_bound() {
            return;
        }

        ctx.set_node_child_warning_depth(self.node, max_depth);
    }

    /// Queues a metadata patch on this parameter node.
    ///
    /// No-op when the handle is not bound yet.
    pub fn patch_meta(&self, ctx: &mut ProcessCtx, patch: NodeMetaPatch) {
        if !self.is_bound() {
            return;
        }

        ctx.patch_node_meta(self.node, patch);
    }
}

impl<T: ParameterValueType + PartialEq + Copy> ParameterHandle<T> {
    /// Returns the locally cached value by copy.
    pub fn get(&self) -> T {
        self.cached
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

    /// Sets or replaces the default warning on this node.
    pub fn set_warning(&self, ctx: &mut ProcessCtx, message: impl Into<String>) {
        ctx.set_node_warning(self.node, message);
    }

    /// Sets or replaces one warning on this node.
    ///
    /// `warning_id = None` uses the default empty warning id.
    pub fn set_warning_with(
        &self,
        ctx: &mut ProcessCtx,
        warning_id: Option<&str>,
        message: impl Into<String>,
        detail: Option<&str>,
    ) {
        ctx.set_node_warning_with(self.node, warning_id, message, detail);
    }

    /// Clears one warning by id on this node.
    ///
    /// `warning_id = None` clears all warnings.
    pub fn clear_warning(&self, ctx: &mut ProcessCtx, warning_id: Option<&str>) {
        ctx.clear_node_warning(self.node, warning_id);
    }

    /// Clears all warnings on this node.
    pub fn clear_warnings(&self, ctx: &mut ProcessCtx) {
        ctx.clear_all_node_warnings(self.node);
    }

    /// Sets descendant warning surfacing depth on this node.
    pub fn set_child_warning_depth(&self, ctx: &mut ProcessCtx, max_depth: u32) {
        ctx.set_node_child_warning_depth(self.node, max_depth);
    }

    /// Queues a metadata patch on this node.
    pub fn patch_meta(&self, ctx: &mut ProcessCtx, patch: NodeMetaPatch) {
        ctx.patch_node_meta(self.node, patch);
    }

    /// Queues one typed mutation callback for this node.
    ///
    /// The callback executes during edit application and receives the concrete
    /// node value plus a mutable process context for queuing follow-up edits.
    pub fn with_mut<N, F>(&self, ctx: &mut ProcessCtx, callback: F)
    where
        N: Node + 'static,
        F: FnOnce(&mut N, &mut ProcessCtx) + Send + 'static,
    {
        let target = self.node;
        ctx.call_node_mutation(target, move |node, child_ctx| {
            let node_type = node.get_type().to_string();
            let Some(typed) = node.as_any_mut().downcast_mut::<N>() else {
                return Err(format!(
                    "node {:?} is type '{}' and cannot be downcast to {}",
                    target,
                    node_type,
                    std::any::type_name::<N>()
                ));
            };
            callback(typed, child_ctx);
            Ok(())
        });
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

    /// Rebinds the parent node for this slot.
    pub fn set_parent(&mut self, parent: NodeId) {
        self.parent = parent;
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

    /// Queues one typed mutation callback when this slot is currently present.
    ///
    /// Returns `true` when a callback was queued, `false` when this slot has no
    /// materialized runtime node yet.
    pub fn with_mut<N, F>(&self, ctx: &mut ProcessCtx, callback: F) -> bool
    where
        N: Node + 'static,
        F: FnOnce(&mut N, &mut ProcessCtx) + Send + 'static,
    {
        let Some(handle) = self.get() else {
            return false;
        };
        handle.with_mut::<N, F>(ctx, callback);
        true
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
            EventKind::ChildAdded { parent, child, decl_id } => self.reconcile_child_added(*parent, *child, decl_id),
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

/// Strongly typed declared child slot generated by `#[children(node ...)]`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeclaredNodeHandle<T: Node + 'static> {
    slot: PotentialNodeHandle,
    _marker: PhantomData<fn() -> T>,
}

impl<T: Node + 'static> DeclaredNodeHandle<T> {
    /// Creates an empty declared slot under `parent`.
    pub fn new(parent: NodeId, decl_id: impl Into<String>) -> Self {
        Self {
            slot: PotentialNodeHandle::new(parent, decl_id),
            _marker: PhantomData,
        }
    }

    /// Creates a declared slot with a known current runtime node id.
    pub fn with_current(parent: NodeId, decl_id: impl Into<String>, current: NodeId) -> Self {
        Self {
            slot: PotentialNodeHandle::with_current(parent, decl_id, current),
            _marker: PhantomData,
        }
    }

    /// Returns the parent node id for this slot.
    pub fn parent(&self) -> NodeId {
        self.slot.parent()
    }

    /// Rebinds the parent node for this slot.
    pub fn set_parent(&mut self, parent: NodeId) {
        self.slot.set_parent(parent);
    }

    /// Returns the declared slot id.
    pub fn decl_id(&self) -> &DeclId {
        self.slot.decl_id()
    }

    /// Returns `true` when a runtime node is currently bound to this slot.
    pub fn is_present(&self) -> bool {
        self.slot.is_present()
    }

    /// Returns `true` when this slot has queued creation and is waiting for `ChildAdded`.
    pub fn is_pending_create(&self) -> bool {
        self.slot.is_pending_create()
    }

    /// Returns the currently bound node id, when present.
    pub fn current_id(&self) -> Option<NodeId> {
        self.slot.current_id()
    }

    /// Returns a concrete node handle when this slot is present.
    pub fn get(&self) -> Option<NodeHandle> {
        self.slot.get()
    }

    /// Binds an existing node id as the current materialized slot value.
    pub fn bind_existing(&mut self, node: NodeId) {
        self.slot.bind_existing(node);
    }

    /// Detaches local knowledge of the current node id without queuing edits.
    pub fn detach_current(&mut self) -> Option<NodeId> {
        self.slot.detach_current()
    }

    /// Moves an existing node under this slot parent and marks it as current.
    pub fn attach_existing(&mut self, ctx: &mut ProcessCtx, node: NodeId, after: Option<NodeId>) {
        self.slot.attach_existing(ctx, node, after);
    }

    /// Removes the current slot node if present and clears local binding.
    pub fn clear(&mut self, ctx: &mut ProcessCtx) -> Option<NodeId> {
        self.slot.clear(ctx)
    }

    /// Queues replacement (or creation) of this slot content.
    pub fn replace_with(&mut self, ctx: &mut ProcessCtx, new_node: T) {
        self.slot.replace_with(ctx, new_node);
    }

    /// Boxed variant of [`Self::replace_with`].
    pub fn replace_with_boxed(&mut self, ctx: &mut ProcessCtx, new_node: Box<dyn Node>) {
        self.slot.replace_with_boxed(ctx, new_node);
    }

    /// Reconciles a `ChildAdded` event and binds this slot when `decl_id` matches.
    pub fn reconcile_child_added(&mut self, parent: NodeId, child: NodeId, decl_id: &DeclId) -> bool {
        self.slot.reconcile_child_added(parent, child, decl_id)
    }

    /// Reconciles a `ChildReplaced` event and binds this slot when `decl_id` matches.
    pub fn reconcile_child_replaced(&mut self, parent: NodeId, old: NodeId, new: NodeId, decl_id: &DeclId) -> bool {
        self.slot.reconcile_child_replaced(parent, old, new, decl_id)
    }

    /// Reconciles a `ChildRemoved` event and clears this slot when its current node is removed.
    pub fn reconcile_child_removed(&mut self, parent: NodeId, child: NodeId) -> bool {
        self.slot.reconcile_child_removed(parent, child)
    }

    /// Reconciles this slot from a generic engine event.
    pub fn reconcile_event(&mut self, event: &EventKind) -> bool {
        self.slot.reconcile_event(event)
    }

    /// Queues one typed mutation callback when this slot is currently present.
    ///
    /// Returns `true` when a callback was queued, `false` when this slot has no
    /// materialized runtime node yet.
    pub fn with_mut<F>(&self, ctx: &mut ProcessCtx, callback: F) -> bool
    where
        F: FnOnce(&mut T, &mut ProcessCtx) + Send + 'static,
    {
        self.slot.with_mut::<T, F>(ctx, callback)
    }
}

#[cfg(test)]
#[path = "handles/tests.rs"]
mod tests;
