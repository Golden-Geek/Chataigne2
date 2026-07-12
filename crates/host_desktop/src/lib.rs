//! Default Tauri desktop and headless host runtime for Golden applications.

#![warn(missing_docs)]

mod desktop;
mod desktop_commands;
#[cfg(test)]
mod desktop_tests;
mod window_state;
#[cfg(test)]
mod window_state_tests;
#[cfg(target_os = "windows")]
mod windows_process_job;

pub use desktop::{
    FrontendDevServerConfig, LaunchArgs, launch_engine_with_args, launch_engine_with_ui_assets,
    launch_engine_with_ui_assets_and_dev_server, launch_with_args, launch_with_ui_assets,
    launch_with_ui_assets_and_dev_server, parse_launch_args, parse_launch_args_from_env, run_default,
    run_default_with_ui_assets, run_default_with_ui_assets_and_dev_server,
};
