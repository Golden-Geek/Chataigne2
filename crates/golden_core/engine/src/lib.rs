//! Core runtime primitives for the Golden node engine.
//!
//! This crate exposes:
//! - node modeling (`node`, `parameter`)
//! - event and edit transport (`events`, `edit`)
//! - runtime processing context and engine (`process_ctx`, `engine`)
#![warn(missing_docs)]

extern crate self as golden_core;

/// Basic color schema types.
pub mod color;

/// Engine data model and edit-application entry points.
pub mod engine;

/// Node identity, metadata, and trait implementations.
pub mod node;

/// Declarative helper macros for defining nodes and node enums.
pub mod node_macros;

/// Parameter node implementations and value conversions.
pub mod parameter;

/// Event models and event inbox utilities.
pub mod events;

/// Edit request models queued by the engine.
pub mod edit;

/// Processing-time context passed to node callbacks.
pub mod process_ctx;

/// Blueprint declaration and instance registry APIs.
pub mod blueprints;

/// UserContext and DynamicContext APIs.
pub mod contexts;

/// Process-wide logger API and `log!` macro support.
pub mod logger;

/// App lifecycle hooks and host-independent runtime preparation.
pub mod app;

/// Stable application-facing facade contracts and the production engine adapter.
pub mod application;

mod runtime_center;

#[cfg(test)]
mod tests;

/// UI sync DTOs and transport-agnostic protocol helpers.
pub mod ui_sync;

/// Immutable UI read projection used by host read paths.
pub mod ui_read_model;

/// Scripting schemas and runtime integrations.
pub mod script;

/// Attribute macros used to declare node types, item kinds, and runtime update rates.
pub use golden_core_macros::{item, node, update};
