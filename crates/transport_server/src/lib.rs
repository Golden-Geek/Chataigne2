//! Built-in HTTP and WebSocket transport host for Golden runtimes.

#![warn(missing_docs)]

mod project_host;
mod ui_server;

pub use ui_server::{
    UiAsset, UiPreferencesConfig, UiReadinessDto, UiServerConfig, run_ui_server, run_with_ui_server_config,
};
