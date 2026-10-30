use std::{any::Any, sync::Arc};

use golden_core::events::CustomEvent;
use golden_core::node::{Node, NodeData, NodeScriptDescriptor, UserContainerRules, UserCreatableItem};
use golden_core::parameter::ParamValue;
use golden_core::process_ctx::ProcessCtx;
use golden_core::script::{ScriptHostPolicy, ScriptNode};
use golden_io::{RevisionGuard, RevisionSource};
use serde_json::Value;

use super::protocol::{ChannelDefinition, MediaKind};

pub struct MediaManagerNode {
    data: NodeData,
    pub(crate) instance: uuid::Uuid,
    project: RevisionSource,
    membership: RevisionSource,
    operation_owner: Arc<()>,
}

impl MediaManagerNode {
    pub(crate) fn new() -> Self {
        let mut data = NodeData::new("Media Manager".into());
        data.meta.decl_id = golden_core::node::DeclId("mapgyver_media_manager".into());
        Self {
            data,
            instance: uuid::Uuid::new_v4(),
            project: RevisionSource::default(),
            membership: RevisionSource::default(),
            operation_owner: Arc::new(()),
        }
    }
    pub(crate) fn project_guard(&self) -> RevisionGuard {
        let reader = self.project.reader();
        reader.guard_at(reader.current())
    }
    pub(crate) fn membership_guard(&self) -> RevisionGuard {
        let reader = self.membership.reader();
        reader.guard_at(reader.current())
    }
}

impl Node for MediaManagerNode {
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
        self.project.advance();
        self.membership.advance();
        self.instance = uuid::Uuid::new_v4();
        self.operation_owner = Arc::new(());
    }
    fn node_data(&self) -> &NodeData {
        &self.data
    }
    fn node_data_mut(&mut self) -> &mut NodeData {
        &mut self.data
    }
    fn get_type(&self) -> &str {
        "mapgyver.media.manager"
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == "mapgyver.media.manager").then(Self::new)
    }
    fn on_structure_changed(&mut self, _: &mut ProcessCtx) {
        self.membership.advance();
    }

    fn script_host_policy(&self) -> Option<ScriptHostPolicy> {
        Some(ScriptHostPolicy::default_scriptable())
    }

    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&[
            super::commands::MEDIA_COMMAND_TYPE,
            "script",
        ]))
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        let mut items = super::commands::catalog();
        items.push(UserCreatableItem::new("script", "script", "Media Script"));
        items
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        match node_type {
            super::commands::MEDIA_COMMAND_TYPE => Some(Box::new(super::commands::MediaCommandNode::create())),
            "script" => Some(Box::new(ScriptNode::new("Media Script", super::scripting::template()))),
            _ => None,
        }
    }

    fn engine_script_descriptor(&self) -> NodeScriptDescriptor {
        super::scripting::manager_descriptor()
    }

    fn engine_call_custom_script_method(
        &mut self,
        ctx: &mut ProcessCtx,
        method: &str,
        args: &[ParamValue],
    ) -> Result<bool, String> {
        super::scripting::manager_method(ctx, self.id(), &self.operation_owner, method, args)
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: CustomEvent) {
        super::scripting::forward_result(ctx, self.id(), event);
    }

    fn on_effective_enabled_changed(&mut self, _: &mut ProcessCtx, enabled: bool) {
        if !enabled {
            self.operation_owner = Arc::new(());
        }
    }
}

pub struct MediaNode {
    data: NodeData,
    pub(crate) kind: MediaKind,
    membership: RevisionSource,
}

impl MediaNode {
    pub(crate) fn new(label: String, kind: MediaKind) -> Self {
        Self {
            data: NodeData::new(label),
            kind,
            membership: RevisionSource::default(),
        }
    }
    pub(crate) fn guard(&self) -> RevisionGuard {
        let reader = self.membership.reader();
        reader.guard_at(reader.current())
    }
}

impl Node for MediaNode {
    fn destroy(&mut self, _: &mut ProcessCtx) {
        self.membership.advance();
    }
    fn on_structure_changed(&mut self, _: &mut ProcessCtx) {
        self.membership.advance();
    }
    fn node_data(&self) -> &NodeData {
        &self.data
    }
    fn node_data_mut(&mut self) -> &mut NodeData {
        &mut self.data
    }
    fn get_type(&self) -> &str {
        "mapgyver.media"
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
    fn project_encode_data(&self) -> Result<Value, String> {
        serde_json::to_value(self.kind).map_err(|e| e.to_string())
    }
    fn project_decode_data(&mut self, data: &Value) -> Result<(), String> {
        if data.is_null() {
            return Ok(());
        }
        self.kind = serde_json::from_value(data.clone()).map_err(|e| e.to_string())?;
        Ok(())
    }
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == "mapgyver.media").then(|| Self::new("Media".into(), MediaKind::SolidColor))
    }
}

pub struct ChannelNode {
    data: NodeData,
    definition: ChannelDefinition,
    revision: RevisionSource,
}

impl ChannelNode {
    pub(crate) fn new(label: String, definition: ChannelDefinition) -> Self {
        Self {
            data: NodeData::new(label),
            definition,
            revision: RevisionSource::default(),
        }
    }
    pub(crate) fn definition(&self) -> &ChannelDefinition {
        &self.definition
    }
    pub(crate) fn set_definition(&mut self, definition: ChannelDefinition) {
        if self.definition != definition {
            self.revision.advance();
            self.definition = definition;
        }
    }
    pub(crate) fn guard(&self) -> RevisionGuard {
        let reader = self.revision.reader();
        reader.guard_at(reader.current())
    }
}

impl Node for ChannelNode {
    fn destroy(&mut self, _: &mut ProcessCtx) {
        self.revision.advance();
    }
    fn on_effective_enabled_changed(&mut self, _: &mut ProcessCtx, _: bool) {
        self.revision.advance();
    }
    fn node_data(&self) -> &NodeData {
        &self.data
    }
    fn node_data_mut(&mut self) -> &mut NodeData {
        &mut self.data
    }
    fn get_type(&self) -> &str {
        "mapgyver.media.channel"
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
    fn project_encode_data(&self) -> Result<Value, String> {
        serde_json::to_value(&self.definition).map_err(|e| e.to_string())
    }
    fn project_decode_data(&mut self, data: &Value) -> Result<(), String> {
        // Golden's sparse codec omits default-valued fields. The authored contract
        // supplies those defaults; history capture still encodes every field.
        if data.is_null() {
            self.set_definition(ChannelDefinition::default());
            return Ok(());
        }
        let definition: ChannelDefinition = serde_json::from_value(data.clone()).map_err(|e| e.to_string())?;
        definition.validate()?;
        self.set_definition(definition);
        Ok(())
    }
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == "mapgyver.media.channel")
            .then(|| Self::new("Channel".into(), ChannelDefinition::for_media(MediaKind::SolidColor)))
    }
    fn engine_validate_project_candidate(&self) -> Result<(), String> {
        self.definition.validate()
    }
}
