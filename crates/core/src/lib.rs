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

#[path = "../app/mod.rs"]
/// App-level runtime helpers.
pub mod app;

/// Attribute macros used to declare node types and runtime update rates.
pub use golden_core_macros::{node, update};
