use super::{AppEngine, FormulaLibrary, ModuleManager, StateMachineManager};

pub(crate) fn initialize_default_project(engine: &mut AppEngine) {
    golden_core::app::add_default_project_nodes(engine);
    engine.add_node(FormulaLibrary::new().into(), None);
    engine.add_node(ModuleManager::new().into(), None);
    engine.add_node(StateMachineManager::new().into(), None);
}

#[cfg(test)]
mod default_project_tests;
