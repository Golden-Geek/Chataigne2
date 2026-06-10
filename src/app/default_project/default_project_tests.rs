use golden_core::node::{Folder, Node};

use super::initialize_default_project;
use crate::app::{AppEngine, AppNode, FormulaLibrary, StateMachineManager};

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
fn default_project_contains_formula_library_with_builtin_formulas() {
    let root: AppNode = Folder::new("root").into();
    let mut engine = AppEngine::new(root);

    initialize_default_project(&mut engine);
    engine.apply_edits().expect("default project nodes should attach");

    let libraries = engine
        .nodes
        .iter()
        .filter(|(_, node)| node.get_type() == FormulaLibrary::NODE_TYPE)
        .collect::<Vec<_>>();

    assert_eq!(libraries.len(), 1);
    assert_eq!(libraries[0].1.node_data().parent, Some(engine.root));

    let library_id = libraries[0].0;
    let action_formulas = engine
        .nodes
        .iter()
        .filter(|(_, node)| {
            node.get_type() == crate::app::ActionBuiltinFormula::NODE_TYPE
                && node.node_data().parent == Some(library_id)
        })
        .count();
    let mapping_formulas = engine
        .nodes
        .iter()
        .filter(|(_, node)| {
            node.get_type() == crate::app::MappingBuiltinFormula::NODE_TYPE
                && node.node_data().parent == Some(library_id)
        })
        .count();

    assert_eq!(action_formulas, 1, "expected one Action built-in formula");
    assert_eq!(mapping_formulas, 1, "expected one Mapping built-in formula");
}
