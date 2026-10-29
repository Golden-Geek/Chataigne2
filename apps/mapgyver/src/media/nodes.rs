use std::any::Any;

use golden_core::node::{Node, NodeData};
use serde_json::Value;

use super::protocol::{ChannelDefinition, MediaKind};

pub struct MediaNode {
    data: NodeData,
    pub(crate) kind: MediaKind,
}

impl MediaNode {
    pub(crate) fn new(label: String, kind: MediaKind) -> Self {
        Self {
            data: NodeData::new(label),
            kind,
        }
    }
}

impl Node for MediaNode {
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
    pub(crate) definition: ChannelDefinition,
}

impl ChannelNode {
    pub(crate) fn new(label: String, definition: ChannelDefinition) -> Self {
        Self {
            data: NodeData::new(label),
            definition,
        }
    }
}

impl Node for ChannelNode {
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
            self.definition = ChannelDefinition::default();
            return Ok(());
        }
        let definition: ChannelDefinition = serde_json::from_value(data.clone()).map_err(|e| e.to_string())?;
        definition.validate()?;
        self.definition = definition;
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
