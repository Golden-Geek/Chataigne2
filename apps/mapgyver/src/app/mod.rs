use golden_core::app::{DesktopHostConfig, FrontendDevServerConfig, ProjectFileSpec, ProjectLifecycle, UiServerConfig};

golden_core::define_node_enum!(
    pub enum AppNode {
        MediaManager(crate::media::MediaManagerNode),
        Media(crate::media::MediaNode),
        MediaChannel(crate::media::ChannelNode),
    }
);

pub type AppEngine = golden_core::engine::Engine<AppNode>;

include!(concat!(env!("OUT_DIR"), "/ui_assets.rs"));

impl ProjectLifecycle for AppNode {
    fn application_id() -> &'static str {
        "mapgyver"
    }

    fn application_version() -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn application_display_name() -> &'static str {
        "MapGyver"
    }

    fn project_file_spec() -> ProjectFileSpec {
        ProjectFileSpec::new("MapGyver Project", "mapgyver")
    }

    fn app_data_directory_name() -> &'static str {
        "MapGyver"
    }

    fn initialize_new_project(engine: &mut AppEngine) -> Result<(), String> {
        crate::media::initialize(engine);
        Ok(())
    }

    fn apply_app_operation(
        engine: &mut AppEngine,
        namespace: &str,
        operation: &str,
        payload: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        if namespace == crate::media::protocol::MEDIA_NAMESPACE {
            crate::media::apply_operation(engine, operation, payload)
        } else {
            Err(format!("unsupported app operation '{namespace}.{operation}'"))
        }
    }

    fn project_opened(engine: &mut AppEngine) -> Result<(), String> {
        crate::media::validate_project(engine)
    }
    fn prepare_project_candidate(engine: &mut AppEngine) -> Result<(), String> {
        crate::media::validate_project(engine)
    }
}

pub fn run() -> std::io::Result<()> {
    let config = DesktopHostConfig {
        ui_server: UiServerConfig {
            bind_addr: "127.0.0.1:7011".to_owned(),
            frontend_assets: APP_UI_ASSETS,
            ..UiServerConfig::default()
        },
        dev_server: Some(FrontendDevServerConfig {
            working_dir: concat!(env!("CARGO_MANIFEST_DIR"), "/ui"),
            npm_script: "dev",
            url: "http://127.0.0.1:5174",
        }),
    };
    golden_core::app::run_default_with_config::<AppNode, tauri::Wry>(tauri::generate_context!(), config)
}

#[cfg(test)]
mod tests;
