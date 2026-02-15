mod ui_server;

use std::sync::{Arc, Mutex};

use golden_core::node::Folder;
use ui_server::{UiServerConfig, run_ui_server};

include!(concat!(env!("OUT_DIR"), "/app_nodes.rs"));
pub type AppEngine = golden_core::engine::Engine<AppNode>;

fn main() -> std::io::Result<()> {
    let root: AppNode = Folder::new("Root".to_string()).into();
    let mut engine = AppEngine::new(root);

    let node = DummyNode::new("Dum Dum");
    // engine.add_node(node.into(), None);
    engine.add_node(VeryDummyNode::create("Very Dummy").into(), None);
    // engine.add_node(SuperDummyNode::create("Super Dummy").into(), None);

    engine.apply_edits().map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, format!("initial apply_edits failed: {err}")))?;
    engine.resolve_if_needed().map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, format!("initial resolve failed: {err}")))?;

    let shared_engine = Arc::new(Mutex::new(engine));
    let mut config = UiServerConfig::default();
    if let Ok(bind_addr) = std::env::var("GC_UI_BIND") {
        if !bind_addr.trim().is_empty() {
            config.bind_addr = bind_addr;
        }
    }
    run_ui_server(shared_engine, config)
}
