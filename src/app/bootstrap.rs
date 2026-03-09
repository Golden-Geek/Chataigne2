use super::{AppEngine, AppNode};
use golden_core::node::{Node, NodeId};

fn find_root_child_by_type(engine: &AppEngine, node_type: &str) -> Option<NodeId> {
    let mut child = engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child);
    while let Some(child_id) = child {
        let node = engine.nodes.get(child_id)?;
        if node.get_type() == node_type {
            return Some(child_id);
        }
        child = node.node_data().next_sibling;
    }
    None
}

fn create_manager_item(engine: &AppEngine, manager: NodeId, node_type: &str, label: &str) -> Result<AppNode, String> {
    let manager_node = engine.nodes.get(manager).ok_or_else(|| format!("module manager node {manager:?} no longer exists during project initialization"))?;
    let mut node = manager_node.create_user_item(node_type).ok_or_else(|| format!("module manager could not create '{node_type}' during project initialization"))?;
    node.node_data_mut().meta.label = label.to_string();
    <AppNode as Node>::from_boxed_node(node).ok_or_else(|| format!("module manager created '{node_type}' outside the app node enum"))
}

impl golden_core::app::ProjectLifecycle for AppNode {
    fn configure_engine(engine: &mut AppEngine) -> Result<(), String> {
        super::nodes_module_demo::register_demo_reference_filters(engine);
        Ok(())
    }

    fn initialize_new_project(engine: &mut AppEngine) -> Result<(), String> {
        golden_core::app::add_default_project_nodes(engine);

        engine.add_node(super::ModuleManager::new().into(), None);
        engine.apply_edits().map_err(|err| format!("failed to create default module manager: {err}"))?;

        let manager = find_root_child_by_type(engine, "module_manager").ok_or_else(|| "default module manager was not created during project initialization".to_string())?;

        for (node_type, label) in [("osc_module", "OSC Module"), ("midi_module", "MIDI Module"), ("dmx_module", "DMX Module")] {
            let node = create_manager_item(engine, manager, node_type, label)?;
            engine.add_user_item(node, Some(manager));
        }

        engine.apply_edits().map_err(|err| format!("failed to create default modules: {err}"))?;

        Ok(())
    }
}
