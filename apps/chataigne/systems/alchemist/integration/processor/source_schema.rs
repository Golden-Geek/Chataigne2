//! Resolve managed source types from Golden parameter declarations, never value samples.

use chataigne_alchemist::{ChannelMetadata, StableRef, ValueTypeId};
use chataigne_state_machine::ChannelSourceSchema;
use golden_core::{
    node::{NodeId, NodeUuid},
    parameter::RangeConstraint,
    process_ctx::ProcessTreeSnapshot,
};

pub(crate) fn managed_source_node(snapshot: &ProcessTreeSnapshot, source: &StableRef) -> Option<NodeId> {
    let uuid = source.stable_id.parse::<uuid::Uuid>().ok().map(NodeUuid)?;
    let node_id = snapshot.node_id_by_uuid(uuid)?;
    snapshot.node(node_id)?.is_parameter().then_some(node_id)
}

pub(crate) fn managed_source_schema(snapshot: &ProcessTreeSnapshot, source: &StableRef) -> Option<ChannelSourceSchema> {
    let node = snapshot.node(managed_source_node(snapshot, source)?)?;
    let value_type = match node.node_type.as_str() {
        "bool" => "bool",
        "int" => "int",
        "float" => "float",
        "str" | "string" | "enum" | "file" => "string",
        "vec2" => "vec2",
        "vec3" => "vec3",
        "color" => "color",
        "trigger" => "trigger",
        _ => return None,
    };
    let mut metadata = ChannelMetadata::default();
    if let Some(RangeConstraint::Uniform { min, max }) = node
        .param_constraints
        .as_ref()
        .and_then(|constraints| constraints.range.as_ref())
    {
        metadata.minimum = *min;
        metadata.maximum = *max;
    }
    Some(ChannelSourceSchema {
        value_type: ValueTypeId::new(value_type),
        metadata,
    })
}
