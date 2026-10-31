//! Public app policy and lifecycle hooks, independent of host and persistence plumbing.

use super::{ProjectFileSpec, add_default_project_nodes};
use crate::{
    engine::Engine,
    node::{DashboardNode, Folder, Node, NodeMeta},
};

/// App node contract required by the default runtime host.
pub trait ProjectNode: Node {
    /// Creates one node from its default constructor path without applying persisted data.
    fn project_create_node(node_type: &str) -> Option<Self>
    where
        Self: Sized;

    /// Recreates one node from project persistence payload.
    fn project_decode_node(node_type: &str, data: &serde_json::Value, meta: &NodeMeta) -> Result<Self, String>
    where
        Self: Sized;
}

/// App-controlled lifecycle hooks for engine recreation and new-project setup.
pub trait ProjectLifecycle: ProjectNode + From<Folder> + From<DashboardNode> {
    /// Stable application identifier used by discovery and client state isolation.
    fn application_id() -> &'static str {
        "golden"
    }

    /// Human-readable application name used by generic desktop hosts.
    fn application_display_name() -> &'static str {
        Self::project_file_spec().display_name
    }

    /// Application release version advertised independently of the UI protocol version.
    fn application_version() -> &'static str {
        "0.0.0"
    }

    /// Creates the root node used for fresh engine instances.
    fn create_project_root() -> Self {
        Folder::new("Root").into()
    }

    /// Declares the project file label and extension used by hosts and UIs.
    fn project_file_spec() -> ProjectFileSpec {
        ProjectFileSpec::default()
    }

    /// Directory name under the OS app-data root for host-managed app data.
    fn app_data_directory_name() -> &'static str {
        Self::project_file_spec().display_name
    }

    /// File name used for the host-managed Preferences tree.
    fn preferences_file_name() -> &'static str {
        "preferences.json"
    }

    /// Applies runtime-only engine setup that must run on every recreation.
    fn configure_engine(_engine: &mut Engine<Self>) -> Result<(), String>
    where
        Self: Sized,
    {
        Ok(())
    }

    /// Validates a user/transport intent before any mutation or history boundary is entered.
    /// App policy may require a domain intent instead of generic structural edits to owned nodes.
    /// Pure engines remain independent of app policy; default application hosts use this hook.
    fn validate_ui_intent(_engine: &Engine<Self>, _intent: &crate::ui_sync::UiEditIntent) -> Result<(), String>
    where
        Self: Sized,
    {
        Ok(())
    }

    /// Applies one app-owned operation inside the host's atomic edit/history boundary.
    ///
    /// Validate domain policy here, queue authored changes through engine edits, and
    /// return the app's result DTO. Do not mutate node state directly or perform
    /// external side effects: a later lifecycle failure rolls authored edits back.
    /// The caller receives the result only after all queued edits have stabilized.
    fn apply_app_operation(
        _engine: &mut Engine<Self>,
        namespace: &str,
        operation: &str,
        _payload: serde_json::Value,
    ) -> Result<serde_json::Value, String>
    where
        Self: Sized,
    {
        Err(format!("unsupported app operation '{namespace}.{operation}'"))
    }

    /// Seeds a fresh project with default content.
    fn initialize_new_project(engine: &mut Engine<Self>) -> Result<(), String>
    where
        Self: Sized,
    {
        add_default_project_nodes(engine);
        Ok(())
    }

    /// Applies post-load setup after deserializing a project file.
    fn project_opened(_engine: &mut Engine<Self>) -> Result<(), String>
    where
        Self: Sized,
    {
        Ok(())
    }

    /// Settles app-owned backend operations requested by ordinary node edits.
    ///
    /// The default runtime invokes this on its authoritative control thread
    /// after UI/headless transactions and runtime ticks. Implementations may
    /// perform atomic, app-specific structural operations that require access
    /// to persistence codecs and therefore cannot run inside a node callback.
    fn settle_pending_operations(_engine: &mut Engine<Self>) -> Result<(), String>
    where
        Self: Sized,
    {
        Ok(())
    }

    /// Performs app-owned, detached candidate validation before live resource handoff.
    ///
    /// Implementations may inspect or mutate only `engine`. They must not acquire output devices,
    /// bind listening sockets, publish externally, or otherwise assume the candidate is live.
    /// Any edits queued here are settled and compiled while the current project remains active.
    fn prepare_project_candidate(_engine: &mut Engine<Self>) -> Result<(), String>
    where
        Self: Sized,
    {
        Ok(())
    }
}
