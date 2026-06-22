use golden_alchemist::{ManagedRegionKind, SurfaceItemKind};
use golden_core::{
    node::{Folder, Node, NodeId, NodeReference},
    parameter::{ParamValue, Parameter},
    process_ctx::ExecutionPhase,
    ui_sync::UiEditIntent,
};

use crate::app::{
    AlchemistFormulaDefinition, AppEngine, AppNode, ConditionManager, FormulaLibrary, InputsManager,
    OutputsManager,
};

use super::{
    FormulaCatalog, FormulaSourceRef, ProcessorFormulaSourceState,
    PROCESSOR_FOLDER_ITEM_KIND, PROCESSOR_FOLDER_NODE_TYPE,
    PROCESSOR_FORMULA_SOURCE_DECL_ID, PROCESSOR_ITEM_KIND,
    PROCESSOR_MANAGED_REGIONS_DECL_ID, StateProcessor,
    StateProcessorFolder, StateProcessorManagedRegion,
    StateProcessorManagedRegions, StateProcessorManager, StateProcessorProperties,
    BUILTIN_ACTION_FORMULA_ID, BUILTIN_FORMULA_PACKAGE, BUILTIN_FORMULA_VERSION,
    BUILTIN_MAPPING_FORMULA_ID, processor_managed_region_decl_id,
};
use crate::app::state_machine_nodes_formula::{
    AlchemistProperty, ANODE_CREATE_PREFIX, ANODE_NODE_TYPE, PROPERTIES_DECL_ID, PROPERTY_CREATE_PREFIX,
    PROPERTY_FOLDER_NODE_TYPE, PROPERTY_MANAGER_CREATE_PREFIX,
};

fn engine_with_formula() -> (
    AppEngine,
    NodeId,
    golden_core::node::NodeUuid,
) {
    let root: AppNode = Folder::new("root").into();
    let mut engine = AppEngine::new(root);
    engine.add_node(FormulaLibrary::new().into(), None);
    engine.apply_edits().expect("formula library should attach");
    let library = engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == FormulaLibrary::NODE_TYPE)
        .map(|(id, _)| id)
        .expect("Formula Library should exist");
    let mut formula = <AlchemistFormulaDefinition as Node>::project_create(
        AlchemistFormulaDefinition::NODE_TYPE,
    )
    .expect("custom Formula should be creatable");
    formula.node_data_mut().meta.label = "Custom Formula".into();
    let formula_uuid = formula.node_data().meta.uuid;
    engine.add_user_item(formula.into(), Some(library));
    engine.apply_edits().expect("formula should attach");
    let formula_id = engine
        .nodes
        .iter()
        .find(|(_, node)| node.node_data().meta.uuid == formula_uuid)
        .map(|(id, _)| id)
        .expect("Formula should attach");
    (engine, formula_id, formula_uuid)
}

#[test]
fn processor_manager_lists_custom_formulas() {
    let (mut engine, _, formula_uuid) = engine_with_formula();
    engine.add_node(StateProcessorManager::new().into(), None);
    engine.apply_edits().expect("manager should attach");

    let manager = engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == StateProcessorManager::NODE_TYPE)
        .map(|(_, node)| node)
        .expect("processor manager should exist");
    let formula_items = manager
        .user_creatable_items()
        .into_iter()
        .filter(|item| item.item_kind == PROCESSOR_ITEM_KIND)
        .collect::<Vec<_>>();

    let labels = formula_items
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();
    assert_eq!(labels, vec!["Action", "Mapping", "Custom Formula"]);
    assert_eq!(
        formula_items[0].node_type,
        "state_processor:builtin:chataigne.action@1"
    );
    assert_eq!(formula_items[0].menu_path, vec!["Built-ins".to_string()]);
    assert_eq!(
        formula_items[1].node_type,
        "state_processor:builtin:chataigne.mapping@1"
    );
    assert_eq!(formula_items[1].menu_path, vec!["Built-ins".to_string()]);
    assert_eq!(
        formula_items[2].node_type,
        format!("state_processor:project:{}", formula_uuid.0)
    );
    assert_eq!(
        formula_items[2].menu_path,
        vec!["Project Formulas".to_string()]
    );
}

#[test]
fn builtins_are_not_formula_library_items() {
    let (engine, _, _) = engine_with_formula();
    let library = engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == FormulaLibrary::NODE_TYPE)
        .map(|(_, node)| node)
        .expect("formula library should exist");
    let labels = library
        .user_creatable_items()
        .into_iter()
        .map(|item| item.label)
        .collect::<Vec<_>>();

    assert!(!labels.iter().any(|label| label == "Action"));
    assert!(!labels.iter().any(|label| label == "Mapping"));
}

#[test]
fn builtin_formula_sources_parse_and_resolve() {
    let catalog = FormulaCatalog::with_builtins();
    let source = FormulaSourceRef::parse_processor_create_type(
        "state_processor:builtin:chataigne.mapping@1",
    )
    .expect("builtin source should parse");
    let FormulaSourceRef::Builtin {
        package,
        formula_id,
        version,
    } = &source
    else {
        panic!("expected builtin source");
    };
    assert_eq!(package.as_ref(), BUILTIN_FORMULA_PACKAGE);
    assert_eq!(formula_id.as_ref(), BUILTIN_MAPPING_FORMULA_ID);
    assert_eq!(*version, BUILTIN_FORMULA_VERSION);

    let formula = catalog
        .resolve_builtin(&source)
        .expect("builtin Mapping should resolve");
    assert_eq!(formula.label, "Mapping");
    assert_eq!(formula.version, BUILTIN_FORMULA_VERSION);
}

