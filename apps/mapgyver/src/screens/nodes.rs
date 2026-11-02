use std::{any::Any, sync::Arc};

use golden_core::{
    edit::NodeTree,
    events::CustomEvent,
    node::{Node, NodeData, NodeScriptDescriptor, UserContainerRules, UserCreatableItem},
    parameter::ParamValue,
    process_ctx::ProcessCtx,
    script::{ScriptHostPolicy, ScriptNode},
};
use golden_io::{RevisionGuard, RevisionSource};

use super::scripting;

pub struct ScreenManagerNode {
    data: NodeData,
    owner: Arc<()>,
    revision: RevisionSource,
}

impl ScreenManagerNode {
    pub(crate) fn new() -> Self {
        let mut data = NodeData::new("Screen Manager".into());
        data.meta.decl_id = golden_core::node::DeclId("mapgyver_screen_manager".into());
        Self {
            data,
            owner: Arc::new(()),
            revision: RevisionSource::default(),
        }
    }
    pub(crate) fn guard(&self) -> RevisionGuard {
        let reader = self.revision.reader();
        reader.guard_at(reader.current())
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
    }
    fn on_structure_changed(&mut self, _: &mut ProcessCtx) {
        self.revision.advance();
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
        Some(UserContainerRules::new(&[
            super::commands::SCREEN_COMMAND_TYPE,
            "mapgyver.screen",
            "script",
        ]))
    }
    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        let mut items = vec![UserCreatableItem::new("mapgyver.screen", "mapgyver.screen", "Screen")];
        items.extend(super::commands::catalog());
        items.push(UserCreatableItem::new("script", "script", "Screen Script"));
        items
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
            self.create_user_item(node_type).map(NodeTree::boxed)
        }
    }
    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        match node_type {
            super::commands::SCREEN_COMMAND_TYPE => Some(Box::new(super::ScreenCommandNode::create())),
            "script" => Some(Box::new(ScriptNode::new("Screen Script", scripting::template()))),
            _ => None,
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
