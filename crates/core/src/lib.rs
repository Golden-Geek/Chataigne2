//! Core runtime primitives for the Golden node engine.
//!
//! This crate exposes:
//! - node modeling (`node`, `parameter`)
//! - event and edit transport (`events`, `edit`)
//! - runtime processing context and engine (`process_ctx`, `engine`)
#![warn(missing_docs)]

extern crate self as golden_core;

#[path = "../schema/color.rs"]
/// Basic color schema types.
pub mod color;

#[path = "../engine/engine.rs"]
/// Engine data model and edit-application entry points.
pub mod engine;

#[path = "../node/node.rs"]
/// Node identity, metadata, and trait implementations.
pub mod node;

#[path = "../node/node_macros.rs"]
/// Declarative helper macros for defining nodes and node enums.
pub mod node_macros;

#[path = "../node/parameter.rs"]
/// Parameter node implementations and value conversions.
pub mod parameter;

#[path = "../events/events.rs"]
/// Event models and event inbox utilities.
pub mod events;

#[path = "../engine/edit.rs"]
/// Edit request models queued by the engine.
pub mod edit;

#[path = "../engine/process_ctx.rs"]
/// Processing-time context passed to node callbacks.
pub mod process_ctx;

#[path = "../engine/blueprints.rs"]
/// Blueprint declaration and instance registry APIs.
pub mod blueprints;

#[path = "../engine/contexts.rs"]
/// UserContext and DynamicContext APIs.
pub mod contexts;

#[path = "../engine/animation_curve.rs"]
/// Animation curve model and high-performance samplers.
pub mod animation_curve;

#[path = "../engine/logger.rs"]
/// Process-wide logger API and `log!` macro support.
pub mod logger;

#[path = "../app/mod.rs"]
/// App-level runtime helpers.
pub mod app;

#[path = "../ui/ui_sync.rs"]
/// UI sync DTOs and transport-agnostic protocol helpers.
pub mod ui_sync;

#[path = "../script/script.rs"]
/// Scripting schemas and runtime integrations.
pub mod script;

/// Attribute macros used to declare node types, item kinds, and runtime update rates.
pub use golden_core_macros::{item, node, update};

/// Canonical host entrypoint: boot a configured engine and run app services.
pub use app::run_app;
