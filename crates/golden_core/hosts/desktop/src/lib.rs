//! Default Tauri desktop and headless host runtime for Golden applications.

#![warn(missing_docs)]

mod desktop;
mod desktop_commands;
mod desktop_window;
mod host_config;
mod native_presentation;
mod output_window;
#[cfg(test)]
mod tests;
mod window_state;
#[cfg(target_os = "windows")]
mod windows_process_job;

pub use desktop::{
    LaunchArgs, launch_engine_with_args, launch_engine_with_config, launch_engine_with_native_presentation,
    launch_engine_with_ui_assets, launch_engine_with_ui_assets_and_dev_server, launch_with_args, launch_with_config,
    launch_with_ui_assets, launch_with_ui_assets_and_dev_server, parse_launch_args, parse_launch_args_from_env,
    run_default, run_default_with_config, run_default_with_native_presentation, run_default_with_ui_assets,
    run_default_with_ui_assets_and_dev_server,
};
pub use host_config::{DesktopHostConfig, FrontendDevServerConfig};
pub use native_presentation::{
    NativePresentation, NativePresentationStop, NativePresentationTask, NativePresentationViewport,
};
pub use output_window::{NativeOutputBounds, NativeOutputWindow};
