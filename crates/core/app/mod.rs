//! App-level integration points (windowing, args parsing, host runtime glue).

use std::io::{Error, ErrorKind};
use std::sync::{Arc, Mutex};

use crate::engine::{Engine, EngineRuntimeError};
use crate::node::Node;

mod ui_server;

pub use ui_server::{UiServerConfig, run_ui_server};

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
        self.engine.run_loop()
    }
}

/// Boots an engine and starts the default UI/API runtime.
///
/// This is the canonical app entry point for hosts using `golden_core`.
/// It applies pending bootstrap edits, resolves runtime scheduling, then
/// serves the UI API using default settings (optionally overridden by env).
pub fn run_app<T: Node + 'static>(mut engine: Engine<T>) -> std::io::Result<()> {
    engine
        .apply_edits()
        .map_err(|err| Error::new(ErrorKind::Other, format!("initial apply_edits failed: {err}")))?;
    engine
        .resolve_if_needed()
        .map_err(|err| Error::new(ErrorKind::Other, format!("initial resolve failed: {err}")))?;

    let mut config = UiServerConfig::default();
    if let Ok(bind_addr) = std::env::var("GC_UI_BIND") {
        if !bind_addr.trim().is_empty() {
            config.bind_addr = bind_addr;
        }
    }

    run_app_with_config(engine, config)
}

/// Boots an engine and starts the UI/API runtime with explicit server config.
pub fn run_app_with_config<T: Node + 'static>(
    engine: Engine<T>,
    config: UiServerConfig,
) -> std::io::Result<()> {
    let shared_engine = Arc::new(Mutex::new(engine));
    run_ui_server(shared_engine, config)
}
