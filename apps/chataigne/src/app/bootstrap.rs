use super::{AppEngine, AppNode};
use golden_core::node::Node;

impl golden_core::app::ProjectLifecycle for AppNode {
    fn create_project_root() -> Self {
        let mut root = golden_core::node::Folder::new("Root");
        root.node_data_mut()
            .meta
            .tags
            .push(super::systems_alchemist_formula::GATE_SEMANTICS_V2_TAG.to_owned());
        root.into()
    }

    fn application_display_name() -> &'static str {
        "Chataigne 2"
    }

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
        super::systems_alchemist_formula::migrate_legacy_gate_semantics(engine)?;
        super::systems_alchemist_processor::sync_external_formulas(engine)
    }
}
