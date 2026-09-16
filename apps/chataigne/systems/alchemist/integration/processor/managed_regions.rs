use chataigne_alchemist::{
    ANodeId, ANodeInstance, ANodeTypeId, AlchemistFormula, ManagedItemId,
    ManagedItemInstance, ManagedItemUiState, ManagedRegionInstance, ManagedRegionInstances,
    StableRef, SurfaceItemKind, ValueTypeId, MANAGED_AUTO_INPUT_COUNT_FIELD,
};
use golden_values::Value as RuntimeValue;
use golden_core::{node::NodeId, process_ctx::ProcessTreeSnapshot};

use crate::app::systems_alchemist_formula::{anode_from_snapshot, ANODE_NODE_TYPE};
use crate::app::systems_alchemist_managed_nodes::{
    is_output_node, mapping_output_binding_config,
};
use chataigne_state_machine::{OUTPUT_BINDINGS_FIELD, OUTPUT_TARGET_FIELD};
use chataigne_state_machine::alchemist::OUTPUT_TARGET_TYPE;

use super::processor_managed_region_decl_id;

pub(crate) fn managed_regions_from_snapshot(
    snapshot: &ProcessTreeSnapshot,
    processor_node: NodeId,
    formula: &AlchemistFormula,
) -> Option<ManagedRegionInstances> {
    let mut regions = ManagedRegionInstances::empty_for(&formula.surface);
    for definition in &formula.surface.managed_regions {
        let decl_id = processor_managed_region_decl_id(definition.id.as_str());
        let Some(region_node) = snapshot.find_child_by_decl_id(processor_node, &decl_id) else {
            continue;
        };
        let mut region = ManagedRegionInstance {
            region_id: definition.id.clone(),
            items: Vec::new(),
        };
        for child in snapshot.child_ids(region_node) {
            let child_node = snapshot.node(child)?;
            let anode = if child_node.node_type == ANODE_NODE_TYPE {
                let mut anode = anode_from_snapshot(snapshot, child).ok()?;
                if definition.kind == chataigne_alchemist::ManagedRegionKind::FilterPipeline {
                    mark_auto_input_count(snapshot, child, &mut anode);
                }
                anode
            } else if definition.accepted_roles.contains(&SurfaceItemKind::Output)
                && is_output_node(snapshot, child)
            {
                command_output_anode(snapshot, child)?
            } else {
                continue;
            };
            region.items.push(ManagedItemInstance {
                id: ManagedItemId::from_uuid(child_node.uuid.0),
                anode,
                enabled: child_node.enabled,
                ui_state: ManagedItemUiState {
                    collapsed: child_node.presentation.collapsed,
                },
            });
        }
        regions.regions.insert(definition.id.clone(), region);
    }
    Some(regions)
}

fn mark_auto_input_count(
    snapshot: &ProcessTreeSnapshot,
    anode_node: NodeId,
    anode: &mut ANodeInstance,
) {
    let auto = snapshot
        .find_child_by_decl_id(anode_node, "config")
        .and_then(|config| snapshot.find_child_by_decl_id(config, "config/num_inputs"))
        .and_then(|count| snapshot.node(count))
        .is_some_and(|count| !count.enabled);
    if auto {
        anode
            .config
            .set(MANAGED_AUTO_INPUT_COUNT_FIELD, RuntimeValue::Bool(true));
    }
}

fn command_output_anode(
    snapshot: &ProcessTreeSnapshot,
    command: NodeId,
) -> Option<ANodeInstance> {
    let command_node = snapshot.node(command)?;
    let mut anode = ANodeInstance::new(
        ANodeTypeId::new(OUTPUT_TARGET_TYPE),
        command_node.label.clone(),
    );
    anode.id = ANodeId::from_uuid(command_node.uuid.0);
    anode.enabled = command_node.enabled;
    anode.config.set(
        OUTPUT_TARGET_FIELD,
        RuntimeValue::Ref(StableRef::new(
            ValueTypeId::new(command_node.node_type.clone()),
            command_node.uuid.0.to_string(),
        )),
    );
    anode.config.set(
        OUTPUT_BINDINGS_FIELD,
        mapping_output_binding_config(snapshot, command)
            .ok()?
            .to_runtime_value()
            .ok()?,
    );
    Some(anode)
}
