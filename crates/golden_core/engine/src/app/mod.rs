//! App lifecycle hooks and host-independent runtime preparation.

use std::io::Error;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::edit::{Edit, NodeTree};
use crate::engine::{
    Engine, EngineRuntimeError, NodeUpdateRate, ProjectLoadRecoveryReport, ProjectPersistenceError,
    runtime_loop_interval_for_frequency_hz,
};
use crate::node::{DashboardNode, DeclId, Folder, Node, NodeId};
use crate::parameter::{ParamValue, Parameter, ParameterChangeCheck};
use crate::process_ctx::{ExecutionPhase, ProcessCtx, ProcessTreeSnapshot};

pub use golden_model::ProjectFileSpec;

/// Declaration id of the app-wide Preferences folder under the project root.
pub const PREFERENCES_DECL_ID: &str = "preferences";
/// Metadata tag marking nodes that belong to app-data persistence, not project persistence.
pub const PREFERENCES_APP_DATA_TAG: &str = "golden.preferences.app_data";
/// Declaration id of the Startup and Update preferences category.
pub const PREFERENCES_STARTUP_AND_UPDATE_DECL_ID: &str = "startup_and_update";
/// Declaration id of the Save and Load preferences category.
pub const PREFERENCES_SAVE_AND_LOAD_DECL_ID: &str = "save_and_load";
/// Declaration id of the Engine preferences category.
pub const PREFERENCES_ENGINE_DECL_ID: &str = "engine";
/// Declaration id of the Interface preferences category.
pub const PREFERENCES_INTERFACE_DECL_ID: &str = "interface";
/// Declaration id of the app data folder file parameter.
pub const PREFERENCES_DATA_FOLDER_DECL_ID: &str = "data_folder";
/// Declaration id of the runtime loop max frequency preference.
pub const PREFERENCES_ENGINE_MAX_FREQUENCY_DECL_ID: &str = "engine_max_frequency";
/// Declaration id of the header warning threshold frequency preference.
pub const PREFERENCES_ENGINE_LOW_FREQUENCY_DECL_ID: &str = "engine_low_frequency";
/// Default runtime loop max frequency preference in hertz.
pub const DEFAULT_ENGINE_MAX_FREQUENCY_HZ: NodeUpdateRate = 200;
/// Default header warning threshold frequency preference in hertz.
pub const DEFAULT_ENGINE_LOW_FREQUENCY_HZ: NodeUpdateRate = 60;

/// Monotonic identity of one authoritative project installed in a production runtime.
///
/// This token is intentionally separate from document history revisions and compiled runtime
/// generation ids. A replacement changes it exactly once at the engine/read-model cutover.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProjectGeneration(u64);

impl ProjectGeneration {
    /// Generation assigned to the project used to create a production runtime.
    pub const INITIAL: Self = Self(1);

