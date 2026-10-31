use super::protocol::{InstanceDefinition, InstanceId};
use golden_core::node::{Node, NodeData};
use golden_core::process_ctx::ProcessCtx;
use golden_io::RevisionSource;
use serde_json::Value;
use std::any::Any;

pub const INSTANCE_TYPE: &str = "mapgyver.composition.instance";

pub struct InstanceNode {
    data: NodeData,
    definition: InstanceDefinition,
    revision: RevisionSource,
}

impl InstanceNode {
    pub(crate) fn new(label: String, definition: InstanceDefinition) -> Self {
        Self {
            data: NodeData::new(label),
            definition,
            revision: RevisionSource::default(),
        }
    }
    pub(crate) fn id(&self) -> InstanceId {
        InstanceId(self.data.meta.uuid)
    }
    pub(crate) fn definition(&self) -> &InstanceDefinition {
        &self.definition
    }
    pub(crate) fn set_definition(&mut self, definition: InstanceDefinition) {
        if self.definition != definition {
            self.revision.advance();
            self.definition = definition;
        }
    }
}

impl Node for InstanceNode {
    fn needs_update(&self) -> bool {
        false
    }
    fn execution_rule(&self) -> golden_core::engine::NodeExecutionRule {
        golden_core::engine::NodeExecutionRule::passive()
    }
    fn node_data(&self) -> &NodeData {
        &self.data
    }
    fn node_data_mut(&mut self) -> &mut NodeData {
        &mut self.data
    }
    fn get_type(&self) -> &str {
        INSTANCE_TYPE
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
    fn destroy(&mut self, _: &mut ProcessCtx) {
        self.revision.advance();
    }
    fn on_effective_enabled_changed(&mut self, _: &mut ProcessCtx, _: bool) {
        self.revision.advance();
    }
    fn project_encode_data(&self) -> Result<Value, String> {
        serde_json::to_value(&self.definition).map_err(|e| e.to_string())
    }
    fn project_decode_data(&mut self, data: &Value) -> Result<(), String> {
        let definition: InstanceDefinition = if data.is_null() {
            InstanceDefinition::default()
        } else {
            serde_json::from_value(data.clone()).map_err(|e| e.to_string())?
        };
        definition.validate()?;
        self.set_definition(definition);
        Ok(())
    }
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == INSTANCE_TYPE).then(|| Self::new("Instance".into(), InstanceDefinition::default()))
    }
    fn engine_validate_project_candidate(&self) -> Result<(), String> {
        self.definition.validate()
    }
}
