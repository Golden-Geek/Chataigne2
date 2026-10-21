use golden_core::{
    engine::NodeExecutionRule,
    events::{CustomEvent, Event},
    node::{Node, NodeCreationContext, NodeId, NodeMetaPatch, NodeScriptDescriptor},
    parameter::ParamValue,
    process_ctx::ProcessCtx,
};

#[golden_core::node("osc_module", label = "OSC")]
pub struct GenericOscModule {
    base: crate::app::OscModuleBase,
}

impl GenericOscModule {
    pub fn create() -> Self {
        Self::new(crate::app::OscModuleBase::create())
    }

    pub(crate) fn queue_captured_osc(
        &self,
        request: crate::app::OscOutboundRequest,
    ) -> Result<(), crate::app::OscQueueError> {
        self.base.enqueue_outbound(request)
    }

    #[cfg(test)]
    pub(crate) fn auto_add_enabled_for_test(&self) -> bool {
        self.base.auto_add_enabled()
    }

    #[cfg(test)]
    pub(crate) fn has_pending_incoming_messages_for_test(&self) -> bool {
        self.base.has_pending_incoming_messages()
    }
}

#[golden_core::item(
    "module",
    node = "osc_module",
    via = base,
    from_struct,
    menu_path = ["Network"]
)]
impl Node for GenericOscModule {
    fn init(&mut self, ctx: &mut ProcessCtx) {
        self.base.init(ctx);
    }

    fn on_node_ready(&mut self, ctx: &mut ProcessCtx, context: NodeCreationContext) {
        self.base.on_node_ready(ctx, context);
    }

    fn on_worker_wake(&mut self, ctx: &mut ProcessCtx) {
        self.base.on_worker_wake(ctx);
    }

    fn destroy(&mut self, ctx: &mut ProcessCtx) {
        self.base.destroy(ctx);
    }

    fn needs_update(&self) -> bool {
        false
    }

    fn worker_wake_requires_tree_snapshot(&self) -> bool {
        self.base.worker_wake_requires_tree_snapshot()
    }

    fn execution_rule(&self) -> NodeExecutionRule {
        self.base.execution_rule()
    }

    fn child_event_interest_depth(&self, event: &Event) -> u32 {
        self.base.child_event_interest_depth(event)
    }

    fn engine_script_descriptor(&self) -> NodeScriptDescriptor {
        self.base.script_descriptor_for_node(self.node_data(), self.get_type())
    }

    fn engine_call_script_method(
        &mut self,
        ctx: &mut ProcessCtx,
        method: &str,
        args: &[ParamValue],
    ) -> Result<bool, String> {
        self.base.engine_call_script_method(ctx, method, args)
    }

    fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, old_value: ParamValue) {
        self.base.on_param_change(ctx, param, old_value);
    }

    fn on_meta_changed(&mut self, ctx: &mut ProcessCtx, node: NodeId, patch: NodeMetaPatch) {
        self.base.on_meta_changed(ctx, node, patch);
    }

    fn on_effective_enabled_changed(&mut self, ctx: &mut ProcessCtx, enabled: bool) {
        self.base.on_effective_enabled_changed(ctx, enabled);
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: CustomEvent) {
        self.base.on_custom_event(ctx, event);
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::create)
    }
}

#[cfg(test)]
mod tests;
