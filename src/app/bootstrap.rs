use super::{AppEngine, AppNode};

impl golden_core::app::ProjectLifecycle for AppNode {
    fn configure_engine(engine: &mut AppEngine) -> Result<(), String> {
        super::nodes_module_demo::register_demo_reference_filters(engine);
        Ok(())
    }

    fn initialize_new_project(engine: &mut AppEngine) -> Result<(), String> {
        golden_core::app::add_default_project_nodes(engine);
        
        Ok(())
    }
}
