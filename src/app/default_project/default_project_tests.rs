use golden_core::node::{Folder, Node};

use super::initialize_default_project;
use crate::app::{
    state_machine_nodes_formula::{FORMULA_EXTERNAL_BUILTIN_TAG_PREFIX, FORMULA_EXTERNAL_READ_ONLY_TAG},
    AlchemistFormulaDefinition, AppEngine, AppNode, FormulaLibrary, StateMachineManager,
};

#[test]
fn default_project_contains_one_top_level_state_machine_manager() {
    let root: AppNode = Folder::new("root").into();
    let mut engine = AppEngine::new(root);

    initialize_default_project(&mut engine);
    engine.apply_edits().expect("default project nodes should attach");

    let managers = engine
        .nodes
        .iter()
        .filter(|(_, node)| node.get_type() == StateMachineManager::NODE_TYPE)
        .collect::<Vec<_>>();

    assert_eq!(managers.len(), 1);
    assert_eq!(managers[0].1.node_data().parent, Some(engine.root));
}

#[test]
fn default_project_contains_builtin_external_formulas() {
    let root: AppNode = Folder::new("root").into();
    let mut engine = AppEngine::new(root);

    initialize_default_project(&mut engine);
    for _ in 0..4 {
        engine.apply_edits().expect("default project nodes should attach");
    }

    let libraries = engine
        .nodes
        .iter()
        .filter(|(_, node)| node.get_type() == FormulaLibrary::NODE_TYPE)
        .collect::<Vec<_>>();

    assert_eq!(libraries.len(), 1);
    assert_eq!(libraries[0].1.node_data().parent, Some(engine.root));

    let formulas = engine
        .process_tree_snapshot()
        .child_ids(libraries[0].0)
        .into_iter()
        .filter_map(|node_id| engine.nodes.get(node_id))
        .filter(|node| node.get_type() == AlchemistFormulaDefinition::NODE_TYPE)
        .map(|node| {
            (
                node.node_data().meta.label.clone(),
                node.node_data().meta.tags.clone(),
                node.node_data().meta.user_permissions.clone(),
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(formulas.len(), 2);
    assert_eq!(formulas[0].0, "Action");
    assert_eq!(formulas[1].0, "Mapping");
    for (_, tags, permissions) in formulas {
        assert!(tags.iter().any(|tag| tag == FORMULA_EXTERNAL_READ_ONLY_TAG));
        assert!(tags
            .iter()
            .any(|tag| tag.starts_with(FORMULA_EXTERNAL_BUILTIN_TAG_PREFIX)));
        assert!(!permissions.can_remove_and_duplicate);
        assert!(!permissions.can_edit_name);
    }
}
