//! A Reference output owns identity, not another source or unused source Parameters.
use golden_core::{node, node::Node};

pub const OUTPUT_TYPE: &str = "mapgyver.media.reference.output";

#[node("mapgyver.media.reference.output", label = "Output")]
pub struct ReferenceOutputNode {}

#[node("mapgyver.media.reference.output", from_struct)]
impl Node for ReferenceOutputNode {
    fn needs_update(&self) -> bool {
        false
    }
    fn execution_rule(&self) -> golden_core::engine::NodeExecutionRule {
        golden_core::engine::NodeExecutionRule::passive()
    }
    fn inbox_requires_tree_snapshot(&self, _: &golden_core::events::EventFrame) -> bool {
        false
    }
    fn lifecycle_requires_tree_snapshot(&self) -> bool {
        false
    }
    fn project_preserve_identity(&self) -> bool {
        true
    }
    fn project_decode_data(&mut self, data: &serde_json::Value) -> Result<(), String> {
        crate::parameters::reject_opaque(data)
    }
}
