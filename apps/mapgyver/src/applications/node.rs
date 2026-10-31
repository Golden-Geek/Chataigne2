use super::protocol::*;
use golden_core::{
    node::{Node, NodeData},
    process_ctx::ProcessCtx,
};
use golden_io::{RevisionGuard, RevisionSource};
use serde_json::Value;
use std::any::Any;

pub(crate) const NODE_TYPE: &str = "mapgyver.application";
pub struct ApplicationNode {
    data: NodeData,
    definition: ApplicationDefinition,
    revision: RevisionSource,
}
impl ApplicationNode {
    pub(crate) fn availability(&self) -> super::demand::DefinitionAvailability {
        use super::demand::DefinitionAvailability;
        if !self.data.effective_enabled {
            DefinitionAvailability::Disabled
        } else if self.definition.executable.is_none() {
            DefinitionAvailability::Unconfigured
        } else {
            DefinitionAvailability::Configured
        }
    }
    pub(super) fn new(label: String, definition: ApplicationDefinition) -> Self {
        Self {
            data: NodeData::new(label),
            definition,
            revision: Default::default(),
        }
    }
    pub fn snapshot(&self) -> ApplicationSnapshot {
        ApplicationSnapshot {
            id: ApplicationId(self.data.meta.uuid),
            label: self.data.meta.label.clone(),
            enabled: self.data.effective_enabled,
            revision: self.revision.reader().current(),
            definition: self.definition.clone(),
        }
    }
    pub fn guard(&self) -> RevisionGuard {
        let reader = self.revision.reader();
        reader.guard_at(reader.current())
    }
    pub(super) fn definition(&self) -> &ApplicationDefinition {
        &self.definition
    }
    pub(super) fn set_definition(&mut self, definition: ApplicationDefinition) {
        if self.definition != definition {
            self.definition = definition;
            self.revision.advance();
        }
    }
}
impl Node for ApplicationNode {
    fn node_data(&self) -> &NodeData {
        &self.data
    }
    fn node_data_mut(&mut self) -> &mut NodeData {
        &mut self.data
    }
    fn get_type(&self) -> &str {
        NODE_TYPE
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
    fn on_effective_enabled_changed(&mut self, _: &mut ProcessCtx, _: bool) {
        self.revision.advance();
    }
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == NODE_TYPE).then(|| Self::new("Application".into(), Default::default()))
    }
    fn project_encode_data(&self) -> Result<Value, String> {
        serde_json::to_value(&self.definition).map_err(|e| e.to_string())
    }
    fn project_decode_data(&mut self, value: &Value) -> Result<(), String> {
        let definition: ApplicationDefinition = serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
        definition.validate()?;
        self.set_definition(definition);
        Ok(())
    }
}