#[test]
fn builtin_formula_package_loads_action_and_single_mapping() {
    let catalog = FormulaCatalog::with_builtins();
    let palette_entries = catalog.processor_palette_entries().collect::<Vec<_>>();
    let labels = palette_entries
        .iter()
        .map(|entry| entry.label.as_str())
        .collect::<Vec<_>>();

    assert_eq!(labels.iter().filter(|label| **label == "Action").count(), 1);
    assert_eq!(labels.iter().filter(|label| **label == "Mapping").count(), 1);
    assert_eq!(
        labels
            .iter()
            .filter(|label| label.starts_with("Mapping"))
            .copied()
            .collect::<Vec<_>>(),
        vec!["Mapping"]
    );

    let action = catalog
        .resolve_builtin(&FormulaSourceRef::builtin(
            BUILTIN_FORMULA_PACKAGE,
            BUILTIN_ACTION_FORMULA_ID,
            BUILTIN_FORMULA_VERSION,
        ))
        .expect("builtin Action should resolve from the shipped package");
    let mapping = catalog
        .resolve_builtin(&FormulaSourceRef::builtin(
            BUILTIN_FORMULA_PACKAGE,
            BUILTIN_MAPPING_FORMULA_ID,
            BUILTIN_FORMULA_VERSION,
        ))
        .expect("builtin Mapping should resolve from the shipped package");

    assert_eq!(action.id.to_string(), "chataigne.action");
    assert_eq!(mapping.id.to_string(), "chataigne.mapping");
    assert!(action.tags.iter().any(|tag| tag == "builtin_package:chataigne"));
    assert!(mapping.tags.iter().any(|tag| tag == "builtin_package:chataigne"));
    assert!(action.graph.nodes.is_empty());
    assert!(mapping.graph.nodes.is_empty());
}

#[test]
fn builtin_formulas_expose_empty_managed_regions() {
    let catalog = FormulaCatalog::with_builtins();
    let action = catalog
        .resolve_builtin(&FormulaSourceRef::builtin(
            BUILTIN_FORMULA_PACKAGE,
            BUILTIN_ACTION_FORMULA_ID,
            BUILTIN_FORMULA_VERSION,
        ))
        .expect("builtin Action should resolve");
    let mapping = catalog
        .resolve_builtin(&FormulaSourceRef::builtin(
            BUILTIN_FORMULA_PACKAGE,
            BUILTIN_MAPPING_FORMULA_ID,
            BUILTIN_FORMULA_VERSION,
        ))
        .expect("builtin Mapping should resolve");

    assert_region_kinds(
        &mapping,
        &[
            ("inputs", ManagedRegionKind::InputSet, SurfaceItemKind::Input),
            (
                "filters",
                ManagedRegionKind::FilterPipeline,
                SurfaceItemKind::Filter,
            ),
            ("outputs", ManagedRegionKind::OutputSet, SurfaceItemKind::Output),
        ],
    );
    assert_region_kinds(
        &action,
        &[
            (
                "trigger",
                ManagedRegionKind::ActionTrigger,
                SurfaceItemKind::Input,
            ),
            (
                "pipeline",
                ManagedRegionKind::FilterPipeline,
                SurfaceItemKind::Filter,
            ),
            (
                "commands",
                ManagedRegionKind::ActionCommands,
                SurfaceItemKind::Action,
            ),
        ],
    );
    assert!(!mapping
        .surface
        .managed_regions
        .iter()
        .any(|region| region.id.as_str().contains("condition")));

    let mapping_instance = mapping.instantiate();
    mapping_instance
        .managed_regions
        .validate_against(&mapping.surface)
        .expect("empty Mapping regions should be valid");
    assert!(mapping_instance
        .managed_regions
        .regions
        .values()
        .all(|region| region.items.is_empty()));

    let action_instance = action.instantiate();
    action_instance
        .managed_regions
        .validate_against(&action.surface)
        .expect("empty Action regions should be valid");
    assert!(action_instance
        .managed_regions
        .regions
        .values()
        .all(|region| region.items.is_empty()));
}

#[test]
fn invalid_builtin_formula_source_fails_cleanly() {
    let catalog = FormulaCatalog::with_builtins();
    let source = FormulaSourceRef::builtin(
        BUILTIN_FORMULA_PACKAGE,
        "missing",
        BUILTIN_FORMULA_VERSION,
    );
    let error = catalog
        .resolve_builtin(&source)
        .expect_err("unknown builtin should fail");

    assert!(error
        .to_string()
        .contains("builtin formula source 'builtin:chataigne.missing@1'"));

    let parse_error = FormulaSourceRef::parse_processor_create_type(
        "state_processor:builtin:chataigne.mapping@bad",
    )
    .expect_err("invalid builtin version should fail");
    assert!(parse_error
        .to_string()
        .contains("invalid builtin formula version"));
}

