use crate::node::{NodeId, NodeMetaPatch};
use crate::parameter::{ParamValue, ParameterChangeCheck, ParameterEventBehaviour, ParameterValueType};
use crate::process_ctx::ProcessCtx;

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