    /// Creates a generation from a runtime-owned monotonic counter.
    pub(crate) const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the numeric generation value.
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl Default for ProjectGeneration {
    fn default() -> Self {
        Self::INITIAL
    }
}

mod lifecycle;
pub use lifecycle::{ProjectLifecycle, ProjectNode};

/// Queues the core default nodes that every fresh project should start with.
pub fn add_default_project_nodes<T>(engine: &mut Engine<T>)
where
    T: Node + From<DashboardNode>,
{
    engine.add_node(DashboardNode::new().into(), None);
}

fn app_data_folder(label: &str, decl_id: &str) -> Folder {
    let mut folder = Folder::new(label);
    let meta = &mut folder.node_data_mut().meta;
    meta.decl_id = DeclId(decl_id.to_string());
    meta.can_be_disabled = false;
    meta.tags.push(PREFERENCES_APP_DATA_TAG.to_string());
    folder
}

fn preferences_data_folder_parameter(default_data_folder: impl Into<String>) -> Parameter {
    let mut parameter = Parameter::new(
        "Data Folder",
        ParamValue::File(default_data_folder.into()),
        ParameterChangeCheck::ValueChange,
    );
    let meta = &mut parameter.node_data_mut().meta;
    meta.decl_id = DeclId(PREFERENCES_DATA_FOLDER_DECL_ID.to_string());
    meta.tags.push(PREFERENCES_APP_DATA_TAG.to_string());
    parameter
}

fn preferences_engine_frequency_parameter(label: &str, decl_id: &str, default_hz: NodeUpdateRate) -> Parameter {
    let mut parameter = Parameter::new(
        label,
        ParamValue::Int(default_hz as i32),
        ParameterChangeCheck::ValueChange,
    );
    let meta = &mut parameter.node_data_mut().meta;
    meta.decl_id = DeclId(decl_id.to_string());
    meta.tags.push(PREFERENCES_APP_DATA_TAG.to_string());
    parameter
}

fn preferences_engine_folder_tree() -> NodeTree {
    let mut engine = NodeTree::new(app_data_folder("Engine", PREFERENCES_ENGINE_DECL_ID));
    engine.push_child(NodeTree::new(preferences_engine_frequency_parameter(
        "Engine Max Frequency",
        PREFERENCES_ENGINE_MAX_FREQUENCY_DECL_ID,
        DEFAULT_ENGINE_MAX_FREQUENCY_HZ,
    )));
    engine.push_child(NodeTree::new(preferences_engine_frequency_parameter(
        "Engine Low Frequency",
        PREFERENCES_ENGINE_LOW_FREQUENCY_DECL_ID,
        DEFAULT_ENGINE_LOW_FREQUENCY_HZ,
    )));
    engine
}

/// Builds the default app-wide Preferences tree.
///
/// Preferences are regular folder/parameter nodes so existing inspectors can edit them, but
/// the subtree is tagged for app-data persistence rather than project/session persistence.
pub fn default_preferences_tree(default_data_folder: impl Into<String>) -> NodeTree {
    let mut preferences = NodeTree::new(app_data_folder("Preferences", PREFERENCES_DECL_ID));
    preferences.push_child(NodeTree::new(app_data_folder(
        "Startup and Update",
        PREFERENCES_STARTUP_AND_UPDATE_DECL_ID,
    )));

    let mut save_and_load = NodeTree::new(app_data_folder("Save and Load", PREFERENCES_SAVE_AND_LOAD_DECL_ID));
    save_and_load.push_child(NodeTree::new(preferences_data_folder_parameter(default_data_folder)));
    preferences.push_child(save_and_load);

    preferences.push_child(preferences_engine_folder_tree());

    preferences.push_child(NodeTree::new(app_data_folder(
        "Interface",
        PREFERENCES_INTERFACE_DECL_ID,
    )));
    preferences
}

fn child_by_decl(snapshot: &ProcessTreeSnapshot, parent: NodeId, decl_id: &str) -> Option<NodeId> {
    snapshot.find_child_by_decl_id(parent, decl_id)
}

fn live_child_by_decl<T: Node>(engine: &Engine<T>, parent: NodeId, decl_id: &str) -> Option<NodeId> {
    let mut child = engine.nodes.get(parent)?.node_data().first_child;
    let mut remaining = engine.nodes.len();
    while let Some(child_id) = child {
        if remaining == 0 {
            return None;
        }
        remaining -= 1;

        let child_node = engine.nodes.get(child_id)?;
        let child_data = child_node.node_data();
        let child_decl_id = child_data.meta.decl_id.0.as_str();
        if child_decl_id == decl_id || child_decl_id.rsplit('/').next() == Some(decl_id) {
            return Some(child_id);
        }
        child = child_data.next_sibling;
    }
    None
}

/// Returns the live Preferences root without building a whole-tree snapshot.
pub(crate) fn live_preferences_root<T: Node>(engine: &Engine<T>) -> Option<NodeId> {
    live_child_by_decl(engine, engine.root, PREFERENCES_DECL_ID)
}

/// Returns the Preferences root id from a tree snapshot.
pub fn preferences_root_from_snapshot(snapshot: &ProcessTreeSnapshot) -> Option<NodeId> {
    child_by_decl(snapshot, snapshot.root(), PREFERENCES_DECL_ID)
}

/// Returns the app-wide Data Folder preference from a tree snapshot.
pub fn preferences_data_folder_from_snapshot(snapshot: &ProcessTreeSnapshot) -> Option<PathBuf> {
    let preferences = preferences_root_from_snapshot(snapshot)?;
    let save_and_load = child_by_decl(snapshot, preferences, PREFERENCES_SAVE_AND_LOAD_DECL_ID)?;
    let data_folder = child_by_decl(snapshot, save_and_load, PREFERENCES_DATA_FOLDER_DECL_ID)?;
    let value = snapshot.node(data_folder)?.param_value.as_ref()?;
    match value {
        ParamValue::File(path) | ParamValue::Str(path) => {
            let path = path.trim();
            (!path.is_empty()).then(|| PathBuf::from(path))
        }
        _ => None,
    }
}

/// Returns the app-wide Data Folder preference from a live engine.
pub fn preferences_data_folder<T: Node>(engine: &Engine<T>) -> Option<PathBuf> {
    let snapshot = engine.process_tree_snapshot();
    preferences_data_folder_from_snapshot(snapshot.as_ref())
}

fn frequency_from_param_value(value: &ParamValue) -> Option<NodeUpdateRate> {
    match value {
        ParamValue::Int(value) => NodeUpdateRate::try_from(*value).ok().filter(|value| *value > 0),
        ParamValue::Float(value) => {
            let value = value.round();
            (value >= 1.0 && value <= f64::from(NodeUpdateRate::MAX)).then_some(value as NodeUpdateRate)
        }
        ParamValue::Str(value) => value.trim().parse::<NodeUpdateRate>().ok().filter(|value| *value > 0),
        _ => None,
    }
}

fn preferences_engine_frequency_hz_from_snapshot(
    snapshot: &ProcessTreeSnapshot,
    decl_id: &str,
    default_hz: NodeUpdateRate,
) -> NodeUpdateRate {
    let Some(preferences) = preferences_root_from_snapshot(snapshot) else {
        return default_hz;
    };
    let Some(engine) = child_by_decl(snapshot, preferences, PREFERENCES_ENGINE_DECL_ID) else {
        return default_hz;
    };
    let Some(frequency) = child_by_decl(snapshot, engine, decl_id) else {
        return default_hz;
    };
    let Some(value) = snapshot.node(frequency).and_then(|node| node.param_value.as_ref()) else {
        return default_hz;
    };
    frequency_from_param_value(value).unwrap_or(default_hz)
}

fn preferences_engine_frequency_hz<T: Node>(
    engine: &Engine<T>,
    decl_id: &str,
    default_hz: NodeUpdateRate,
) -> NodeUpdateRate {
    let Some(preferences) = live_preferences_root(engine) else {
        return default_hz;
    };
    let Some(engine_preferences) = live_child_by_decl(engine, preferences, PREFERENCES_ENGINE_DECL_ID) else {
        return default_hz;
    };
    let Some(frequency) = live_child_by_decl(engine, engine_preferences, decl_id) else {
        return default_hz;
    };
    let Some(value) = engine
        .nodes
        .get(frequency)
        .and_then(Node::engine_param_snapshot)
        .map(|snapshot| snapshot.value)
    else {
        return default_hz;
    };
    frequency_from_param_value(&value).unwrap_or(default_hz)
}

/// Returns the Engine Max Frequency preference from a tree snapshot.
pub fn preferences_engine_max_frequency_hz_from_snapshot(snapshot: &ProcessTreeSnapshot) -> NodeUpdateRate {
    preferences_engine_frequency_hz_from_snapshot(
        snapshot,
        PREFERENCES_ENGINE_MAX_FREQUENCY_DECL_ID,
        DEFAULT_ENGINE_MAX_FREQUENCY_HZ,
    )
}

/// Returns the Engine Low Frequency preference from a tree snapshot.
pub fn preferences_engine_low_frequency_hz_from_snapshot(snapshot: &ProcessTreeSnapshot) -> NodeUpdateRate {
    preferences_engine_frequency_hz_from_snapshot(
        snapshot,
        PREFERENCES_ENGINE_LOW_FREQUENCY_DECL_ID,
        DEFAULT_ENGINE_LOW_FREQUENCY_HZ,
    )
}

/// Returns the Engine Max Frequency preference from a live engine.
pub fn preferences_engine_max_frequency_hz<T: Node>(engine: &Engine<T>) -> NodeUpdateRate {
    preferences_engine_frequency_hz(
        engine,
        PREFERENCES_ENGINE_MAX_FREQUENCY_DECL_ID,
        DEFAULT_ENGINE_MAX_FREQUENCY_HZ,
    )
}

/// Returns the Engine Low Frequency preference from a live engine.
pub fn preferences_engine_low_frequency_hz<T: Node>(engine: &Engine<T>) -> NodeUpdateRate {
    preferences_engine_frequency_hz(
        engine,
        PREFERENCES_ENGINE_LOW_FREQUENCY_DECL_ID,
        DEFAULT_ENGINE_LOW_FREQUENCY_HZ,
    )
}

/// Returns the runtime loop cap interval implied by the Engine Max Frequency preference.
pub fn preferences_engine_max_frequency_interval_from_snapshot(snapshot: &ProcessTreeSnapshot) -> Duration {
    runtime_loop_interval_for_frequency_hz(preferences_engine_max_frequency_hz_from_snapshot(snapshot))
}

/// Returns the runtime loop cap interval implied by the live Engine Max Frequency preference.
pub fn preferences_engine_max_frequency_interval<T: Node>(engine: &Engine<T>) -> Duration {
    runtime_loop_interval_for_frequency_hz(preferences_engine_max_frequency_hz(engine))
}

/// Applies app Preferences that affect runtime guardrails.
pub fn apply_preferences_runtime_limits<T: Node>(engine: &mut Engine<T>) {
    let max_loop_frequency_hz = preferences_engine_max_frequency_hz(engine);
    let mut limits = engine.runtime_limits();
    if limits.max_loop_frequency_hz != max_loop_frequency_hz {
        limits.max_loop_frequency_hz = max_loop_frequency_hz;
        engine.set_runtime_limits(limits);
    }
}

/// Queues any missing Preferences defaults into a live engine.
pub fn ensure_preferences_tree<T: Node>(engine: &mut Engine<T>, default_data_folder: impl Into<String>) {
    let default_data_folder = default_data_folder.into();
    let snapshot = engine.process_tree_snapshot();
    let Some(preferences) = preferences_root_from_snapshot(snapshot.as_ref()) else {
        engine.edits.push(Edit::AddNodeTree {
            tree: default_preferences_tree(default_data_folder),
            parent: engine.root,
            prev_sibling: None,
        });
        return;
    };

    if child_by_decl(snapshot.as_ref(), preferences, PREFERENCES_STARTUP_AND_UPDATE_DECL_ID).is_none() {
        engine.edits.push(Edit::AddNodeTree {
            tree: NodeTree::new(app_data_folder(
                "Startup and Update",
                PREFERENCES_STARTUP_AND_UPDATE_DECL_ID,
            )),
            parent: preferences,
            prev_sibling: None,
        });
    }

    let save_and_load = child_by_decl(snapshot.as_ref(), preferences, PREFERENCES_SAVE_AND_LOAD_DECL_ID);
    let save_and_load = match save_and_load {
        Some(id) => id,
        None => {
            let mut tree = NodeTree::new(app_data_folder("Save and Load", PREFERENCES_SAVE_AND_LOAD_DECL_ID));
            tree.push_child(NodeTree::new(preferences_data_folder_parameter(default_data_folder)));
            engine.edits.push(Edit::AddNodeTree {
                tree,
                parent: preferences,
                prev_sibling: None,
            });
            return;
        }
    };

    if child_by_decl(snapshot.as_ref(), save_and_load, PREFERENCES_DATA_FOLDER_DECL_ID).is_none() {
        engine.edits.push(Edit::AddNodeTree {
            tree: NodeTree::new(preferences_data_folder_parameter(default_data_folder)),
            parent: save_and_load,
            prev_sibling: None,
        });
    }

    if let Some(engine_folder) = child_by_decl(snapshot.as_ref(), preferences, PREFERENCES_ENGINE_DECL_ID) {
        if child_by_decl(
            snapshot.as_ref(),
            engine_folder,
            PREFERENCES_ENGINE_MAX_FREQUENCY_DECL_ID,
        )
        .is_none()
        {
            engine.edits.push(Edit::AddNodeTree {
                tree: NodeTree::new(preferences_engine_frequency_parameter(
                    "Engine Max Frequency",
                    PREFERENCES_ENGINE_MAX_FREQUENCY_DECL_ID,
                    DEFAULT_ENGINE_MAX_FREQUENCY_HZ,
                )),
                parent: engine_folder,
                prev_sibling: None,
            });
        }
        if child_by_decl(
            snapshot.as_ref(),
            engine_folder,
            PREFERENCES_ENGINE_LOW_FREQUENCY_DECL_ID,
        )
        .is_none()
        {
            engine.edits.push(Edit::AddNodeTree {
                tree: NodeTree::new(preferences_engine_frequency_parameter(
                    "Engine Low Frequency",
                    PREFERENCES_ENGINE_LOW_FREQUENCY_DECL_ID,
                    DEFAULT_ENGINE_LOW_FREQUENCY_HZ,
                )),
                parent: engine_folder,
                prev_sibling: None,
            });
        }
    } else {
        engine.edits.push(Edit::AddNodeTree {
            tree: preferences_engine_folder_tree(),
            parent: preferences,
            prev_sibling: None,
        });
    }

    if child_by_decl(snapshot.as_ref(), preferences, PREFERENCES_INTERFACE_DECL_ID).is_none() {
        engine.edits.push(Edit::AddNodeTree {
            tree: NodeTree::new(app_data_folder("Interface", PREFERENCES_INTERFACE_DECL_ID)),
            parent: preferences,
            prev_sibling: None,
        });
    }
}

/// Loads one persisted Preferences tree under the engine root.
pub fn insert_sparse_preferences_json<T>(
    engine: &mut Engine<T>,
    json: &str,
) -> Result<Option<NodeId>, ProjectPersistenceError>
where
    T: ProjectNode + From<Folder>,
{
    if json.trim().is_empty() {
        return Ok(None);
    }
    insert_sparse_subtree_json(engine, engine.root, None, json).map(Some)
}

/// Serializes the live Preferences tree for app-data persistence.
pub fn to_sparse_preferences_json_pretty<T>(engine: &Engine<T>) -> Result<Option<String>, ProjectPersistenceError>
where
    T: ProjectNode + From<Folder>,
{
    let snapshot = engine.process_tree_snapshot();
    let Some(preferences) = preferences_root_from_snapshot(snapshot.as_ref()) else {
        return Ok(None);
    };
    to_sparse_subtree_json_pretty(engine, preferences).map(Some)
}

fn node_is_app_data_persisted<T: Node>(node: &T) -> bool {
    let meta = &node.node_data().meta;
    meta.decl_id.0 == PREFERENCES_DECL_ID || meta.tags.iter().any(|tag| tag == PREFERENCES_APP_DATA_TAG)
}

/// Creates a fresh engine instance and applies the app runtime configuration.
pub fn create_engine<T>() -> Result<Engine<T>, String>
where
    T: ProjectLifecycle,
{
    let mut engine = Engine::new(T::create_project_root());
    T::configure_engine(&mut engine)?;
    Ok(engine)
}

/// Creates a fresh engine instance and seeds a new project template.
pub fn create_new_project_engine<T>() -> Result<Engine<T>, String>
where
    T: ProjectLifecycle,
{
    let mut engine = create_engine::<T>()?;
    T::initialize_new_project(&mut engine)?;
    Ok(engine)
}

/// Reapplies app-owned runtime configuration after deserializing a project.
pub fn configure_loaded_engine<T>(engine: &mut Engine<T>) -> Result<(), String>
where
    T: ProjectLifecycle,
{
    T::configure_engine(engine)?;
    T::project_opened(engine)
}

/// Runs side-effect-free node validation after detached candidate initialization.
pub fn validate_engine_project_candidate<T: Node>(engine: &Engine<T>) -> Result<(), String> {
    let mut nodes = engine.nodes.iter().collect::<Vec<_>>();
    nodes.sort_unstable_by_key(|(node_id, _)| node_id.0);
    for (node_id, node) in nodes {
        node.engine_validate_project_candidate().map_err(|error| {
            format!(
                "candidate node {} ({}) failed validation: {error}",
                node_id.0,
                node.get_type()
            )
        })?;
    }
    Ok(())
}

/// Prepares a detached candidate without running callbacks that may acquire live resources.
///
/// Node construction, attached/init callbacks, script evaluation, graph stabilization, and
/// schedule resolution happen here. Deferred `on_node_ready` callbacks form the explicit live
/// activation boundary and are retained for [`activate_engine_for_runtime`].
pub fn prepare_engine_candidate_for_runtime<T: Node>(engine: &mut Engine<T>) -> std::io::Result<()> {
    let trace_enabled = runtime_prepare_perf_trace_enabled();
    let total_started = Instant::now();
    let apply_started = Instant::now();
    engine
        .apply_project_load_edits()
        .map_err(|err| Error::other(format!("initial apply_edits failed: {err}")))?;
    let apply = apply_started.elapsed();
    let resolve_started = Instant::now();
    engine
        .resolve_if_needed()
        .map_err(|err| Error::other(format!("initial resolve failed: {err}")))?;
    let resolve = resolve_started.elapsed();
    if trace_enabled {
        eprintln!(
            "[engine] runtime_candidate_prepare apply_ms={} resolve_ms={} total_ms={}",
            apply.as_millis(),
            resolve.as_millis(),
            total_started.elapsed().as_millis()
        );
    }
    Ok(())
}

/// Activates a prepared engine after the previous project has released live resources.
pub fn activate_engine_for_runtime<T: Node>(engine: &mut Engine<T>) -> std::io::Result<()> {
    engine
        .run_pending_node_ready_callbacks()
        .map_err(|err| Error::other(format!("initial node-ready callbacks failed: {err}")))?;
    engine
        .resolve_if_needed()
        .map_err(|err| Error::other(format!("initial resolve after activation failed: {err}")))?;

    // Startup shape is already reflected by the in-memory graph and initial snapshot.
    // Dropping bootstrap inbox events avoids a very expensive first runtime tick for large graphs.
    engine.inbox.clear();
    engine.clear_history(); // keep runtime undo history strictly post-start
    engine.prepare_param_control_index_for_runtime();
    engine.prepare_first_tick_snapshot_if_needed();
    Ok(())
}

/// Applies startup edits, activates resources, resolves scheduling, and clears bootstrap state.
pub fn prepare_engine_for_runtime<T: Node>(engine: &mut Engine<T>) -> std::io::Result<()> {
    prepare_engine_candidate_for_runtime(engine)?;
    activate_engine_for_runtime(engine)
}

fn runtime_prepare_perf_trace_enabled() -> bool {
    std::env::var_os("GOLDEN_PERF_TRACE").is_some_and(|value| {
        let value = value.to_string_lossy();
        !matches!(value.trim().to_ascii_lowercase().as_str(), "" | "0" | "false" | "off")
    })
}

/// Applies detached candidate work for a best-effort project recovery load.
pub fn prepare_engine_candidate_for_runtime_recovering<T: Node>(engine: &mut Engine<T>) -> ProjectLoadRecoveryReport {
    let mut recovery = ProjectLoadRecoveryReport::default();

    if let Err(err) = engine.apply_project_load_edits() {
        recovery.push_runtime_startup_error(format!("initial apply_edits failed: {err}"));
        engine.edits.pending.clear();
    }
    if let Err(err) = engine.resolve_if_needed() {
        recovery.push_runtime_startup_error(format!("initial resolve failed: {err}"));
    }

    recovery
}

/// Applies startup work for a best-effort project recovery load.
pub fn prepare_engine_for_runtime_recovering<T: Node>(engine: &mut Engine<T>) -> ProjectLoadRecoveryReport {
    let mut recovery = prepare_engine_candidate_for_runtime_recovering(engine);
    if let Err(err) = engine.run_pending_node_ready_callbacks() {
        recovery.push_runtime_startup_error(format!("initial node-ready callbacks failed: {err}"));
        engine.edits.pending.clear();
    }
    if let Err(err) = engine.resolve_if_needed() {
        recovery.push_runtime_startup_error(format!("initial resolve after activation failed: {err}"));
    }

    engine.inbox.clear();
    engine.edits.pending.clear();
    engine.clear_history();
    engine.prepare_param_control_index_for_runtime();
    engine.prepare_first_tick_snapshot_if_needed();
    recovery
}

/// Runs node destroy callbacks before discarding or replacing one live engine.
///
/// Any edits queued during destroy are intentionally ignored because the engine is about to be dropped.
pub fn shutdown_engine_for_runtime<T: Node>(engine: &mut Engine<T>) {
    let Ok(mut node_ids) = engine.collect_subtree(0, "ShutdownEngine", engine.root) else {
        return;
    };
    node_ids.reverse();

    let tree_snapshot = engine.build_process_tree_snapshot();

    for node_id in node_ids {
        let mut destroy_ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
        destroy_ctx.set_tree_snapshot(Arc::clone(&tree_snapshot));

        if let Some(node) = engine.nodes.get_mut(node_id) {
            crate::logger::with_node_origin(node_id, || {
                node.destroy(&mut destroy_ctx);
            });
        }
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
        self.engine.run_pending_node_ready_callbacks()?;
        self.engine.resolve_if_needed()?;
        self.engine.clear_history();
        apply_preferences_runtime_limits(&mut self.engine);
        self.engine.run_loop()
    }
}

mod project_capture;
mod project_io;
mod sparse;

pub use project_io::{
    capture_sparse_project_file_with_ui_state, capture_sparse_subtree_file, from_sparse_project_json,
    from_sparse_project_json_with_ui_state, from_sparse_project_json_with_ui_state_recovering,
    insert_sparse_subtree_json, load_sparse_project_file, load_sparse_project_file_with_ui_state,
    load_sparse_project_file_with_ui_state_recovering, to_sparse_project_json_pretty,
    to_sparse_project_json_pretty_with_ui_state, to_sparse_subtree_json_pretty,
};
#[cfg(test)]
mod tests;

pub(crate) use project_capture::{CapturedProjectNode, ProjectGraphCapture, sparse_project_file_from_capture};
