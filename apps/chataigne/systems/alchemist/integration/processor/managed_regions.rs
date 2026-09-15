use chataigne_alchemist::{
    AlchemistFormula, ManagedItemId, ManagedItemInstance, ManagedItemUiState, ManagedRegionInstance,
    ManagedRegionInstances,
};
use golden_core::{node::NodeId, process_ctx::ProcessTreeSnapshot};

use crate::app::systems_alchemist_formula::{anode_from_snapshot, ANODE_NODE_TYPE};

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
            if child_node.node_type != ANODE_NODE_TYPE {
                continue;
            }
            let anode = anode_from_snapshot(snapshot, child).ok()?;
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
