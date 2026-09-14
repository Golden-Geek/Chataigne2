use super::*;

use chataigne_alchemist::ManagedRegionDefinition;
use golden_core::edit::{Edit, NodeTree};

use crate::app::systems_alchemist_formula::FORMULA_MANAGED_REGIONS_JSON_DECL_ID;

#[test]
fn processor_factory_tracks_live_formula_region_metadata() {
    let (mut engine, mapping_uuid, first_processor) = mapping_engine();
    let snapshot = engine.process_tree_snapshot();
    let mapping = snapshot.node_id_by_uuid(mapping_uuid).unwrap();
    let manager = snapshot.node(first_processor).unwrap().parent.unwrap();
    let metadata = snapshot
        .find_child_by_decl_id(mapping, FORMULA_MANAGED_REGIONS_JSON_DECL_ID)
        .unwrap();
    let Some(ParamValue::Str(raw)) = snapshot.node(metadata).unwrap().param_value.as_ref() else {
        panic!("Mapping should expose managed-region metadata");
    };
    let mut regions: Vec<ManagedRegionDefinition> = serde_json::from_str(raw).unwrap();
    let region_id = processor_managed_region_decl_id(regions[0].id.as_str());
    regions[0].label = "Updated Mapping Inputs".to_owned();

    engine.edits.push(Edit::SetParam {
        node: metadata,
        value: ParamValue::Str(serde_json::to_string(&regions).unwrap()),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().unwrap();
    for _ in 0..2 {
        engine.run_tick(Duration::from_millis(8)).unwrap();
    }

    engine.add_user_item_tree(NodeTree::new(crate::app::StateProcessorFolder::new()), Some(manager));
    engine.apply_edits().unwrap();
    let snapshot = engine.process_tree_snapshot();
    let folder = snapshot
        .child_ids(manager)
        .into_iter()
        .find(|child| snapshot.node(*child).is_some_and(|node| node.node_type == crate::app::StateProcessorFolder::NODE_TYPE))
        .expect("processor folder should be created");

    let create_type = FormulaSourceRef::project_uuid(mapping_uuid).processor_create_type();
    for parent in [manager, folder] {
        let tree = engine.nodes.get(parent).unwrap().create_user_item_tree(&create_type).unwrap();
        let root = tree
            .children
            .iter()
            .find(|child| child.node.node_data().meta.decl_id.0 == PROCESSOR_MANAGED_REGIONS_DECL_ID)
            .expect("detached processor should contain its managed regions");
        let updated = root
            .children
            .iter()
            .find(|child| child.node.node_data().meta.decl_id.0 == region_id)
            .expect("detached processor should contain the updated region");
        assert_eq!(updated.node.node_data().meta.label, "Updated Mapping Inputs");
    }
}
