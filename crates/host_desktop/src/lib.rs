//! Default Tauri desktop and headless host runtime for Golden applications.

#![warn(missing_docs)]

mod desktop;
mod desktop_commands;

pub use desktop::{
    LaunchArgs, launch_engine_with_args, launch_with_args, parse_launch_args, parse_launch_args_from_env, run_default,
};
