use super::{AppEngine, ModuleManager};

pub(crate) fn initialize_default_project(engine: &mut AppEngine) {
    golden_core::app::add_default_project_nodes(engine);
    engine.add_node(ModuleManager::new().into(), None);
}