fn assert_region_kinds(
    formula: &golden_alchemist::AlchemistFormula,
    expected: &[(&str, ManagedRegionKind, SurfaceItemKind)],
) {
    let actual = formula
        .surface
        .managed_regions
        .iter()
        .map(|region| {
            (
                region.id.as_str().to_owned(),
                region.kind,
                region.accepted_roles.clone(),
            )
        })
        .collect::<Vec<_>>();
    let expected = expected
        .iter()
        .map(|(id, kind, role)| ((*id).to_owned(), *kind, vec![*role]))
        .collect::<Vec<_>>();

    assert_eq!(actual, expected);
}

#[test]
fn processor_created_from_typed_project_item_keeps_source_and_reference() {
    let (_, _, formula_uuid) = engine_with_formula();
    let manager = StateProcessorManager::new();
    let processor = manager
        .create_user_item(&format!("state_processor:project:{}", formula_uuid.0))
        .expect("typed project processor should be creatable");
    let processor = processor
        .as_any()
        .downcast_ref::<StateProcessor>()
        .expect("created item should be a state processor");

    assert!(matches!(
        processor.formula_source.to_source_ref(),
        Ok(Some(FormulaSourceRef::ProjectNode(reference))) if reference.uuid() == formula_uuid
    ));
    assert_eq!(processor.formula.get_ref().uuid(), formula_uuid);
}

#[test]
fn processor_created_from_builtin_mapping_item_keeps_source_without_project_reference() {
    let manager = StateProcessorManager::new();
    let processor = manager
        .create_user_item("state_processor:builtin:chataigne.mapping@1")
        .expect("builtin Mapping processor should be creatable");
    let processor = processor
        .as_any()
        .downcast_ref::<StateProcessor>()
        .expect("created item should be a state processor");

    assert!(matches!(
        processor.formula_source.to_source_ref(),
        Ok(Some(FormulaSourceRef::Builtin {
            package,
            formula_id,
            version,
        })) if package.as_ref() == BUILTIN_FORMULA_PACKAGE
            && formula_id.as_ref() == BUILTIN_MAPPING_FORMULA_ID
            && version == BUILTIN_FORMULA_VERSION
    ));
    assert!(processor.formula.get_ref().is_empty());
}

#[test]
fn processor_formula_source_state_serializes_builtin_source() {
    let state = ProcessorFormulaSourceState::from_source(&FormulaSourceRef::builtin(
        BUILTIN_FORMULA_PACKAGE,
        BUILTIN_MAPPING_FORMULA_ID,
        BUILTIN_FORMULA_VERSION,
    ));
    let json = serde_json::to_string(&state).expect("source state should serialize");
    let restored: ProcessorFormulaSourceState =
        serde_json::from_str(&json).expect("source state should deserialize");

    assert_eq!(restored, state);
    assert!(matches!(
        restored.to_source_ref(),
        Ok(Some(FormulaSourceRef::Builtin {
            package,
            formula_id,
            version,
        })) if package.as_ref() == BUILTIN_FORMULA_PACKAGE
            && formula_id.as_ref() == BUILTIN_MAPPING_FORMULA_ID
            && version == BUILTIN_FORMULA_VERSION
    ));
}

#[test]
fn builtin_mapping_processor_has_no_missing_formula_warning() {
    let (mut engine, _, _) = engine_with_formula();
    engine.add_node(StateProcessorManager::new().into(), None);
    engine.apply_edits().expect("manager should attach");
    let manager_id = engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == StateProcessorManager::NODE_TYPE)
        .map(|(id, _)| id)
        .expect("processor manager should exist");
    let mut processor = StateProcessor::new();
    processor.set_formula_source(FormulaSourceRef::builtin(
        BUILTIN_FORMULA_PACKAGE,
        BUILTIN_MAPPING_FORMULA_ID,
        BUILTIN_FORMULA_VERSION,
    ));
    engine.add_user_item(processor.into(), Some(manager_id));
    engine.apply_edits().expect("Processor should attach");
    let processor_id = engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == StateProcessor::NODE_TYPE)
        .map(|(id, _)| id)
        .expect("Processor should exist");

    assert!(!node_has_warning_id(
        &engine,
        processor_id,
        "state_processor_formula"
    ));
}

