//! Processor lifecycle and Formula hierarchy notifications.

use super::*;

#[node(
    "state_processor",
    from_struct,
    contextualizable = golden_core::node::UserContextHostPolicy::multiplex_contextualizable()
)]
impl Node for StateProcessor {
    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&[USER_CONTEXT_ITEM_KIND]))
    }

    fn user_container_accepts_item(&self, item_type: &str, item_kind: &str) -> bool {
        item_kind == USER_CONTEXT_ITEM_KIND && item_type == USER_CONTEXT_NODE_TYPE
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        vec![processor_context_creatable_item(false)]
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        create_processor_context_item(node_type)
    }

    fn init(&mut self, ctx: &mut ProcessCtx) {
        initialize_processor_item(self);
        self.sync_formula_source_key();
        self.reconcile_formula(ctx);
    }

    fn on_node_ready(&mut self, ctx: &mut ProcessCtx, _context: NodeCreationContext) {
        self.refresh_formula_subscription(ctx);
        self.refresh_condition_valid_params(ctx);
        self.reconcile_formula(ctx);
    }

    fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, old_value: ParamValue) {
        if self.compression_enabled.get() && (param == self.formula.id() || param == self.convert_to_formula.id()) {
            ctx.set_param(param, old_value);
            ctx.set_param(
                self.compression_error.id(),
                ParamValue::Str("Mapping workflow is compressed; expand it before editing the Formula".to_owned()),
            );
            return;
        }
        if param == self.convert_to_formula.id() {
            self.convert_mapping_to_formula(ctx);
            return;
        }
        if param == self.compress.id() {
            self.compression_request = Some(true);
            return;
        }
        if param == self.expand.id() {
            self.compression_request = Some(false);
            return;
        }
        if self.condition_valid_params.contains(&param) {
            return;
        }
        if param == self.formula.id() {
            self.sync_formula_source_from_reference();
            self.refresh_formula_subscription(ctx);
        }
        self.reconcile_formula(ctx);
    }

    fn on_node_created(&mut self, ctx: &mut ProcessCtx, _node: NodeId) {
        self.refresh_condition_valid_params(ctx);
        self.reconcile_formula(ctx);
    }

    fn on_node_deleted(&mut self, ctx: &mut ProcessCtx, _node: NodeId) {
        self.refresh_condition_valid_params(ctx);
        self.refresh_formula_subscription(ctx);
        self.reconcile_formula(ctx);
    }

    fn on_child_added(&mut self, ctx: &mut ProcessCtx, _parent: NodeId, _child: NodeId) {
        self.refresh_condition_valid_params(ctx);
        self.reconcile_formula(ctx);
    }

    fn on_child_removed(&mut self, ctx: &mut ProcessCtx, _parent: NodeId, _child: NodeId) {
        self.refresh_condition_valid_params(ctx);
        self.reconcile_formula(ctx);
    }

    fn on_child_moved(&mut self, ctx: &mut ProcessCtx, _child: NodeId, _old_parent: NodeId, _new_parent: NodeId) {
        self.refresh_condition_valid_params(ctx);
        self.reconcile_formula(ctx);
    }

    fn on_child_reordered(&mut self, ctx: &mut ProcessCtx, _parent: NodeId, _child: NodeId) {
        self.reconcile_formula(ctx);
    }

    fn on_meta_changed(&mut self, ctx: &mut ProcessCtx, _node: NodeId, _patch: NodeMetaPatch) {
        self.reconcile_formula(ctx);
    }

    fn child_event_interest_depth(&self, _event: &Event) -> u32 {
        3
    }

    fn inbox_requires_tree_snapshot(&self, events: &EventFrame) -> bool {
        events.iter().any(|event| match &event.kind {
            EventKind::Custom(_) => false,
            EventKind::ParamChanged { param, .. } => !self.condition_valid_params.contains(param),
            _ => true,
        })
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}
