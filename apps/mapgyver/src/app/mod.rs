use golden_core::app::{DesktopHostConfig, FrontendDevServerConfig, ProjectFileSpec, ProjectLifecycle, UiServerConfig};

pub(crate) mod labels;

golden_core::define_node_enum!(
    pub enum AppNode {
        MediaManager(crate::media::MediaManagerNode),
        MediaFolder(crate::media::MediaFolderNode),
        Media(crate::media::MediaNode),
        ReferenceMedia(crate::media::ReferenceNode),
        MediaChannel(crate::media::ChannelNode),
        MediaCommand(crate::media::MediaCommandNode),
        MediaAsset(crate::media::AssetNode),
        Application(crate::applications::ApplicationNode),
        CompositionPlacement(crate::media::composition::PlacementNode),
        MatrixElement(crate::matrix::ElementNode),
        MatrixCell(crate::matrix::CellNode),
        ScreenManager(crate::screens::ScreenManagerNode),
        Screen(crate::screens::ScreenNode),
        Surface(crate::screens::SurfaceNode),
    }
);

pub type AppEngine = golden_core::engine::Engine<AppNode>;

include!(concat!(env!("OUT_DIR"), "/ui_assets.rs"));

impl ProjectLifecycle for AppNode {
    fn validate_project_file(project: &golden_core::engine::ProjectFile) -> Result<(), String> {
        crate::media::validate_project_file(project)
    }

    fn project_lifecycle_limits() -> golden_core::engine::LifecycleLimits {
        // The admitted 100k-instance document now has real declared parameter children.
        // Bound generated expansion explicitly; keep Golden's shared runaway-pass limit.
        golden_core::engine::LifecycleLimits {
            max_work: std::num::NonZeroUsize::new(4_194_304).unwrap(),
            ..Default::default()
        }
    }

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
        crate::screens::initialize(engine);
        Ok(())
    }

    fn query_app_operation(
        engine: &AppEngine,
        namespace: &str,
        operation: &str,
        payload: &serde_json::Value,
    ) -> Option<Result<serde_json::Value, String>> {
        if namespace == crate::media::protocol::MEDIA_NAMESPACE {
            crate::media::query_operation(engine, operation, payload)
        } else {
            None
        }
    }

    fn apply_app_operation(
        engine: &mut AppEngine,
        namespace: &str,
        operation: &str,
        payload: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        if namespace == crate::media::protocol::MEDIA_NAMESPACE {
            crate::media::apply_operation(engine, operation, payload)
        } else if namespace == crate::screens::protocol::SCREEN_NAMESPACE {
            crate::screens::apply_operation(engine, operation, payload)
        } else {
            Err(format!("unsupported app operation '{namespace}.{operation}'"))
        }
    }

    fn prepare_project_candidate(engine: &mut AppEngine) -> Result<(), String> {
        crate::media::structure::classify_authored_items(engine);
        crate::media::validate_project(engine)?;
        crate::screens::validate_project(engine)
    }

    fn validate_ui_intent(engine: &AppEngine, intent: &golden_core::ui_sync::UiEditIntent) -> Result<(), String> {
        crate::media::structure::validate_ui_intent(engine, intent)?;
        crate::matrix::validate_ui_intent(engine, intent)?;
        crate::applications::validate_ui_intent(engine, intent)
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
    let services = golden_core::app::HostServices::default()
        .with_headless(crate::applications::headless::HeadlessApplications::start);
    #[cfg(all(windows, feature = "native-video"))]
    let services = services.with_native_presentation(crate::preview::host::presentation);
    golden_core::app::run_default_with_services::<AppNode, tauri::Wry>(tauri::generate_context!(), config, services)
}

#[cfg(test)]
mod tests;