#[test]
fn builtin_mapping_processor_instantiates_managed_region_folders() {
    let (mut engine, _, _) = engine_with_formula();
    let processor_id = attach_builtin_mapping_processor(&mut engine);
    let snapshot = engine.process_tree_snapshot();
    let source_key = snapshot
        .find_child_by_decl_id(processor_id, PROCESSOR_FORMULA_SOURCE_DECL_ID)
        .expect("Processor should expose its formula source key");

    assert_eq!(
        snapshot
            .node(source_key)
            .and_then(|node| node.param_value.as_ref()),
        Some(&ParamValue::Str(
            "state_processor:builtin:chataigne.mapping@1".to_owned()
        ))
    );

    let regions_root = snapshot
        .find_child_by_decl_id(processor_id, PROCESSOR_MANAGED_REGIONS_DECL_ID)
        .expect("Processor should own a Managed Regions root");
    assert_eq!(
        engine
            .nodes
            .get(regions_root)
            .expect("Managed Regions root should exist")
            .get_type(),
        StateProcessorManagedRegions::NODE_TYPE
    );

    let actual = snapshot
        .child_ids(regions_root)
        .into_iter()
        .map(|child| {
            let node = engine.nodes.get(child).expect("region should exist");
            (
                node.get_type().to_owned(),
                node.node_data().meta.decl_id.0.clone(),
                node.node_data().meta.label.clone(),
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(
        actual,
        vec![
            (
                StateProcessorManagedRegion::NODE_TYPE.to_owned(),
                processor_managed_region_decl_id("inputs"),
                "Inputs".to_owned()
            ),
            (
                StateProcessorManagedRegion::NODE_TYPE.to_owned(),
                processor_managed_region_decl_id("filters"),
                "Filters".to_owned()
            ),
            (
                StateProcessorManagedRegion::NODE_TYPE.to_owned(),
                processor_managed_region_decl_id("outputs"),
                "Outputs".to_owned()
            )
        ]
    );
}

#[test]
fn builtin_action_processor_instantiates_managed_region_folders() {
    let (mut engine, _, _) = engine_with_formula();
    let processor_id = attach_builtin_action_processor(&mut engine);
    let snapshot = engine.process_tree_snapshot();
    let source_key = snapshot
        .find_child_by_decl_id(processor_id, PROCESSOR_FORMULA_SOURCE_DECL_ID)
        .expect("Processor should expose its formula source key");

    assert_eq!(
        snapshot
            .node(source_key)
            .and_then(|node| node.param_value.as_ref()),
        Some(&ParamValue::Str(
            "state_processor:builtin:chataigne.action@1".to_owned()
        ))
    );

    let regions_root = snapshot
        .find_child_by_decl_id(processor_id, PROCESSOR_MANAGED_REGIONS_DECL_ID)
        .expect("Processor should own a Managed Regions root");
    let actual = snapshot
        .child_ids(regions_root)
        .into_iter()
        .map(|child| {
            let node = engine.nodes.get(child).expect("region should exist");
            (
                node.get_type().to_owned(),
                node.node_data().meta.decl_id.0.clone(),
                node.node_data().meta.label.clone(),
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(
        actual,
        vec![
            (
                StateProcessorManagedRegion::NODE_TYPE.to_owned(),
                processor_managed_region_decl_id("trigger"),
                "Trigger".to_owned()
            ),
            (
                StateProcessorManagedRegion::NODE_TYPE.to_owned(),
                processor_managed_region_decl_id("pipeline"),
                "Pipeline".to_owned()
            ),
            (
                StateProcessorManagedRegion::NODE_TYPE.to_owned(),
                processor_managed_region_decl_id("commands"),
                "Commands".to_owned()
            )
        ]
    );
}

#[test]
fn managed_region_palette_accepts_only_matching_anode_roles() {
    let (mut engine, _, _) = engine_with_formula();
    let processor_id = attach_builtin_mapping_processor(&mut engine);
    let snapshot = engine.process_tree_snapshot();
    let regions_root = snapshot
        .find_child_by_decl_id(processor_id, PROCESSOR_MANAGED_REGIONS_DECL_ID)
        .expect("Processor should own Managed Regions");
    let filters = snapshot
        .find_child_by_decl_id(
            regions_root,
            &processor_managed_region_decl_id("filters"),
        )
        .expect("Mapping should expose a Filters region");
    let inputs = snapshot
        .find_child_by_decl_id(
            regions_root,
            &processor_managed_region_decl_id("inputs"),
        )
        .expect("Mapping should expose an Inputs region");
    let outputs = snapshot
        .find_child_by_decl_id(
            regions_root,
            &processor_managed_region_decl_id("outputs"),
        )
        .expect("Mapping should expose an Outputs region");
    let condition_gate_type = format!("{ANODE_CREATE_PREFIX}condition_gate");
    let input_type = format!(
        "{ANODE_CREATE_PREFIX}{}",
        chataigne_state_machine::alchemist::INPUTS_MANAGER_TYPE
    );
    let output_type = format!(
        "{ANODE_CREATE_PREFIX}{}",
        chataigne_state_machine::alchemist::OUTPUTS_MANAGER_TYPE
    );

    let filter_items = engine
        .nodes
        .get(filters)
        .expect("Filters region should exist")
        .user_creatable_items();
    let input_items = engine
        .nodes
        .get(inputs)
        .expect("Inputs region should exist")
        .user_creatable_items();
    let output_items = engine
        .nodes
        .get(outputs)
        .expect("Outputs region should exist")
        .user_creatable_items();

    assert!(filter_items
        .iter()
        .any(|item| item.node_type == condition_gate_type));
    assert!(input_items
        .iter()
        .any(|item| item.node_type == input_type));
    assert!(output_items
        .iter()
        .any(|item| item.node_type == output_type));
    assert!(!input_items
        .iter()
        .any(|item| item.node_type == condition_gate_type));

    let rejected = engine.apply_ui_intent(UiEditIntent::CreateUserItem {
        parent: inputs,
        node_type: condition_gate_type.clone(),
        label: None,
        initial_params: Vec::new(),
    });
    assert!(
        !rejected.success,
        "Inputs region should reject filter-only ANodes"
    );

    let accepted = engine.apply_ui_intent(UiEditIntent::CreateUserItem {
        parent: filters,
        node_type: condition_gate_type,
        label: None,
        initial_params: Vec::new(),
    });
    assert!(
        accepted.success,
        "Filters region should accept ConditionGate: {accepted:?}"
    );
    let snapshot = engine.process_tree_snapshot();
    assert!(snapshot.child_ids(filters).into_iter().any(|child| {
        engine
            .nodes
            .get(child)
            .is_some_and(|node| node.get_type() == ANODE_NODE_TYPE)
    }));

    let accepted = engine.apply_ui_intent(UiEditIntent::CreateUserItem {
        parent: inputs,
        node_type: input_type,
        label: None,
        initial_params: Vec::new(),
    });
    assert!(
        accepted.success,
        "Inputs region should accept Inputs manager ref ANodes: {accepted:?}"
    );

    let accepted = engine.apply_ui_intent(UiEditIntent::CreateUserItem {
        parent: outputs,
        node_type: output_type,
        label: None,
        initial_params: Vec::new(),
    });
    assert!(
        accepted.success,
        "Outputs region should accept Output Commands manager ref ANodes: {accepted:?}"
    );
}

#[test]
fn managed_region_items_survive_sparse_project_reload() {
    let (mut engine, _, _) = engine_with_formula();
    let processor_id = attach_builtin_mapping_processor(&mut engine);
    let snapshot = engine.process_tree_snapshot();
    let regions_root = snapshot
        .find_child_by_decl_id(processor_id, PROCESSOR_MANAGED_REGIONS_DECL_ID)
        .expect("Processor should own Managed Regions");
    let inputs = snapshot
        .find_child_by_decl_id(
            regions_root,
            &processor_managed_region_decl_id("inputs"),
        )
        .expect("Mapping should expose an Inputs region");
    let filters = snapshot
        .find_child_by_decl_id(
            regions_root,
            &processor_managed_region_decl_id("filters"),
        )
        .expect("Mapping should expose a Filters region");
    let outputs = snapshot
        .find_child_by_decl_id(
            regions_root,
            &processor_managed_region_decl_id("outputs"),
        )
        .expect("Mapping should expose an Outputs region");
    let input_type = format!(
        "{ANODE_CREATE_PREFIX}{}",
        chataigne_state_machine::alchemist::INPUTS_MANAGER_TYPE
    );
    let condition_gate_type = format!("{ANODE_CREATE_PREFIX}condition_gate");
    let output_type = format!(
        "{ANODE_CREATE_PREFIX}{}",
        chataigne_state_machine::alchemist::OUTPUTS_MANAGER_TYPE
    );

    for (parent, node_type, label) in [
        (inputs, input_type, "Inputs"),
        (filters, condition_gate_type, "ConditionGate"),
        (outputs, output_type, "Outputs"),
    ] {
        let result = engine.apply_ui_intent(UiEditIntent::CreateUserItem {
            parent,
            node_type,
            label: None,
            initial_params: Vec::new(),
        });
        assert!(
            result.success,
            "{label} region item should be accepted: {result:?}"
        );
    }
    engine
        .apply_edits()
        .expect("managed region items should materialize");

    let json = golden_core::app::to_sparse_project_json_pretty(&engine)
        .expect("project should serialize");
    let mut loaded = golden_core::app::from_sparse_project_json::<AppNode>(&json)
        .expect("project should reload");
    for _ in 0..4 {
        loaded
            .apply_edits()
            .expect("declared processor children should reconcile");
    }
    let round_tripped = golden_core::app::to_sparse_project_json_pretty(&loaded)
        .expect("reloaded project should serialize");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&round_tripped)
            .expect("round-tripped project json should parse"),
        serde_json::from_str::<serde_json::Value>(&json).expect("project json should parse"),
        "save, reload, and save should keep managed-region item data stable"
    );

    let loaded_processor = loaded
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == StateProcessor::NODE_TYPE)
        .map(|(id, _)| id)
        .expect("processor should reload");
    let loaded_snapshot = loaded.process_tree_snapshot();
    let loaded_regions_root = loaded_snapshot
        .find_child_by_decl_id(loaded_processor, PROCESSOR_MANAGED_REGIONS_DECL_ID)
        .expect("Managed Regions root should reload");
    for region_id in ["inputs", "filters", "outputs"] {
        let loaded_region = loaded_snapshot
            .find_child_by_decl_id(
                loaded_regions_root,
                &processor_managed_region_decl_id(region_id),
            )
            .unwrap_or_else(|| panic!("{region_id} region should reload"));
        let loaded_anodes = loaded_snapshot
            .child_ids(loaded_region)
            .into_iter()
            .filter(|child| {
                loaded
                    .nodes
                    .get(*child)
                    .is_some_and(|node| node.get_type() == ANODE_NODE_TYPE)
            })
            .count();
        assert_eq!(
            loaded_anodes, 1,
            "the user-authored {region_id} item should survive sparse project reload"
        );
    }
}

#[test]
fn processor_created_from_formula_item_keeps_a_stable_reference() {
    let (mut engine, _, formula_uuid) = engine_with_formula();
    engine.add_node(StateProcessorManager::new().into(), None);
    engine.apply_edits().expect("manager should attach");
    let manager_id = engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == StateProcessorManager::NODE_TYPE)
        .map(|(id, _)| id)
        .expect("processor manager should exist");
    let mut processor = StateProcessor::new();
    processor.formula.apply_runtime_value(&ParamValue::Reference(
        NodeReference::new(formula_uuid),
    ));
    engine.add_user_item(processor.into(), Some(manager_id));
    engine.apply_edits().expect("Processor should attach");
    let processor_id = engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == StateProcessor::NODE_TYPE)
        .map(|(id, _)| id)
        .expect("Processor should exist");
    let formula_param = engine
        .nodes
        .iter()
        .filter(|(_, node)| node.node_data().parent == Some(processor_id))
        .find_map(|(_, node)| node.as_any().downcast_ref::<Parameter>())
        .expect("Processor should have a Formula reference");

    assert!(matches!(
        &formula_param.value,
        ParamValue::Reference(reference) if reference.uuid() == formula_uuid
    ));
}

#[test]
fn processor_folder_accepts_processors_and_folders() {
    let manager = StateProcessorManager::new();
    let folder = StateProcessorFolder::new();
    assert!(manager.user_container_accepts_item(StateProcessor::NODE_TYPE, PROCESSOR_ITEM_KIND));
    assert!(manager.user_container_accepts_item(
        PROCESSOR_FOLDER_NODE_TYPE,
        PROCESSOR_FOLDER_ITEM_KIND
    ));
    assert!(folder.user_container_accepts_item(
        PROCESSOR_FOLDER_NODE_TYPE,
        PROCESSOR_FOLDER_ITEM_KIND
    ));
    assert!(folder
        .create_user_item(PROCESSOR_FOLDER_NODE_TYPE)
        .is_some());
    assert!(folder.user_container_accepts_item(
        &format!("state_processor:{}", uuid::Uuid::nil()),
        PROCESSOR_ITEM_KIND
    ));
    assert!(!manager.user_container_accepts_item("module", "module"));
    assert!(!manager.user_container_accepts_item("module_folder", "module_folder"));
    assert!(!folder.user_container_accepts_item("folder", PROCESSOR_FOLDER_ITEM_KIND));
    assert!(!folder.user_container_accepts_item("state", "state"));
}

#[test]
fn processor_nodes_do_not_accept_nested_processors() {
    let root: AppNode = Folder::new("root").into();
    let mut engine = AppEngine::new(root);
    engine.add_node(StateProcessorManager::new().into(), None);
    engine.apply_edits().expect("manager should attach");
    let manager_id = engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == StateProcessorManager::NODE_TYPE)
        .map(|(id, _)| id)
        .expect("processor manager should exist");

    engine.add_user_item(StateProcessor::new().into(), Some(manager_id));
    engine.add_user_item(StateProcessor::new().into(), Some(manager_id));
    engine
        .apply_edits()
        .expect("processors should attach to the manager");

    let processors = engine
        .process_tree_snapshot()
        .child_ids(manager_id)
        .into_iter()
        .filter(|child| {
            engine
                .nodes
                .get(*child)
                .is_some_and(|node| node.get_type() == StateProcessor::NODE_TYPE)
        })
        .collect::<Vec<_>>();
    assert_eq!(processors.len(), 2);

    let ack = engine.apply_ui_intent(UiEditIntent::MoveNode {
        node: processors[1],
        new_parent: processors[0],
        new_prev_sibling: None,
    });
    assert!(
        !ack.success,
        "processor nodes must not accept nested processor drops"
    );
}

