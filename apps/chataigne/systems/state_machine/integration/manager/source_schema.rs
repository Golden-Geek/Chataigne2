//! Resolve managed input types from authored Golden parameter declarations at rebuild time.

use std::collections::{HashMap, HashSet};

use chataigne_alchemist::{ChannelMetadata, ChannelProvenance, RuntimeInputSnapshot, StableRef, ValueTypeId};
use chataigne_state_machine::{ChannelSourceSchema, Processor, ProcessorRuntime};
use golden_core::{node::{NodeId, NodeUuid}, parameter::{ParamValue, RangeConstraint}, process_ctx::ProcessTreeSnapshot};
use golden_values::Value as RuntimeValue;

pub(super) fn managed_source_node(snapshot: &ProcessTreeSnapshot, source: &StableRef) -> Option<NodeId> {
    let uuid = source.stable_id.parse::<uuid::Uuid>().ok().map(NodeUuid)?;
    let node_id = snapshot.node_id_by_uuid(uuid)?;
    snapshot.node(node_id)?.is_parameter().then_some(node_id)
}

pub(super) fn managed_source_schema(
    snapshot: &ProcessTreeSnapshot,
    source: &StableRef,
) -> Option<ChannelSourceSchema> {
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
    Some(ChannelSourceSchema { value_type: ValueTypeId::new(value_type), metadata })
}

pub(super) fn managed_source_bindings(
    snapshot: &ProcessTreeSnapshot,
    processor: &Processor,
    runtime: &ProcessorRuntime,
) -> Vec<(StableRef, NodeId)> {
    let mut references = Vec::new();
    if let Some(layout) = runtime.managed_formula.as_ref().and_then(|managed| managed.input_layout()) {
        references.extend(layout.channels().iter().filter_map(|channel| {
            match &channel.provenance {
                ChannelProvenance::Input(source) => Some(source.clone()),
                _ => None,
            }
        }));
    }
    for region in processor.formula_instance.managed_regions.regions.values() {
        for item in &region.items {
            references.extend(item.anode.input_defaults.values().filter_map(|value| {
                match value {
                    RuntimeValue::Ref(source) => Some(source.clone()),
                    _ => None,
                }
            }));
        }
    }
    let mut seen = HashSet::new();
    references.into_iter().filter_map(|source| {
        if !seen.insert(source.clone()) {
            return None;
        }
        managed_source_node(snapshot, &source).map(|node| (source, node))
    }).collect()
}

pub(super) fn insert_managed_source_values(
    snapshot: &ProcessTreeSnapshot,
    live_param_values: &HashMap<NodeId, ParamValue>,
    bindings: &[(StableRef, NodeId)],
    inputs: &mut RuntimeInputSnapshot,
) {
    for (reference, source) in bindings {
        let Some(node) = snapshot.node(*source).filter(|node| node.enabled) else {
            continue;
        };
        if let Some(value) = live_param_values.get(source).or(node.param_value.as_ref()).and_then(super::param_to_runtime_value) {
            inputs.insert(reference.clone(), value);
        }
    }
}
