use golden_core::edit::{Edit, NodeTree};

use super::{AppEngine, FormulaLibrary, ModuleManager, StateMachineManager};
use crate::app::state_machine_nodes_processor::FormulaCatalog;

pub(crate) fn initialize_default_project(engine: &mut AppEngine) {
    golden_core::app::add_default_project_nodes(engine);
    engine.edits.push(Edit::AddNodeTree {
        tree: formula_library_tree(),
        parent: engine.root,
        prev_sibling: None,
    });
    engine.add_node(ModuleManager::new().into(), None);
    engine.add_node(StateMachineManager::new().into(), None);
}

fn formula_library_tree() -> NodeTree {
    let mut tree = NodeTree::new(FormulaLibrary::new());
    for formula in FormulaCatalog::default_builtin_formula_trees().expect("built-in formula files should load") {
        tree.push_child(formula);
    }
    for formula in FormulaCatalog::default_all_shared_formula_trees().expect("shared formula files should load") {
        tree.push_child(formula);
    }
    tree
}

#[cfg(test)]
mod default_project_tests;