#[test]
fn processor_materializes_formula_properties_as_real_children() {
    let (mut engine, formula, formula_uuid) = engine_with_formula();
    let properties = engine
        .process_tree_snapshot()
        .find_child_by_decl_id(formula, PROPERTIES_DECL_ID)
        .expect("Formula Properties should exist");
    let ack = engine.apply_ui_intent(UiEditIntent::CreateUserItem {
        parent: properties,
        node_type: format!("{PROPERTY_CREATE_PREFIX}float"),
        label: Some("Amount".into()),
        initial_params: Vec::new(),
    });
    assert!(ack.success, "Formula parameter creation should succeed: {ack:?}");
    let formula_property = engine
        .process_tree_snapshot()
        .child_ids(properties)
        .into_iter()
        .find(|child| {
            engine.nodes.get(*child).is_some_and(|node| {
                node.get_type() == AlchemistProperty::NODE_TYPE
            })
        })
        .expect("Formula parameter should exist");
    for (role, label) in [
        ("condition", "Conditions"),
        ("input", "Inputs"),
        ("output", "Output Commands"),
    ] {
        let ack = engine.apply_ui_intent(UiEditIntent::CreateUserItem {
            parent: properties,
            node_type: format!("{PROPERTY_MANAGER_CREATE_PREFIX}{role}"),
            label: Some(label.into()),
            initial_params: Vec::new(),
        });
        assert!(
            ack.success,
            "Formula {role} manager creation should succeed: {ack:?}"
        );
    }

    engine.add_node(StateProcessorManager::new().into(), None);
    engine.apply_edits().expect("manager should attach");
    let manager_id = engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == StateProcessorManager::NODE_TYPE)
        .map(|(id, _)| id)
        .expect("processor manager should exist");
    let mut processor = StateProcessor::new();
    processor.formula.apply_runtime_value(&ParamValue::Reference(
        NodeReference::new(formula_uuid),
    ));
    engine.add_user_item(processor.into(), Some(manager_id));
    engine.apply_edits().expect("Processor should attach");
    let processor_id = engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == StateProcessor::NODE_TYPE)
        .map(|(id, _)| id)
        .expect("Processor should exist");
    let snapshot = engine.process_tree_snapshot();
    let instance_properties = snapshot
        .find_child_by_decl_id(processor_id, PROPERTIES_DECL_ID)
        .expect("Processor should instantiate Formula Properties");
    assert_eq!(
        engine
            .nodes
            .get(instance_properties)
            .expect("Processor Properties should exist")
            .get_type(),
        StateProcessorProperties::NODE_TYPE
    );
    let instance_children = snapshot.child_ids(instance_properties);
    let parameter = instance_children
        .iter()
        .copied()
        .find(|child| {
            engine.nodes.get(*child).is_some_and(|node| {
                node.engine_param_snapshot().is_some()
                    && node.node_data().meta.label == "Amount"
            })
        })
        .expect("Processor should own an editable Amount parameter");
    assert_eq!(
        engine
            .nodes
            .get(parameter)
            .and_then(Node::engine_param_snapshot)
            .map(|parameter| parameter.value),
        Some(ParamValue::Float(0.0))
    );
    for (node_type, label) in [
        (ConditionManager::NODE_TYPE, "Conditions"),
        (InputsManager::NODE_TYPE, "Inputs"),
        (OutputsManager::NODE_TYPE, "Output Commands"),
    ] {
        assert!(instance_children.iter().copied().any(|child| {
            engine.nodes.get(child).is_some_and(|node| {
                node.get_type() == node_type
                    && node.node_data().meta.label == label
            })
        }));
    }
    let source_uuid = engine
        .nodes
        .get(formula_property)
        .expect("Formula parameter should exist")
        .node_data()
        .meta
        .uuid
        .0
        .to_string();
    assert_eq!(
        engine
            .nodes
            .get(parameter)
            .expect("Processor parameter should exist")
            .node_data()
            .meta
            .decl_id
        .0,
        format!("surface/{source_uuid}")
    );

    let ack = engine.apply_ui_intent(UiEditIntent::CreateUserItem {
        parent: properties,
        node_type: format!("{PROPERTY_CREATE_PREFIX}bool"),
        label: Some("Enabled".into()),
        initial_params: Vec::new(),
    });
    assert!(ack.success, "Formula update should succeed: {ack:?}");
    engine
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .expect("Processor should receive the Formula hierarchy event");
    engine
        .apply_edits()
        .expect("Processor property reconciliation should stabilize");
    let snapshot = engine.process_tree_snapshot();
    assert!(
        snapshot.child_ids(instance_properties).into_iter().any(|child| {
            engine.nodes.get(child).is_some_and(|node| {
                node.engine_param_snapshot().is_some()
                    && node.node_data().meta.label == "Enabled"
            })
        }),
        "Processor should follow Formula property additions"
    );
}

