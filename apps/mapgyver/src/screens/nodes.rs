use std::{any::Any, sync::Arc};

use golden_core::{
    events::CustomEvent,
    node::{Node, NodeData, NodeScriptDescriptor, UserContainerRules, UserCreatableItem},
    parameter::ParamValue,
    process_ctx::ProcessCtx,
    script::{ScriptHostPolicy, ScriptNode},
};
use golden_io::{RevisionGuard, RevisionSource};
use serde_json::Value;

use super::{
    protocol::{ScreenDefinition, SurfaceDefinition},
    scripting,
};

pub struct ScreenManagerNode {
    data: NodeData,
    owner: Arc<()>,
}

impl ScreenManagerNode {
    pub(crate) fn new() -> Self {
        let mut data = NodeData::new("Screen Manager".into());
        data.meta.decl_id = golden_core::node::DeclId("mapgyver_screen_manager".into());
        Self {
            data,
            owner: Arc::new(()),
        }
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
        self.owner = Arc::new(());
    }
    fn on_effective_enabled_changed(&mut self, _: &mut ProcessCtx, enabled: bool) {
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
            "script",
        ]))
    }
    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        let mut items = super::commands::catalog();
        items.push(UserCreatableItem::new("script", "script", "Screen Script"));
        items
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

pub struct ScreenNode {
    data: NodeData,
    definition: ScreenDefinition,
    revision: RevisionSource,
}

pub struct SurfaceNode {
    data: NodeData,
    definition: SurfaceDefinition,
    revision: RevisionSource,
}

// These two nodes differ only in their typed authored definition and persistent type tag.
macro_rules! authored_node {
    ($node:ident, $definition:ty, $kind:literal, $label:literal) => {
        impl $node {
            pub(crate) fn new(label: String, definition: $definition) -> Self {
                Self {
                    data: NodeData::new(label),
                    definition,
                    revision: RevisionSource::default(),
                }
            }
            pub fn definition(&self) -> &$definition {
                &self.definition
            }
            pub(crate) fn set_definition(&mut self, definition: $definition) {
                if self.definition != definition {
                    self.revision.advance();
                    self.definition = definition;
                }
            }
            pub fn guard(&self) -> RevisionGuard {
                let reader = self.revision.reader();
                reader.guard_at(reader.current())
            }
        }
        impl Node for $node {
            fn node_data(&self) -> &NodeData {
                &self.data
            }
            fn node_data_mut(&mut self) -> &mut NodeData {
                &mut self.data
            }
            fn get_type(&self) -> &str {
                $kind
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
            fn destroy(&mut self, _: &mut ProcessCtx) {
                self.revision.advance();
            }
            fn on_structure_changed(&mut self, _: &mut ProcessCtx) {
                self.revision.advance();
            }
            fn on_effective_enabled_changed(&mut self, _: &mut ProcessCtx, _: bool) {
                self.revision.advance();
            }
            fn project_encode_data(&self) -> Result<Value, String> {
                serde_json::to_value(&self.definition).map_err(|error| error.to_string())
            }
            fn project_decode_data(&mut self, data: &Value) -> Result<(), String> {
                let definition: $definition = if data.is_null() {
                    Default::default()
                } else {
                    serde_json::from_value(data.clone()).map_err(|error| error.to_string())?
                };
                definition.validate()?;
                self.set_definition(definition);
                Ok(())
            }
            fn project_create(node_type: &str) -> Option<Self> {
                (node_type == $kind).then(|| Self::new($label.into(), Default::default()))
            }
            fn engine_validate_project_candidate(&self) -> Result<(), String> {
                self.definition.validate()
            }
        }
    };
}

authored_node!(ScreenNode, ScreenDefinition, "mapgyver.screen", "Screen");
authored_node!(SurfaceNode, SurfaceDefinition, "mapgyver.surface", "Surface");
