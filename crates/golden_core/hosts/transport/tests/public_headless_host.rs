use golden_engine::{app::ProjectLifecycle, engine::Engine};
use golden_transport_server::{UiServerConfig, UiServerShutdown, run_with_ui_server_config};

#[allow(dead_code)]
fn launch_through_public_headless_host<T>(engine: Engine<T>, config: UiServerConfig) -> std::io::Result<()>
where
    T: ProjectLifecycle + 'static,
{
    run_with_ui_server_config(engine, config)
}

#[test]
fn headless_host_configuration_is_public_without_desktop_dependencies() {
    let config = UiServerConfig::default();

    assert!(!config.bind_addr.is_empty());
    assert!(config.frontend_assets.is_empty());
    let shutdown = UiServerShutdown::default();
    let config = UiServerConfig {
        shutdown: Some(shutdown.clone()),
        ..config
    };
    shutdown.request();
    assert!(config.shutdown.unwrap().is_requested());
}