#[test]
fn processor_mirrors_formula_property_folders_and_nested_properties() {
    let (mut engine, formula, formula_uuid) = engine_with_formula();
    let properties = engine
        .process_tree_snapshot()
        .find_child_by_decl_id(formula, PROPERTIES_DECL_ID)
        .expect("Formula Properties should exist");

    let ack = engine.apply_ui_intent(UiEditIntent::CreateUserItem {
        parent: properties,
        node_type: PROPERTY_FOLDER_NODE_TYPE.to_owned(),
        label: Some("Group".into()),
        initial_params: Vec::new(),
    });
    assert!(ack.success, "Folder creation should succeed: {ack:?}");

    let formula_folder = engine
        .process_tree_snapshot()
        .child_ids(properties)
        .into_iter()
        .find(|child| {
            engine
                .nodes
                .get(*child)
                .is_some_and(|n| n.get_type() == PROPERTY_FOLDER_NODE_TYPE)
        })
        .expect("Folder should exist in formula properties");

    let ack = engine.apply_ui_intent(UiEditIntent::CreateUserItem {
        parent: formula_folder,
        node_type: format!("{PROPERTY_CREATE_PREFIX}float"),
        label: Some("Speed".into()),
        initial_params: Vec::new(),
    });
    assert!(ack.success, "Nested property creation should succeed: {ack:?}");

    engine.add_node(StateProcessorManager::new().into(), None);
    engine.apply_edits().expect("manager should attach");
    let manager_id = engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == StateProcessorManager::NODE_TYPE)
        .map(|(id, _)| id)
        .expect("processor manager should exist");

    let mut processor = StateProcessor::new();
    processor.formula.apply_runtime_value(&ParamValue::Reference(
        NodeReference::new(formula_uuid),
    ));
    engine.add_user_item(processor.into(), Some(manager_id));
    engine.apply_edits().expect("Processor should attach");

    let processor_id = engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == StateProcessor::NODE_TYPE)
        .map(|(id, _)| id)
        .expect("Processor should exist");

    let snapshot = engine.process_tree_snapshot();
    let instance_properties = snapshot
        .find_child_by_decl_id(processor_id, PROPERTIES_DECL_ID)
        .expect("Processor should instantiate Formula Properties");

    let instance_folder = snapshot
        .child_ids(instance_properties)
        .into_iter()
        .find(|child| {
            engine
                .nodes
                .get(*child)
                .is_some_and(|n| n.get_type() == StateProcessorFolder::NODE_TYPE)
        })
        .expect("Processor should mirror formula folder");

    assert_eq!(
        engine
            .nodes
            .get(instance_folder)
            .map(|n| n.node_data().meta.label.as_str()),
        Some("Group"),
        "Mirrored folder should preserve formula folder label"
    );

    let nested = snapshot
        .child_ids(instance_folder)
        .into_iter()
        .find(|child| {
            engine
                .nodes
                .get(*child)
                .is_some_and(|n| n.engine_param_snapshot().is_some())
        })
        .expect("Processor folder should contain the nested property");

    assert_eq!(
        engine
            .nodes
            .get(nested)
            .map(|n| n.node_data().meta.label.as_str()),
        Some("Speed"),
        "Nested property should be mirrored inside processor folder"
    );

    // verify dynamic reconciliation: add a second property to the formula folder
    let ack = engine.apply_ui_intent(UiEditIntent::CreateUserItem {
        parent: formula_folder,
        node_type: format!("{PROPERTY_CREATE_PREFIX}bool"),
        label: Some("Active".into()),
        initial_params: Vec::new(),
    });
    assert!(ack.success, "Dynamic nested property addition should succeed: {ack:?}");
    engine
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .expect("Processor should receive hierarchy event");
    engine
        .apply_edits()
        .expect("Reconciliation should stabilize");

    let snapshot = engine.process_tree_snapshot();
    let instance_folder = snapshot
        .child_ids(instance_properties)
        .into_iter()
        .find(|child| {
            engine
                .nodes
                .get(*child)
                .is_some_and(|n| n.get_type() == StateProcessorFolder::NODE_TYPE)
        })
        .expect("Processor folder should still exist after reconciliation");

    assert!(
        snapshot.child_ids(instance_folder).into_iter().any(|child| {
            engine
                .nodes
                .get(child)
                .is_some_and(|n| n.node_data().meta.label == "Active")
        }),
        "Dynamically added nested property should appear in processor folder"
    );
}

