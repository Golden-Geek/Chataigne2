use super::{AppEngine, AppNode};
use golden_core::node::Node;

impl golden_core::app::ProjectLifecycle for AppNode {
    fn application_id() -> &'static str {
        "chataigne"
    }

    fn application_version() -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn create_project_root() -> Self {
        let mut root = golden_core::node::Folder::new("Root");
        root.node_data_mut()
            .meta
            .tags
            .push(super::systems_alchemist_formula::GATE_SEMANTICS_V2_TAG.to_owned());
        root.node_data_mut()
            .meta
            .tags
            .push(super::systems_alchemist_formula::OUTPUT_BINDINGS_V2_TAG.to_owned());
        root.node_data_mut()
            .meta
            .tags
            .push(super::systems_alchemist_processor::MAPPING_CONCRETE_OUTPUTS_V1_TAG.to_owned());
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

    fn apply_app_operation(
        engine: &mut AppEngine,
        namespace: &str,
        operation: &str,
        payload: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        if namespace == chataigne_alchemist::authoring::FORMULA_OPERATION_NAMESPACE {
            super::systems_alchemist_formula::apply_formula_operation(engine, operation, payload)
        } else if namespace == chataigne_state_machine::authoring::STATE_GRAPH_NAMESPACE {
            super::systems_state_machine_state::apply_graph_operation(engine, operation, payload)
        } else if namespace == chataigne_spatializer_protocol::SPATIALIZER_NAMESPACE {
            super::SpatializerModule::apply_authoring_operation(engine, operation, payload)
        } else {
            Err(format!("unsupported app operation '{namespace}.{operation}'"))
        }
    }

    fn initialize_new_project(engine: &mut AppEngine) -> Result<(), String> {
        super::default_project::initialize_default_project(engine);
        Ok(())
    }

    fn project_opened(engine: &mut AppEngine) -> Result<(), String> {
        super::systems_alchemist_formula::migrate_legacy_gate_semantics(engine)?;
        super::systems_alchemist_formula::migrate_output_binding_documents(engine)?;
        super::systems_alchemist_processor::migrate_mapping_output_adapters(engine)?;
        super::systems_alchemist_processor::sync_external_formulas(engine)?;
        super::systems_alchemist_processor::settle_pending_mapping_transitions(engine)
    }

    fn settle_pending_operations(engine: &mut AppEngine) -> Result<(), String> {
        super::systems_alchemist_processor::settle_pending_mapping_transitions(engine)
    }
}
