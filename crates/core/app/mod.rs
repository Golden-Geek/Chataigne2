//! App-level integration points (windowing, args parsing, host runtime glue).

use std::io::{Error, ErrorKind};
use std::sync::{Arc, Mutex};

use crate::engine::{Engine, EngineRuntimeError};
use crate::node::{Node, NodeMeta};

mod desktop;
mod ui_server;

pub use desktop::{run_app, run_app_with_project_codec};
pub use ui_server::{UiServerConfig, run_ui_server};

/// Project-save callback signature.
pub type ProjectEncodeFn<T> = dyn Fn(&T) -> Result<serde_json::Value, String> + Send + Sync + 'static;
/// Project-load callback signature.
pub type ProjectDecodeFn<T> = dyn Fn(&str, &serde_json::Value, &NodeMeta) -> Result<T, String> + Send + Sync + 'static;

/// Runtime project persistence codec for one app node type.
pub struct ProjectCodec<T: Node> {
    encode: Arc<ProjectEncodeFn<T>>,
    decode: Arc<ProjectDecodeFn<T>>,
}

impl<T: Node> Clone for ProjectCodec<T> {
    fn clone(&self) -> Self {
        Self {
            encode: self.encode.clone(),
            decode: self.decode.clone(),
        }
    }
}

impl<T: Node> ProjectCodec<T> {
    /// Creates a project codec from encode/decode callbacks.
    pub fn new<Encode, Decode>(encode: Encode, decode: Decode) -> Self
    where
        Encode: Fn(&T) -> Result<serde_json::Value, String> + Send + Sync + 'static,
        Decode: Fn(&str, &serde_json::Value, &NodeMeta) -> Result<T, String> + Send + Sync + 'static,
    {
        Self { encode: Arc::new(encode), decode: Arc::new(decode) }
    }

    pub(crate) fn encode_node(&self, node: &T) -> Result<serde_json::Value, String> {
        (self.encode)(node)
    }

    pub(crate) fn decode_node(&self, node_type: &str, data: &serde_json::Value, meta: &NodeMeta) -> Result<T, String> {
        (self.decode)(node_type, data, meta)
    }
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
pub fn run_app_with_config<T: Node + 'static>(mut engine: Engine<T>, config: UiServerConfig, project_codec: Option<ProjectCodec<T>>) -> std::io::Result<()> {
    engine.apply_edits().map_err(|err| Error::new(ErrorKind::Other, format!("initial apply_edits failed: {err}")))?;
    engine.resolve_if_needed().map_err(|err| Error::new(ErrorKind::Other, format!("initial resolve failed: {err}")))?;
    // Startup shape is already reflected by the in-memory graph and initial snapshot.
    // Dropping bootstrap inbox events avoids a very expensive first runtime tick for large graphs.
    engine.inbox.clear();
    engine.clear_history(); // keep runtime undo history strictly post-start

    let shared_engine = Arc::new(Mutex::new(engine));
    run_ui_server(shared_engine, config, project_codec)
}
