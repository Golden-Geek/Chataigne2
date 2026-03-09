//! Runtime project persistence and UI-server integration.

use std::io::Error;
use std::sync::{Arc, Mutex};

use crate::engine::{Engine, EngineRuntimeError};
use crate::node::{DashboardNode, Folder, Node, NodeMeta};

mod ui_server;

pub use ui_server::{UiServerConfig, run_ui_server};

/// App node contract required by the default runtime host.
pub trait ProjectNode: Node {
    /// Recreates one node from project persistence payload.
    fn project_decode_node(node_type: &str, data: &serde_json::Value, meta: &NodeMeta) -> Result<Self, String>
    where
        Self: Sized;
}

/// App-controlled lifecycle hooks for engine recreation and new-project setup.
pub trait ProjectLifecycle: ProjectNode + From<Folder> + From<DashboardNode> {
    /// Creates the root node used for fresh engine instances.
    fn create_project_root() -> Self {
        Folder::new("Root").into()
    }

    /// Applies runtime-only engine setup that must run on every recreation.
    fn configure_engine(_engine: &mut Engine<Self>) -> Result<(), String>
    where
        Self: Sized,
    {
        Ok(())
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
}

/// Queues the core default nodes that every fresh project should start with.
pub fn add_default_project_nodes<T>(engine: &mut Engine<T>)
where
    T: Node + From<DashboardNode>,
{
    engine.add_node(DashboardNode::new("Dashboard").into(), None);
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

pub(crate) fn prepare_engine_for_runtime<T: Node>(engine: &mut Engine<T>) -> std::io::Result<()> {
    engine.apply_edits().map_err(|err| Error::other(format!("initial apply_edits failed: {err}")))?;
    engine.resolve_if_needed().map_err(|err| Error::other(format!("initial resolve failed: {err}")))?;
    // Startup shape is already reflected by the in-memory graph and initial snapshot.
    // Dropping bootstrap inbox events avoids a very expensive first runtime tick for large graphs.
    engine.inbox.clear();
    engine.clear_history(); // keep runtime undo history strictly post-start
    Ok(())
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
pub fn run_with_ui_server_config<T: ProjectLifecycle + 'static>(mut engine: Engine<T>, config: UiServerConfig) -> std::io::Result<()> {
    prepare_engine_for_runtime(&mut engine)?;
    let shared_engine = Arc::new(Mutex::new(engine));
    run_ui_server(shared_engine, config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::Node;

    #[crate::node("lifecycle_marker_node")]
    struct LifecycleMarkerNode {}

    #[crate::node("lifecycle_marker_node", from_struct)]
    impl Node for LifecycleMarkerNode {}

    #[crate::node("auto_loaded_node")]
    struct AutoLoadedNode {}

    #[crate::node("auto_loaded_node", from_struct)]
    impl Node for AutoLoadedNode {}

    #[crate::node("managed_item_manager_node")]
    struct ManagedItemManagerNode {}

    #[crate::node("managed_item_manager_node", from_struct)]
    impl Node for ManagedItemManagerNode {
        crate::define_user_item_factory_methods! {
            accepts = ["managed_item"];
            items = [
                {
                    node_type: "managed_item_node",
                    item_kind: "managed_item",
                    label: "Managed Item",
                    create: |_: &Self, label: String| ManagedItemNode::create(label),
                },
            ];
        }
    }

    #[crate::node("managed_item_base_node")]
    struct ManagedItemBaseNode {}

    #[crate::node("managed_item_base_node", from_struct)]
    impl Node for ManagedItemBaseNode {}

    #[crate::node("managed_item_node")]
    struct ManagedItemNode {
        base: ManagedItemBaseNode,
    }

    impl ManagedItemNode {
        fn create(label: impl Into<String>) -> Self {
            let label = label.into();
            Self::new(label.clone(), ManagedItemBaseNode::new(label))
        }
    }

    #[crate::item("managed_item", node = "managed_item_node", via = base, from_struct)]
    impl Node for ManagedItemNode {}

    crate::define_node_enum!(
        enum LifecycleTestAppNode {
            LifecycleMarkerNode,
        }
    );

    crate::define_node_enum!(
        enum ProjectDecodeTestAppNode {
            AutoLoadedNode,
            ManagedItemManagerNode,
            ManagedItemBaseNode,
            ManagedItemNode,
        }
    );

    impl ProjectLifecycle for LifecycleTestAppNode {
        fn configure_engine(engine: &mut Engine<Self>) -> Result<(), String> {
            engine.set_ui_event_log_capacity(42);
            Ok(())
        }

        fn initialize_new_project(engine: &mut Engine<Self>) -> Result<(), String> {
            add_default_project_nodes(engine);
            engine.add_node(LifecycleMarkerNode::new("Marker").into(), None);
            Ok(())
        }

        fn project_opened(engine: &mut Engine<Self>) -> Result<(), String> {
            engine.set_ui_event_log_capacity(7);
            Ok(())
        }
    }

    #[test]
    fn create_new_project_engine_runs_app_lifecycle_hooks() {
        let mut engine = create_new_project_engine::<LifecycleTestAppNode>().expect("new project engine should be created");
        assert_eq!(engine.ui_event_log_capacity(), 42, "configure_engine should run during new-project creation");

        prepare_engine_for_runtime(&mut engine).expect("new project engine should prepare");

        let first_child = engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child).expect("dashboard should exist under root");
        let second_child = engine.nodes.get(first_child).and_then(|node| node.node_data().next_sibling).expect("marker node should exist under root");

        assert_eq!(engine.nodes.get(first_child).map(Node::get_type), Some(crate::node::DASHBOARD_NODE_TYPE));
        assert_eq!(engine.nodes.get(second_child).map(Node::get_type), Some("lifecycle_marker_node"));
    }

    #[test]
    fn configure_loaded_engine_reapplies_runtime_setup_after_deserialize() {
        let mut engine = create_new_project_engine::<LifecycleTestAppNode>().expect("new project engine should be created");
        prepare_engine_for_runtime(&mut engine).expect("new project engine should prepare");

        let json = engine.to_project_json_with(|node| node.project_encode_data()).expect("project should encode");
        let mut loaded = Engine::<LifecycleTestAppNode>::from_project_json_with(&json, <LifecycleTestAppNode as ProjectNode>::project_decode_node).expect("project should decode");

        assert_eq!(loaded.ui_event_log_capacity(), 8192, "deserialized engine should start with default runtime settings");

        configure_loaded_engine(&mut loaded).expect("loaded engine should accept runtime configuration");
        assert_eq!(loaded.ui_event_log_capacity(), 7, "project_opened should run after loading");
    }

    #[test]
    fn from_struct_nodes_without_special_ctor_decode_without_manual_project_create() {
        let root: ProjectDecodeTestAppNode = Folder::new("Root").into();
        let mut engine = Engine::new(root);
        engine.add_node(AutoLoadedNode::new("Simple").into(), None);
        engine.apply_edits().expect("simple node should attach");

        let json = engine.to_project_json_with(|node| node.project_encode_data()).expect("project should encode");
        let loaded = Engine::<ProjectDecodeTestAppNode>::from_project_json_with(&json, <ProjectDecodeTestAppNode as ProjectNode>::project_decode_node).expect("project should decode");

        let simple = loaded.nodes.get(loaded.root).and_then(|root| root.node_data().first_child).expect("simple node should be restored under root");
        assert_eq!(loaded.nodes.get(simple).map(Node::get_type), Some("auto_loaded_node"));
    }

    #[test]
    fn managed_item_nodes_decode_from_parent_factory_without_manual_project_create() {
        let root: ProjectDecodeTestAppNode = Folder::new("Root").into();
        let mut engine = Engine::new(root);
        engine.add_node(ManagedItemManagerNode::new("Manager").into(), None);
        engine.apply_edits().expect("manager should attach");

        let manager = engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child).expect("manager should exist under root");
        engine.add_user_item(ManagedItemNode::create("Item").into(), Some(manager));
        engine.apply_edits().expect("managed item should attach");

        let json = engine.to_project_json_with(|node| node.project_encode_data()).expect("project should encode");
        let loaded = Engine::<ProjectDecodeTestAppNode>::from_project_json_with(&json, <ProjectDecodeTestAppNode as ProjectNode>::project_decode_node).expect("project should decode");

        let loaded_manager = loaded.nodes.get(loaded.root).and_then(|root| root.node_data().first_child).expect("manager should be restored");
        let loaded_item = loaded.nodes.get(loaded_manager).and_then(|node| node.node_data().first_child).expect("managed item should be restored");

        assert_eq!(loaded.nodes.get(loaded_item).map(Node::get_type), Some("managed_item_node"));
        assert_eq!(loaded.nodes.get(loaded_item).expect("managed item should exist").node_data().user_role, crate::node::UserNodeRole::ItemRoot);
    }

    #[test]
    fn managed_item_nodes_duplicate_from_parent_factory_without_manual_project_create() {
        let root: ProjectDecodeTestAppNode = Folder::new("Root").into();
        let mut engine = Engine::new(root);
        engine.add_node(ManagedItemManagerNode::new("Manager").into(), None);
        engine.apply_edits().expect("manager should attach");

        let manager = engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child).expect("manager should exist under root");
        engine.add_user_item(ManagedItemNode::create("Item").into(), Some(manager));
        engine.apply_edits().expect("managed item should attach");

        let item = engine.nodes.get(manager).and_then(|node| node.node_data().first_child).expect("managed item should exist under manager");
        let duplicated = engine
            .duplicate_subtree_with(item, manager, Some(item), Some("Item Copy".to_string()), |node| node.project_encode_data(), <ProjectDecodeTestAppNode as ProjectNode>::project_decode_node)
            .expect("managed item should duplicate");

        assert_eq!(engine.nodes.get(duplicated).map(Node::get_type), Some("managed_item_node"));
        assert_eq!(engine.nodes.get(duplicated).expect("duplicated managed item should exist").node_data().meta.label, "Item Copy");
        assert_eq!(engine.nodes.get(duplicated).expect("duplicated managed item should exist").node_data().user_role, crate::node::UserNodeRole::ItemRoot);
    }
}
