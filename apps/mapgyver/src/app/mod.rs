use golden_core::app::{DesktopHostConfig, FrontendDevServerConfig, ProjectFileSpec, ProjectLifecycle, UiServerConfig};

pub(crate) mod labels;

golden_core::define_node_enum!(
    pub enum AppNode {
        MediaManager(crate::media::MediaManagerNode),
        MediaFolder(crate::media::MediaFolderNode),
        Media(crate::media::MediaNode),
        ReferenceMedia(crate::media::ReferenceNode),
        ReferenceOutput(crate::media::ReferenceOutputNode),
        MediaChannel(crate::media::ChannelNode),
        NdiAudioRoute(crate::audio::NdiAudioRouteNode),
        MediaCommand(crate::media::MediaCommandNode),
        MediaAsset(crate::media::AssetNode),
        Application(crate::applications::AppControlNode),
        ApplicationArgument(crate::applications::ArgumentNode),
        ApplicationEnvironment(crate::applications::EnvironmentNode),
        CompositionPlacement(crate::media::composition::PlacementNode),
        MatrixElement(crate::matrix::ElementNode),
        MatrixColumn(crate::matrix::ColumnNode),
        MatrixColumnGroup(crate::matrix::ColumnGroupNode),
        MatrixCell(crate::matrix::CellNode),
        MatrixPresets(crate::matrix::presets::PresetCollectionNode),
        MatrixPreset(crate::matrix::presets::PresetNode),
        MatrixPresetEntry(crate::matrix::presets::PresetEntryNode),
        ScreenManager(crate::screens::ScreenManagerNode),
        Screen(crate::screens::ScreenNode),
        Surface(crate::screens::SurfaceNode),
        VideoPreferences(crate::preferences::VideoPreferencesNode),
        AudioPreferences(crate::audio::AudioPreferencesNode),
        AudioOutput(crate::audio::OutputNode),
    }
);

pub type AppEngine = golden_core::engine::Engine<AppNode>;
pub(crate) const DISPLAY_NAME: &str = "MapGyver 2";

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
        DISPLAY_NAME
    }

    fn project_file_spec() -> ProjectFileSpec {
        ProjectFileSpec::new("MapGyver 2 Project", "mapgyver")
    }

    fn app_data_directory_name() -> &'static str {
        "MapGyver"
    }

    fn oscquery_default_port() -> u16 {
        43000
    }

    fn preferences_tree(default_data_folder: String) -> golden_core::edit::NodeTree {
        crate::preferences::tree(default_data_folder)
    }

    fn project_settings_tree() -> Option<golden_core::edit::NodeTree> {
        Some(golden_core::app::default_project_settings_tree())
    }

    fn initialize_new_project(engine: &mut AppEngine) -> Result<(), String> {
        crate::media::initialize(engine);
        crate::screens::initialize(engine);
        Ok(())
    }

    fn project_opened(engine: &mut AppEngine) -> Result<(), String> {
        // New candidates may still hold their initialization forest as queued edits.
        engine.apply_edits().map_err(|error| error.to_string())?;
        crate::screens::host_inventory::prepare(engine)
    }

    fn query_app_operation(
        engine: &AppEngine,
        namespace: &str,
        operation: &str,
        payload: &serde_json::Value,
    ) -> Option<Result<serde_json::Value, String>> {
        if namespace == crate::media::protocol::MEDIA_NAMESPACE {
            crate::media::query_operation(engine, operation, payload)
        } else if namespace == crate::screens::protocol::SCREEN_NAMESPACE {
            crate::screens::query_operation(engine, operation, payload)
        } else if namespace == crate::audio::NAMESPACE && operation == "inspect-output" {
            Some(crate::audio::inspect(engine))
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
        } else if namespace == crate::matrix::controls::NAMESPACE {
            crate::matrix::controls::apply(engine, operation, payload)
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
    #[cfg(any(
        all(windows, feature = "native-video"),
        target_os = "macos",
        feature = "wgpu-preview"
    ))]
    let browser_previews = golden_core::transport::BrowserPreviews::default();
    #[cfg(all(
        not(all(windows, feature = "native-video")),
        any(target_os = "macos", feature = "wgpu-preview")
    ))]
    let headless_previews = browser_previews.clone();
    let config = DesktopHostConfig {
        ui_server: UiServerConfig {
            bind_addr: "127.0.0.1:7011".to_owned(),
            frontend_assets: APP_UI_ASSETS,
            #[cfg(any(
                all(windows, feature = "native-video"),
                target_os = "macos",
                feature = "wgpu-preview"
            ))]
            browser_previews: Some(browser_previews.clone()),
            ..UiServerConfig::default()
        },
        dev_server: Some(FrontendDevServerConfig {
            working_dir: concat!(env!("CARGO_MANIFEST_DIR"), "/ui"),
            npm_script: "dev",
            url: "http://127.0.0.1:5174",
        }),
    };
    let services = golden_core::app::HostServices::default()
        .with_desktop_plugin(crate::media::shader::library::plugin())
        .with_headless(move |runtime| {
            #[cfg(feature = "video-files")]
            let videos = crate::media::video::metadata::Service::start(runtime.clone())?;
            #[cfg(all(
                not(all(windows, feature = "native-video")),
                any(target_os = "macos", feature = "wgpu-preview")
            ))]
            let previews = crate::preview::browser::start(runtime.clone(), headless_previews)?;
            let applications = crate::applications::headless::HeadlessApplications::start(runtime)?;
            #[cfg(feature = "video-files")]
            let services = (applications, videos);
            #[cfg(not(feature = "video-files"))]
            let services = applications;
            #[cfg(all(
                not(all(windows, feature = "native-video")),
                any(target_os = "macos", feature = "wgpu-preview")
            ))]
            let services = (services, previews);
            Ok(services)
        });
    #[cfg(all(windows, feature = "native-video"))]
    let services = services.with_background(move |runtime| {
        let audio = crate::audio::start(runtime.clone())?;
        let previews = crate::preview::browser::start(runtime, browser_previews)?;
        Ok((audio, previews))
    });
    #[cfg(not(all(windows, feature = "native-video")))]
    let services = services.with_background(crate::audio::start);
    #[cfg(all(windows, feature = "native-video"))]
    let services = services.with_native_presentation(crate::preview::host::presentation);
    #[cfg(all(
        not(all(windows, feature = "native-video")),
        any(target_os = "macos", feature = "wgpu-preview")
    ))]
    let services = services
        .with_native_presentation(move |runtime| Ok(crate::preview::browser::presentation(runtime, browser_previews)));
    golden_core::app::run_default_with_services::<AppNode, tauri::Wry>(tauri::generate_context!(), config, services)
}

#[cfg(test)]
mod tests;
