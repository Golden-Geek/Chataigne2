use std::io::Error;

use golden_core::{app::run_app, node::{Folder, Node}};
use crate::nodes_module_demo::MODULE_MANAGER_UUID;

include!(concat!(env!("OUT_DIR"), "/app_nodes.rs"));
pub type AppEngine = golden_core::engine::Engine<AppNode>;

fn main() -> std::io::Result<()> {
    let root: AppNode = Folder::new("Root".to_string()).into();
    let mut engine = AppEngine::new(root);
    engine.register_reference_filter("module_values_parameters", |engine, _param_node, _root, candidate| {
        let Some(candidate_node) = engine.nodes.get(candidate) else {
            return false;
        };
        if candidate_node.engine_param_snapshot().is_none() {
            return false;
        }

        let Some(parent_id) = candidate_node.node_data().parent else {
            return false;
        };
        let Some(parent_node) = engine.nodes.get(parent_id) else {
            return false;
        };
        if parent_node.node_data().meta.decl_id.0 != "values" {
            return false;
        }

        let mut current = Some(parent_id);
        let mut has_module_ancestor = false;
        let mut has_module_manager_ancestor = false;
        while let Some(node_id) = current {
            let Some(node) = engine.nodes.get(node_id) else {
                break;
            };
            if node.user_item_kind() == "module" {
                has_module_ancestor = true;
            }
            if node.get_type() == "module_manager" {
                has_module_manager_ancestor = true;
            }
            current = node.node_data().parent;
        }

        has_module_ancestor && has_module_manager_ancestor
    });

    let mut manager_node = ModuleManager::create("Module Manager", true);
    manager_node.node_data_mut().meta.uuid = MODULE_MANAGER_UUID;
    engine.add_node(manager_node.into(), None);
    engine.apply_edits().map_err(|err| Error::other(format!("failed to create module manager: {err}")))?;

    let manager = engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child).ok_or_else(|| Error::other("module manager node was not attached under root"))?;

    engine.add_node(Folder::new("Module Folder").into(), Some(manager));
    engine.apply_edits().map_err(|err| Error::other(format!("failed to create module folder: {err}")))?;

    let module_folder = engine.nodes.get(manager).and_then(|manager_node| manager_node.node_data().first_child).ok_or_else(|| Error::other("module folder node was not attached under module manager"))?;

    engine.add_user_item(OscModule::create("OSC Module").into(), Some(module_folder));
    engine.add_user_item(MidiModule::create("MIDI Module").into(), Some(manager));
    engine.add_user_item(DmxModule::create("DMX Module").into(), Some(module_folder));

    run_app(engine)
}
