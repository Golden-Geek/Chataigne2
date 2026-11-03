use std::{any::Any, sync::Arc};

use golden_core::{
    edit::NodeTree,
    events::CustomEvent,
    node::{Node, NodeData, NodeScriptDescriptor, UserContainerRules, UserCreatableItem},
    parameter::ParamValue,
    process_ctx::ProcessCtx,
    script::ScriptHostPolicy,
};
use golden_io::{RevisionGuard, RevisionSource};

use super::{protocol::DisplayInfo, scripting};

pub struct ScreenManagerNode {
    data: NodeData,
    owner: Arc<()>,
    revision: RevisionSource,
    displays: Arc<[DisplayInfo]>,
    known_displays: std::collections::HashMap<String, DisplayInfo>,
    resolved_displays: std::collections::HashMap<super::protocol::ScreenId, super::catalog_runtime::RecoveredDisplay>,
    allocation_guard: Option<RevisionGuard>,
    pub(super) catalog_inbox: super::catalog_runtime::CatalogInbox,
}

impl ScreenManagerNode {
    pub(crate) fn new() -> Self {
        let mut data = NodeData::new("Screen Manager".into());
        data.meta.decl_id = golden_core::node::DeclId("mapgyver_screen_manager".into());
        Self {
            data,
            owner: Arc::new(()),
            revision: RevisionSource::default(),
            displays: Arc::from([]),
            known_displays: Default::default(),
            resolved_displays: Default::default(),
            allocation_guard: None,
            catalog_inbox: Default::default(),
        }
    }
    pub(crate) fn guard(&self) -> RevisionGuard {
        let reader = self.revision.reader();
        reader.guard_at(reader.current())
    }
    pub fn displays(&self) -> Arc<[DisplayInfo]> {
        self.displays.clone()
    }
    /// Publishes ephemeral native metadata without writing any authored Parameter or history.
    pub fn set_displays(&mut self, displays: Arc<[DisplayInfo]>) -> bool {
        self.known_displays
            .extend(displays.iter().map(|display| (display.id.clone(), display.clone())));
        if self.displays.as_ref() == displays.as_ref() {
            return false;
        }
        self.displays = displays;
        true
    }
    pub(crate) fn mark_catalog_dirty(&mut self) {
        self.revision.advance();
        self.catalog_inbox.invalidate();
    }
    pub(crate) fn known_display(&self, identity: &str) -> Option<&DisplayInfo> {
        self.known_displays.get(identity)
    }
    pub(crate) fn has_runtime_allocation(&self) -> bool {
        self.allocation_guard.is_some()
    }
    pub(crate) fn allocation_pending(&self, displays: &[DisplayInfo]) -> bool {
        self.allocation_guard
            .as_ref()
            .is_some_and(|guard| !guard.is_current() || self.displays.as_ref() != displays)
    }
    pub(crate) fn resolved_display(
        &self,
        screen: super::protocol::ScreenId,
        intended: &super::protocol::DisplayIdentityHint,
    ) -> Option<&DisplayInfo> {
        self.resolved_displays
            .get(&screen)
            .filter(|resolved| &resolved.intended == intended)
            .map(|resolved| &resolved.display)
    }
    pub(crate) fn catalog_publisher(&self) -> Option<super::catalog_runtime::Publisher> {
        self.catalog_inbox.publisher()
    }
}

impl Node for ScreenManagerNode {
    fn node_data(&self) -> &NodeData {
        &self.data
    }
    fn node_data_mut(&mut self) -> &mut NodeData {
        &mut self.data
    }
    fn get_type(&self) -> &str {
        "mapgyver.screen.manager"
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
    fn needs_update(&self) -> bool {
        false
    }
    fn execution_rule(&self) -> golden_core::engine::NodeExecutionRule {
        golden_core::engine::NodeExecutionRule::passive()
    }
    fn inbox_requires_tree_snapshot(&self, _: &golden_core::events::EventFrame) -> bool {
        false
    }
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == "mapgyver.screen.manager").then(Self::new)
    }
    fn destroy(&mut self, _: &mut ProcessCtx) {
        self.revision.advance();
        self.owner = Arc::new(());
        self.catalog_inbox = Default::default();
        self.resolved_displays.clear();
        self.allocation_guard = None;
        self.known_displays.clear();
    }
    fn on_structure_changed(&mut self, _: &mut ProcessCtx) {
        self.mark_catalog_dirty();
    }
    fn on_node_ready(&mut self, ctx: &mut ProcessCtx, _: golden_core::node::NodeCreationContext) {
        if let Err(error) = self.catalog_inbox.register(ctx) {
            ctx.set_node_warning(self.id(), error);
        }
    }
    fn on_worker_wake(&mut self, ctx: &mut ProcessCtx) {
        if let Some(mut update) = self.catalog_inbox.take() {
            if !update.guard.is_current() {
                return;
            }
            let allocation_changed = self.allocation_guard.as_ref().is_none_or(|guard| !guard.is_current());
            self.allocation_guard = Some(update.guard.clone());
            let changed = self.set_displays(update.displays.clone())
                || self.resolved_displays != update.resolved
                || allocation_changed;
            for display in &update.profiles {
                self.known_displays
                    .entry(display.id.clone())
                    .or_insert_with(|| display.clone());
            }
            self.resolved_displays = std::mem::take(&mut update.resolved);
            if changed {
                self.revision.advance();
            }
            update.apply(ctx);
        }
    }
    fn worker_wake_when_disabled(&self) -> bool {
        true
    }
    fn on_effective_enabled_changed(&mut self, _: &mut ProcessCtx, enabled: bool) {
        self.revision.advance();
        if !enabled {
            self.owner = Arc::new(());
        }
    }
    fn script_host_policy(&self) -> Option<ScriptHostPolicy> {
        Some(ScriptHostPolicy::default_scriptable())
    }
    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&["mapgyver.screen"]))
    }
    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        vec![UserCreatableItem::new("mapgyver.screen", "mapgyver.screen", "Screen")]
    }
    fn create_user_item_tree(&self, node_type: &str) -> Option<NodeTree> {
        if node_type == "mapgyver.screen" {
            Some(
                NodeTree::new(super::ScreenNode::from_definition(
                    "Screen".into(),
                    super::protocol::ScreenDefinition::default(),
                ))
                .with_child(
                    NodeTree::new(super::SurfaceNode::from_definition(
                        "Surface 1".into(),
                        super::protocol::SurfaceDefinition::default(),
                    ))
                    .as_user_item(),
                ),
            )
        } else {
            None
        }
    }
    fn create_user_item_tree_with_context(
        &self,
        node_type: &str,
        context: &golden_core::node::UserItemCreationContext<'_>,
    ) -> Option<NodeTree> {
        if node_type == "mapgyver.screen" {
            super::creation::tree(context, "Screen".into()).ok()
        } else {
            self.create_user_item_tree(node_type)
        }
    }
    fn engine_script_descriptor(&self) -> NodeScriptDescriptor {
        scripting::manager_descriptor()
    }
    fn engine_call_custom_script_method(
        &mut self,
        ctx: &mut ProcessCtx,
        method: &str,
        args: &[ParamValue],
    ) -> Result<bool, String> {
        if matches!(method, "editScreens" | "inspectScreens") && !self.node_data().effective_enabled {
            return Err("Screen Manager is disabled".into());
        }
        scripting::manager_method(ctx, self.id(), &self.owner, method, args)
    }
    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: CustomEvent) {
        scripting::forward_result(ctx, self.id(), event);
    }
}
