use golden_core::{
    app::{
        DesktopHostConfig, LaunchArgs, NativePresentation, ProjectLifecycle, launch_engine_with_args,
        launch_engine_with_config, launch_engine_with_native_presentation,
    },
    engine::Engine,
};
use tauri::Runtime;

#[allow(dead_code)]
fn launch_through_public_default_host<T, R>(
    engine: Engine<T>,
    args: LaunchArgs,
    context: tauri::Context<R>,
) -> std::io::Result<()>
where
    T: ProjectLifecycle + 'static,
    R: Runtime,
{
    launch_engine_with_args(engine, args, context)
}

#[allow(dead_code)]
fn launch_app_configured_public_host<T, R>(
    engine: Engine<T>,
    args: LaunchArgs,
    context: tauri::Context<R>,
) -> std::io::Result<()>
where
    T: ProjectLifecycle + 'static,
    R: Runtime,
{
    let mut config = DesktopHostConfig::default();
    config.ui_server.bind_addr = "127.0.0.1:7011".into();
    launch_engine_with_config(engine, args, context, config)
}

#[allow(dead_code)]
fn launch_with_public_native_presentation<T, R>(
    engine: Engine<T>,
    args: LaunchArgs,
    context: tauri::Context<R>,
    presentation: NativePresentation<R>,
) -> std::io::Result<()>
where
    T: ProjectLifecycle + 'static,
    R: Runtime,
{
    launch_engine_with_native_presentation(engine, args, context, DesktopHostConfig::default(), presentation)
}

#[test]
fn default_full_host_launch_contract_is_public() {
    let args = golden_core::app::parse_launch_args(["--headless", "--no-remote"])
        .expect("public default-host arguments should parse");

    assert!(args.headless);
    assert!(args.no_remote);
}
