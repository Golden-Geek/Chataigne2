use super::{AppEngine, AppNode};

impl golden_core::app::ProjectLifecycle for AppNode {
    fn project_file_spec() -> golden_core::app::ProjectFileSpec {
        golden_core::app::ProjectFileSpec::new("Noisette", "noisette")
    }

    fn configure_engine(engine: &mut AppEngine) -> Result<(), String> {
        super::module::register_module_reference_filters(engine);
        Ok(())
    }

    fn initialize_new_project(engine: &mut AppEngine) -> Result<(), String> {
        golden_core::app::add_default_project_nodes(engine);
        engine.add_node(super::ModuleManager::new().into(), None);
        Ok(())
    }
}