fn node_has_warning_id(engine: &AppEngine, node: NodeId, warning_id: &str) -> bool {
    engine
        .process_tree_snapshot()
        .node(node)
        .is_some_and(|snapshot| {
            snapshot
                .presentation
                .warnings
                .iter()
                .any(|warning| warning.id == warning_id)
        })
}

fn attach_processor_referencing(
    engine: &mut AppEngine,
    formula_uuid: golden_core::node::NodeUuid,
) -> NodeId {
    engine.add_node(StateProcessorManager::new().into(), None);
    engine.apply_edits().expect("manager should attach");
    let manager_id = engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == StateProcessorManager::NODE_TYPE)
        .map(|(id, _)| id)
        .expect("processor manager should exist");
    let mut processor = StateProcessor::new();
    processor.formula.apply_runtime_value(&ParamValue::Reference(
        NodeReference::new(formula_uuid),
    ));
    engine.add_user_item(processor.into(), Some(manager_id));
    engine.apply_edits().expect("Processor should attach");
    engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == StateProcessor::NODE_TYPE)
        .map(|(id, _)| id)
        .expect("Processor should exist")
}

fn attach_builtin_mapping_processor(engine: &mut AppEngine) -> NodeId {
    attach_builtin_processor(engine, BUILTIN_MAPPING_FORMULA_ID)
}

