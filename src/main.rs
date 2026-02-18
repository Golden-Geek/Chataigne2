use std::io::Error;

use golden_core::{app::run_app, node::{Folder, Node}};

include!(concat!(env!("OUT_DIR"), "/app_nodes.rs"));
pub type AppEngine = golden_core::engine::Engine<AppNode>;

fn main() -> std::io::Result<()> {
    let root: AppNode = Folder::new("Root".to_string()).into();
    let mut engine = AppEngine::new(root);

    engine.add_node(ModuleManager::create("Module Manager", true).into(), None);
    engine.add_node(ModuleManager::create("Restricted Module Manager", false).into(), None);
    engine.apply_edits().map_err(|err| Error::other(format!("failed to create module managers: {err}")))?;

    let manager = engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child).ok_or_else(|| Error::other("module manager node was not attached under root"))?;
    let restricted_manager = engine.nodes.get(manager).and_then(|manager_node| manager_node.node_data().next_sibling).ok_or_else(|| Error::other("restricted module manager node was not attached under root"))?;

    engine.add_node(Folder::new("Module Folder").into(), Some(manager));
    engine.apply_edits().map_err(|err| Error::other(format!("failed to create module folder: {err}")))?;

    let module_folder = engine.nodes.get(manager).and_then(|manager_node| manager_node.node_data().first_child).ok_or_else(|| Error::other("module folder node was not attached under module manager"))?;

    engine.add_user_item(OscModule::create("OSC Module").into(), Some(manager));
    engine.add_user_item(MidiModule::create("MIDI Module").into(), Some(manager));
    engine.add_user_item(DmxModule::create("DMX Module").into(), Some(module_folder));
    engine.add_user_item(OscModule::create("OSC Module (Restricted)").into(), Some(restricted_manager));

    run_app(engine)
}
