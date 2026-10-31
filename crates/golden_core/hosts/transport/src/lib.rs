//! Built-in HTTP and WebSocket transport host for Golden runtimes.

#![warn(missing_docs)]

mod project_host;
mod ui_server;

pub use ui_server::{
    UiAsset, UiPreferencesConfig, UiReadinessDto, UiServerConfig, UiServerShutdown, run_ui_server,
    run_with_ui_server_config, run_with_ui_server_config_and_ready,
};
