//! Runtime project persistence and UI-server integration.

use std::io::Error;

use crate::engine::{Engine, EngineRuntimeError};
use crate::node::{DashboardNode, Folder, Node, NodeMeta};

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
    engine.add_node(DashboardNode::new().into(), None);
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

/// Applies startup edits, resolves scheduling, and clears bootstrap-only runtime state.
pub fn prepare_engine_for_runtime<T: Node>(engine: &mut Engine<T>) -> std::io::Result<()> {
    engine
        .apply_edits()
        .map_err(|err| Error::other(format!("initial apply_edits failed: {err}")))?;
    engine
        .resolve_if_needed()
        .map_err(|err| Error::other(format!("initial resolve failed: {err}")))?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::Node;

    #[crate::node("lifecycle_marker_node", label = "Marker")]
    struct LifecycleMarkerNode {}

    #[crate::node("lifecycle_marker_node", from_struct)]
    impl Node for LifecycleMarkerNode {}

    #[crate::node("auto_loaded_node")]
    struct AutoLoadedNode {}

    #[crate::node("auto_loaded_node", from_struct)]
    impl Node for AutoLoadedNode {}

    #[crate::node("defaulted_loaded_node")]
    #[defaults(flag = true)]
    struct DefaultedLoadedNode {
        flag: bool,
    }

    #[crate::node("defaulted_loaded_node", from_struct)]
    impl Node for DefaultedLoadedNode {}

    #[crate::node("persisted_state_node")]
    struct PersistedStateNode {
        #[state(default = true, persist)]
        flag: bool,
    }

    #[crate::node("persisted_state_node", from_struct)]
    impl Node for PersistedStateNode {}

    #[crate::node("via_persisted_state_base_node")]
    struct ViaPersistedStateBaseNode {
        #[state(default = true, persist)]
        base_flag: bool,
    }

    #[crate::node("via_persisted_state_base_node", from_struct)]
    impl Node for ViaPersistedStateBaseNode {}

    #[crate::node("via_persisted_state_wrapper_node")]
    struct ViaPersistedStateWrapperNode {
        base: ViaPersistedStateBaseNode,
        #[state(default = false, persist)]
        wrapper_flag: bool,
    }

    #[crate::node("via_persisted_state_wrapper_node", via = base, from_struct)]
    impl Node for ViaPersistedStateWrapperNode {
        fn project_create(node_type: &str) -> Option<Self> {
            (node_type == "via_persisted_state_wrapper_node").then(|| Self::new(ViaPersistedStateBaseNode::new()))
        }
    }

    #[crate::node("managed_item_manager_node", label = "Manager")]
    struct ManagedItemManagerNode {}

    #[crate::node("managed_item_manager_node", from_struct)]
    impl Node for ManagedItemManagerNode {
        crate::define_user_item_factory_methods! {
            accepts = ["managed_item"];
            items = [
                {
                    type: ManagedItemNode,
                },
            ];
        }
    }

    #[crate::node("managed_item_base_node")]
    struct ManagedItemBaseNode {}

    #[crate::node("managed_item_base_node", from_struct)]
    impl Node for ManagedItemBaseNode {}

    #[crate::node("managed_item_node", label = "Managed Item")]
    struct ManagedItemNode {
        base: ManagedItemBaseNode,
    }

    impl ManagedItemNode {
        fn create() -> Self {
            Self::new(ManagedItemBaseNode::new())
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
            DefaultedLoadedNode,
            PersistedStateNode,
            ViaPersistedStateBaseNode,
            ViaPersistedStateWrapperNode,
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
            engine.add_node(LifecycleMarkerNode::new().into(), None);
            Ok(())
        }

        fn project_opened(engine: &mut Engine<Self>) -> Result<(), String> {
            engine.set_ui_event_log_capacity(7);
            Ok(())
        }
    }

    #[test]
    fn create_new_project_engine_runs_app_lifecycle_hooks() {
        let mut engine =
            create_new_project_engine::<LifecycleTestAppNode>().expect("new project engine should be created");
        assert_eq!(
            engine.ui_event_log_capacity(),
            42,
            "configure_engine should run during new-project creation"
        );

        prepare_engine_for_runtime(&mut engine).expect("new project engine should prepare");

        let first_child = engine
            .nodes
            .get(engine.root)
            .and_then(|root| root.node_data().first_child)
            .expect("dashboard should exist under root");
        let second_child = engine
            .nodes
            .get(first_child)
            .and_then(|node| node.node_data().next_sibling)
            .expect("marker node should exist under root");

        assert_eq!(
            engine.nodes.get(first_child).map(Node::get_type),
            Some(crate::node::DASHBOARD_NODE_TYPE)
        );
        assert_eq!(
            engine.nodes.get(second_child).map(Node::get_type),
            Some("lifecycle_marker_node")
        );
    }

    #[test]
    fn configure_loaded_engine_reapplies_runtime_setup_after_deserialize() {
        let mut engine =
            create_new_project_engine::<LifecycleTestAppNode>().expect("new project engine should be created");
        prepare_engine_for_runtime(&mut engine).expect("new project engine should prepare");

        let json = engine
            .to_project_json_with(|node| node.project_encode_data())
            .expect("project should encode");
        let mut loaded = Engine::<LifecycleTestAppNode>::from_project_json_with(
            &json,
            <LifecycleTestAppNode as ProjectNode>::project_decode_node,
        )
        .expect("project should decode");

        assert_eq!(
            loaded.ui_event_log_capacity(),
            8192,
            "deserialized engine should start with default runtime settings"
        );

        configure_loaded_engine(&mut loaded).expect("loaded engine should accept runtime configuration");
        assert_eq!(
            loaded.ui_event_log_capacity(),
            7,
            "project_opened should run after loading"
        );
    }

    #[test]
    fn from_struct_nodes_without_special_ctor_decode_without_manual_project_create() {
        let root: ProjectDecodeTestAppNode = Folder::new("Root").into();
        let mut engine = Engine::new(root);
        engine.add_node(AutoLoadedNode::new().into(), None);
        engine.apply_edits().expect("simple node should attach");

        let json = engine
            .to_project_json_with(|node| node.project_encode_data())
            .expect("project should encode");
        let loaded = Engine::<ProjectDecodeTestAppNode>::from_project_json_with(
            &json,
            <ProjectDecodeTestAppNode as ProjectNode>::project_decode_node,
        )
        .expect("project should decode");

        let simple = loaded
            .nodes
            .get(loaded.root)
            .and_then(|root| root.node_data().first_child)
            .expect("simple node should be restored under root");
        assert_eq!(loaded.nodes.get(simple).map(Node::get_type), Some("auto_loaded_node"));
    }

    #[test]
    fn defaulted_struct_fields_feed_generated_new_and_project_create() {
        let node = DefaultedLoadedNode::new();
        assert!(node.flag, "generated constructor should apply #[defaults(...)] values");

        let root: ProjectDecodeTestAppNode = Folder::new("Root").into();
        let mut engine = Engine::new(root);
        engine.add_node(node.into(), None);
        engine.apply_edits().expect("defaulted node should attach");

        let json = engine
            .to_project_json_with(|node| node.project_encode_data())
            .expect("project should encode");
        let loaded = Engine::<ProjectDecodeTestAppNode>::from_project_json_with(
            &json,
            <ProjectDecodeTestAppNode as ProjectNode>::project_decode_node,
        )
        .expect("project should decode");

        let restored = loaded
            .nodes
            .get(loaded.root)
            .and_then(|root| root.node_data().first_child)
            .expect("defaulted node should be restored under root");
        let ProjectDecodeTestAppNode::DefaultedLoadedNode(node) =
            loaded.nodes.get(restored).expect("defaulted node should exist")
        else {
            panic!("restored node should be a DefaultedLoadedNode");
        };

        assert!(
            node.flag,
            "autoloaded node should preserve generated default-backed constructor state"
        );
    }

    #[test]
    fn state_fields_can_define_default_and_persistence_in_one_place() {
        let mut node = PersistedStateNode::new();
        assert!(
            node.flag,
            "generated constructor should apply #[state(default = ...)] values"
        );
        node.flag = false;

        let root: ProjectDecodeTestAppNode = Folder::new("Root").into();
        let mut engine = Engine::new(root);
        engine.add_node(node.into(), None);
        engine.apply_edits().expect("persisted-state node should attach");

        let json = engine
            .to_project_json_with(|node| node.project_encode_data())
            .expect("project should encode");
        let loaded = Engine::<ProjectDecodeTestAppNode>::from_project_json_with(
            &json,
            <ProjectDecodeTestAppNode as ProjectNode>::project_decode_node,
        )
        .expect("project should decode");

        let restored = loaded
            .nodes
            .get(loaded.root)
            .and_then(|root| root.node_data().first_child)
            .expect("persisted-state node should be restored under root");
        let ProjectDecodeTestAppNode::PersistedStateNode(node) =
            loaded.nodes.get(restored).expect("persisted-state node should exist")
        else {
            panic!("restored node should be a PersistedStateNode");
        };

        assert!(
            !node.flag,
            "persisted #[state(..., persist)] field should round-trip through project save/load"
        );
    }

    #[test]
    fn via_nodes_can_persist_wrapper_and_base_state_without_manual_codecs() {
        let mut node = ViaPersistedStateWrapperNode::new(ViaPersistedStateBaseNode::new());
        assert!(
            node.base.base_flag,
            "via base should use generated default-backed constructor"
        );
        assert!(
            !node.wrapper_flag,
            "via wrapper should use generated default-backed constructor"
        );

        node.base.base_flag = false;
        node.wrapper_flag = true;

        let root: ProjectDecodeTestAppNode = Folder::new("Root").into();
        let mut engine = Engine::new(root);
        engine.add_node(node.into(), None);
        engine.apply_edits().expect("via persisted-state node should attach");

        let json = engine
            .to_project_json_with(|node| node.project_encode_data())
            .expect("project should encode");
        let loaded = Engine::<ProjectDecodeTestAppNode>::from_project_json_with(
            &json,
            <ProjectDecodeTestAppNode as ProjectNode>::project_decode_node,
        )
        .expect("project should decode");

        let restored = loaded
            .nodes
            .get(loaded.root)
            .and_then(|root| root.node_data().first_child)
            .expect("via persisted-state node should be restored under root");
        let ProjectDecodeTestAppNode::ViaPersistedStateWrapperNode(node) = loaded
            .nodes
            .get(restored)
            .expect("via persisted-state node should exist")
        else {
            panic!("restored node should be a ViaPersistedStateWrapperNode");
        };

        assert!(
            !node.base.base_flag,
            "via base persisted state should round-trip through project save/load"
        );
        assert!(
            node.wrapper_flag,
            "via wrapper persisted state should round-trip through project save/load"
        );
    }

    #[test]
    fn managed_item_nodes_decode_from_parent_factory_without_manual_project_create() {
        let root: ProjectDecodeTestAppNode = Folder::new("Root").into();
        let mut engine = Engine::new(root);
        engine.add_node(ManagedItemManagerNode::new().into(), None);
        engine.apply_edits().expect("manager should attach");

        let manager = engine
            .nodes
            .get(engine.root)
            .and_then(|root| root.node_data().first_child)
            .expect("manager should exist under root");
        engine.add_user_item(ManagedItemNode::create().into(), Some(manager));
        engine.apply_edits().expect("managed item should attach");

        let json = engine
            .to_project_json_with(|node| node.project_encode_data())
            .expect("project should encode");
        let loaded = Engine::<ProjectDecodeTestAppNode>::from_project_json_with(
            &json,
            <ProjectDecodeTestAppNode as ProjectNode>::project_decode_node,
        )
        .expect("project should decode");

        let loaded_manager = loaded
            .nodes
            .get(loaded.root)
            .and_then(|root| root.node_data().first_child)
            .expect("manager should be restored");
        let loaded_item = loaded
            .nodes
            .get(loaded_manager)
            .and_then(|node| node.node_data().first_child)
            .expect("managed item should be restored");

        assert_eq!(
            loaded.nodes.get(loaded_item).map(Node::get_type),
            Some("managed_item_node")
        );
        assert_eq!(
            loaded
                .nodes
                .get(loaded_item)
                .expect("managed item should exist")
                .node_data()
                .user_role,
            crate::node::UserNodeRole::ItemRoot
        );
    }

    #[test]
    fn managed_item_nodes_duplicate_from_parent_factory_without_manual_project_create() {
        let root: ProjectDecodeTestAppNode = Folder::new("Root").into();
        let mut engine = Engine::new(root);
        engine.add_node(ManagedItemManagerNode::new().into(), None);
        engine.apply_edits().expect("manager should attach");

        let manager = engine
            .nodes
            .get(engine.root)
            .and_then(|root| root.node_data().first_child)
            .expect("manager should exist under root");
        engine.add_user_item(ManagedItemNode::create().into(), Some(manager));
        engine.apply_edits().expect("managed item should attach");

        let item = engine
            .nodes
            .get(manager)
            .and_then(|node| node.node_data().first_child)
            .expect("managed item should exist under manager");
        let duplicated = engine
            .duplicate_subtree_with(
                item,
                manager,
                Some(item),
                Some("Item Copy".to_string()),
                |node| node.project_encode_data(),
                <ProjectDecodeTestAppNode as ProjectNode>::project_decode_node,
            )
            .expect("managed item should duplicate");

        assert_eq!(
            engine.nodes.get(duplicated).map(Node::get_type),
            Some("managed_item_node")
        );
        assert_eq!(
            engine
                .nodes
                .get(duplicated)
                .expect("duplicated managed item should exist")
                .node_data()
                .meta
                .label,
            "Item Copy"
        );
        assert_eq!(
            engine
                .nodes
                .get(duplicated)
                .expect("duplicated managed item should exist")
                .node_data()
                .user_role,
            crate::node::UserNodeRole::ItemRoot
        );
    }
}