fn attach_builtin_action_processor(engine: &mut AppEngine) -> NodeId {
    attach_builtin_processor(engine, BUILTIN_ACTION_FORMULA_ID)
}

fn attach_builtin_processor(engine: &mut AppEngine, formula_id: &str) -> NodeId {
    engine.add_node(StateProcessorManager::new().into(), None);
    engine.apply_edits().expect("manager should attach");
    let manager_id = engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == StateProcessorManager::NODE_TYPE)
        .map(|(id, _)| id)
        .expect("processor manager should exist");
    let mut processor = StateProcessor::new();
    processor.set_formula_source(FormulaSourceRef::builtin(
        BUILTIN_FORMULA_PACKAGE,
        formula_id,
        BUILTIN_FORMULA_VERSION,
    ));
    engine.add_user_item(processor.into(), Some(manager_id));
    engine.apply_edits().expect("Processor should attach");
    engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == StateProcessor::NODE_TYPE)
        .map(|(id, _)| id)
        .expect("Processor should exist")
}

#[test]
fn processor_with_valid_formula_has_no_warning() {
    let (mut engine, _, formula_uuid) = engine_with_formula();
    let processor_id = attach_processor_referencing(&mut engine, formula_uuid);
    assert!(
        !node_has_warning_id(&engine, processor_id, super::PROCESSOR_FORMULA_WARNING_ID),
        "Processor with a valid formula reference should not warn"
    );
}

#[test]
fn processor_with_missing_formula_reference_sets_warning() {
    let (mut engine, _, _) = engine_with_formula();
    let missing_uuid = golden_core::node::NodeUuid(uuid::Uuid::new_v4());
    let processor_id = attach_processor_referencing(&mut engine, missing_uuid);
    assert!(
        node_has_warning_id(&engine, processor_id, super::PROCESSOR_FORMULA_WARNING_ID),
        "Processor referencing a missing formula should expose a warning"
    );
}
