use super::{AppEngine, AppNode};

impl golden_core::app::ProjectLifecycle for AppNode {
    fn project_file_spec() -> golden_core::app::ProjectFileSpec {
        golden_core::app::ProjectFileSpec::new("Noisette", "noisette")
    }

    fn app_data_directory_name() -> &'static str {
        "Chataigne"
    }

    fn configure_engine(engine: &mut AppEngine) -> Result<(), String> {
        super::module::register_module_reference_filters(engine);
        Ok(())
    }

    fn initialize_new_project(engine: &mut AppEngine) -> Result<(), String> {
        super::default_project::initialize_default_project(engine);
        Ok(())
    }

    fn project_opened(engine: &mut AppEngine) -> Result<(), String> {
        super::state_machine_nodes_processor::sync_external_formulas(engine)
    }
}
