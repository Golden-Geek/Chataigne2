//! Bind managed sources to live engine parameter values at processor rebuild time.

use std::collections::{HashMap, HashSet};

use chataigne_alchemist::{ChannelProvenance, RuntimeInputSnapshot, StableRef};
use chataigne_state_machine::{Processor, ProcessorContextProvider, ProcessorId, ProcessorRuntime};
use golden_core::{node::NodeId, parameter::ParamValue, process_ctx::ProcessTreeSnapshot};
use golden_values::Value as RuntimeValue;

use crate::app::systems_alchemist_processor::managed_source_node;
pub(super) use crate::app::systems_alchemist_processor::managed_source_schema;
use super::{LaneParamResolver, SnapshotProcessorContextProvider, context_control_multiplex_axes};

pub(super) fn managed_source_bindings(
    snapshot: &ProcessTreeSnapshot,
    processor: &Processor,
    runtime: &ProcessorRuntime,
) -> Vec<(StableRef, NodeId)> {
    let mut references = Vec::new();
    if let Some(layout) = runtime
        .managed_formula
        .as_ref()
        .and_then(|managed| managed.input_layout())
    {
        references.extend(
            layout
                .channels()
                .iter()
                .filter_map(|channel| match &channel.provenance {
                    ChannelProvenance::Input(source) => Some(source.clone()),
                    _ => None,
                }),
        );
    }
    for region in processor.formula_instance.managed_regions.regions.values() {
        for item in &region.items {
            references.extend(item.anode.input_defaults.values().filter_map(|value| match value {
                RuntimeValue::Ref(source) => Some(source.clone()),
                _ => None,
            }));
        }
    }
    let mut seen = HashSet::new();
    references
        .into_iter()
        .filter_map(|source| {
            if !seen.insert(source.clone()) {
                return None;
            }
            managed_source_node(snapshot, &source).map(|node| (source, node))
        })
        .collect()
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
        if let Some(value) = live_param_values
            .get(source)
            .or(node.param_value.as_ref())
            .and_then(super::param_to_runtime_value)
        {
            inputs.insert(reference.clone(), value);
        }
    }
}

pub(super) fn insert_managed_source_values_with_context(
    snapshot: &ProcessTreeSnapshot,
    live_param_values: &HashMap<NodeId, ParamValue>,
    bindings: &[(StableRef, NodeId)],
    processor_id: ProcessorId,
    provider: &SnapshotProcessorContextProvider,
    inputs: &mut RuntimeInputSnapshot,
) {
    insert_managed_source_values(snapshot, live_param_values, bindings, inputs);
    for (reference, source) in bindings {
        if snapshot.node(*source).is_none_or(|node| !node.enabled) {
            continue;
        }
        let axes = context_control_multiplex_axes(snapshot, *source, processor_id, provider);
        if axes.is_empty() {
            continue;
        }
        for context_key in provider.iter_context_keys(processor_id, &axes) {
            let resolver = LaneParamResolver { processor_id, context_key: &context_key, context_provider: provider };
            if let Some(value) = resolver.param_value_with_live(snapshot, live_param_values, *source)
                .as_ref().and_then(super::param_to_runtime_value)
            {
                inputs.insert_context(reference.clone(), &axes, context_key, value);
            }
        }
    }
}
