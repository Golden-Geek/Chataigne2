use golden_core::node::Node;

use super::{ActionBuiltinFormula, CustomAlchemistFormula, FormulaLibrary, MappingBuiltinFormula, CUSTOM_FORMULA_ITEM_KIND};

#[test]
fn formula_library_has_action_and_mapping_as_fixed_children() {
    let library = FormulaLibrary::new();
    let _ = library.action;
    let _ = library.mapping;
}

#[test]
fn formula_library_accepts_only_custom_formula_items() {
    let library = FormulaLibrary::new();

    assert!(library.user_container_accepts_item("alchemist_formula_custom", CUSTOM_FORMULA_ITEM_KIND));
    assert!(!library.user_container_accepts_item("alchemist_formula_action", CUSTOM_FORMULA_ITEM_KIND));
    assert!(!library.user_container_accepts_item("alchemist_formula_custom", "state_processor"));
}

#[test]
fn formula_library_cannot_be_removed_by_user() {
    let library = FormulaLibrary::new();
    assert!(!library.node_data().meta.user_permissions.can_remove_and_duplicate);
}

#[test]
fn builtin_formulas_have_no_user_permissions() {
    let action = ActionBuiltinFormula::new();
    let mapping = MappingBuiltinFormula::new();

    assert!(!action.node_data().meta.user_permissions.can_remove_and_duplicate);
    assert!(!action.node_data().meta.user_permissions.can_edit_name);
    assert!(!mapping.node_data().meta.user_permissions.can_remove_and_duplicate);
    assert!(!mapping.node_data().meta.user_permissions.can_edit_name);
}

#[test]
fn custom_formula_has_empty_authored_graph_by_default() {
    let formula = CustomAlchemistFormula::new();
    assert!(formula.authored_graph.get_ref().is_empty());
}

#[test]
fn custom_formula_project_create_returns_correct_type() {
    let created = <CustomAlchemistFormula as Node>::project_create("alchemist_formula_custom");
    assert!(created.is_some(), "project_create should succeed for the custom formula type");
    let created = <CustomAlchemistFormula as Node>::project_create("alchemist_formula_action");
    assert!(created.is_none(), "project_create should return None for a different type");
}
