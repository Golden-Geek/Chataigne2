use golden_core::node::{Folder, Node};

use super::initialize_default_project;
use crate::app::{
    state_machine_nodes_formula::{FORMULA_EXTERNAL_BUILTIN_TAG_PREFIX, FORMULA_EXTERNAL_READ_ONLY_TAG},
    state_machine_nodes_processor::FormulaCatalog,
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
    // Isolate from whatever the current machine's real Shared formulas
    // folder happens to contain (e.g. formulas saved there by a running
    // app instance) so this only ever sees the shipped built-ins.
    let _shared_dir_guard = shared_formula_dir_test_override();

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

    let expected_labels = FormulaCatalog::default_builtin_formula_trees()
        .expect("built-in formulas should load")
        .into_iter()
        .map(|tree| tree.node.node_data().meta.label.clone())
        .collect::<Vec<_>>();
    assert_eq!(
        formulas.iter().map(|(label, _, _)| label.clone()).collect::<Vec<_>>(),
        expected_labels
    );
    for (_, tags, permissions) in formulas {
        assert!(tags.iter().any(|tag| tag == FORMULA_EXTERNAL_READ_ONLY_TAG));
        assert!(tags
            .iter()
            .any(|tag| tag.starts_with(FORMULA_EXTERNAL_BUILTIN_TAG_PREFIX)));
        assert!(!permissions.can_remove_and_duplicate);
        assert!(!permissions.can_edit_name);
    }
}

/// Points CHATAIGNE_SHARED_FORMULAS_DIR at a directory that doesn't exist,
/// so `default_shared_formula_trees()` sees zero shared formulas regardless
/// of what the current machine's real Shared formulas folder contains.
/// Restores the previous value (if any) when dropped.
fn shared_formula_dir_test_override() -> crate::test_support::ScopedSharedFormulaDir {
    crate::test_support::scoped_shared_formula_dir(Some(std::path::Path::new(
        "chataigne2-tests-nonexistent-shared-formulas-dir",
    )))
}
