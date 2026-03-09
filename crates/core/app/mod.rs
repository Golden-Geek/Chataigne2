//! Runtime project persistence and UI-server integration.

use std::io::{Error, ErrorKind};
use std::sync::{Arc, Mutex};

use crate::engine::{Engine, EngineRuntimeError};
use crate::node::{Node, NodeMeta};

mod ui_server;

pub use ui_server::{UiServerConfig, run_ui_server};

/// App node contract required by the default runtime host.
pub trait ProjectNode: Node {
    /// Recreates one node from project persistence payload.
    fn project_decode_node(node_type: &str, data: &serde_json::Value, meta: &NodeMeta) -> Result<Self, String>
    where
        Self: Sized;
}

/// App-level runtime wrapper around a configured engine.
///
/// This is the intermediate layer where app concerns can grow later while the
/// engine remains responsible for runtime ticking/scheduling.
pub struct GoldenApp<T: Node> {
    engine: Engine<T>,
}

impl<T: Node> GoldenApp<T> {
    /// Creates an app wrapper around an engine instance.
    pub fn new(engine: Engine<T>) -> Self {
        Self { engine }
    }

    /// Returns a shared reference to the wrapped engine.
    pub fn engine(&self) -> &Engine<T> {
        &self.engine
    }

    /// Returns a mutable reference to the wrapped engine.
    pub fn engine_mut(&mut self) -> &mut Engine<T> {
        &mut self.engine
    }

    /// Consumes the wrapper and returns the wrapped engine.
    pub fn into_engine(self) -> Engine<T> {
        self.engine
    }

    /// Applies bootstrap edits, resolves scheduling, then enters the engine loop.
    pub fn run(mut self) -> Result<(), EngineRuntimeError> {
        self.engine.apply_edits()?;
        self.engine.resolve_if_needed()?;
        self.engine.clear_history();
        self.engine.run_loop()
    }
}

/// Boots an engine and starts the UI/API runtime with explicit server config.
pub fn run_with_ui_server_config<T: ProjectNode + 'static>(mut engine: Engine<T>, config: UiServerConfig) -> std::io::Result<()> {
    engine.apply_edits().map_err(|err| Error::new(ErrorKind::Other, format!("initial apply_edits failed: {err}")))?;
    engine.resolve_if_needed().map_err(|err| Error::new(ErrorKind::Other, format!("initial resolve failed: {err}")))?;
    // Startup shape is already reflected by the in-memory graph and initial snapshot.
    // Dropping bootstrap inbox events avoids a very expensive first runtime tick for large graphs.
    engine.inbox.clear();
    engine.clear_history(); // keep runtime undo history strictly post-start

    let shared_engine = Arc::new(Mutex::new(engine));
    run_ui_server(shared_engine, config)
}
