//! Facade crate that exposes the default Golden runtime stack through stable public boundaries.

#![warn(missing_docs)]

extern crate self as golden_core;

pub use golden_core_macros::{item, node, update};
pub use golden_engine::{define_node_enum, define_user_item_factory_methods};
pub use golden_engine::{log, logerror, logsuccess, logwarning};

/// Basic color schema types.
pub mod color {
    pub use golden_engine::color::*;
}

/// Engine data model and edit-application entry points.
pub mod engine {
    pub use golden_engine::engine::*;
}

/// Node identity, metadata, and trait implementations.
pub mod node {
    pub use golden_engine::node::*;
}

/// Declarative helper macros for defining nodes and node enums.
pub use golden_engine::node_macros;

/// Parameter node implementations and value conversions.
pub mod parameter {
    pub use golden_engine::parameter::*;
}

/// Event models and event inbox utilities.
pub mod events {
    pub use golden_engine::events::*;
}

/// Edit request models queued by the engine.
pub mod edit {
    pub use golden_engine::edit::*;
}

/// Processing-time context passed to node callbacks.
pub mod process_ctx {
    pub use golden_engine::process_ctx::*;
}

/// Blueprint declaration and instance registry APIs.
pub mod blueprints {
    pub use golden_engine::blueprints::*;
}

/// UserContext and DynamicContext APIs.
pub mod contexts {
    pub use golden_engine::contexts::*;
}

/// Curve model and samplers.
pub mod curve {
    pub use golden_engine::node::curve::*;
}

/// Process-wide logger API and `log!` macro support.
pub mod logger {
    pub use golden_engine::logger::*;
}

/// Host-independent app lifecycle hooks plus the default host/runtime entry points.
pub mod app {
    pub use golden_engine::app::{
        GoldenApp, ProjectFileSpec, ProjectLifecycle, ProjectNode, add_default_project_nodes, configure_loaded_engine,
        create_engine, create_new_project_engine, from_sparse_project_json, load_sparse_project_file,
        prepare_engine_for_runtime, save_sparse_project_file, shutdown_engine_for_runtime,
        to_sparse_project_json_pretty,
    };
    pub use golden_host_desktop::{
        FrontendDevServerConfig, LaunchArgs, launch_engine_with_args, launch_engine_with_ui_assets,
        launch_engine_with_ui_assets_and_dev_server, launch_with_args, launch_with_ui_assets,
        launch_with_ui_assets_and_dev_server, parse_launch_args, parse_launch_args_from_env, run_default,
        run_default_with_ui_assets, run_default_with_ui_assets_and_dev_server,
    };
    pub use golden_transport_server::{UiAsset, UiServerConfig, run_ui_server, run_with_ui_server_config};
}

/// UI protocol DTOs and transport-oriented helpers.
pub mod ui_sync {
    pub use golden_protocol::*;
}

/// Incremental immutable projection used by UI transports and reloads.
pub mod ui_read_model {
    pub use golden_engine::ui_read_model::*;
}

/// Script authoring and runtime integrations.
pub mod script {
    pub use golden_script::*;
}

/// Persistence DTOs and codec error types.
pub mod persistence {
    pub use golden_persistence::*;
}

/// Transport server host helpers.
pub mod transport {
    pub use golden_transport_server::*;
}

/// Desktop host helpers.
pub mod host {
    pub use golden_host_desktop::*;
}

/// Launches an app node type through the default reusable host runtime.
#[macro_export]
macro_rules! run_default_app {
    ($node_ty:ty) => {
        $crate::app::run_default::<$node_ty, tauri::Wry>(tauri::generate_context!())
    };
}
