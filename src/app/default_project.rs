use super::{AppEngine, FormulaLibrary, ModuleManager, StateMachineManager};

pub(crate) fn initialize_default_project(engine: &mut AppEngine) {
    golden_core::app::add_default_project_nodes(engine);
    // FormulaLibrary must be added first so StateProcessorManager can build its
    // formula item cache when it initialises (on_node_ready has a live snapshot).
    engine.add_node(FormulaLibrary::new().into(), None);
    engine.add_node(ModuleManager::new().into(), None);
    engine.add_node(StateMachineManager::new().into(), None);
}

#[cfg(test)]
mod default_project_tests;
