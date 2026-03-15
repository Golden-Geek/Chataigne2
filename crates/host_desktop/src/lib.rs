//! Default Tauri desktop and headless host runtime for Golden applications.

#![warn(missing_docs)]

mod desktop;
mod desktop_commands;

pub use desktop::{
    FrontendDevServerConfig, LaunchArgs, launch_engine_with_args, launch_engine_with_ui_assets,
    launch_engine_with_ui_assets_and_dev_server, launch_with_args, launch_with_ui_assets,
    launch_with_ui_assets_and_dev_server, parse_launch_args, parse_launch_args_from_env, run_default,
    run_default_with_ui_assets, run_default_with_ui_assets_and_dev_server,
};
