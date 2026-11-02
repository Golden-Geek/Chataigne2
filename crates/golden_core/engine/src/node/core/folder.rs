use std::any::Any;

use super::{EventPropagation, FOLDER_NODE_TYPE, Node, NodeData, UserContainerRules};
use crate::events::{Event, EventFrame};

mod persistence;

/// Organizational node, either open to its owner's catalog or owned by a fixed schema.
pub struct Folder {
    node_data: NodeData,
    schema_owned: bool,
}

impl Folder {
    /// Creates an ordinary folder that inherits its owning container's catalog.
    pub fn new(label: impl Into<String>) -> Self {
        let mut node_data = NodeData::new(label.into());
        node_data.meta.can_be_disabled = false;
        Self {
            node_data,
            schema_owned: false,
        }
    }

    /// Creates a fixed-schema folder that accepts no user-curated children.
    /// Params DSL declarations use this capability independently of labels or declaration ids.
    pub fn new_schema_owned(label: impl Into<String>) -> Self {
        Self {
            schema_owned: true,
            ..Self::new(label)
        }
    }
}

impl Node for Folder {
    fn user_container_rules(&self) -> Option<UserContainerRules> {
        self.schema_owned.then_some(UserContainerRules::new(&[]))
    }
    fn node_data(&self) -> &NodeData {
        &self.node_data
    }
    fn node_data_mut(&mut self) -> &mut NodeData {
        &mut self.node_data
    }
    fn get_type(&self) -> &str {
        FOLDER_NODE_TYPE
    }
    fn type_description(&self) -> Option<&str> {
        Some("Organizational folder; schema-owned folders have a closed user-item catalog.")
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == FOLDER_NODE_TYPE).then(|| Self::new("Folder"))
    }
    fn project_encode_data(&self) -> Result<serde_json::Value, String> {
        persistence::encode(self.schema_owned)
    }
    fn project_decode_data(&mut self, data: &serde_json::Value) -> Result<(), String> {
        self.schema_owned = persistence::decode(data)?;
        Ok(())
    }
    fn event_propagation(&self, _: &Event, _: u32) -> EventPropagation {
        EventPropagation::PassOn
    }
    fn inbox_requires_tree_snapshot(&self, _: &EventFrame) -> bool {
        false
    }
    fn lifecycle_requires_tree_snapshot(&self) -> bool {
        false
    }
}
