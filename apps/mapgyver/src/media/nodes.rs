use std::any::Any;

use golden_core::node::{Node, NodeData};
use golden_core::process_ctx::ProcessCtx;
use golden_io::{RevisionGuard, RevisionSource};
use serde_json::Value;

use super::protocol::{ChannelDefinition, MediaKind};

pub struct MediaManagerNode {
    data: NodeData,
    pub(crate) instance: uuid::Uuid,
    project: RevisionSource,
    membership: RevisionSource,
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
    fn destroy(&mut self, _: &mut ProcessCtx) {
        self.project.advance();
        self.membership.advance();
        self.instance = uuid::Uuid::new_v4();
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
